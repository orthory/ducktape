//! Headless runtime used by generated Ice tests.

mod trace;

use crate::{SemanticEnd, SemanticSnapshot, SemanticState, StableId};
use iced::advanced::Renderer as _;
use iced::advanced::renderer::Headless as _;
use iced::advanced::text::Paragraph as _;
use iced::advanced::widget::operation::Outcome;
use iced::advanced::widget::{self, Operation as _};
use iced::keyboard;
use iced::mouse;
use iced::theme;
use iced::theme::Base as _;
use iced::touch;
use iced::window;
use iced::{Background, Border, Color, Font, Point, Rectangle, Shadow, Size};
use iced_test::futures::futures::StreamExt as _;
use iced_test::futures::futures::channel::mpsc;
use iced_test::futures::subscription;
use iced_test::futures::{Executor as _, Runtime};
use iced_test::program::Program;
use iced_test::runtime::core::{clipboard, input_method};
use iced_test::runtime::task;
use iced_test::runtime::user_interface::{self, UserInterface};
use iced_test::runtime::{self, Task};
use iced_test::selector::{Candidate, Selector};
use std::any::{Any, TypeId};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::Hasher as _;
use std::marker::PhantomData;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock, Mutex, PoisonError};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use trace::Recorder as TraceRecorder;
use ui_lang_template::trace::Phase;

const MAX_SCREENSHOT_PIXELS: usize = 16_777_216;

/// Source location attached to a generated test operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Location {
    pub path: &'static str,
    pub line: usize,
    pub column: usize,
    pub statement: &'static str,
}

impl Location {
    pub const fn new(
        path: &'static str,
        line: usize,
        column: usize,
        statement: &'static str,
    ) -> Self {
        Self {
            path,
            line,
            column,
            statement,
        }
    }
}

impl fmt::Display for Location {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}:{}", self.path, self.line, self.column)
    }
}

thread_local! {
    static RENDER_SOURCE_STACK: RefCell<Vec<Location>> = const { RefCell::new(Vec::new()) };
    static RENDER_SOURCES: RefCell<HashMap<String, Vec<Location>>> = RefCell::new(HashMap::new());
    /// Component instance scopes sighted by a render, per component name.
    /// `mounted` storage keeps its own active set; `retained` storage has
    /// none by design — its map only gains an entry when an event is
    /// delivered — so a harness that just rendered would otherwise be unable
    /// to name an instance that has not been typed into yet.
    static COMPONENT_SIGHTINGS: RefCell<HashMap<String, Vec<String>>> = RefCell::new(HashMap::new());
    static APP_INSTANCE: Cell<Option<u64>> = const { Cell::new(None) };
}

static NEXT_APP_INSTANCE: AtomicU64 = AtomicU64::new(1);

/// Which driven application this thread is doing work for.
///
/// A shipped process runs one application, so state that belongs to the whole
/// machine — a key store, a device handle, a connection pool — is a `static` in
/// production and correct there. A test binary runs one application per test
/// thread against that same `static`, and the sharing is invisible until the
/// suite is busy enough for two of them to overlap: one test's boot clears the
/// keys another had just unlocked, one test's import is read back by another,
/// and which test fails moves with the machine rather than with the code.
///
/// So application state that is global in production is keyed on this in test
/// builds. Every thread that reaches an instance's state answers with that
/// instance: the driver's own thread, and the executor threads its tasks and
/// subscriptions are polled on, which the driver enrols as it hands work to
/// them. A thread with no driver behind it — a plain `#[test]` — gets an
/// instance of its own, which is the same isolation for the same reason.
pub fn app_instance() -> u64 {
    APP_INSTANCE.with(|instance| match instance.get() {
        Some(id) => id,
        None => {
            let id = NEXT_APP_INSTANCE.fetch_add(1, Ordering::Relaxed);
            instance.set(Some(id));
            id
        }
    })
}

/// Enrols this thread into a driver's application instance.
fn adopt_app_instance(id: u64) {
    APP_INSTANCE.with(|instance| instance.set(Some(id)));
}

/// Every live driver's logical clock, keyed by application instance.
///
/// [`every`] reads it once, as its stream starts, to phase its first tick from
/// the driver's current logical time instead of from the wall clock. Entries
/// are one `Instant` per driven application in a binary that exits with the
/// suite, so, like the instance ids they hang off, they are never reclaimed.
static LOGICAL_CLOCKS: LazyLock<Mutex<HashMap<u64, Instant>>> = LazyLock::new(Mutex::default);

fn publish_logical_time(instance: u64, time: Instant) {
    LOGICAL_CLOCKS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(instance, time);
}

fn published_logical_time(instance: u64) -> Option<Instant> {
    LOGICAL_CLOCKS
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&instance)
        .copied()
}

/// Carries the driver's application instance onto whichever executor thread
/// polls this stream, so a task or subscription reaches the same process-global
/// state its own application does.
struct Instanced<S> {
    inner: S,
    instance: u64,
}

impl<S: iced_test::futures::futures::Stream + Unpin> iced_test::futures::futures::Stream
    for Instanced<S>
{
    type Item = S::Item;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        adopt_app_instance(self.instance);
        Pin::new(&mut self.inner).poll_next(context)
    }
}

/// Restores the previous rendered-node source when generated view construction exits.
#[doc(hidden)]
pub struct RenderSourceGuard;

/// Enters one generated `.ice` view-node source while its widget is constructed.
#[doc(hidden)]
pub fn push_render_source(source: Location) -> RenderSourceGuard {
    RENDER_SOURCE_STACK.with(|stack| stack.borrow_mut().push(source));
    RenderSourceGuard
}

/// Opens a render pass, discarding the id-to-source map the previous one built.
///
/// The boundary is the view function, not the first guard on an empty stack.
/// A view fills its slot table first — that is where every compiled hole
/// constructs its widgets and registers their ids — and only then does the
/// renderer walk the node tree, pushing a guard of its own for the root. On
/// the old boundary that walk counted as a new pass and threw the slot table's
/// registrations away moments before they were read, so a named widget inside
/// a hole lost its `.ice` line.
#[doc(hidden)]
pub fn begin_render_pass() {
    RENDER_SOURCES.with(|sources| sources.borrow_mut().clear());
}

impl Drop for RenderSourceGuard {
    fn drop(&mut self) {
        RENDER_SOURCE_STACK.with(|stack| {
            stack.borrow_mut().pop();
        });
    }
}

pub(crate) fn current_render_source() -> Option<Location> {
    RENDER_SOURCE_STACK.with(|stack| stack.borrow().last().copied())
}

/// Records that `component` rendered an instance at `scope`. Test-only, and
/// generated only for `retained` storage — see [`component_sightings`].
#[doc(hidden)]
pub fn register_component_sighting(component: &str, scope: &str) {
    COMPONENT_SIGHTINGS.with(|sightings| {
        let mut sightings = sightings.borrow_mut();
        let scopes = sightings.entry(component.to_owned()).or_default();
        if !scopes.iter().any(|seen| seen == scope) {
            scopes.push(scope.to_owned());
        }
    });
}

/// Every instance scope of `component` a render has sighted in this thread.
/// The generated `__ice_test_scopes_*` unions this with the storage map's own
/// keys, so a freshly rendered instance and a materialized one are both
/// nameable.
#[doc(hidden)]
pub fn component_sightings(component: &str) -> Vec<String> {
    COMPONENT_SIGHTINGS.with(|sightings| {
        sightings
            .borrow()
            .get(component)
            .cloned()
            .unwrap_or_default()
    })
}

/// Associates a generated native widget ID with the current `.ice` view node.
#[doc(hidden)]
pub fn register_render_source(id: &str) {
    let Some(source) = current_render_source() else {
        return;
    };
    RENDER_SOURCES.with(|sources| {
        let mut sources = sources.borrow_mut();
        let candidates = sources.entry(id.to_owned()).or_default();
        if !candidates.contains(&source) {
            candidates.push(source);
        }
    });
}

fn render_source_for_id(id: &str) -> Option<Location> {
    RENDER_SOURCES.with(|sources| {
        let sources = sources.borrow();
        let candidates = sources.get(id)?;
        (candidates.len() == 1).then_some(candidates[0])
    })
}

struct RenderSourceState(Location);
struct RenderSourceEnd;

/// Wraps a generated test widget in its originating `.ice` view-node frame.
#[doc(hidden)]
pub fn sourced<'a, Message, Theme, Renderer>(
    content: impl Into<iced::Element<'a, Message, Theme, Renderer>>,
    source: Location,
) -> iced::Element<'a, Message, Theme, Renderer>
where
    Message: 'static,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    iced::Element::new(Sourced {
        content: content.into(),
        source,
    })
}

struct Sourced<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    content: iced::Element<'a, Message, Theme, Renderer>,
    source: Location,
}

impl<Message, Theme, Renderer> iced::advanced::Widget<Message, Theme, Renderer>
    for Sourced<'_, Message, Theme, Renderer>
where
    Message: 'static,
    Renderer: iced::advanced::Renderer,
{
    fn tag(&self) -> iced::advanced::widget::tree::Tag {
        iced::advanced::widget::tree::Tag::of::<RenderSourceState>()
    }

    fn state(&self) -> iced::advanced::widget::tree::State {
        iced::advanced::widget::tree::State::new(RenderSourceState(self.source))
    }

    fn children(&self) -> Vec<iced::advanced::widget::Tree> {
        vec![iced::advanced::widget::Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut iced::advanced::widget::Tree) {
        tree.state.downcast_mut::<RenderSourceState>().0 = self.source;
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn size(&self) -> iced::Size<iced::Length> {
        self.content.as_widget().size()
    }

    fn size_hint(&self) -> iced::Size<iced::Length> {
        self.content.as_widget().size_hint()
    }

    fn layout(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        renderer: &Renderer,
        limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn iced::advanced::widget::Operation,
    ) {
        operation.custom(
            None,
            layout.bounds(),
            tree.state.downcast_mut::<RenderSourceState>(),
        );
        operation.traverse(&mut |operation| {
            self.content.as_widget_mut().operate(
                &mut tree.children[0],
                layout,
                renderer,
                operation,
            );
        });
        operation.custom(None, layout.bounds(), &mut RenderSourceEnd);
    }

    fn update(
        &mut self,
        tree: &mut iced::advanced::widget::Tree,
        event: &iced::Event,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, Message>,
        viewport: &iced::Rectangle,
    ) {
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
    }

    fn draw(
        &self,
        tree: &iced::advanced::widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::mouse::Cursor,
        viewport: &iced::Rectangle,
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
        tree: &iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'_>,
        cursor: iced::mouse::Cursor,
        viewport: &iced::Rectangle,
        renderer: &Renderer,
    ) -> iced::mouse::Interaction {
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
        tree: &'a mut iced::advanced::widget::Tree,
        layout: iced::advanced::Layout<'a>,
        renderer: &Renderer,
        viewport: &iced::Rectangle,
        translation: iced::Vector,
    ) -> Option<iced::advanced::overlay::Element<'a, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

/// A theme mode understood by the semantic test driver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    None,
    Light,
    Dark,
}

impl ThemeMode {
    fn iced(self) -> theme::Mode {
        match self {
            Self::None => theme::Mode::None,
            Self::Light => theme::Mode::Light,
            Self::Dark => theme::Mode::Dark,
        }
    }
}

/// The platform contract used by platform-sensitive semantic actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Linux,
    Windows,
    Macos,
    Wasm,
}

impl Platform {
    const fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::Macos
        } else if cfg!(target_family = "wasm") {
            Self::Wasm
        } else {
            Self::Linux
        }
    }
}

/// A mouse button independent from the renderer backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
    Other(u16),
}

impl MouseButton {
    fn iced(self) -> mouse::Button {
        match self {
            Self::Left => mouse::Button::Left,
            Self::Right => mouse::Button::Right,
            Self::Middle => mouse::Button::Middle,
            Self::Back => mouse::Button::Back,
            Self::Forward => mouse::Button::Forward,
            Self::Other(value) => mouse::Button::Other(value),
        }
    }
}

/// A mouse-wheel movement in either logical lines or physical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WheelDelta {
    Lines { x: f32, y: f32 },
    Pixels { x: f32, y: f32 },
}

/// Keyboard modifier state retained between semantic key actions.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub logo: bool,
}

impl Modifiers {
    pub const NONE: Self = Self::new(false, false, false, false);

    pub const fn new(shift: bool, control: bool, alt: bool, logo: bool) -> Self {
        Self {
            shift,
            control,
            alt,
            logo,
        }
    }

    fn iced(self) -> keyboard::Modifiers {
        let mut modifiers = keyboard::Modifiers::empty();
        modifiers.set(keyboard::Modifiers::SHIFT, self.shift);
        modifiers.set(keyboard::Modifiers::CTRL, self.control);
        modifiers.set(keyboard::Modifiers::ALT, self.alt);
        modifiers.set(keyboard::Modifiers::LOGO, self.logo);
        modifiers
    }
}

/// A logical key accepted by the semantic test driver.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Key {
    Named(keyboard::key::Named),
    Character(String),
    Unidentified,
}

impl Key {
    pub const fn named(name: keyboard::key::Named) -> Self {
        Self::Named(name)
    }

    pub fn character(value: impl Into<String>) -> Self {
        Self::Character(value.into())
    }
}

/// The physical location of a keyboard key.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum KeyLocation {
    #[default]
    Standard,
    Left,
    Right,
    Numpad,
}

impl KeyLocation {
    fn iced(self) -> keyboard::Location {
        match self {
            Self::Standard => keyboard::Location::Standard,
            Self::Left => keyboard::Location::Left,
            Self::Right => keyboard::Location::Right,
            Self::Numpad => keyboard::Location::Numpad,
        }
    }
}

/// Optional metadata for an exact keyboard event.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyMetadata {
    pub modified_key: Option<Key>,
    pub physical_key: Option<keyboard::key::Physical>,
    pub location: KeyLocation,
    pub text: Option<String>,
    pub repeat: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum HeldKeyIdentity {
    Physical(keyboard::key::Physical),
    Logical { key: Key, location: KeyLocation },
}

/// One phase of an input-method composition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompositionPhase {
    Start,
    Update {
        text: String,
        selection: Option<Range<usize>>,
    },
    Commit(String),
    Cancel,
}

/// One phase of a retained touch contact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchPhase {
    Down,
    Move,
    Up,
    Cancel,
}

/// An in-memory RGBA screenshot captured by a semantic test action.
#[derive(Debug, Clone)]
pub struct Capture {
    pub name: String,
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
    pub png_path: PathBuf,
    pub metadata_path: PathBuf,
}

/// A semantic accessibility action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessibilityAction {
    Click,
    Focus,
}

/// The part of the status item a `expect tray` step asserts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayField {
    Label,
    Icon,
    Item,
    /// Whether the row carrying the text is a command the reader can choose
    /// rather than a stat the platform draws disabled.
    Command,
}

/// The declaration indices of every row whose text carries `value`.
/// Separators hold no text, so they never match.
///
/// Every row, at every depth: a menu is one flat table, so a row inside a
/// submenu is named by its text exactly like a row at the top level. That is
/// also why this returns all of them rather than the first. One text naming
/// two rows is what grouping makes likely — two submenus can each hold a
/// `Close` — and picking the earlier one would run the wrong handler while the
/// test passed, which is the failure a row-to-handler table exists to stop.
///
/// A row a `when` guard has taken out of the menu is not in the table the
/// reader sees, so it is not here either — choosing it is what the platform
/// could not do, and [`Driver::check_tray`] reports it as missing.
fn tray_rows_containing(tray: &crate::tray::TraySnapshot, value: &str) -> Vec<usize> {
    tray.items
        .iter()
        .enumerate()
        .filter(|(index, item)| {
            !item.is_empty() && item.contains(value) && tray_visible(tray, *index)
        })
        .map(|(index, _)| index)
        .collect()
}

/// A snapshot with no `hidden` entry for a row — one built by hand — has
/// hidden nothing.
fn tray_visible(tray: &crate::tray::TraySnapshot, index: usize) -> bool {
    !tray.hidden.get(index).copied().unwrap_or(false)
}

fn hidden_rows_note(hidden: &[String]) -> String {
    match hidden.is_empty() {
        true => String::new(),
        false => format!("\nhidden by a `when` guard: {hidden:?}"),
    }
}

/// The rows carrying `value` that a `when` guard has hidden, for the failure
/// that names them: a test asserting a row that exists but is not showing
/// should be told which of the two it is looking at.
fn tray_hidden_rows_containing(tray: &crate::tray::TraySnapshot, value: &str) -> Vec<String> {
    tray.items
        .iter()
        .enumerate()
        .filter(|(index, item)| {
            !item.is_empty() && item.contains(value) && !tray_visible(tray, *index)
        })
        .map(|(_, item)| item.clone())
        .collect()
}

/// A semantic accessibility property used by generated expectations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessibilityProperty {
    Role,
    Name,
    Value,
    Checked,
    Expanded,
    Disabled,
    Focused,
    Action,
}

/// A semantic action accepted by [`Driver::perform_action`] without exposing raw Iced events.
#[derive(Debug, Clone)]
pub enum Action {
    Leave,
    MoveTo(String),
    MoveToPoint(Point),
    Click {
        target: String,
        button: MouseButton,
        count: u8,
    },
    ClickAt {
        position: Point,
        button: MouseButton,
        count: u8,
    },
    Press {
        target: String,
        button: MouseButton,
    },
    Release(MouseButton),
    Wheel(WheelDelta),
    ScrollTo {
        target: String,
        x: f32,
        y: f32,
    },
    ScrollBy {
        target: String,
        x: f32,
        y: f32,
    },
    Snap {
        target: String,
        x: f32,
        y: f32,
    },
    SnapEnd(String),
    Drag {
        from: String,
        to: String,
    },
    DropAt(String),
    Focus(String),
    FocusNext,
    FocusPrevious,
    Blur,
    WindowFocus(bool),
    Type(String),
    Clear,
    Replace(String),
    Select {
        start: usize,
        end: usize,
    },
    SelectAll,
    Cursor(usize),
    CursorFront,
    CursorEnd,
    Composition(CompositionPhase),
    Key(Key),
    KeyDown {
        key: Key,
        metadata: KeyMetadata,
    },
    KeyUp {
        key: Key,
        metadata: KeyMetadata,
    },
    Modifiers(Modifiers),
    Chord {
        modifiers: Modifiers,
        key: Key,
    },
    Repeat {
        key: Key,
        count: usize,
    },
    Touch {
        phase: TouchPhase,
        id: u64,
        position: Point,
    },
    Tap {
        target: String,
        count: u8,
    },
    WindowOpened,
    WindowClosed,
    WindowMove(Point),
    Resize(Size),
    Rescale(f32),
    CloseRequested,
    Redraw,
    SystemTheme(ThemeMode),
    FileHover(PathBuf),
    FileDrop(PathBuf),
    FileLeave,
    Wait(Duration),
    Advance(Duration),
    Idle,
    Capture(String),
    Accessibility {
        action: AccessibilityAction,
        target: String,
    },
}

#[derive(Debug, Clone)]
struct HeldKeyRecord {
    key: Key,
    location: KeyLocation,
}

/// Configuration for one generated Ice test.
#[derive(Debug, Clone)]
pub struct Config {
    pub name: &'static str,
    pub source: Option<Location>,
    pub viewport: Size,
    pub timeout: Duration,
    pub preset: Option<&'static str>,
    pub theme: Option<ThemeMode>,
    pub system_theme: ThemeMode,
    pub scale_factor: Option<f32>,
    pub locale: Option<&'static str>,
    pub platform: Platform,
    pub reduced_motion: Option<bool>,
    pub artifact_dir: Option<PathBuf>,
}

impl Config {
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            source: None,
            viewport: Size::new(1024.0, 768.0),
            // Long enough that a loaded machine running the whole workspace in
            // parallel does not read scheduling delay as a stalled task: two
            // seconds failed roughly one run in three while the same test
            // settled in well under one when run alone. A test that wants a
            // tighter bound says so with `timeout`.
            timeout: Duration::from_secs(10),
            preset: None,
            theme: None,
            system_theme: ThemeMode::None,
            scale_factor: None,
            locale: None,
            platform: Platform::current(),
            reduced_motion: None,
            artifact_dir: None,
        }
    }

    pub const fn source(mut self, source: Location) -> Self {
        self.source = Some(source);
        self
    }

    pub const fn viewport(mut self, width: f32, height: f32) -> Self {
        self.viewport = Size::new(width, height);
        self
    }

    pub const fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub const fn preset(mut self, preset: &'static str) -> Self {
        self.preset = Some(preset);
        self
    }

    pub const fn theme(mut self, mode: ThemeMode) -> Self {
        self.theme = Some(mode);
        self
    }

    pub const fn system_theme(mut self, mode: ThemeMode) -> Self {
        self.system_theme = mode;
        self
    }

    pub const fn scale_factor(mut self, scale_factor: f32) -> Self {
        self.scale_factor = Some(scale_factor);
        self
    }

    pub const fn locale(mut self, locale: &'static str) -> Self {
        self.locale = Some(locale);
        self
    }

    pub const fn platform(mut self, platform: Platform) -> Self {
        self.platform = platform;
        self
    }

    pub const fn reduced_motion(mut self, reduced_motion: bool) -> Self {
        self.reduced_motion = Some(reduced_motion);
        self
    }

    pub fn artifact_dir(mut self, artifact_dir: impl Into<PathBuf>) -> Self {
        self.artifact_dir = Some(artifact_dir.into());
        self
    }
}

/// Runs the hidden headless inspection entry point generated for every Ice app.
#[doc(hidden)]
pub fn agent_inspect<P>(program: impl Fn() -> P, source_path: &'static str)
where
    P: Program + 'static,
    P::Renderer: 'static,
    P::Message: Clone,
{
    let Some(requested_source) = std::env::var_os("ICE_AGENT_INSPECT_SOURCE") else {
        return;
    };
    let requested_source = PathBuf::from(requested_source);
    let generated_source = PathBuf::from(source_path);
    let requested_source = requested_source.canonicalize().unwrap_or(requested_source);
    let generated_source = generated_source.canonicalize().unwrap_or(generated_source);
    if requested_source != generated_source {
        return;
    }

    let name = leaked_env("ICE_AGENT_INSPECT_NAME").unwrap_or("inspection");
    let source = Location::new(source_path, 1, 1, "agent inspection");
    let mut config = Config::new(name).source(source);
    if let (Some(width), Some(height)) = (
        parsed_env::<f32>("ICE_AGENT_INSPECT_WIDTH"),
        parsed_env::<f32>("ICE_AGENT_INSPECT_HEIGHT"),
    ) {
        config = config.viewport(width, height);
    }
    if let Some(preset) = leaked_env("ICE_AGENT_INSPECT_PRESET") {
        config = config.preset(preset);
    }
    if let Some(theme) = mode_env("ICE_AGENT_INSPECT_THEME") {
        config = config.theme(theme);
    }
    if let Some(theme) = mode_env("ICE_AGENT_INSPECT_SYSTEM_THEME") {
        config = config.system_theme(theme);
    }
    if let Some(scale) = parsed_env::<f32>("ICE_AGENT_INSPECT_SCALE") {
        config = config.scale_factor(scale);
    }
    if let Some(locale) = leaked_env("ICE_AGENT_INSPECT_LOCALE") {
        config = config.locale(locale);
    }
    if let Some(platform) = platform_env("ICE_AGENT_INSPECT_PLATFORM") {
        config = config.platform(platform);
    }
    if let Some(reduced_motion) = bool_env("ICE_AGENT_INSPECT_REDUCED_MOTION") {
        config = config.reduced_motion(reduced_motion);
    }
    if let Some(directory) = std::env::var_os("ICE_AGENT_INSPECT_ARTIFACT_DIR") {
        config = config.artifact_dir(directory);
    }

    if let Some(campaign) = trace::Campaign::from_env() {
        let artifact_dir = config.artifact_dir.clone().unwrap_or_else(|| {
            PathBuf::from("target")
                .join("ice-test-artifacts")
                .join(safe_path_component(name))
        });
        let mut artifact = trace::run_campaign(program, config, campaign);
        artifact.app_root = manifest_source_path(source_path);
        artifact.package =
            std::env::var("ICE_TRACE_PACKAGE").unwrap_or_else(|_| "<generated-package>".into());
        artifact.validate().unwrap_or_else(|error| {
            panic!("{source}: generated invalid interaction trace: {error}")
        });
        std::fs::create_dir_all(&artifact_dir).unwrap_or_else(|error| {
            panic!(
                "{source}: cannot create trace output {}: {error}",
                artifact_dir.display()
            )
        });
        let trace_path = artifact_dir.join("trace.json");
        let mut bytes = serde_json::to_vec_pretty(&artifact)
            .expect("interaction trace artifact is serializable");
        bytes.push(b'\n');
        std::fs::write(&trace_path, bytes).unwrap_or_else(|error| {
            panic!(
                "{source}: cannot write trace artifact {}: {error}",
                trace_path.display()
            )
        });
        write_agent_result(
            source,
            serde_json::json!({
                "source": generated_source,
                "trace": trace_path,
            }),
        );
        return;
    }

    let mut driver = Driver::new(program(), config);
    let capture = driver.capture(name, source);
    let result = serde_json::json!({
        "source": generated_source,
        "png": capture.png_path,
        "manifest": capture.metadata_path,
    });
    write_agent_result(source, result);
}

fn write_agent_result(source: Location, result: serde_json::Value) {
    let result_path = std::env::var_os("ICE_AGENT_INSPECT_RESULT")
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("{source}: ICE_AGENT_INSPECT_RESULT is required"));
    std::fs::write(
        &result_path,
        serde_json::to_vec_pretty(&result).expect("inspection result is serializable"),
    )
    .unwrap_or_else(|error| {
        panic!(
            "{source}: cannot write inspection result {}: {error}",
            result_path.display()
        )
    });
}

fn leaked_env(name: &str) -> Option<&'static str> {
    std::env::var(name)
        .ok()
        .map(|value| Box::leak(value.into_boxed_str()) as &'static str)
}

fn parsed_env<T>(name: &str) -> Option<T>
where
    T: std::str::FromStr,
    T::Err: fmt::Display,
{
    std::env::var(name).ok().map(|value| {
        value
            .parse()
            .unwrap_or_else(|error| panic!("invalid {name} value {value:?}: {error}"))
    })
}

fn mode_env(name: &str) -> Option<ThemeMode> {
    std::env::var(name).ok().map(|value| match value.as_str() {
        "none" => ThemeMode::None,
        "light" => ThemeMode::Light,
        "dark" => ThemeMode::Dark,
        _ => panic!("invalid {name} value {value:?}; expected none, light, or dark"),
    })
}

fn platform_env(name: &str) -> Option<Platform> {
    std::env::var(name).ok().map(|value| match value.as_str() {
        "linux" => Platform::Linux,
        "windows" => Platform::Windows,
        "macos" => Platform::Macos,
        "wasm" => Platform::Wasm,
        _ => panic!("invalid {name} value {value:?}; expected linux, windows, macos, or wasm"),
    })
}

fn bool_env(name: &str) -> Option<bool> {
    std::env::var(name).ok().map(|value| match value.as_str() {
        "true" => true,
        "false" => false,
        _ => panic!("invalid {name} value {value:?}; expected true or false"),
    })
}

/// Runs generated Rust for one Ice test statement with source-mapped panic context.
#[doc(hidden)]
pub fn step<T>(test_name: &'static str, source: Location, operation: impl FnOnce() -> T) -> T {
    with_panic_context(test_name, Some(source), operation)
}

#[derive(Debug, Clone)]
struct SurfacePaint {
    pub background: Background,
    pub border: Border,
    pub shadow: Shadow,
}

#[derive(Debug, Clone)]
struct TextPaint {
    pub content: Option<String>,
    pub bounds: Rectangle,
    pub color: Color,
    pub size: Option<f64>,
    pub font: Option<Font>,
    pub line_height: Option<iced::widget::text::LineHeight>,
    pub baseline: Option<f64>,
}

#[derive(Debug, Clone, Copy)]
struct ImagePaint {
    pub bounds: Rectangle,
    /// The svg tint, when the primitive is a vector image drawn with one.
    pub color: Option<Color>,
}

#[derive(Debug, Clone)]
struct AccessibilityData {
    role: crate::Role,
    name: Option<String>,
    description: Option<String>,
    value: Option<String>,
    checked: Option<bool>,
    expanded: Option<bool>,
    disabled: bool,
    focused: bool,
    supports_activate: bool,
    supports_focus: bool,
}

/// A fresh post-layout snapshot of an identified rendered widget.
#[derive(Debug, Clone)]
pub struct Target {
    pub id: String,
    pub kind: String,
    bounds: Rectangle,
    visible: Option<Rectangle>,
    content: Option<Rectangle>,
    translation: Option<iced::Vector>,
    value: Option<String>,
    test_name: &'static str,
    source: Location,
    paint_error: Option<&'static str>,
    surfaces: Vec<SurfacePaint>,
    texts: Vec<TextPaint>,
    images: Vec<ImagePaint>,
    accessibility: Option<AccessibilityData>,
    focused: Option<bool>,
    scale_factor: f32,
    render_source: Option<Location>,
}

impl Target {
    pub fn kind(&self) -> String {
        self.kind.clone()
    }

    pub fn value(&self) -> String {
        self.value.clone().unwrap_or_else(|| {
            self.fail(
                "value",
                "expected: rendered text content\nactual: unavailable for this target kind",
            )
        })
    }

    pub fn background(&self) -> Background {
        self.surface("background").background
    }

    pub fn border(&self) -> Border {
        self.surface("border").border
    }

    pub fn shadow(&self) -> Shadow {
        self.surface("shadow").shadow
    }

    pub fn text_color(&self) -> Color {
        self.text("text_color").color
    }

    pub fn text_size(&self) -> f64 {
        self.text("text_size").size.unwrap_or_else(|| {
            self.fail(
                "text_size",
                "expected: retained text size\nactual: unavailable for this text primitive",
            )
        })
    }

    pub fn font(&self) -> Font {
        self.text("font").font.unwrap_or_else(|| {
            self.fail(
                "font",
                "expected: retained text font\nactual: unavailable for this text primitive",
            )
        })
    }

    pub fn x(&self) -> f64 {
        self.bounds.x.into()
    }

    pub fn y(&self) -> f64 {
        self.bounds.y.into()
    }

    pub fn width(&self) -> f64 {
        self.bounds.width.into()
    }

    pub fn height(&self) -> f64 {
        self.bounds.height.into()
    }

    pub fn left(&self) -> f64 {
        self.bounds.x.into()
    }

    pub fn top(&self) -> f64 {
        self.bounds.y.into()
    }

    /// Adds in `f32` and widens after, as the renderer does: widening first
    /// and adding in `f64` moves the low bits, and these numbers land in the
    /// ice-test artifact manifests and in `expect root.right == root.x +
    /// root.width` assertions.
    pub fn right(&self) -> f64 {
        (self.bounds.x + self.bounds.width).into()
    }

    pub fn bottom(&self) -> f64 {
        (self.bounds.y + self.bounds.height).into()
    }

    pub fn center_x(&self) -> f64 {
        self.bounds.center_x().into()
    }

    pub fn center_y(&self) -> f64 {
        self.bounds.center_y().into()
    }

    pub fn visible(&self) -> bool {
        self.visible.is_some()
    }

    pub fn visible_x(&self) -> f64 {
        self.required_number("visible_x", self.visible.map(|bounds| bounds.x.into()))
    }

    pub fn visible_y(&self) -> f64 {
        self.required_number("visible_y", self.visible.map(|bounds| bounds.y.into()))
    }

    pub fn visible_width(&self) -> f64 {
        self.required_number(
            "visible_width",
            self.visible.map(|bounds| bounds.width.into()),
        )
    }

    pub fn visible_height(&self) -> f64 {
        self.required_number(
            "visible_height",
            self.visible.map(|bounds| bounds.height.into()),
        )
    }

    pub fn content_x(&self) -> f64 {
        self.required_number("content_x", self.content.map(|bounds| bounds.x.into()))
    }

    pub fn content_y(&self) -> f64 {
        self.required_number("content_y", self.content.map(|bounds| bounds.y.into()))
    }

    pub fn content_width(&self) -> f64 {
        self.required_number(
            "content_width",
            self.content.map(|bounds| bounds.width.into()),
        )
    }

    pub fn content_height(&self) -> f64 {
        self.required_number(
            "content_height",
            self.content.map(|bounds| bounds.height.into()),
        )
    }

    pub fn translation_x(&self) -> f64 {
        self.required_number(
            "translation_x",
            self.translation.map(|translation| translation.x.into()),
        )
    }

    pub fn translation_y(&self) -> f64 {
        self.required_number(
            "translation_y",
            self.translation.map(|translation| translation.y.into()),
        )
    }

    /// Reads the same retained transform as [`Self::translation_x`], but names
    /// `scroll_x` so the "cannot inspect" panic quotes the field the author
    /// wrote.
    pub fn scroll_x(&self) -> f64 {
        self.required_number(
            "scroll_x",
            self.translation.map(|translation| translation.x.into()),
        )
    }

    pub fn scroll_y(&self) -> f64 {
        self.required_number(
            "scroll_y",
            self.translation.map(|translation| translation.y.into()),
        )
    }

    pub fn line_height(&self) -> iced::widget::text::LineHeight {
        self.text("line_height").line_height.unwrap_or_else(|| {
            self.fail(
                "line_height",
                "expected: retained text line height\nactual: unavailable for this text primitive",
            )
        })
    }

    pub fn surface_count(&self) -> usize {
        self.require_paint("surface_count");
        self.surfaces.len()
    }

    pub fn text_count(&self) -> usize {
        self.require_paint("text_count");
        self.texts.len()
    }

    pub fn image_count(&self) -> usize {
        self.require_paint("image_count");
        self.images.len()
    }

    pub fn text_x(&self) -> f64 {
        f64::from(self.text("text_x").bounds.x)
    }

    pub fn text_y(&self) -> f64 {
        f64::from(self.text("text_y").bounds.y)
    }

    pub fn text_width(&self) -> f64 {
        f64::from(self.text("text_width").bounds.width)
    }

    pub fn text_height(&self) -> f64 {
        f64::from(self.text("text_height").bounds.height)
    }

    pub fn text_baseline(&self) -> f64 {
        self.text("text_baseline").baseline.unwrap_or_else(|| {
            self.fail(
                "text_baseline",
                "expected: a retained shaped first-line baseline\nactual: unavailable for this text primitive",
            )
        })
    }

    pub fn image_x(&self) -> f64 {
        f64::from(self.image("image_x").bounds.x)
    }

    pub fn image_y(&self) -> f64 {
        f64::from(self.image("image_y").bounds.y)
    }

    pub fn image_width(&self) -> f64 {
        f64::from(self.image("image_width").bounds.width)
    }

    pub fn image_height(&self) -> f64 {
        f64::from(self.image("image_height").bounds.height)
    }

    /// The tint an svg primitive is drawn with. A raster image or an svg
    /// drawing its own intrinsic colors has no tint and fails the assertion.
    pub fn image_color(&self) -> Color {
        self.image("image_color").color.unwrap_or_else(|| {
            self.fail(
                "image_color",
                "expected: a tinted svg primitive\nactual: this image carries no tint color",
            )
        })
    }

    pub fn pixel_aligned(&self) -> bool {
        rectangle_pixel_aligned(self.bounds, self.scale_factor)
    }

    pub fn focused(&self) -> bool {
        self.accessibility
            .as_ref()
            .map(|data| data.focused)
            .unwrap_or(false)
            || self.focused.unwrap_or(false)
    }

    pub fn accessibility_role(&self) -> crate::Role {
        self.accessibility("role").role
    }

    pub fn accessibility_role_name(&self) -> String {
        accessibility_role_name(self.accessibility_role())
    }

    pub fn accessibility_name(&self) -> String {
        self.accessibility("name").name.clone().unwrap_or_else(|| {
            self.fail(
                "name",
                "expected: retained accessibility name\nactual: property is absent",
            )
        })
    }

    pub fn accessibility_description(&self) -> String {
        self.accessibility("description")
            .description
            .clone()
            .unwrap_or_else(|| {
                self.fail(
                    "description",
                    "expected: retained accessibility description\nactual: property is absent",
                )
            })
    }

    pub fn accessibility_value(&self) -> String {
        self.accessibility("value")
            .value
            .clone()
            .unwrap_or_else(|| {
                self.fail(
                    "value",
                    "expected: retained accessibility value\nactual: property is absent",
                )
            })
    }

    pub fn accessibility_checked(&self) -> bool {
        self.accessibility("checked").checked.unwrap_or_else(|| {
            self.fail(
                "checked",
                "expected: retained accessibility checked state\nactual: property is absent",
            )
        })
    }

    pub fn accessibility_expanded(&self) -> bool {
        self.accessibility("expanded").expanded.unwrap_or_else(|| {
            self.fail(
                "expanded",
                "expected: retained accessibility expanded state\nactual: property is absent",
            )
        })
    }

    pub fn accessibility_disabled(&self) -> bool {
        self.accessibility("disabled").disabled
    }

    pub fn accessibility_focused(&self) -> bool {
        self.accessibility("focused").focused
    }

    pub fn accessibility_supports_activate(&self) -> bool {
        self.accessibility("activate action").supports_activate
    }

    pub fn accessibility_supports_focus(&self) -> bool {
        self.accessibility("focus action").supports_focus
    }

    fn required_number(&self, field: &str, value: Option<f64>) -> f64 {
        value.unwrap_or_else(|| {
            self.fail(
                field,
                "expected: retained target geometry\nactual: unavailable for this target kind",
            )
        })
    }

    fn surface(&self, field: &str) -> &SurfacePaint {
        if let Some(reason) = self.paint_error {
            self.fail(
                field,
                &format!(
                    "expected: structured tiny-skia surface paint\nactual: unavailable ({reason})"
                ),
            );
        }
        match self.surfaces.as_slice() {
            [surface] => surface,
            [] => self.fail(
                field,
                "expected: exactly 1 quad matching the target bounds\nactual: 0 matching quads",
            ),
            surfaces => self.fail(
                field,
                &format!(
                    "expected: exactly 1 quad matching the target bounds\nactual: {} matching quads; use a narrower #id",
                    surfaces.len()
                ),
            ),
        }
    }

    fn text(&self, field: &str) -> &TextPaint {
        self.require_paint(field);
        match self.texts.as_slice() {
            [text] => text,
            [] => self.fail(
                field,
                "expected: exactly 1 visible text primitive inside the target\nactual: 0 visible text primitives",
            ),
            texts => self.fail(
                field,
                &format!(
                    "expected: exactly 1 visible text primitive inside the target\nactual: {} visible text primitives; use a narrower #id",
                    texts.len()
                ),
            ),
        }
    }

    fn image(&self, field: &str) -> &ImagePaint {
        self.require_paint(field);
        match self.images.as_slice() {
            [image] => image,
            [] => self.fail(
                field,
                "expected: exactly 1 visible image primitive inside the target\nactual: 0 visible image primitives",
            ),
            images => self.fail(
                field,
                &format!(
                    "expected: exactly 1 visible image primitive inside the target\nactual: {} visible image primitives; use a narrower #id",
                    images.len()
                ),
            ),
        }
    }

    fn accessibility(&self, field: &str) -> &AccessibilityData {
        self.accessibility.as_ref().unwrap_or_else(|| {
            self.fail(
                field,
                "expected: a semantic accessibility node\nactual: target has no accessibility contract",
            )
        })
    }

    fn require_paint(&self, field: &str) {
        if let Some(reason) = self.paint_error {
            self.fail(
                field,
                &format!("expected: structured tiny-skia paint\nactual: unavailable ({reason})"),
            );
        }
    }

    #[track_caller]
    fn fail(&self, field: &str, reason: &str) -> ! {
        panic!(
            "{}: test `{}` target `{}` cannot inspect `{field}`\n{}\nstatement: {}\nselector: {}\nbounds: {:?}",
            self.source,
            self.test_name,
            self.id,
            reason,
            self.source.statement,
            self.id,
            self.bounds,
        )
    }
}

#[derive(Debug, Clone)]
struct LayoutTarget {
    semantic: bool,
    semantic_group: Option<usize>,
    state_key: Option<usize>,
    kind: String,
    bounds: Rectangle,
    visible_bounds: Option<Rectangle>,
    content_bounds: Option<Rectangle>,
    translation: Option<iced::Vector>,
    value: Option<String>,
    accessibility: Option<AccessibilityData>,
    focused: Option<bool>,
    source: Option<Location>,
}

struct IdSelector<Message> {
    logical_id: String,
    native_id: widget::Id,
    stable_id: widget::Id,
    semantic_frames: Vec<Option<usize>>,
    next_semantic_group: usize,
    source_frames: Vec<Location>,
    identified_bounds: Vec<Option<Rectangle>>,
    marker: PhantomData<fn() -> Message>,
}

#[derive(Clone)]
struct SemanticActionTarget<Message> {
    activate: Option<Message>,
    focus: Option<crate::SemanticFocus>,
    disabled: bool,
}

struct SemanticActionSelector<Message> {
    logical_id: String,
    occurrences: HashMap<StableId, u64>,
    marker: PhantomData<fn() -> Message>,
}

impl<Message> SemanticActionSelector<Message> {
    fn new(logical_id: &str) -> Self {
        Self {
            logical_id: logical_id.to_owned(),
            occurrences: HashMap::new(),
            marker: PhantomData,
        }
    }
}

impl<Message: Clone + 'static> Selector for SemanticActionSelector<Message> {
    type Output = SemanticActionTarget<Message>;

    fn select(&mut self, candidate: Candidate<'_>) -> Option<Self::Output> {
        let Candidate::Custom { state, .. } = candidate else {
            return None;
        };
        let state = state.downcast_ref::<SemanticState<Message>>()?;
        let occurrence = self.occurrences.entry(state.semantics.id).or_default();
        let current = *occurrence;
        *occurrence += 1;
        (state.semantics.logical_id.as_deref() == Some(&self.logical_id)).then(|| {
            SemanticActionTarget {
                activate: state.semantics.activate.clone(),
                focus: (state.semantics.focus != crate::FocusBehavior::None).then_some(
                    crate::SemanticFocus {
                        base: state.semantics.id,
                        occurrence: current,
                    },
                ),
                disabled: state.semantics.disabled,
            }
        })
    }

    fn description(&self) -> String {
        format!("semantic logical id == {:?}", self.logical_id)
    }
}

impl<Message> IdSelector<Message> {
    fn new(logical_id: &str) -> Self {
        Self {
            logical_id: logical_id.to_owned(),
            native_id: logical_id.to_owned().into(),
            stable_id: StableId::new(logical_id).widget_id(),
            semantic_frames: Vec::new(),
            next_semantic_group: 0,
            source_frames: Vec::new(),
            identified_bounds: Vec::new(),
            marker: PhantomData,
        }
    }

    fn matches_id(&self, id: Option<&widget::Id>) -> bool {
        id.is_some_and(|id| id == &self.native_id || id == &self.stable_id)
    }
}

impl<Message: 'static> Selector for IdSelector<Message> {
    type Output = LayoutTarget;

    fn select(&mut self, candidate: Candidate<'_>) -> Option<Self::Output> {
        let mut semantic_group = self.semantic_frames.last().copied().flatten();
        if let Candidate::Custom { state, .. } = &candidate {
            if state.downcast_ref::<RenderSourceEnd>().is_some() {
                self.source_frames.pop();
                self.identified_bounds.pop();
                return None;
            }
            if let Some(state) = state.downcast_ref::<RenderSourceState>() {
                self.source_frames.push(state.0);
                self.identified_bounds.push(None);
                return None;
            }
            if state.downcast_ref::<SemanticEnd>().is_some() {
                self.semantic_frames.pop();
                return None;
            }
            if let Some(state) = state.downcast_ref::<SemanticSnapshot>() {
                // An identified extern call is a generated container around a
                // native element. Its root semantics may use an adapter-owned
                // logical id, so pair equal bounds inside the same source frame.
                let matches = state.logical_id.as_deref() == Some(&self.logical_id)
                    || self.matches_id(candidate.id())
                    || self.identified_bounds.last().copied().flatten() == Some(candidate.bounds());
                let group = matches.then(|| {
                    let group = self.next_semantic_group;
                    self.next_semantic_group += 1;
                    group
                });
                self.semantic_frames.push(group);
                if !matches {
                    return None;
                }
                semantic_group = group;
            } else if semantic_group.is_some() && !self.matches_id(candidate.id()) {
                return None;
            }
        } else if semantic_group.is_some() && !self.matches_id(candidate.id()) {
            return None;
        }

        if !matches!(&candidate, Candidate::Custom { state, .. } if state.downcast_ref::<SemanticSnapshot>().is_some())
            && !self.matches_id(candidate.id())
        {
            return None;
        }

        if self.matches_id(candidate.id())
            && let Some(bounds) = self.identified_bounds.last_mut()
        {
            *bounds = Some(candidate.bounds());
        }

        let bounds = candidate.bounds();
        let visible_bounds = candidate.visible_bounds();
        let source = self
            .source_frames
            .last()
            .copied()
            .or_else(|| render_source_for_id(&self.logical_id));
        let (
            semantic,
            state_key,
            kind,
            content_bounds,
            translation,
            value,
            accessibility,
            focused,
            source,
        ) = match candidate {
            Candidate::Container { .. } => (
                false,
                None,
                "container",
                None,
                None,
                None,
                None,
                None,
                source,
            ),
            Candidate::Focusable { state, .. } => (
                false,
                Some(data_address(state)),
                "focusable",
                None,
                None,
                None,
                None,
                Some(state.is_focused()),
                source,
            ),
            Candidate::Scrollable {
                content_bounds,
                translation,
                state,
                ..
            } => (
                false,
                Some(data_address(state)),
                "scrollable",
                Some(content_bounds),
                Some(translation),
                None,
                None,
                None,
                source,
            ),
            Candidate::TextInput { state, .. } => (
                false,
                Some(data_address(state)),
                "text_input",
                None,
                None,
                Some(state.text().to_owned()),
                None,
                None,
                source,
            ),
            Candidate::Text { content, .. } => (
                false,
                None,
                "text",
                None,
                None,
                Some(content.to_owned()),
                None,
                None,
                source,
            ),
            Candidate::Custom { state, .. } => {
                if let Some(state) = state.downcast_ref::<SemanticSnapshot>() {
                    (
                        true,
                        Some(data_address(state)),
                        role_name(state.role),
                        None,
                        None,
                        state.value.clone(),
                        Some(AccessibilityData {
                            role: state.role,
                            name: state.label.clone(),
                            description: state.description.clone(),
                            value: state.value.clone(),
                            checked: state.checked,
                            expanded: state.expanded,
                            disabled: state.disabled,
                            focused: state.focused,
                            supports_activate: !state.disabled && state.supports_activate,
                            supports_focus: !state.disabled
                                && state.focus != crate::FocusBehavior::None,
                        }),
                        Some(state.focused),
                        state.source,
                    )
                } else {
                    (
                        false,
                        Some(data_address(state)),
                        "custom",
                        None,
                        None,
                        None,
                        None,
                        None,
                        source,
                    )
                }
            }
        };

        Some(LayoutTarget {
            semantic,
            semantic_group,
            state_key,
            kind: kind.to_owned(),
            bounds,
            visible_bounds,
            content_bounds,
            translation,
            value,
            accessibility,
            focused,
            source,
        })
    }

    fn description(&self) -> String {
        format!("logical id == {:?}", self.logical_id)
    }
}

fn role_name(role: accesskit::Role) -> &'static str {
    use accesskit::Role;

    match role {
        Role::Button | Role::DefaultButton => "button",
        Role::CheckBox => "checkbox",
        Role::Switch => "switch",
        Role::TextInput | Role::MultilineTextInput | Role::SearchInput | Role::PasswordInput => {
            "text_input"
        }
        Role::Label => "text",
        Role::Image => "image",
        Role::List => "list",
        Role::ListItem => "list_item",
        Role::Slider => "slider",
        Role::ProgressIndicator => "progress",
        _ => "semantic",
    }
}

fn accessibility_role_name(role: accesskit::Role) -> String {
    use accesskit::Role;

    match role {
        Role::Button | Role::DefaultButton => "button".to_owned(),
        Role::CheckBox => "checkbox".to_owned(),
        Role::TextInput => "text-input".to_owned(),
        Role::MultilineTextInput => "multiline-text-input".to_owned(),
        Role::SearchInput => "search-input".to_owned(),
        Role::PasswordInput => "password-input".to_owned(),
        Role::ListItem => "list-item".to_owned(),
        role => camel_to_kebab(&format!("{role:?}")),
    }
}

fn camel_to_kebab(value: &str) -> String {
    let mut output =
        String::with_capacity(value.len() + value.chars().filter(char::is_ascii_uppercase).count());
    let mut characters = value.chars().peekable();
    let mut previous: Option<char> = None;
    while let Some(character) = characters.next() {
        if character.is_ascii_uppercase() {
            let boundary = previous.is_some_and(|previous| {
                previous.is_ascii_lowercase()
                    || previous.is_ascii_digit()
                    || (previous.is_ascii_uppercase()
                        && characters
                            .peek()
                            .is_some_and(|next| next.is_ascii_lowercase()))
            });
            if boundary {
                output.push('-');
            }
            output.push(character.to_ascii_lowercase());
        } else {
            output.push(character);
        }
        previous = Some(character);
    }
    output
}

fn data_address<T: ?Sized>(value: &T) -> usize {
    value as *const T as *const () as usize
}

enum DriverEvent<Message> {
    Action(runtime::Action<Message>),
    Finished,
    Panicked(Box<dyn Any + Send>),
    SubscriptionStarted(SubscriptionKey),
    SubscriptionEventHandled(SubscriptionKey),
    SubscriptionStopped(SubscriptionKey),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct SubscriptionKey {
    id: u64,
    generation: u64,
}

struct SubscriptionState {
    key: SubscriptionKey,
    listening: AtomicBool,
    consumed: AtomicUsize,
}

impl SubscriptionState {
    fn new(key: SubscriptionKey) -> Self {
        Self {
            key,
            listening: AtomicBool::new(false),
            consumed: AtomicUsize::new(0),
        }
    }
}

struct SubscriptionInput {
    inner: subscription::EventStream,
    state: Arc<SubscriptionState>,
}

impl SubscriptionInput {
    fn new(inner: subscription::EventStream, state: Arc<SubscriptionState>) -> Self {
        state.listening.store(true, Ordering::Release);
        Self { inner, state }
    }
}

impl iced_test::futures::futures::Stream for SubscriptionInput {
    type Item = subscription::Event;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let result = self.inner.as_mut().poll_next(context);
        match result {
            Poll::Ready(Some(event)) => {
                self.state.consumed.fetch_add(1, Ordering::AcqRel);
                Poll::Ready(Some(event))
            }
            Poll::Ready(None) => {
                self.state.listening.store(false, Ordering::Release);
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl Drop for SubscriptionInput {
    fn drop(&mut self) {
        self.state.listening.store(false, Ordering::Release);
    }
}

struct PanicRecipe<Message> {
    inner: Box<dyn subscription::Recipe<Output = DriverEvent<Message>>>,
    state: Arc<SubscriptionState>,
    instance: u64,
}

struct SubscriptionStream<Message> {
    inner: iced_test::futures::BoxStream<DriverEvent<Message>>,
    state: Arc<SubscriptionState>,
    acknowledged: usize,
    started: bool,
    pending_start: bool,
    pending_events: usize,
    pending_stop: bool,
    terminal: bool,
}

impl<Message> SubscriptionStream<Message> {
    fn prepare_handoffs(&mut self, stopped: bool) {
        if !self.started {
            self.started = true;
            self.pending_start = true;
        }
        let consumed = self.state.consumed.load(Ordering::Acquire);
        self.pending_events += consumed.saturating_sub(self.acknowledged);
        self.acknowledged = consumed;
        self.pending_stop |= stopped;
    }

    fn next_handoff(&mut self) -> Option<DriverEvent<Message>> {
        if self.pending_start {
            self.pending_start = false;
            return Some(DriverEvent::SubscriptionStarted(self.state.key));
        }
        if self.pending_events > 0 {
            self.pending_events -= 1;
            return Some(DriverEvent::SubscriptionEventHandled(self.state.key));
        }
        if self.pending_stop {
            self.pending_stop = false;
            return Some(DriverEvent::SubscriptionStopped(self.state.key));
        }
        None
    }
}

impl<Message> iced_test::futures::futures::Stream for SubscriptionStream<Message> {
    type Item = DriverEvent<Message>;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Some(event) = self.next_handoff() {
            return Poll::Ready(Some(event));
        }
        if self.terminal {
            return Poll::Ready(None);
        }

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.inner.as_mut().poll_next(context)
        }));
        match result {
            Ok(Poll::Ready(Some(event))) => Poll::Ready(Some(event)),
            Ok(Poll::Ready(None)) => {
                self.terminal = true;
                self.prepare_handoffs(true);
                self.next_handoff()
                    .map_or(Poll::Ready(None), |event| Poll::Ready(Some(event)))
            }
            Ok(Poll::Pending) => {
                self.prepare_handoffs(false);
                self.next_handoff()
                    .map_or(Poll::Pending, |event| Poll::Ready(Some(event)))
            }
            Err(payload) => {
                self.terminal = true;
                Poll::Ready(Some(DriverEvent::Panicked(payload)))
            }
        }
    }
}

impl<Message: Send + 'static> subscription::Recipe for PanicRecipe<Message> {
    type Output = DriverEvent<Message>;

    fn hash(&self, state: &mut subscription::Hasher) {
        self.inner.hash(state);
    }

    fn stream(
        self: Box<Self>,
        input: subscription::EventStream,
    ) -> iced_test::futures::BoxStream<Self::Output> {
        let PanicRecipe {
            inner,
            state,
            instance,
        } = *self;
        let input = SubscriptionInput::new(input, Arc::clone(&state)).boxed();
        let stream =
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| inner.stream(input))) {
                Ok(stream) => stream,
                Err(payload) => {
                    return iced_test::futures::futures::stream::once(async {
                        DriverEvent::Panicked(payload)
                    })
                    .boxed();
                }
            };
        Instanced {
            inner: SubscriptionStream {
                inner: stream,
                state,
                acknowledged: 0,
                started: false,
                pending_start: false,
                pending_events: 0,
                pending_stop: false,
                terminal: false,
            },
            instance,
        }
        .boxed()
    }
}

/// A test build's `every Ns`: an interval that ticks off the driving test's
/// logical clock — which the driver broadcasts to every subscription as
/// `RedrawRequested` — instead of off the executor's wall-clock timers.
///
/// Wall time is what made `every` flake: under a loaded suite a test's wall
/// duration stretches across periods the test never scripted, and a reload
/// lands mid-assertion. Logical time moves only when the test says so —
/// `advance` moves it without waiting, `wait` moves it by exactly the waited
/// duration — so a tick count is a property of the script, not of the machine.
/// One redraw that crosses several periods delivers every crossed tick, the
/// way a wall-clock interval that fell behind would catch up.
pub fn every(period: Duration) -> iced::Subscription<Instant> {
    assert!(
        !period.is_zero(),
        "`every` requires a positive period, like the wall-clock interval it stands in for"
    );
    iced::advanced::subscription::from_recipe(LogicalEvery { period })
}

struct LogicalEvery {
    period: Duration,
}

impl subscription::Recipe for LogicalEvery {
    type Output = Instant;

    fn hash(&self, state: &mut subscription::Hasher) {
        use std::hash::Hash as _;
        TypeId::of::<Self>().hash(state);
        self.period.hash(state);
    }

    fn stream(
        self: Box<Self>,
        input: subscription::EventStream,
    ) -> iced_test::futures::BoxStream<Self::Output> {
        // Tracked from the driver's own thread, so the thread's instance is
        // the driver's and the published clock is that driver's logical time.
        // A stream started with no driver behind it phases from the wall
        // clock and then never fires, because only a driver broadcasts
        // redraws.
        let period = self.period;
        let mut next_due = published_logical_time(app_instance())
            .unwrap_or_else(Instant::now)
            .checked_add(period);
        input
            .flat_map(move |event| {
                let mut ticks = Vec::new();
                if let subscription::Event::Interaction {
                    event: iced::Event::Window(window::Event::RedrawRequested(at)),
                    ..
                } = event
                {
                    while let Some(due) = next_due
                        && due <= at
                    {
                        ticks.push(due);
                        next_due = due.checked_add(period);
                    }
                }
                iced_test::futures::futures::stream::iter(ticks)
            })
            .boxed()
    }
}

type HeadlessRuntime<P> = Runtime<
    <P as Program>::Executor,
    mpsc::Sender<DriverEvent<<P as Program>::Message>>,
    DriverEvent<<P as Program>::Message>,
>;

struct TestClipboard {
    standard: Option<String>,
    primary: Option<String>,
}

impl TestClipboard {
    fn value(&self, kind: clipboard::Kind) -> Option<String> {
        match kind {
            clipboard::Kind::Standard => self.standard.clone(),
            clipboard::Kind::Primary => self.primary.clone(),
        }
    }

    fn set(&mut self, kind: clipboard::Kind, value: String) {
        match kind {
            clipboard::Kind::Standard => self.standard = Some(value),
            clipboard::Kind::Primary => self.primary = Some(value),
        }
    }
}

impl iced::advanced::Clipboard for TestClipboard {
    fn read(&self, kind: clipboard::Kind) -> Option<String> {
        self.value(kind)
    }

    fn write(&mut self, kind: clipboard::Kind, contents: String) {
        self.set(kind, contents);
    }
}

/// A persistent headless Iced program runtime used by generated tests.
pub struct Driver<P>
where
    P: Program,
{
    program: P,
    state: P::State,
    runtime: HeadlessRuntime<P>,
    receiver: mpsc::Receiver<DriverEvent<P::Message>>,
    renderer: P::Renderer,
    cache: Option<user_interface::Cache>,
    clipboard: TestClipboard,
    cursor: mouse::Cursor,
    cursor_inside: bool,
    pressed_mouse: HashSet<MouseButton>,
    modifiers: Modifiers,
    pressed_keys: HashMap<HeldKeyIdentity, HeldKeyRecord>,
    touches: HashMap<u64, Point>,
    ime_open: bool,
    window: window::Id,
    size: Size,
    window_position: Option<Point>,
    window_focused: bool,
    scale_factor_override: Option<f32>,
    theme_override: Option<ThemeMode>,
    system_theme: ThemeMode,
    locale: Option<&'static str>,
    platform: Platform,
    reduced_motion: Option<bool>,
    logical_time: Instant,
    artifact_dir: PathBuf,
    timeout: Duration,
    test_name: &'static str,
    instance: u64,
    pending_tasks: usize,
    subscriptions: HashMap<u64, Arc<SubscriptionState>>,
    next_subscription_generation: u64,
    pending_subscription_starts: HashSet<SubscriptionKey>,
    pending_subscription_events: HashMap<SubscriptionKey, usize>,
    trace: Option<TraceRecorder>,
    action_index: usize,
    capture_before_action: Option<usize>,
    capture_after_action: Option<usize>,
    trace_capture_result: Option<PathBuf>,
}

impl<P> Driver<P>
where
    P: Program + 'static,
    P::Renderer: 'static,
    P::Message: Clone,
{
    #[track_caller]
    pub fn new(program: P, config: Config) -> Self {
        with_panic_context(config.name, config.source, || {
            Self::new_inner(program, config)
        })
    }

    fn new_inner(program: P, config: Config) -> Self {
        let trace_config = config.clone();
        // Claimed before the program boots, because a boot preset is already
        // application code and may already reach state this instance owns.
        let instance = app_instance();
        let boot_origin = || failure_origin(config.name, config.source);
        if !valid_dimension(config.viewport.width) || !valid_dimension(config.viewport.height) {
            panic!(
                "{}\nconfiguration failed\nexpected: finite, positive viewport dimensions\nactual: {:?}",
                boot_origin(),
                config.viewport
            );
        }
        if config.timeout.is_zero() {
            panic!(
                "{}\nconfiguration failed\nexpected: positive timeout\nactual: 0ns",
                boot_origin()
            );
        }
        if config
            .scale_factor
            .is_some_and(|scale| !valid_dimension(scale))
        {
            panic!(
                "{}\nconfiguration failed\nexpected: a finite, positive scale factor\nactual: {:?}",
                boot_origin(),
                config.scale_factor
            );
        }
        if config.locale.is_some_and(str::is_empty) {
            panic!(
                "{}\nconfiguration failed\nexpected: a non-empty locale\nactual: empty locale",
                boot_origin()
            );
        }
        if config
            .artifact_dir
            .as_ref()
            .is_some_and(|path| path.as_os_str().is_empty())
        {
            panic!(
                "{}\nconfiguration failed\nexpected: a non-empty artifact directory\nactual: empty path",
                boot_origin()
            );
        }
        let settings = program.settings();
        let executor = P::Executor::new().unwrap_or_else(|error| {
            panic!(
                "{}\nexpected: a working test executor\nactual: {error}",
                boot_origin()
            )
        });
        let backend =
            (TypeId::of::<P::Renderer>() == TypeId::of::<iced::Renderer>()).then_some("tiny-skia");
        let mut renderer = executor
            .block_on(P::Renderer::new(
                settings.default_font,
                settings.default_text_size,
                backend,
            ))
            .unwrap_or_else(|| {
                panic!(
                    "{}\nexpected: a headless renderer\nactual: renderer initialization returned unavailable",
                    boot_origin()
                )
            });

        for font in settings.fonts {
            iced_test::renderer::graphics::text::font_system()
                .write()
                .unwrap_or_else(|_| {
                    panic!(
                        "{}\nexpected: writable Iced font system\nactual: poisoned font-system lock",
                        boot_origin()
                    )
                })
                .load_font(font);
        }

        // Establish the viewport before any task-issued widget operation runs.
        renderer.reset(Rectangle::with_size(config.viewport));

        let (sender, receiver) = mpsc::channel(100);
        let runtime = Runtime::new(executor, sender);
        let (state, task) = match config.preset {
            Some(name) => program
                .presets()
                .iter()
                .find(|preset| preset.name() == name)
                .unwrap_or_else(|| {
                    let available = program
                        .presets()
                        .iter()
                        .map(|preset| preset.name())
                        .collect::<Vec<_>>()
                        .join(", ");
                    panic!(
                        "{}\nconfiguration failed\nexpected: one of [{}]\nactual: unknown preset `{name}`",
                        boot_origin(),
                        if available.is_empty() { "<none>" } else { &available },
                    )
                })
                .boot(),
            None => program.boot(),
        };

        // Published before `resubscribe` streams the first recipe: an `every`
        // phases its ticks from this clock the moment its stream starts.
        let logical_time = Instant::now();
        publish_logical_time(instance, logical_time);

        let mut driver = Self {
            program,
            state,
            runtime,
            receiver,
            renderer,
            cache: Some(user_interface::Cache::default()),
            clipboard: TestClipboard {
                standard: None,
                primary: None,
            },
            cursor: mouse::Cursor::Unavailable,
            cursor_inside: false,
            pressed_mouse: HashSet::new(),
            modifiers: Modifiers::NONE,
            pressed_keys: HashMap::new(),
            touches: HashMap::new(),
            ime_open: false,
            window: window::Id::unique(),
            size: config.viewport,
            window_position: None,
            window_focused: true,
            scale_factor_override: config.scale_factor,
            theme_override: config.theme,
            system_theme: config.system_theme,
            locale: config.locale,
            platform: config.platform,
            reduced_motion: config.reduced_motion,
            logical_time,
            artifact_dir: config.artifact_dir.unwrap_or_else(|| {
                std::env::var_os("ICE_TEST_ARTIFACT_DIR")
                    .filter(|path| !path.is_empty())
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from("target").join("ice-test-artifacts"))
                    .join(safe_path_component(config.name))
            }),
            timeout: config.timeout,
            test_name: config.name,
            instance,
            pending_tasks: 0,
            subscriptions: HashMap::new(),
            next_subscription_generation: 0,
            pending_subscription_starts: HashSet::new(),
            pending_subscription_events: HashMap::new(),
            trace: None,
            action_index: 0,
            capture_before_action: parsed_env("ICE_TRACE_CAPTURE_BEFORE_ACTION"),
            capture_after_action: parsed_env("ICE_TRACE_CAPTURE_ACTION"),
            trace_capture_result: std::env::var_os("ICE_TRACE_CAPTURE_RESULT").map(PathBuf::from),
        };
        driver.resubscribe(config.source);
        driver.run_task(task, config.source);
        driver.settle(config.source);
        driver.trace = TraceRecorder::from_env(&driver, &trace_config);
        driver
    }

    pub fn state(&self) -> &P::State {
        &self.state
    }

    /// The state a probe wants to put a screen into directly, rather than
    /// through the handlers that would reach it. Ablating one panel to price
    /// it is not a sequence of messages — several panels share a handler, and
    /// the ones that do not are cleared by a handler that clears three more.
    ///
    /// Not an interaction. A test that asserts about behaviour reaches its
    /// state through `dispatch`, because what the handlers do to state is
    /// part of what it is asserting.
    ///
    /// Not a state write either, as far as the generated program can tell:
    /// only the generated writers tick a field's revision and clear the
    /// derived cells that read it. A field written here keeps both, so a
    /// `derived` value over it hands the next frame its stale cell unless
    /// the probe resets `__ice_derived` to its default (or writes before the
    /// first redraw), and a `lazy` keyed on the field's revision goes on
    /// showing the subtree it cached before the write — a probe that writes
    /// here must not read the result through a state-rooted `lazy`.
    pub fn state_mut(&mut self) -> &mut P::State {
        &mut self.state
    }

    pub fn window(&self) -> window::Id {
        self.window
    }

    pub fn viewport(&self) -> Size {
        self.size
    }

    pub fn scale_factor(&self) -> f32 {
        let scale_factor = self
            .scale_factor_override
            .unwrap_or_else(|| self.program.scale_factor(&self.state, self.window));
        assert!(
            valid_dimension(scale_factor),
            "test `{}` runtime invariant failed\nexpected: finite, positive scale factor\nactual: {scale_factor:?}",
            self.test_name,
        );
        scale_factor
    }

    pub fn locale(&self) -> Option<&'static str> {
        self.locale
    }

    pub fn platform(&self) -> Platform {
        self.platform
    }

    pub fn reduced_motion(&self) -> Option<bool> {
        self.reduced_motion
    }

    pub fn dispatch(&mut self, message: P::Message, source: Location) {
        let test_name = self.test_name;
        with_panic_context(test_name, Some(source), || {
            self.update(message, Some(source));
            self.settle(Some(source));
        });
    }

    #[track_caller]
    pub fn check(&self, condition: bool, source: Location) {
        if !condition {
            panic!(
                "{source}: test `{}` boolean expectation failed\nstatement: {}\nexpected: true\nactual: false",
                self.test_name, source.statement
            );
        }
    }

    #[track_caller]
    pub fn check_eq<L, R>(&self, left: L, right: R, source: Location)
    where
        L: PartialEq<R> + fmt::Debug,
        R: fmt::Debug,
    {
        if left != right {
            panic!(
                "{source}: test `{}` equality expectation failed\nstatement: {}\nactual (left): {left:?}\nexpected (right): {right:?}",
                self.test_name, source.statement
            );
        }
    }

    #[track_caller]
    pub fn check_ne<L, R>(&self, left: L, right: R, source: Location)
    where
        L: PartialEq<R> + fmt::Debug,
        R: fmt::Debug,
    {
        if left == right {
            panic!(
                "{source}: test `{}` inequality expectation failed\nstatement: {}\nexpected: different values\nactual (left): {left:?}\nactual (right): {right:?}",
                self.test_name, source.statement
            );
        }
    }

    /// One declared state of a component instance, read through the generated
    /// seam. `None` is a scope no render ever sighted, which fails naming the
    /// scope — and the instances that ARE live — rather than comparing nothing.
    #[track_caller]
    pub fn check_component_state<T>(
        &self,
        scope: &str,
        actual: Option<T>,
        expected: T,
        negated: bool,
        live: &[String],
        source: Location,
    ) where
        T: PartialEq + fmt::Debug,
    {
        let Some(actual) = actual else {
            panic!(
                "{source}: test `{}` component state expectation failed\nstatement: {}\nexpected: a component instance at scope `{scope}`\nactual: no render materialized that scope\nlive instances: {live:?}",
                self.test_name, source.statement
            );
        };
        if negated {
            self.check_ne(actual, expected, source);
        } else {
            self.check_eq(actual, expected, source);
        }
    }

    #[track_caller]
    pub fn check_approx(&self, left: f64, right: f64, source: Location) {
        if !left.is_finite() || !right.is_finite() || (left - right).abs() > 0.001 {
            panic!(
                "{source}: test `{}` approximate expectation failed\nstatement: {}\nactual (left): {left:?}\nexpected (right): {right:?}\ntolerance: 0.001",
                self.test_name, source.statement
            );
        }
    }

    #[track_caller]
    pub fn check_exists(&mut self, id: &str, expected: bool, source: Location) {
        let target = self.inspect(id, false, source);
        let actual = target.is_some();
        if actual != expected {
            let details = target.as_ref().map_or_else(
                || {
                    format!(
                        "bounds: unavailable\nknown runtime ids: {}",
                        self.known_ids_display()
                    )
                },
                |target| format!("bounds: {:?}", target.bounds),
            );
            panic!(
                "{source}: test `{}` target presence expectation failed\nstatement: {}\nselector: {id}\nexpected: {}\nactual: {}\n{details}",
                self.test_name,
                source.statement,
                if expected { "present" } else { "missing" },
                if actual { "present" } else { "missing" },
            );
        }
    }

    #[track_caller]
    pub fn check_text(
        &mut self,
        value: &str,
        within: Option<&str>,
        negated: bool,
        source: Location,
    ) {
        self.redraw(source);
        let within = within.map(|id| self.require_target(id, false, source));
        // The question is about drawn ink, so the region is the target's
        // visible one: its layout bounds carried through the scroll offsets
        // above it and clipped to the window. Its layout bounds alone are
        // where it would sit unscrolled, and a card scrolled into view was
        // searched at the position it had scrolled away from. A target with
        // nothing visible has no region, and ink inside it is then missing —
        // the same answer as a target off the bottom of a window.
        let region = within.as_ref().map(|target| {
            target
                .visible
                .unwrap_or(Rectangle::new(Point::ORIGIN, Size::ZERO))
        });
        let actual = self.drawn_text_exists(value, region, source);
        if actual == negated {
            let selector = within.as_ref().map_or_else(
                || format!("visible text {value:?}"),
                |target| format!("visible text {value:?} within {}", target.id),
            );
            let bounds = within.as_ref().map_or_else(
                || format!("viewport: {:?}", self.size),
                |target| format!("bounds: {:?}\nvisible: {:?}", target.bounds, target.visible),
            );
            panic!(
                "{source}: test `{}` text expectation failed\nstatement: {}\nselector: {selector}\nexpected: {}\nactual: {}\n{bounds}",
                self.test_name,
                source.statement,
                if negated { "missing" } else { "present" },
                if actual { "present" } else { "missing" },
            );
        }
    }

    /// Asserts what the program last decided the status item should show.
    ///
    /// Does not redraw: the tray is not in the widget tree and nothing about
    /// it is on screen. `label` and `item` match by SUBSTRING, unlike
    /// [`Self::check_text`], which matches a drawn run exactly — a tray row is
    /// one composed string rather than a tree of runs, so the author asserts
    /// the fragment that carries the meaning. `item` matching by text and not
    /// by index is what makes reordering rows harmless and deleting one fatal.
    /// `icon` matches the declared path exactly. `command` asks whether the
    /// row carrying the text is one the reader can choose, and fails either
    /// way when no row carries it — a `no tray command` that passed because
    /// the text was misspelled would record nothing.
    ///
    /// `item` asks whether any row carries the text and `command` asks what
    /// one particular row is, so only the second needs the text to name a
    /// single row. Presence is still presence when two rows show it; deciding
    /// whether "the" row is choosable is not a question two rows have one
    /// answer to.
    #[track_caller]
    pub fn check_tray(&mut self, field: TrayField, value: &str, negated: bool, source: Location) {
        let tray = crate::tray::rendered();
        let actual = match field {
            TrayField::Label => tray.label.contains(value),
            TrayField::Icon => tray.icon == Some(value),
            TrayField::Item => !tray_rows_containing(&tray, value).is_empty(),
            TrayField::Command => crate::tray::is_command(self.tray_row(&tray, value, source)),
        };
        if actual == negated {
            let (selector, shown) = match field {
                TrayField::Label => ("tray label containing", format!("{:?}", tray.label)),
                TrayField::Icon => ("tray icon", format!("{:?}", tray.icon)),
                TrayField::Item | TrayField::Command => (
                    "a tray row containing",
                    format!(
                        "{:?}{}",
                        tray.items,
                        hidden_rows_note(&tray_hidden_rows_containing(&tray, value))
                    ),
                ),
            };
            panic!(
                "{source}: test `{}` tray expectation failed\nstatement: {}\nselector: {selector} {value:?}\nexpected: {}\nactual: {shown}",
                self.test_name,
                source.statement,
                if negated { "missing" } else { "present" },
            );
        }
    }

    /// The declaration index of the command row carrying `value`, for the
    /// `tray choose` step. Choosing is the platform's own act, so this fails
    /// on exactly what the platform would refuse: a row that is not there, and
    /// a stat, which macOS draws disabled and will not let anyone press.
    #[track_caller]
    pub fn tray_command_row(&mut self, value: &str, source: Location) -> usize {
        let tray = crate::tray::rendered();
        let row = self.tray_row(&tray, value, source);
        if !crate::tray::is_command(row) {
            let refusal = if crate::tray::is_submenu(row) {
                "a submenu, which the platform opens rather than delivers"
            } else {
                "a stat, which the platform draws disabled"
            };
            panic!(
                "{source}: test `{}` cannot choose a tray row that is not a command\nstatement: {}\nselector: a tray row containing {value:?}\nexpected: a row routed with `-> handler`\nactual: row {row} is {refusal}",
                self.test_name, source.statement,
            );
        }
        row
    }

    /// The one row carrying `value`, or a failure naming why there is not one.
    #[track_caller]
    fn tray_row(&self, tray: &crate::tray::TraySnapshot, value: &str, source: Location) -> usize {
        match tray_rows_containing(tray, value).as_slice() {
            [row] => *row,
            [] => self.tray_row_missing(value, source),
            rows => panic!(
                "{source}: test `{}` found {} tray rows containing {value:?}\nstatement: {}\nmatched: {:?}\nexpected: text that names one row\nactual: a menu is one flat table across every submenu, so a text carried by two rows names neither",
                self.test_name,
                rows.len(),
                source.statement,
                rows.iter()
                    .map(|row| tray.items[*row].clone())
                    .collect::<Vec<_>>(),
            ),
        }
    }

    #[track_caller]
    fn tray_row_missing(&self, value: &str, source: Location) -> ! {
        let tray = crate::tray::rendered();
        panic!(
            "{source}: test `{}` found no tray row containing {value:?}\nstatement: {}\nactual: {:?}{}",
            self.test_name,
            source.statement,
            tray.items,
            hidden_rows_note(&tray_hidden_rows_containing(&tray, value)),
        )
    }

    pub fn check_accessibility_str(
        &mut self,
        id: &str,
        property: AccessibilityProperty,
        expected: &str,
        source: Location,
    ) {
        // Every value below comes from the selector walk, so the frame this
        // pump delivers is for the statements after it, not this one:
        // `Target::accessibility` never calls `require_paint`, and drawing
        // the frame only to scan its quads would be dead work.
        let target = self.require_target(id, false, source);
        self.redraw(source);
        let actual = match property {
            AccessibilityProperty::Role => target.accessibility_role_name(),
            AccessibilityProperty::Name => target.accessibility_name(),
            AccessibilityProperty::Value => target.accessibility_value(),
            _ => self.invalid_action(
                "accessibility string expectation",
                "role, name, or value",
                format!("{property:?}"),
                source,
            ),
        };
        if actual != expected {
            self.accessibility_expectation_failed(
                id,
                property,
                format!("{expected:?}"),
                format!("{actual:?}"),
                source,
            );
        }
    }

    pub fn check_accessibility_bool(
        &mut self,
        id: &str,
        property: AccessibilityProperty,
        expected: bool,
        source: Location,
    ) {
        // Every value below comes from the selector walk, so the frame this
        // pump delivers is for the statements after it, not this one:
        // `Target::accessibility` never calls `require_paint`, and drawing
        // the frame only to scan its quads would be dead work.
        let target = self.require_target(id, false, source);
        self.redraw(source);
        let actual = match property {
            AccessibilityProperty::Checked => target.accessibility_checked(),
            AccessibilityProperty::Expanded => target.accessibility_expanded(),
            AccessibilityProperty::Disabled => target.accessibility_disabled(),
            AccessibilityProperty::Focused => target.accessibility_focused(),
            _ => self.invalid_action(
                "accessibility boolean expectation",
                "checked, expanded, disabled, or focused",
                format!("{property:?}"),
                source,
            ),
        };
        if actual != expected {
            self.accessibility_expectation_failed(
                id,
                property,
                expected.to_string(),
                actual.to_string(),
                source,
            );
        }
    }

    pub fn check_accessibility_action(
        &mut self,
        id: &str,
        action: AccessibilityAction,
        expected: bool,
        source: Location,
    ) {
        // Every value below comes from the selector walk, so the frame this
        // pump delivers is for the statements after it, not this one:
        // `Target::accessibility` never calls `require_paint`, and drawing
        // the frame only to scan its quads would be dead work.
        let target = self.require_target(id, false, source);
        self.redraw(source);
        let actual = match action {
            AccessibilityAction::Click => target.accessibility_supports_activate(),
            AccessibilityAction::Focus => target.accessibility_supports_focus(),
        };
        if actual != expected {
            self.accessibility_expectation_failed(
                id,
                AccessibilityProperty::Action,
                format!("action {action:?}: {expected}"),
                format!("action {action:?}: {actual}"),
                source,
            );
        }
    }

    fn accessibility_expectation_failed(
        &self,
        id: &str,
        property: AccessibilityProperty,
        expected: String,
        actual: String,
        source: Location,
    ) -> ! {
        panic!(
            "{source}: test `{}` accessibility expectation failed\nstatement: {}\nselector: {id}\nproperty: {property:?}\nexpected: {expected}\nactual: {actual}",
            self.test_name, source.statement
        )
    }

    /// Performs one semantic action. A capture action returns its artifact.
    pub fn perform_action(&mut self, action: Action, source: Location) -> Option<Capture> {
        let target_source = self
            .trace
            .is_some()
            .then(|| {
                trace::primary_target(&action).and_then(|target| self.target_render_source(target))
            })
            .flatten();
        if self.trace.is_some()
            || self.capture_before_action.is_some()
            || self.capture_after_action.is_some()
        {
            self.action_index = self.action_index.saturating_add(1);
        }
        self.capture_selected_trace_state(source, true);
        if matches!(&action, Action::Capture(_)) {
            if let Some(trace) = &mut self.trace {
                trace.record_untimed(&action, source, target_source);
            }
            let output = self.perform_action_inner(action, source);
            self.capture_selected_trace_state(source, false);
            return output;
        }
        if let Some(trace) = &mut self.trace {
            trace.begin(&action, source, target_source);
        }
        let output = self.perform_action_inner(action, source);
        if let Some(trace) = &mut self.trace {
            trace.finish();
        }
        self.capture_selected_trace_state(source, false);
        output
    }

    fn capture_selected_trace_state(&mut self, source: Location, before: bool) {
        let selected = if before {
            self.capture_before_action
        } else {
            self.capture_after_action
        };
        let Some(selected) = selected else {
            return;
        };
        if Some(selected) != self.action_index.checked_sub(1) {
            return;
        }
        let capture = self.capture(
            &format!("trace_worst_{}", self.action_index.saturating_sub(1)),
            source,
        );
        if let Some(path) = self.trace_capture_result.take() {
            let result = serde_json::json!({
                "action_index": self.action_index.saturating_sub(1),
                "png": capture.png_path,
                "manifest": capture.metadata_path,
            });
            std::fs::write(
                &path,
                serde_json::to_vec_pretty(&result).expect("trace capture result is serializable"),
            )
            .unwrap_or_else(|error| {
                panic!(
                    "{source}: cannot write trace capture result {}: {error}",
                    path.display()
                )
            });
        }
        if before {
            self.capture_before_action = None;
        } else {
            self.capture_after_action = None;
        }
    }

    fn perform_action_inner(&mut self, action: Action, source: Location) -> Option<Capture> {
        match action {
            Action::Leave => self.leave(source),
            Action::MoveTo(target) => self.move_to(&target, source),
            Action::MoveToPoint(position) => {
                self.move_to_point(position.x, position.y, source);
            }
            Action::Click {
                target,
                button,
                count,
            } => self.click_with(&target, button, count, source),
            Action::ClickAt {
                position,
                button,
                count,
            } => self.click_at_with_count(position.x, position.y, button, count, source),
            Action::Press { target, button } => self.press_with(&target, button, source),
            Action::Release(button) => self.release_button(button, source),
            Action::Wheel(delta) => self.wheel_delta(delta, source),
            Action::ScrollTo { target, x, y } => self.scroll_to(&target, x, y, source),
            Action::ScrollBy { target, x, y } => self.scroll_by(&target, x, y, source),
            Action::Snap { target, x, y } => self.snap(&target, x, y, source),
            Action::SnapEnd(target) => self.snap_end(&target, source),
            Action::Drag { from, to } => self.drag(&from, &to, source),
            Action::DropAt(target) => self.drop_at(&target, source),
            Action::Focus(target) => self.focus(&target, source),
            Action::FocusNext => self.focus_next(source),
            Action::FocusPrevious => self.focus_previous(source),
            Action::Blur => self.blur(source),
            Action::WindowFocus(focused) => self.window_focus(focused, source),
            Action::Type(value) => self.typewrite(&value, source),
            Action::Clear => self.clear(source),
            Action::Replace(value) => self.replace(&value, source),
            Action::Select { start, end } => self.select(start, end, source),
            Action::SelectAll => self.select_all(source),
            Action::Cursor(position) => self.cursor(position, source),
            Action::CursorFront => self.cursor_front(source),
            Action::CursorEnd => self.cursor_end(source),
            Action::Composition(phase) => self.composition(phase, source),
            Action::Key(key) => self.key(key, source),
            Action::KeyDown { key, metadata } => self.key_down_with(key, metadata, source),
            Action::KeyUp { key, metadata } => self.key_up_with(key, metadata, source),
            Action::Modifiers(modifiers) => self.modifiers(modifiers, source),
            Action::Chord { modifiers, key } => self.key_chord(modifiers, key, source),
            Action::Repeat { key, count } => self.key_repeat(key, count, source),
            Action::Touch {
                phase,
                id,
                position,
            } => self.touch(phase, id, position.x, position.y, source),
            Action::Tap { target, count } => self.tap(&target, count, source),
            Action::WindowOpened => self.window_opened(source),
            Action::WindowClosed => self.window_closed(source),
            Action::WindowMove(position) => self.window_move(position.x, position.y, source),
            Action::Resize(size) => self.resize(size.width, size.height, source),
            Action::Rescale(scale_factor) => self.rescale(scale_factor, source),
            Action::CloseRequested => self.close_requested(source),
            Action::Redraw => self.redraw(source),
            Action::SystemTheme(mode) => self.system_theme(mode, source),
            Action::FileHover(path) => self.file_hover(path, source),
            Action::FileDrop(path) => self.file_drop(path, source),
            Action::FileLeave => self.file_leave(source),
            Action::Wait(duration) => self.wait(duration, source),
            Action::Advance(duration) => self.advance(duration, source),
            Action::Idle => self.idle(source),
            Action::Capture(name) => return Some(self.capture(&name, source)),
            Action::Accessibility { action, target } => match action {
                AccessibilityAction::Click => self.accessibility_activate(&target, source),
                AccessibilityAction::Focus => self.accessibility_focus(&target, source),
            },
        }
        None
    }

    pub fn leave(&mut self, source: Location) {
        self.cursor_inside = false;
        self.cursor = mouse::Cursor::Unavailable;
        self.simulate([iced::Event::Mouse(mouse::Event::CursorLeft)], source);
    }

    pub fn move_to(&mut self, id: &str, source: Location) {
        let bounds = self.interaction_bounds("move to", id, source);
        self.set_cursor(bounds.center(), true, source);
    }

    pub fn move_to_point(&mut self, x: f32, y: f32, source: Location) {
        self.require_point("move", Point::new(x, y), source);
        self.set_cursor(Point::new(x, y), true, source);
    }

    pub fn click_with(&mut self, id: &str, button: MouseButton, count: u8, source: Location) {
        let bounds = self.interaction_bounds("click", id, source);
        self.set_cursor(bounds.center(), true, source);
        self.click_current(button, count, source);
    }

    pub fn click_at(&mut self, x: f32, y: f32, button: MouseButton, source: Location) {
        self.click_at_with_count(x, y, button, 1, source);
    }

    pub fn click_at_with_count(
        &mut self,
        x: f32,
        y: f32,
        button: MouseButton,
        count: u8,
        source: Location,
    ) {
        let position = Point::new(x, y);
        self.require_point("click", position, source);
        self.set_cursor(position, true, source);
        self.click_current(button, count, source);
    }

    pub fn press_with(&mut self, id: &str, button: MouseButton, source: Location) {
        let bounds = self.interaction_bounds("press", id, source);
        self.set_cursor(bounds.center(), true, source);
        self.press_current(button, source);
    }

    pub fn release_button(&mut self, button: MouseButton, source: Location) {
        if !self.pressed_mouse.remove(&button) {
            self.invalid_action(
                "release pointer button",
                "a pressed pointer button",
                format!("{button:?} is not pressed"),
                source,
            );
        }
        self.simulate(
            [iced::Event::Mouse(mouse::Event::ButtonReleased(
                button.iced(),
            ))],
            source,
        );
    }

    pub fn wheel_lines(&mut self, x: f32, y: f32, source: Location) {
        self.wheel_delta(WheelDelta::Lines { x, y }, source);
    }

    pub fn wheel_delta(&mut self, delta: WheelDelta, source: Location) {
        let (x, y) = match delta {
            WheelDelta::Lines { x, y } | WheelDelta::Pixels { x, y } => (x, y),
        };
        self.require_finite_pair("wheel", x, y, source);
        let delta = match delta {
            WheelDelta::Lines { x, y } => mouse::ScrollDelta::Lines { x, y },
            WheelDelta::Pixels { x, y } => mouse::ScrollDelta::Pixels { x, y },
        };
        self.simulate(
            [iced::Event::Mouse(mouse::Event::WheelScrolled { delta })],
            source,
        );
    }

    pub fn scroll_to(&mut self, id: &str, x: f32, y: f32, source: Location) {
        self.require_finite_pair("scroll to", x, y, source);
        let scroll_id = self.require_scroll_target(id, source);
        self.perform_widget(Box::new(
            iced::advanced::widget::operation::scrollable::scroll_to::<()>(
                scroll_id,
                iced::advanced::widget::operation::scrollable::AbsoluteOffset {
                    x: Some(x),
                    y: Some(y),
                },
            ),
        ));
        self.settle(Some(source));
    }

    pub fn scroll_by(&mut self, id: &str, x: f32, y: f32, source: Location) {
        self.require_finite_pair("scroll by", x, y, source);
        let scroll_id = self.require_scroll_target(id, source);
        self.perform_widget(Box::new(
            iced::advanced::widget::operation::scrollable::scroll_by::<()>(
                scroll_id,
                iced::advanced::widget::operation::scrollable::AbsoluteOffset { x, y },
            ),
        ));
        self.settle(Some(source));
    }

    pub fn snap(&mut self, id: &str, x: f32, y: f32, source: Location) {
        if !(0.0..=1.0).contains(&x) || !(0.0..=1.0).contains(&y) {
            self.invalid_action(
                "snap scroll",
                "finite normalized x and y in 0..=1",
                format!("({x:?}, {y:?})"),
                source,
            );
        }
        let scroll_id = self.require_scroll_target(id, source);
        self.perform_widget(Box::new(
            iced::advanced::widget::operation::scrollable::snap_to::<()>(
                scroll_id,
                iced::advanced::widget::operation::scrollable::RelativeOffset {
                    x: Some(x),
                    y: Some(y),
                },
            ),
        ));
        self.settle(Some(source));
    }

    pub fn snap_end(&mut self, id: &str, source: Location) {
        self.snap(id, 1.0, 1.0, source);
    }

    pub fn drag(&mut self, from: &str, to: &str, source: Location) {
        let from = self.interaction_bounds("drag from", from, source).center();
        let to = self.interaction_bounds("drag to", to, source).center();
        self.set_cursor(from, true, source);
        self.press_current(MouseButton::Left, source);
        self.set_cursor(to, true, source);
        self.release_button(MouseButton::Left, source);
    }

    pub fn drop_at(&mut self, id: &str, source: Location) {
        let bounds = self.interaction_bounds("drop at", id, source);
        self.set_cursor(bounds.center(), true, source);
        self.release_button(MouseButton::Left, source);
    }

    pub fn focus(&mut self, id: &str, source: Location) {
        let target = self.require_target(id, false, source);
        if target.accessibility.is_some() {
            self.accessibility_focus(id, source);
            return;
        }
        let focus_id = self.require_widget_capability(
            "focus",
            id,
            WidgetCapability::Focusable,
            "a focusable target",
            source,
        );
        self.perform_widget(Box::new(
            iced::advanced::widget::operation::focusable::focus::<()>(focus_id),
        ));
        self.settle(Some(source));
        if !self.require_target(id, false, source).focused() {
            self.invalid_action(
                "focus",
                "the target to gain focus",
                format!("{id} remained unfocused"),
                source,
            );
        }
    }

    pub fn focus_next(&mut self, source: Location) {
        self.perform_widget(Box::new(
            iced::advanced::widget::operation::focusable::focus_next::<()>(),
        ));
        self.settle(Some(source));
    }

    pub fn focus_previous(&mut self, source: Location) {
        self.perform_widget(Box::new(
            iced::advanced::widget::operation::focusable::focus_previous::<()>(),
        ));
        self.settle(Some(source));
    }

    pub fn blur(&mut self, source: Location) {
        self.perform_widget(Box::new(
            iced::advanced::widget::operation::focusable::unfocus::<()>(),
        ));
        self.settle(Some(source));
    }

    pub fn window_focus(&mut self, focused: bool, source: Location) {
        self.window_focused = focused;
        self.simulate(
            [iced::Event::Window(if focused {
                window::Event::Focused
            } else {
                window::Event::Unfocused
            })],
            source,
        );
    }

    pub fn typewrite(&mut self, text: &str, source: Location) {
        for character in text.chars() {
            let key = Key::character(character.to_string());
            self.key_down_with(
                key.clone(),
                KeyMetadata {
                    text: Some(character.to_string()),
                    ..KeyMetadata::default()
                },
                source,
            );
            self.key_up(key, source);
        }
    }

    pub fn key(&mut self, key: Key, source: Location) {
        self.key_down(key.clone(), source);
        self.key_up(key, source);
    }

    pub fn key_down(&mut self, key: Key, source: Location) {
        self.key_down_with(key, KeyMetadata::default(), source);
    }

    pub fn key_down_with(&mut self, key: Key, metadata: KeyMetadata, source: Location) {
        let identity = held_key_identity(&key, &metadata);
        let held = self.pressed_keys.get(&identity);
        if metadata.repeat && held.is_none() {
            self.invalid_action(
                "key repeat",
                "a key that is already pressed",
                format!("{identity:?} is not pressed"),
                source,
            );
        }
        if metadata.repeat
            && let Some(held) = held
            && held.location != metadata.location
        {
            self.invalid_action(
                "key repeat",
                "the same key location as the initial press",
                format!(
                    "repeat location {:?} does not match pressed location {:?}",
                    metadata.location, held.location,
                ),
                source,
            );
        }
        if !metadata.repeat
            && let Some(held) = held
        {
            self.invalid_action(
                "key down",
                "a key that is not already pressed",
                format!("{identity:?} is already pressed as {:?}", held.key),
                source,
            );
        }
        let event = self.keyboard_pressed_event(&key, &metadata, source);
        if !metadata.repeat {
            self.pressed_keys.insert(
                identity,
                HeldKeyRecord {
                    key,
                    location: metadata.location,
                },
            );
        }
        self.simulate([iced::Event::Keyboard(event)], source);
    }

    pub fn key_up(&mut self, key: Key, source: Location) {
        self.key_up_with(key, KeyMetadata::default(), source);
    }

    pub fn key_up_with(&mut self, key: Key, metadata: KeyMetadata, source: Location) {
        let event = self.keyboard_released_event(&key, &metadata, source);
        let identity = held_key_identity(&key, &metadata);
        let Some(held) = self.pressed_keys.get(&identity) else {
            self.invalid_action(
                "key up",
                "a key that is currently pressed",
                format!("{identity:?} is not pressed"),
                source,
            );
        };
        if held.location != metadata.location {
            self.invalid_action(
                "key up",
                "the same key location as the initial press",
                format!(
                    "release location {:?} does not match pressed location {:?}",
                    metadata.location, held.location,
                ),
                source,
            );
        }
        self.pressed_keys.remove(&identity);
        self.simulate([iced::Event::Keyboard(event)], source);
    }

    pub fn modifiers(&mut self, modifiers: Modifiers, source: Location) {
        self.modifiers = modifiers;
        self.simulate(
            [iced::Event::Keyboard(keyboard::Event::ModifiersChanged(
                modifiers.iced(),
            ))],
            source,
        );
    }

    pub fn key_chord(&mut self, modifiers: Modifiers, key: Key, source: Location) {
        let previous = self.modifiers;
        self.modifiers(modifiers, source);
        self.key(key, source);
        self.modifiers(previous, source);
    }

    pub fn key_repeat(&mut self, key: Key, count: usize, source: Location) {
        if count == 0 {
            self.invalid_action(
                "repeat key",
                "a positive repeat count",
                "0".to_owned(),
                source,
            );
        }
        self.key_down(key.clone(), source);
        for _ in 1..count {
            self.key_down_with(
                key.clone(),
                KeyMetadata {
                    repeat: true,
                    ..KeyMetadata::default()
                },
                source,
            );
        }
        self.key_up(key, source);
    }

    pub fn clear(&mut self, source: Location) {
        self.select_all(source);
        self.key(Key::named(keyboard::key::Named::Backspace), source);
    }

    pub fn replace(&mut self, text: &str, source: Location) {
        self.clear(source);
        self.typewrite(text, source);
    }

    pub fn select(&mut self, start: usize, end: usize, source: Location) {
        let id = self.require_focused_text_input("select text", source);
        self.perform_widget(Box::new(
            iced::advanced::widget::operation::text_input::select_range::<()>(id, start, end),
        ));
        self.settle(Some(source));
    }

    pub fn select_all(&mut self, source: Location) {
        let id = self.require_focused_text_input("select all text", source);
        self.perform_widget(Box::new(
            iced::advanced::widget::operation::text_input::select_all::<()>(id),
        ));
        self.settle(Some(source));
    }

    pub fn cursor(&mut self, position: usize, source: Location) {
        let id = self.require_focused_text_input("move text cursor", source);
        self.perform_widget(Box::new(
            iced::advanced::widget::operation::text_input::move_cursor_to::<()>(id, position),
        ));
        self.settle(Some(source));
    }

    pub fn cursor_front(&mut self, source: Location) {
        let id = self.require_focused_text_input("move text cursor to front", source);
        self.perform_widget(Box::new(
            iced::advanced::widget::operation::text_input::move_cursor_to_front::<()>(id),
        ));
        self.settle(Some(source));
    }

    pub fn cursor_end(&mut self, source: Location) {
        let id = self.require_focused_text_input("move text cursor to end", source);
        self.perform_widget(Box::new(
            iced::advanced::widget::operation::text_input::move_cursor_to_end::<()>(id),
        ));
        self.settle(Some(source));
    }

    pub fn composition(&mut self, phase: CompositionPhase, source: Location) {
        let events = match phase {
            CompositionPhase::Start => {
                if self.ime_open {
                    self.invalid_action(
                        "IME start",
                        "a closed composition",
                        "composition is already open".to_owned(),
                        source,
                    );
                }
                self.ime_open = true;
                vec![iced::Event::InputMethod(input_method::Event::Opened)]
            }
            CompositionPhase::Update { text, selection } => {
                self.require_ime("update", source);
                if let Some(range) = &selection
                    && (range.start > range.end
                        || range.end > text.len()
                        || !text.is_char_boundary(range.start)
                        || !text.is_char_boundary(range.end))
                {
                    self.invalid_action(
                        "IME update",
                        "an ordered UTF-8 byte range within the composition text at character boundaries",
                        format!("selection {range:?} for {text:?} ({} bytes)", text.len()),
                        source,
                    );
                }
                vec![iced::Event::InputMethod(input_method::Event::Preedit(
                    text, selection,
                ))]
            }
            CompositionPhase::Commit(text) => {
                self.require_ime("commit", source);
                vec![
                    iced::Event::InputMethod(input_method::Event::Preedit(String::new(), None)),
                    iced::Event::InputMethod(input_method::Event::Commit(text)),
                ]
            }
            CompositionPhase::Cancel => {
                self.require_ime("cancel", source);
                self.ime_open = false;
                vec![
                    iced::Event::InputMethod(input_method::Event::Preedit(String::new(), None)),
                    iced::Event::InputMethod(input_method::Event::Closed),
                ]
            }
        };
        self.simulate(events, source);
    }

    pub fn touch(&mut self, phase: TouchPhase, id: u64, x: f32, y: f32, source: Location) {
        let position = Point::new(x, y);
        self.require_point("touch", position, source);
        let finger = touch::Finger(id);
        let event = match phase {
            TouchPhase::Down => {
                if self.touches.contains_key(&id) {
                    self.invalid_action(
                        "touch down",
                        "an unused touch id",
                        format!("touch {id} is already active"),
                        source,
                    );
                }
                self.touches.insert(id, position);
                touch::Event::FingerPressed {
                    id: finger,
                    position,
                }
            }
            TouchPhase::Move => {
                self.require_touch(id, "move", source);
                self.touches.insert(id, position);
                touch::Event::FingerMoved {
                    id: finger,
                    position,
                }
            }
            TouchPhase::Up => {
                self.require_touch(id, "lift", source);
                self.touches.remove(&id);
                touch::Event::FingerLifted {
                    id: finger,
                    position,
                }
            }
            TouchPhase::Cancel => {
                self.require_touch(id, "cancel", source);
                self.touches.remove(&id);
                touch::Event::FingerLost {
                    id: finger,
                    position,
                }
            }
        };
        self.simulate([iced::Event::Touch(event)], source);
    }

    pub fn tap(&mut self, id: &str, count: u8, source: Location) {
        if count == 0 {
            self.invalid_action("tap", "a positive tap count", "0".to_owned(), source);
        }
        let position = self.interaction_bounds("tap", id, source).center();
        let mut finger = 0_u64;
        while self.touches.contains_key(&finger) {
            finger = finger.checked_add(1).unwrap_or_else(|| {
                self.invalid_action(
                    "tap",
                    "an unused touch id",
                    "all touch ids are active".to_owned(),
                    source,
                )
            });
        }
        for _ in 0..count {
            self.touch(TouchPhase::Down, finger, position.x, position.y, source);
            self.touch(TouchPhase::Up, finger, position.x, position.y, source);
        }
    }

    pub fn window_opened(&mut self, source: Location) {
        self.simulate(
            [iced::Event::Window(window::Event::Opened {
                position: self.window_position,
                size: self.size,
            })],
            source,
        );
    }

    pub fn window_closed(&mut self, source: Location) {
        self.simulate([iced::Event::Window(window::Event::Closed)], source);
    }

    pub fn window_move(&mut self, x: f32, y: f32, source: Location) {
        let position = Point::new(x, y);
        if !x.is_finite() || !y.is_finite() {
            self.invalid_action(
                "move window",
                "finite coordinates",
                format!("{position:?}"),
                source,
            );
        }
        self.window_position = Some(position);
        self.simulate(
            [iced::Event::Window(window::Event::Moved(position))],
            source,
        );
    }

    pub fn resize(&mut self, width: f32, height: f32, source: Location) {
        if !valid_dimension(width) || !valid_dimension(height) {
            self.invalid_action(
                "resize",
                "finite, positive width and height",
                format!("({width:?}, {height:?})"),
                source,
            );
        }
        let size = Size::new(width, height);
        self.size = size;
        self.renderer.reset(Rectangle::with_size(size));
        self.simulate([iced::Event::Window(window::Event::Resized(size))], source);
    }

    pub fn rescale(&mut self, scale_factor: f32, source: Location) {
        if !valid_dimension(scale_factor) {
            self.invalid_action(
                "rescale",
                "a finite, positive scale factor",
                format!("{scale_factor:?}"),
                source,
            );
        }
        self.scale_factor_override = Some(scale_factor);
        self.simulate(
            [iced::Event::Window(window::Event::Rescaled(scale_factor))],
            source,
        );
    }

    pub fn close_requested(&mut self, source: Location) {
        self.simulate([iced::Event::Window(window::Event::CloseRequested)], source);
    }

    pub fn system_theme(&mut self, mode: ThemeMode, source: Location) {
        self.system_theme = mode;
        self.broadcast(subscription::Event::SystemThemeChanged(mode.iced()));
        self.settle(Some(source));
    }

    pub fn file_hover(&mut self, path: impl Into<PathBuf>, source: Location) {
        self.simulate(
            [iced::Event::Window(window::Event::FileHovered(path.into()))],
            source,
        );
    }

    pub fn file_drop(&mut self, path: impl Into<PathBuf>, source: Location) {
        self.simulate(
            [iced::Event::Window(window::Event::FileDropped(path.into()))],
            source,
        );
    }

    pub fn file_leave(&mut self, source: Location) {
        self.simulate(
            [iced::Event::Window(window::Event::FilesHoveredLeft)],
            source,
        );
    }

    /// Waits for real executor time, then advances the logical clock by the
    /// waited duration, redraws, and settles.
    ///
    /// The sleep is real because tasks and streams run real futures; the
    /// logical step is the *requested* duration rather than the measured one,
    /// so an `every` — which ticks off the logical clock — fires the same
    /// count on a loaded machine as on an idle one.
    pub fn wait(&mut self, duration: Duration, source: Location) {
        if duration.is_zero() {
            self.invalid_action("wait", "a positive duration", "0ns".to_owned(), source);
        }
        std::thread::sleep(duration);
        let time = self.logical_time.checked_add(duration).unwrap_or_else(|| {
            self.invalid_action(
                "wait",
                "a duration within the platform Instant range",
                format!("{duration:?}"),
                source,
            )
        });
        self.set_logical_time(time);
        self.redraw_at(self.logical_time, source);
    }

    /// Advances the deterministic redraw timestamp without sleeping.
    pub fn advance(&mut self, duration: Duration, source: Location) {
        if duration.is_zero() {
            self.invalid_action("advance", "a positive duration", "0ns".to_owned(), source);
        }
        let time = self.logical_time.checked_add(duration).unwrap_or_else(|| {
            self.invalid_action(
                "advance",
                "a duration within the platform Instant range",
                format!("{duration:?}"),
                source,
            )
        });
        self.set_logical_time(time);
        self.redraw_at(self.logical_time, source);
    }

    /// Moves the logical clock, for this driver and for the `every` streams
    /// that phase themselves from its published copy. Nothing else moves it:
    /// the wall clock a loaded machine spends on reading the screen must not
    /// leak into the application's time.
    fn set_logical_time(&mut self, time: Instant) {
        self.logical_time = time;
        publish_logical_time(self.instance, time);
    }

    pub fn idle(&mut self, source: Location) {
        self.settle(Some(source));
    }

    pub fn accessibility_activate(&mut self, id: &str, source: Location) {
        let target = self.require_semantic_action_target(id, source);
        if target.disabled {
            self.invalid_action(
                "accessibility activate",
                "an enabled semantic target",
                format!("{id} is disabled"),
                source,
            );
        }
        let message = target.activate.unwrap_or_else(|| {
            self.invalid_action(
                "accessibility activate",
                "a target supporting the Click action",
                format!("{id} has no activation action"),
                source,
            )
        });
        self.dispatch(message, source);
    }

    pub fn accessibility_focus(&mut self, id: &str, source: Location) {
        let target = self.require_semantic_action_target(id, source);
        if target.disabled {
            self.invalid_action(
                "accessibility focus",
                "an enabled semantic target",
                format!("{id} is disabled"),
                source,
            );
        }
        let focus = target.focus.unwrap_or_else(|| {
            self.invalid_action(
                "accessibility focus",
                "a target supporting the Focus action",
                format!("{id} is not focusable"),
                source,
            )
        });
        self.run_task(crate::focus_semantic(focus), Some(source));
        self.settle(Some(source));
    }

    fn require_semantic_action_target(
        &mut self,
        id: &str,
        source: Location,
    ) -> SemanticActionTarget<P::Message> {
        let mut targets = self.with_interface(|interface, renderer, _| {
            let mut operation = SemanticActionSelector::<P::Message>::new(id).find_all();
            interface.operate(renderer, &mut widget::operation::black_box(&mut operation));
            match operation.finish() {
                Outcome::Some(targets) => targets,
                _ => Vec::new(),
            }
        });
        match targets.len() {
            1 => targets.pop().expect("length checked"),
            0 => self.invalid_action(
                "accessibility action",
                "one semantic target",
                format!("no semantic target `{id}`"),
                source,
            ),
            count => self.invalid_action(
                "accessibility action",
                "one unambiguous semantic target",
                format!("{count} semantic targets matched `{id}`"),
                source,
            ),
        }
    }

    /// Captures a PNG and a structured JSON frame manifest.
    pub fn capture(&mut self, name: &str, source: Location) -> Capture {
        if name.is_empty()
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        {
            self.invalid_action(
                "capture",
                "a non-empty snake_case artifact name",
                format!("{name:?}"),
                source,
            );
        }

        let mut targets = self
            .known_ids()
            .into_iter()
            .filter_map(|id| self.inspect(&id, false, source))
            .collect::<Vec<_>>();
        let resolved_theme = self.theme();
        let screenshot = self.screenshot_with_theme(&resolved_theme, Some(source));
        for target in &mut targets {
            match inspect_paint(&mut self.renderer, target.bounds) {
                Ok(paint) => {
                    target.paint_error = None;
                    target.surfaces = paint.surfaces;
                    target.texts = paint.texts;
                    target.images = paint.images;
                }
                Err(error) => target.paint_error = Some(error),
            }
        }
        let targets = targets.iter().map(target_manifest).collect::<Vec<_>>();
        std::fs::create_dir_all(&self.artifact_dir).unwrap_or_else(|error| {
            self.invalid_action(
                "capture",
                "a writable artifact directory",
                format!("{} ({error})", self.artifact_dir.display()),
                source,
            )
        });
        let artifact_dir = self
            .artifact_dir
            .canonicalize()
            .unwrap_or_else(|_| self.artifact_dir.clone());
        let png_path = artifact_dir.join(format!("{name}.png"));
        let metadata_path = artifact_dir.join(format!("{name}.json"));
        write_png(
            &png_path,
            screenshot.as_ref(),
            screenshot.size.width,
            screenshot.size.height,
        )
        .unwrap_or_else(|error| {
            self.invalid_action(
                "capture",
                "a writable PNG artifact",
                format!("{} ({error})", png_path.display()),
                source,
            )
        });

        let manifest = serde_json::json!({
            "schema_version": 2,
            "name": name,
            "png": format!("{name}.png"),
            "capture_source": {
                "path": manifest_source_path(source.path),
                "line": source.line,
                "column": source.column,
                "statement": source.statement,
            },
            "viewport": { "width": self.size.width, "height": self.size.height },
            "physical_size": {
                "width": screenshot.size.width,
                "height": screenshot.size.height,
            },
            "scale_factor": screenshot.scale_factor,
            "configured_theme": self.theme_override.map(theme_mode_name),
            "resolved_theme": {
                "mode": theme_mode_name(theme_mode(resolved_theme.mode())),
                "name": resolved_theme.name(),
            },
            "system_theme": theme_mode_name(self.system_theme),
            "locale": self.locale,
            "platform": platform_name(self.platform),
            "reduced_motion": self.reduced_motion,
            "window": {
                "position": self.window_position.map(|point| serde_json::json!({
                    "x": point.x,
                    "y": point.y,
                })),
                "focused": self.window_focused,
            },
            "clock": {
                "supports_virtual_redraw_advance": true,
                "iced_timer_futures_are_virtual": false,
            },
            "targets": targets,
        });
        std::fs::write(
            &metadata_path,
            serde_json::to_vec_pretty(&manifest).expect("JSON values are serializable"),
        )
        .unwrap_or_else(|error| {
            self.invalid_action(
                "capture",
                "a writable JSON artifact",
                format!("{} ({error})", metadata_path.display()),
                source,
            )
        });

        Capture {
            name: name.to_owned(),
            rgba: screenshot.rgba.to_vec(),
            width: screenshot.size.width,
            height: screenshot.size.height,
            scale_factor: screenshot.scale_factor,
            png_path,
            metadata_path,
        }
    }

    #[track_caller]
    pub fn target(&mut self, id: &str, source: Location) -> Target {
        self.require_target(id, true, source)
    }

    fn require_target(&mut self, id: &str, paint: bool, source: Location) -> Target {
        self.inspect(id, paint, source).unwrap_or_else(|| {
            let nearby = self.known_ids();
            panic!(
                "{source}: test `{}` could not find target `{id}`\nstatement: {}\nselector: {id}\nexpected: present\nactual: missing\nbounds: unavailable\nknown runtime ids: {}",
                self.test_name,
                source.statement,
                known_ids_display(&nearby),
            )
        })
    }

    fn drawn_text_exists(
        &mut self,
        text: &str,
        within: Option<Rectangle>,
        source: Location,
    ) -> bool {
        let theme = self.theme();
        let style = self.program.style(&self.state, &theme);
        let cursor = self.cursor;
        let rendered = self.with_interface(|interface, renderer, clipboard| {
            // An Ice `layer` draws through iced's overlay, and a freshly built
            // interface draws only an overlay it has already laid out — so
            // without this the modal's ink is missing from the index entirely
            // and every question about it answers "missing", including the
            // negative form, which then passes for text plainly on screen.
            // An update with no events lays the overlay out and dispatches
            // nothing.
            let _ = interface.update(&[], cursor, renderer, clipboard, &mut Vec::new());
            interface.draw(
                renderer,
                &theme,
                &iced::advanced::renderer::Style {
                    text_color: style.text_color,
                },
                cursor,
            );
            rendered_text_exists(renderer, text, within)
        });
        rendered.unwrap_or_else(|reason| {
            panic!(
                "{source}: test `{}` could not complete a visible-text query\nstatement: {}\nselector: visible text {text:?}\nexpected: a complete rendered-text search\nactual: unavailable ({reason})\nsearch bounds: {:?}",
                self.test_name,
                source.statement,
                within.unwrap_or(Rectangle::with_size(self.size)),
            )
        })
    }

    fn inspect(&mut self, id: &str, paint: bool, source: Location) -> Option<Target> {
        let mut layouts = self.with_interface(|interface, renderer, _| {
            find_targets::<P::Message, P::Renderer>(interface, renderer, id)
        });
        normalize_target_matches(&mut layouts);

        if layouts.len() > 1 {
            let bounds = layouts
                .iter()
                .enumerate()
                .map(|(index, target)| format!("{}: {:?}", index + 1, target.bounds))
                .collect::<Vec<_>>()
                .join(", ");
            let nearby = self.known_ids_display();
            panic!(
                "{source}: test `{}` target lookup is ambiguous\nstatement: {}\nselector: {id}\nexpected: exactly 1 candidate\nactual: {} candidates\ncandidate bounds: [{bounds}]\nknown runtime ids: {nearby}",
                self.test_name,
                source.statement,
                layouts.len(),
            );
        }
        let layout = layouts.pop()?;

        let id = id.to_owned();
        let test_name = self.test_name;
        let paint = if paint {
            let cursor = self.cursor;
            let theme = self.theme();
            let style = self.program.style(&self.state, &theme);
            let paint_bounds = layout.bounds;
            let events = vec![iced::Event::Window(window::Event::RedrawRequested(
                self.logical_time,
            ))];
            let (paint, messages, statuses) =
                self.with_interface(|interface, renderer, clipboard| {
                    let mut messages = Vec::new();
                    let (_, statuses) =
                        interface.update(&events, cursor, renderer, clipboard, &mut messages);
                    interface.draw(
                        renderer,
                        &theme,
                        &iced::advanced::renderer::Style {
                            text_color: style.text_color,
                        },
                        cursor,
                    );
                    let paint = inspect_paint(renderer, paint_bounds);
                    (paint, messages, statuses)
                });
            self.finish_simulation(events, messages, statuses, source);
            paint
        } else {
            Ok(PaintInspection::default())
        };

        Some(target_from_layout(
            id,
            test_name,
            source,
            layout,
            paint,
            self.scale_factor(),
        ))
    }

    fn target_render_source(&mut self, id: &str) -> Option<Location> {
        let mut layouts = self.with_interface(|interface, renderer, _| {
            find_targets::<P::Message, P::Renderer>(interface, renderer, id)
        });
        normalize_target_matches(&mut layouts);
        match layouts.as_slice() {
            [layout] => layout.source,
            _ => None,
        }
    }

    /// Every walk runs against one interface. `with_interface` rebuilds the
    /// view and re-lays it out on each call, so asking per id — once for the
    /// layout and twice for capabilities — cost the fuzz loop `1 + 3n` view
    /// builds per generated action where one does.
    fn interaction_inventory(&mut self) -> Vec<trace::InteractionTarget> {
        self.with_interface(|interface, renderer, _| {
            collect_known_ids::<P::Message, P::Renderer>(interface, renderer)
                .into_iter()
                .filter_map(|id| {
                    let mut layouts =
                        find_targets::<P::Message, P::Renderer>(interface, renderer, &id);
                    normalize_target_matches(&mut layouts);
                    let [layout] = layouts.as_slice() else {
                        return None;
                    };
                    let visible = layout.visible_bounds.is_some();
                    let scrollable = layout.translation.is_some()
                        && widget_capability::<P::Message, P::Renderer>(
                            interface,
                            renderer,
                            &id,
                            WidgetCapability::Scrollable,
                        );
                    let focusable = layout.accessibility.as_ref().is_some_and(|accessibility| {
                        accessibility.supports_focus && !accessibility.disabled
                    }) || widget_capability::<P::Message, P::Renderer>(
                        interface,
                        renderer,
                        &id,
                        WidgetCapability::Focusable,
                    );
                    Some(trace::InteractionTarget {
                        id,
                        visible,
                        scrollable,
                        focusable,
                    })
                })
                .collect()
        })
    }

    fn enable_trace(
        &mut self,
        config: &Config,
        configuration: ui_lang_template::trace::Configuration,
        seed: Option<u64>,
    ) {
        self.trace = Some(TraceRecorder::for_campaign(
            self,
            config,
            configuration,
            seed,
        ));
    }

    fn finish_trace_action(&mut self) {
        if let Some(trace) = &mut self.trace {
            trace.finish();
        }
    }

    fn take_trace(&mut self) -> ui_lang_template::trace::Artifact {
        self.trace
            .take()
            .expect("campaign trace is enabled")
            .into_artifact()
    }

    fn known_ids(&mut self) -> Vec<String> {
        self.with_interface(|interface, renderer, _| {
            collect_known_ids::<P::Message, P::Renderer>(interface, renderer)
        })
    }

    fn known_ids_display(&mut self) -> String {
        known_ids_display(&self.known_ids())
    }

    fn interaction_bounds(&mut self, action: &str, id: &str, source: Location) -> Rectangle {
        let target = self.require_target(id, false, source);
        target.visible.unwrap_or_else(|| {
            panic!(
                "{source}: test `{}` cannot {action} hidden target `{id}`\nstatement: {}\nselector: {id}\nexpected: visible target\nactual: hidden target\nbounds: {:?}",
                self.test_name,
                source.statement,
                target.bounds,
            )
        })
    }

    fn set_cursor(&mut self, position: Point, inside: bool, source: Location) {
        let mut events = Vec::with_capacity(2);
        if inside && !self.cursor_inside {
            events.push(iced::Event::Mouse(mouse::Event::CursorEntered));
        }
        self.cursor_inside = inside;
        self.cursor = mouse::Cursor::Available(position);
        events.push(iced::Event::Mouse(mouse::Event::CursorMoved { position }));
        self.simulate(events, source);
    }

    pub fn redraw(&mut self, source: Location) {
        self.redraw_at(self.logical_time, source);
    }

    fn redraw_at(&mut self, time: Instant, source: Location) {
        self.simulate(
            [iced::Event::Window(window::Event::RedrawRequested(time))],
            source,
        );
    }

    fn click_current(&mut self, button: MouseButton, count: u8, source: Location) {
        if count == 0 {
            self.invalid_action("click", "a positive click count", "0".to_owned(), source);
        }
        for _ in 0..count {
            self.press_current(button, source);
            self.release_button(button, source);
        }
    }

    fn press_current(&mut self, button: MouseButton, source: Location) {
        if !self.pressed_mouse.insert(button) {
            self.invalid_action(
                "press pointer button",
                "a pointer button that is not already pressed",
                format!("{button:?} is already pressed"),
                source,
            );
        }
        self.simulate(
            [iced::Event::Mouse(mouse::Event::ButtonPressed(
                button.iced(),
            ))],
            source,
        );
    }

    fn require_point(&self, action: &str, position: Point, source: Location) {
        self.require_finite_pair(action, position.x, position.y, source);
    }

    fn require_finite_pair(&self, action: &str, x: f32, y: f32, source: Location) {
        if !x.is_finite() || !y.is_finite() {
            self.invalid_action(
                action,
                "finite coordinates",
                format!("({x:?}, {y:?})"),
                source,
            );
        }
    }

    fn require_scroll_target(&mut self, id: &str, source: Location) -> widget::Id {
        let target = self.require_target(id, false, source);
        if target.translation.is_none() {
            self.invalid_action(
                "scroll",
                "a scrollable target",
                format!("{id} is {}", target.kind),
                source,
            );
        }
        self.require_widget_capability(
            "scroll",
            id,
            WidgetCapability::Scrollable,
            "a scrollable target",
            source,
        )
    }

    fn invalid_action(&self, action: &str, expected: &str, actual: String, source: Location) -> ! {
        panic!(
            "{source}: test `{}` {action} failed\nstatement: {}\nexpected: {expected}\nactual: {actual}",
            self.test_name, source.statement
        )
    }

    fn keyboard_pressed_event(
        &self,
        key: &Key,
        metadata: &KeyMetadata,
        source: Location,
    ) -> keyboard::Event {
        self.require_non_empty_key("key down", "logical key", key, source);
        if let Some(modified_key) = &metadata.modified_key {
            self.require_non_empty_key("key down", "modified key", modified_key, source);
        }
        let key = iced_key(key);
        let modified_key = metadata
            .modified_key
            .as_ref()
            .map(iced_key)
            .unwrap_or_else(|| key.clone());
        let text = metadata.text.clone().or_else(|| match &key {
            keyboard::Key::Character(value) if !self.modifiers.control && !self.modifiers.logo => {
                Some(value.to_string())
            }
            _ => None,
        });
        if text.as_ref().is_some_and(String::is_empty) {
            self.invalid_action(
                "key down",
                "non-empty produced text when text metadata is present",
                "empty text".to_owned(),
                source,
            );
        }
        keyboard::Event::KeyPressed {
            key,
            modified_key,
            physical_key: metadata
                .physical_key
                .unwrap_or_else(unidentified_physical_key),
            location: metadata.location.iced(),
            modifiers: self.modifiers.iced(),
            text: text.map(Into::into),
            repeat: metadata.repeat,
        }
    }

    fn keyboard_released_event(
        &self,
        key: &Key,
        metadata: &KeyMetadata,
        source: Location,
    ) -> keyboard::Event {
        self.require_non_empty_key("key up", "logical key", key, source);
        if let Some(modified_key) = &metadata.modified_key {
            self.require_non_empty_key("key up", "modified key", modified_key, source);
        }
        if metadata.text.is_some() || metadata.repeat {
            self.invalid_action(
                "key up",
                "release metadata without produced text or repeat",
                format!("text: {:?}, repeat: {}", metadata.text, metadata.repeat),
                source,
            );
        }
        let key = iced_key(key);
        let modified_key = metadata
            .modified_key
            .as_ref()
            .map(iced_key)
            .unwrap_or_else(|| key.clone());
        keyboard::Event::KeyReleased {
            key,
            modified_key,
            physical_key: metadata
                .physical_key
                .unwrap_or_else(unidentified_physical_key),
            location: metadata.location.iced(),
            modifiers: self.modifiers.iced(),
        }
    }

    fn require_non_empty_key(&self, action: &str, label: &str, key: &Key, source: Location) {
        if matches!(key, Key::Character(value) if value.is_empty()) {
            self.invalid_action(
                action,
                &format!("a non-empty character value for the {label}"),
                "empty character key".to_owned(),
                source,
            );
        }
    }

    fn require_focused_text_input(&mut self, action: &str, source: Location) -> widget::Id {
        let (focused, text_inputs) = self.with_interface(|interface, renderer, _| {
            let mut operation = FocusedIds::<P::Message>::new().find_all();
            interface.operate(renderer, &mut widget::operation::black_box(&mut operation));
            let focused = match operation.finish() {
                Outcome::Some(ids) => ids,
                _ => Vec::new(),
            };
            let mut operation = TextInputIds.find_all();
            interface.operate(renderer, &mut widget::operation::black_box(&mut operation));
            let text_inputs = match operation.finish() {
                Outcome::Some(ids) => ids,
                _ => Vec::new(),
            };
            (focused, text_inputs)
        });
        let focused_id = match focused.as_slice() {
            [id] => id,
            [] => self.invalid_action(
                action,
                "exactly one focused text input with an id",
                "no focused widget with an id".to_owned(),
                source,
            ),
            ids => self.invalid_action(
                action,
                "exactly one focused text input with an id",
                format!("{} widgets are focused", ids.len()),
                source,
            ),
        };
        let matches = text_inputs
            .iter()
            .filter(|text_input| *text_input == focused_id)
            .count();
        if matches != 1 {
            self.invalid_action(
                action,
                "exactly one focused text input with an id",
                format!("focused widget exposes {matches} text-input operation candidates"),
                source,
            );
        }
        focused_id.clone()
    }

    fn require_widget_capability(
        &mut self,
        action: &str,
        id: &str,
        capability: WidgetCapability,
        expected: &str,
        source: Location,
    ) -> widget::Id {
        let ids = self.with_interface(|interface, renderer, _| {
            let mut operation = MatchingWidgetIds::new(id, capability).find_all();
            interface.operate(renderer, &mut widget::operation::black_box(&mut operation));
            match operation.finish() {
                Outcome::Some(ids) => ids,
                _ => Vec::new(),
            }
        });
        match ids.as_slice() {
            [id] => id.clone(),
            [] => self.invalid_action(action, expected, format!("{id} lacks {capability}"), source),
            ids => self.invalid_action(
                action,
                "one unambiguous widget operation target",
                format!("{id} matched {} {capability} widget ids", ids.len()),
                source,
            ),
        }
    }

    fn require_ime(&self, action: &str, source: Location) {
        if !self.ime_open {
            self.invalid_action(
                &format!("IME {action}"),
                "an open composition",
                "composition is closed".to_owned(),
                source,
            );
        }
    }

    fn require_touch(&self, id: u64, action: &str, source: Location) {
        if !self.touches.contains_key(&id) {
            self.invalid_action(
                &format!("touch {action}"),
                "an active touch id",
                format!("touch {id} is not active"),
                source,
            );
        }
    }

    fn simulate(&mut self, events: impl IntoIterator<Item = iced::Event>, source: Location) {
        let events = events.into_iter().collect::<Vec<_>>();
        let cursor = self.cursor;
        let trace = self.tracing_action();
        let (messages, statuses, elapsed) =
            self.with_interface(|interface, renderer, clipboard| {
                let mut messages = Vec::new();
                let started = trace.then(Instant::now);
                let (_, statuses) =
                    interface.update(&events, cursor, renderer, clipboard, &mut messages);
                (messages, statuses, started.map(|started| started.elapsed()))
            });
        self.record_phase(Phase::EventDispatch, elapsed);
        self.finish_simulation(events, messages, statuses, source);
    }

    fn finish_simulation(
        &mut self,
        events: Vec<iced::Event>,
        messages: Vec<P::Message>,
        statuses: Vec<iced::event::Status>,
        source: Location,
    ) {
        let window = self.window;
        for (event, status) in events.into_iter().zip(statuses) {
            self.broadcast(subscription::Event::Interaction {
                window,
                event,
                status,
            });
        }
        for message in messages {
            self.update(message, Some(source));
        }
        self.settle(Some(source));
    }

    fn update(&mut self, message: P::Message, source: Option<Location>) {
        let test_name = self.test_name;
        let started = self.phase_start();
        let task = with_panic_context(test_name, source, || {
            self.runtime
                .enter(|| self.program.update(&mut self.state, message))
        });
        self.record_phase(
            Phase::ProgramUpdate,
            started.map(|started| started.elapsed()),
        );
        self.resubscribe(source);
        self.run_task(task, source);
    }

    fn resubscribe(&mut self, source: Option<Location>) {
        let test_name = self.test_name;
        with_panic_context(test_name, source, || {
            let recipes = subscription::into_recipes(self.runtime.enter(|| {
                self.program
                    .subscription(&self.state)
                    .map(|message| DriverEvent::Action(runtime::Action::Output(message)))
            }));
            let mut identified = Vec::with_capacity(recipes.len());
            for inner in recipes {
                let mut hasher = subscription::Hasher::default();
                inner.hash(&mut hasher);
                identified.push((hasher.finish(), inner));
            }

            let mut next_subscriptions = HashMap::new();
            for (id, _) in &identified {
                if next_subscriptions.contains_key(id) {
                    continue;
                }
                let state = self.subscriptions.get(id).cloned().unwrap_or_else(|| {
                    self.next_subscription_generation = self
                        .next_subscription_generation
                        .checked_add(1)
                        .expect("subscription generation overflow");
                    let key = SubscriptionKey {
                        id: *id,
                        generation: self.next_subscription_generation,
                    };
                    self.pending_subscription_starts.insert(key);
                    Arc::new(SubscriptionState::new(key))
                });
                next_subscriptions.insert(*id, state);
            }

            let active = next_subscriptions
                .values()
                .map(|state| state.key)
                .collect::<HashSet<_>>();
            self.pending_subscription_starts
                .retain(|key| active.contains(key));
            self.pending_subscription_events
                .retain(|key, _| active.contains(key));
            self.subscriptions = next_subscriptions;

            let recipes = identified.into_iter().map(|(id, inner)| {
                Box::new(PanicRecipe {
                    inner,
                    state: Arc::clone(&self.subscriptions[&id]),
                    instance: self.instance,
                })
                    as Box<dyn subscription::Recipe<Output = DriverEvent<P::Message>>>
            });
            self.runtime.track(recipes);
        });
    }

    fn broadcast(&mut self, event: subscription::Event) {
        for state in self.subscriptions.values() {
            if state.listening.load(Ordering::Acquire) {
                *self
                    .pending_subscription_events
                    .entry(state.key)
                    .or_default() += 1;
            }
        }
        self.runtime.broadcast(event);
    }

    fn run_task(&mut self, task: Task<P::Message>, source: Option<Location>) {
        let Some(stream) = with_panic_context(self.test_name, source, || task::into_stream(task))
        else {
            return;
        };
        self.pending_tasks += 1;
        self.runtime.run(
            Instanced {
                inner: std::panic::AssertUnwindSafe(stream)
                    .catch_unwind()
                    .map(|result| match result {
                        Ok(action) => DriverEvent::Action(action),
                        Err(payload) => DriverEvent::Panicked(payload),
                    })
                    .chain(iced_test::futures::futures::stream::once(async {
                        DriverEvent::Finished
                    }))
                    .boxed(),
                instance: self.instance,
            }
            .boxed(),
        );
    }

    fn settle(&mut self, source: Option<Location>) {
        let start = Instant::now();
        loop {
            while let Ok(event) = self.receiver.try_recv() {
                match event {
                    DriverEvent::Action(action) => self.perform_runtime_action(action, source),
                    DriverEvent::Finished => {
                        self.pending_tasks = self.pending_tasks.saturating_sub(1);
                    }
                    DriverEvent::Panicked(payload) => {
                        resume_panic_with_context(payload, self.test_name, source)
                    }
                    DriverEvent::SubscriptionStarted(key) => {
                        self.pending_subscription_starts.remove(&key);
                    }
                    DriverEvent::SubscriptionEventHandled(key) => {
                        if let Some(pending) = self.pending_subscription_events.get_mut(&key) {
                            *pending = pending.saturating_sub(1);
                            if *pending == 0 {
                                self.pending_subscription_events.remove(&key);
                            }
                        }
                    }
                    DriverEvent::SubscriptionStopped(key) => {
                        self.pending_subscription_starts.remove(&key);
                        self.pending_subscription_events.remove(&key);
                    }
                }
            }

            if self.pending_tasks == 0
                && self.pending_subscription_starts.is_empty()
                && self.pending_subscription_events.is_empty()
            {
                let elapsed = self.tracing_action().then(|| start.elapsed());
                self.record_phase(Phase::TaskSettle, elapsed);
                return;
            }
            if start.elapsed() >= self.timeout {
                let origin = failure_origin(self.test_name, source);
                let pending_subscription_events = self
                    .pending_subscription_events
                    .values()
                    .copied()
                    .sum::<usize>();
                panic!(
                    "{origin}\nexpected: quiescence within {:?}\nactual: {} task stream(s) still pending after {:?}; {} subscription startup and {} event handoff(s) pending",
                    self.timeout,
                    self.pending_tasks,
                    start.elapsed(),
                    self.pending_subscription_starts.len(),
                    pending_subscription_events,
                );
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn perform_runtime_action(
        &mut self,
        action: runtime::Action<P::Message>,
        source: Option<Location>,
    ) {
        match action {
            runtime::Action::Output(message) => self.update(message, source),
            runtime::Action::LoadFont { bytes, channel } => {
                iced_test::renderer::graphics::text::font_system()
                    .write()
                    .unwrap_or_else(|_| {
                        let origin = failure_origin(self.test_name, source);
                        panic!(
                            "{origin}\nexpected: writable Iced font system\nactual: poisoned font-system lock"
                        )
                    })
                    .load_font(bytes);
                let _ = channel.send(Ok(()));
            }
            runtime::Action::Widget(operation) => self.perform_widget(operation),
            runtime::Action::Clipboard(action) => match action {
                runtime::clipboard::Action::Read { target, channel } => {
                    let _ = channel.send(self.clipboard.value(target));
                }
                runtime::clipboard::Action::Write { target, contents } => {
                    self.clipboard.set(target, contents);
                }
            },
            runtime::Action::Window(action) => self.perform_window(action, source),
            runtime::Action::System(action) => match action {
                runtime::system::Action::GetInformation(channel) => {
                    let _ = channel.send(runtime::system::Information {
                        system_name: Some("Ice test runtime".to_owned()),
                        system_kernel: None,
                        system_version: None,
                        system_short_version: None,
                        cpu_brand: String::new(),
                        cpu_cores: None,
                        memory_total: 0,
                        memory_used: None,
                        graphics_backend: "tiny-skia".to_owned(),
                        graphics_adapter: "headless".to_owned(),
                    });
                }
                runtime::system::Action::GetTheme(channel) => {
                    let _ = channel.send(self.system_theme.iced());
                }
                runtime::system::Action::NotifyTheme(mode) => {
                    self.system_theme = theme_mode(mode);
                    self.broadcast(subscription::Event::SystemThemeChanged(mode));
                }
            },
            runtime::Action::Image(runtime::image::Action::Allocate(handle, channel)) => {
                self.renderer.allocate_image(&handle, move |result| {
                    let _ = channel.send(result);
                });
            }
            runtime::Action::Reload | runtime::Action::Exit => {}
        }
    }

    fn perform_widget(&mut self, mut operation: Box<dyn widget::Operation>) {
        let trace = self.tracing_action();
        let elapsed = self.with_interface(|interface, renderer, _| {
            let started = trace.then(Instant::now);
            loop {
                interface.operate(renderer, &mut operation);
                match operation.finish() {
                    Outcome::None | Outcome::Some(()) => break,
                    Outcome::Chain(next) => operation = next,
                }
            }
            started.map(|started| started.elapsed())
        });
        self.record_phase(Phase::WidgetOperation, elapsed);
    }

    fn perform_window(&mut self, action: runtime::window::Action, source: Option<Location>) {
        use runtime::window::Action;

        match action {
            Action::Open(id, settings, channel) => {
                if !valid_dimension(settings.size.width) || !valid_dimension(settings.size.height) {
                    let origin = failure_origin(self.test_name, source);
                    panic!(
                        "{origin}\nexpected: finite, positive opened-window dimensions\nactual: {:?}",
                        settings.size
                    );
                }
                self.reset_window_local_state();
                self.window = id;
                self.size = settings.size;
                self.renderer.reset(Rectangle::with_size(settings.size));
                self.window_position = match settings.position {
                    window::Position::Specific(position) => Some(position),
                    window::Position::SpecificWith(position) => {
                        Some(position(settings.size, settings.size))
                    }
                    window::Position::Centered => Some(Point::ORIGIN),
                    window::Position::Default => None,
                };
                let _ = channel.send(id);
            }
            Action::Close(_) => {}
            Action::GetOldest(channel) | Action::GetLatest(channel) => {
                let _ = channel.send(Some(self.window));
            }
            Action::Resize(id, size) if id == self.window => {
                if !valid_dimension(size.width) || !valid_dimension(size.height) {
                    let origin = failure_origin(self.test_name, source);
                    panic!(
                        "{origin}\nexpected: finite, positive task-issued resize dimensions\nactual: {size:?}"
                    );
                }
                self.size = size;
                self.renderer.reset(Rectangle::with_size(size));
            }
            Action::Move(id, position) if id == self.window => {
                self.window_position = Some(position);
            }
            Action::GainFocus(id) if id == self.window => {
                self.window_focused = true;
            }
            Action::GetSize(id, channel) if id == self.window => {
                let _ = channel.send(self.size);
            }
            Action::GetMaximized(id, channel) if id == self.window => {
                let _ = channel.send(false);
            }
            Action::GetMinimized(id, channel) if id == self.window => {
                let _ = channel.send(None);
            }
            Action::GetPosition(id, channel) if id == self.window => {
                let _ = channel.send(self.window_position);
            }
            Action::GetScaleFactor(id, channel) if id == self.window => {
                let _ = channel.send(self.scale_factor());
            }
            Action::GetMode(id, channel) if id == self.window => {
                let _ = channel.send(window::Mode::Windowed);
            }
            Action::GetRawId(id, channel) if id == self.window => {
                let _ = channel.send(0);
            }
            Action::GetMonitorSize(id, channel) if id == self.window => {
                let _ = channel.send(Some(self.size));
            }
            Action::Screenshot(id, channel) if id == self.window => {
                let _ = channel.send(self.screenshot_at(source));
            }
            Action::Run(id, _) if id == self.window => {
                let origin = failure_origin(self.test_name, source);
                panic!(
                    "{origin}\nexpected: a headless-compatible window task\nactual: native window handle requested"
                );
            }
            Action::Drag(_)
            | Action::DragResize(_, _)
            | Action::Maximize(_, _)
            | Action::Minimize(_, _)
            | Action::SetMode(_, _)
            | Action::ToggleMaximize(_)
            | Action::ToggleDecorations(_)
            | Action::RequestUserAttention(_, _)
            | Action::SetLevel(_, _)
            | Action::ShowSystemMenu(_)
            | Action::SetIcon(_, _)
            | Action::EnableMousePassthrough(_)
            | Action::DisableMousePassthrough(_)
            | Action::SetMinSize(_, _)
            | Action::SetMaxSize(_, _)
            | Action::SetResizable(_, _)
            | Action::SetResizeIncrements(_, _)
            | Action::SetAllowAutomaticTabbing(_)
            | Action::RedrawAll
            | Action::RelayoutAll => {}
            Action::GetSize(_, channel) => {
                let _ = channel.send(Size::ZERO);
            }
            Action::GetMaximized(_, channel) => {
                let _ = channel.send(false);
            }
            Action::GetMinimized(_, channel) => {
                let _ = channel.send(None);
            }
            Action::GetPosition(_, channel) => {
                let _ = channel.send(None);
            }
            Action::GetScaleFactor(_, channel) => {
                let _ = channel.send(1.0);
            }
            Action::GetMode(_, channel) => {
                let _ = channel.send(window::Mode::Windowed);
            }
            Action::GetRawId(_, channel) => {
                let _ = channel.send(0);
            }
            Action::GetMonitorSize(_, channel) => {
                let _ = channel.send(None);
            }
            Action::Screenshot(_, channel) => {
                let _ = channel.send(window::Screenshot::new(Vec::new(), Size::new(0, 0), 1.0));
            }
            Action::Run(_, _)
            | Action::Resize(_, _)
            | Action::Move(_, _)
            | Action::GainFocus(_) => {}
        }
    }

    /// Replaces the one window modeled by the headless driver.
    ///
    /// Application state and process-scoped test context remain intact. Widget
    /// cache and input state belong to the displaced window and must not leak
    /// into the newly opened one.
    fn reset_window_local_state(&mut self) {
        self.cache = Some(user_interface::Cache::default());
        self.cursor = mouse::Cursor::Unavailable;
        self.cursor_inside = false;
        self.pressed_mouse.clear();
        self.modifiers = Modifiers::NONE;
        self.pressed_keys.clear();
        self.touches.clear();
        self.ime_open = false;
        self.window_focused = true;
    }

    fn screenshot_at(&mut self, source: Option<Location>) -> window::Screenshot {
        let theme = self.theme();
        self.screenshot_with_theme(&theme, source)
    }

    fn screenshot_with_theme(
        &mut self,
        theme: &P::Theme,
        source: Option<Location>,
    ) -> window::Screenshot {
        let scale_factor = self.scale_factor();
        let (physical_size, expected_rgba_len) = self.checked_physical_size(scale_factor, source);
        let style = self.program.style(&self.state, theme);
        let cursor = self.cursor;
        let events = [iced::Event::Window(window::Event::RedrawRequested(
            self.logical_time,
        ))];
        let rgba = self.with_interface(|interface, renderer, clipboard| {
            let mut ignored_messages = Vec::new();
            let _ = interface.update(&events, cursor, renderer, clipboard, &mut ignored_messages);
            interface.draw(
                renderer,
                theme,
                &iced::advanced::renderer::Style {
                    text_color: style.text_color,
                },
                cursor,
            );
            renderer.screenshot(physical_size, scale_factor, style.background_color)
        });
        if rgba.len() != expected_rgba_len {
            let origin = failure_origin(self.test_name, source);
            panic!(
                "{origin}\nscreenshot failed\nexpected: {expected_rgba_len} RGBA8 bytes for {}x{} pixels\nactual: {} bytes returned by the headless renderer",
                physical_size.width,
                physical_size.height,
                rgba.len(),
            );
        }
        window::Screenshot::new(rgba, physical_size, scale_factor)
    }

    fn checked_physical_size(
        &self,
        scale_factor: f32,
        source: Option<Location>,
    ) -> (Size<u32>, usize) {
        let width = (f64::from(self.size.width) * f64::from(scale_factor)).round();
        let height = (f64::from(self.size.height) * f64::from(scale_factor)).round();
        let dimension = |value: f64| {
            (value.is_finite() && (1.0..=f64::from(u32::MAX)).contains(&value))
                .then_some(value as u32)
        };
        let (Some(width_u32), Some(height_u32)) = (dimension(width), dimension(height)) else {
            let origin = failure_origin(self.test_name, source);
            panic!(
                "{origin}\nscreenshot failed\nexpected: rounded physical width and height in 1..=u32::MAX\nactual: viewport {:?} at scale {scale_factor:?} rounds to ({width:?}, {height:?})",
                self.size,
            );
        };
        let pixels = usize::try_from(width_u32).ok().and_then(|width| {
            usize::try_from(height_u32)
                .ok()
                .and_then(|height| width.checked_mul(height))
        });
        let Some(pixels) = pixels else {
            let origin = failure_origin(self.test_name, source);
            panic!(
                "{origin}\nscreenshot failed\nexpected: an addressable RGBA8 physical buffer\nactual: {width_u32}x{height_u32} pixels",
            );
        };
        if pixels > MAX_SCREENSHOT_PIXELS {
            let origin = failure_origin(self.test_name, source);
            panic!(
                "{origin}\nscreenshot failed\nexpected: at most {MAX_SCREENSHOT_PIXELS} physical pixels\nactual: {pixels} pixels ({width_u32}x{height_u32})",
            );
        }
        let Some(rgba_len) = pixels.checked_mul(4) else {
            let origin = failure_origin(self.test_name, source);
            panic!(
                "{origin}\nscreenshot failed\nexpected: an addressable RGBA8 physical buffer\nactual: {pixels} pixels",
            );
        };

        (Size::new(width_u32, height_u32), rgba_len)
    }

    fn theme(&self) -> P::Theme {
        if let Some(mode) = self.theme_override {
            return P::Theme::default(mode.iced());
        }
        self.program
            .theme(&self.state, self.window)
            .unwrap_or_else(|| P::Theme::default(self.system_theme.iced()))
    }

    fn with_interface<R>(
        &mut self,
        f: impl FnOnce(
            &mut UserInterface<'_, P::Message, P::Theme, P::Renderer>,
            &mut P::Renderer,
            &mut TestClipboard,
        ) -> R,
    ) -> R {
        let view_started = self.phase_start();
        let element = self.program.view(&self.state, self.window);
        let view_elapsed = view_started.map(|started| started.elapsed());
        let build_started = self.phase_start();
        let mut interface = UserInterface::build(
            element,
            self.size,
            self.cache.take().unwrap_or_else(|| {
                panic!(
                    "test `{}` runtime invariant failed\nexpected: persistent UI cache\nactual: cache unavailable",
                    self.test_name
                )
            }),
            &mut self.renderer,
        );
        let build_elapsed = build_started.map(|started| started.elapsed());
        let output = f(&mut interface, &mut self.renderer, &mut self.clipboard);
        self.cache = Some(interface.into_cache());
        self.record_phase(Phase::View, view_elapsed);
        self.record_phase(Phase::UiBuildLayout, build_elapsed);
        output
    }

    fn tracing_action(&self) -> bool {
        self.trace
            .as_ref()
            .is_some_and(TraceRecorder::is_recording_action)
    }

    fn phase_start(&self) -> Option<Instant> {
        self.tracing_action().then(Instant::now)
    }

    fn record_phase(&mut self, phase: Phase, elapsed: Option<Duration>) {
        if let (Some(trace), Some(elapsed)) = (&mut self.trace, elapsed) {
            trace.phase(phase, elapsed);
        }
    }
}

fn find_targets<Message: 'static, Renderer>(
    interface: &mut UserInterface<'_, Message, impl theme::Base, Renderer>,
    renderer: &Renderer,
    id: &str,
) -> Vec<LayoutTarget>
where
    Renderer: iced::advanced::Renderer,
{
    let mut operation = IdSelector::<Message>::new(id).find_all();
    interface.operate(renderer, &mut widget::operation::black_box(&mut operation));
    match operation.finish() {
        Outcome::Some(targets) => targets,
        _ => Vec::new(),
    }
}

fn collect_known_ids<Message: 'static, Renderer>(
    interface: &mut UserInterface<'_, Message, impl theme::Base, Renderer>,
    renderer: &Renderer,
) -> Vec<String>
where
    Renderer: iced::advanced::Renderer,
{
    let mut operation = KnownIds::<Message>::new().find_all();
    interface.operate(renderer, &mut widget::operation::black_box(&mut operation));
    let mut ids = match operation.finish() {
        Outcome::Some(ids) => ids,
        _ => Vec::new(),
    };
    ids.retain(|id| !internal_auto_id(id));
    ids.sort();
    ids.dedup();
    ids
}

/// Asks the whole tree, not one node: a duplicated id answers `false`, which
/// is what keeps an ambiguous selector out of the fuzz inventory.
fn widget_capability<Message: 'static, Renderer>(
    interface: &mut UserInterface<'_, Message, impl theme::Base, Renderer>,
    renderer: &Renderer,
    id: &str,
    capability: WidgetCapability,
) -> bool
where
    Renderer: iced::advanced::Renderer,
{
    let mut operation = MatchingWidgetIds::new(id, capability).find_all();
    interface.operate(renderer, &mut widget::operation::black_box(&mut operation));
    matches!(operation.finish(), Outcome::Some(ids) if ids.len() == 1)
}

fn normalize_target_matches(targets: &mut Vec<LayoutTarget>) {
    let mut normalized: Vec<LayoutTarget> = Vec::with_capacity(targets.len());
    for target in targets.drain(..) {
        let duplicate = if let Some(group) = target.semantic_group {
            normalized.iter().position(|candidate| {
                candidate.semantic_group == Some(group)
                    || (!candidate.semantic
                        && candidate.kind == "container"
                        && candidate.bounds == target.bounds)
            })
        } else if target.kind == "container" {
            normalized.iter().position(|candidate| {
                candidate.semantic_group.is_some() && candidate.bounds == target.bounds
            })
        } else {
            target.state_key.and_then(|state_key| {
                normalized.iter().position(|candidate| {
                    candidate.semantic_group.is_none() && candidate.state_key == Some(state_key)
                })
            })
        };
        if let Some(index) = duplicate {
            merge_target_match(&mut normalized[index], target);
        } else {
            normalized.push(target);
        }
    }
    *targets = normalized;
}

fn merge_target_match(existing: &mut LayoutTarget, mut candidate: LayoutTarget) {
    if target_match_rank(&candidate) > target_match_rank(existing) {
        std::mem::swap(existing, &mut candidate);
    }
    existing.content_bounds = existing.content_bounds.or(candidate.content_bounds);
    existing.translation = existing.translation.or(candidate.translation);
    let focused = existing.focused.unwrap_or(false) || candidate.focused.unwrap_or(false);
    existing.accessibility = existing.accessibility.take().or(candidate.accessibility);
    if let Some(accessibility) = &mut existing.accessibility {
        accessibility.focused |= focused;
    }
    existing.focused =
        (existing.focused.is_some() || candidate.focused.is_some()).then_some(focused);
    existing.source = existing.source.or(candidate.source);
    if !existing.semantic {
        existing.value = existing.value.take().or(candidate.value);
    }
}

fn target_match_rank(target: &LayoutTarget) -> u8 {
    if target.semantic {
        2
    } else if matches!(target.kind.as_str(), "focusable" | "container") {
        0
    } else {
        1
    }
}

fn target_from_layout(
    id: String,
    test_name: &'static str,
    source: Location,
    layout: LayoutTarget,
    paint: Result<PaintInspection, &'static str>,
    scale_factor: f32,
) -> Target {
    let (paint_error, paint) = match paint {
        Ok(paint) => (None, paint),
        Err(error) => (Some(error), PaintInspection::default()),
    };
    Target {
        id,
        kind: layout.kind,
        bounds: layout.bounds,
        visible: layout.visible_bounds,
        content: layout.content_bounds,
        translation: layout.translation,
        value: layout.value,
        test_name,
        source,
        paint_error,
        surfaces: paint.surfaces,
        texts: paint.texts,
        images: paint.images,
        accessibility: layout.accessibility,
        focused: layout.focused,
        scale_factor,
        render_source: layout.source,
    }
}

#[derive(Default)]
struct PaintInspection {
    surfaces: Vec<SurfacePaint>,
    texts: Vec<TextPaint>,
    images: Vec<ImagePaint>,
}

fn inspect_paint<Renderer: 'static>(
    renderer: &mut Renderer,
    bounds: Rectangle,
) -> Result<PaintInspection, &'static str> {
    let renderer = tiny_skia_renderer(renderer)?;

    let mut surfaces = Vec::new();
    let mut texts = Vec::new();
    let mut images = Vec::new();
    for layer in renderer.layers() {
        for (quad, background) in &layer.quads {
            if rectangle_eq(quad.bounds, bounds) {
                surfaces.push(SurfacePaint {
                    background: *background,
                    border: quad.border,
                    shadow: quad.shadow,
                });
            }
        }
        for group in &layer.text {
            let transformation = group.transformation();
            for text in group.as_slice() {
                let Some(text_bounds) = text
                    .visible_bounds()
                    .map(|bounds| bounds * transformation)
                    .and_then(|bounds| bounds.intersection(&(group.clip_bounds() * transformation)))
                    .and_then(|bounds| bounds.intersection(&layer.bounds))
                else {
                    continue;
                };
                if !bounds.contains(text_bounds.center()) {
                    continue;
                }
                if let Some(paint) = text_paint(text, transformation, text_bounds) {
                    texts.push(paint);
                }
            }
        }
        for image in &layer.images {
            let (clip_bounds, color) = match image {
                iced_tiny_skia::graphics::Image::Raster { clip_bounds, .. } => (*clip_bounds, None),
                iced_tiny_skia::graphics::Image::Vector {
                    clip_bounds, svg, ..
                } => (*clip_bounds, svg.color),
            };
            if let Some(visible) = image
                .bounds()
                .intersection(&clip_bounds)
                .and_then(|visible| visible.intersection(&layer.bounds))
                && bounds.contains(visible.center())
            {
                images.push(ImagePaint {
                    bounds: visible,
                    color,
                });
            }
        }
    }
    Ok(PaintInspection {
        surfaces,
        texts,
        images,
    })
}

fn rendered_text_exists<Renderer: 'static>(
    renderer: &mut Renderer,
    expected: &str,
    within: Option<Rectangle>,
) -> Result<bool, &'static str> {
    // A primitive holding the whole string answers most queries, and answers
    // them without reading the rest of the screen.
    let mut singles = Vec::new();
    let found = for_each_visible_text(renderer, within, |paint| {
        if paint.content.as_deref() == Some(expected) {
            return true;
        }
        if paint
            .content
            .as_deref()
            .is_some_and(|content| crate::graphemes(content).count() == 1)
        {
            singles.push(paint);
        }
        false
    })?;
    if found {
        return Ok(true);
    }
    // Otherwise it may be drawn one grapheme at a time, and only the
    // graphemes collected on the way past can say so.
    Ok(tracked_runs(&singles).iter().any(|run| run == expected))
}

/// Walks every text primitive that is actually on screen, stopping early when
/// `visit` is satisfied.
///
/// A canvas text group's clip rectangle is recorded in the canvas's own
/// coordinates while its text, once transformed, is in the window's — so the
/// clip is carried through the same transformation before the two meet, or a
/// canvas laid out past its own width and height finds none of its text.
fn for_each_visible_text<Renderer: 'static>(
    renderer: &mut Renderer,
    within: Option<Rectangle>,
    mut visit: impl FnMut(TextPaint) -> bool,
) -> Result<bool, &'static str> {
    let renderer = tiny_skia_renderer(renderer)?;
    for layer in renderer.layers() {
        for group in &layer.text {
            let transformation = group.transformation();
            for text in group.as_slice() {
                let Some(bounds) = text
                    .visible_bounds()
                    .map(|bounds| bounds * transformation)
                    .and_then(|bounds| bounds.intersection(&(group.clip_bounds() * transformation)))
                    .and_then(|bounds| bounds.intersection(&layer.bounds))
                else {
                    continue;
                };
                if let Some(paint) = text_paint(text, transformation, bounds)
                    && within.is_none_or(|within| within.contains(paint.bounds.center()))
                    && visit(paint)
                {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

/// Text drawn with `tracking=` is one widget per grapheme, so no primitive
/// ever holds the whole string. Rebuilding those runs is what lets a question
/// about what is on screen be answered for tracked text at all — including
/// its negative form, which would otherwise pass for a label that is plainly
/// there.
///
/// A run is consecutive single-grapheme primitives that share a baseline and
/// a style and are evenly spaced. Even spacing is what separates one tracked
/// label from the next along the same row, where baseline and style match but
/// the layout gap does not. A gap wider than the run's own by about a space
/// is a space: the character paints nothing, so it arrives as a hole rather
/// than a primitive.
///
/// A run learns its spacing from its first gap, and a run one grapheme long
/// — a count beside a heading, a lone `0` — has only the gap crossing to the
/// NEXT label to learn it from. Taking that gap on trust glued the count to
/// the label after it and broke that label one letter in, so `POSITIONS 0`
/// beside `FUNDING IN` rebuilt as `0F` and `UNDING IN`. The gap after it says
/// which it was: tracking is the tightest spacing on the row, so a first gap
/// wider than the one following it is a label boundary rather than a step.
fn tracked_runs(texts: &[TextPaint]) -> Vec<String> {
    let mut buckets: Vec<(f64, Color, f64, Font, Vec<&TextPaint>)> = Vec::new();
    for text in texts {
        let (Some(baseline), Some(size), Some(font)) = (text.baseline, text.size, text.font) else {
            continue;
        };
        if text
            .content
            .as_deref()
            .is_none_or(|content| crate::graphemes(content).count() != 1)
        {
            continue;
        }
        if let Some(bucket) = buckets.iter_mut().find(|(at, color, at_size, at_font, _)| {
            (at - baseline).abs() < 0.5
                && *color == text.color
                && (at_size - size).abs() < 0.01
                && *at_font == font
        }) {
            bucket.4.push(text);
        } else {
            buckets.push((baseline, text.color, size, font, vec![text]));
        }
    }

    let mut runs = Vec::new();
    for (_, _, size, _, mut row) in buckets {
        row.sort_by(|a, b| a.bounds.x.total_cmp(&b.bounds.x));
        let mut run = String::new();
        let mut end: Option<f32> = None;
        let mut spacing: Option<f32> = None;
        for (index, text) in row.iter().enumerate() {
            let Some(content) = text.content.as_deref() else {
                continue;
            };
            let gap = end.map(|end| text.bounds.x - end);
            let next = row
                .get(index + 1)
                .map(|next| next.bounds.x - (text.bounds.x + text.bounds.width));
            match gap {
                // A gap that matches the run's spacing continues it; one
                // about a space wider crosses a space; anything else is the
                // next label along. A run with no spacing yet takes this gap
                // as its own only when the gap after it is no tighter —
                // otherwise the tighter one is the tracking and this one is
                // the boundary it was mistaken for.
                Some(gap) if spacing.is_some_and(|spacing| close(gap, spacing)) => {}
                Some(gap)
                    if spacing.is_none()
                        && next.is_none_or(|next| gap <= next || close(gap, next)) =>
                {
                    spacing = Some(gap);
                }
                Some(gap) if spacing.is_some_and(|spacing| is_space(gap - spacing, size)) => {
                    run.push(' ');
                }
                Some(_) => {
                    runs.push(std::mem::take(&mut run));
                    spacing = None;
                }
                None => {}
            }
            run.push_str(content);
            end = Some(text.bounds.x + text.bounds.width);
        }
        runs.push(run);
    }
    runs.retain(|run| !run.is_empty());
    runs
}

fn close(gap: f32, spacing: f32) -> bool {
    (gap - spacing).abs() <= TRACKED_GAP_TOLERANCE
}

/// A space paints nothing, so it shows up as extra room between two painted
/// graphemes. No font puts a space outside this band of its size.
fn is_space(extra: f32, size: f64) -> bool {
    let size = size as f32;
    (0.15 * size..=0.75 * size).contains(&extra)
}

const TRACKED_GAP_TOLERANCE: f32 = 0.75;

fn tiny_skia_renderer<Renderer: 'static>(
    renderer: &mut Renderer,
) -> Result<&mut iced_tiny_skia::Renderer, &'static str> {
    let Some(renderer) = (renderer as &mut dyn Any).downcast_mut::<iced::Renderer>() else {
        return Err("the program uses a custom renderer");
    };
    let iced_test::renderer::fallback::Renderer::Secondary(renderer) = renderer else {
        return Err("the default renderer is not using its tiny-skia backend");
    };
    Ok(renderer)
}

fn buffer_text(buffer: &iced_tiny_skia::graphics::text::cosmic_text::Buffer) -> String {
    buffer
        .lines
        .iter()
        .map(|line| line.text())
        .collect::<Vec<_>>()
        .join("\n")
}

fn text_paint(
    text: &iced_tiny_skia::graphics::text::Text,
    transformation: iced::Transformation,
    bounds: Rectangle,
) -> Option<TextPaint> {
    use iced_tiny_skia::graphics::text::Text;

    let scale = f64::from(transformation.scale_factor());
    match text {
        Text::Paragraph {
            paragraph,
            position,
            color,
            transformation: text_transformation,
            ..
        } => {
            let paragraph = paragraph.upgrade()?;
            let size = paragraph.size();
            let total_scale = scale * f64::from(text_transformation.scale_factor());
            Some(TextPaint {
                content: Some(buffer_text(paragraph.buffer())),
                bounds,
                color: *color,
                size: Some(f64::from(size.0) * total_scale),
                font: Some(paragraph.font()),
                line_height: Some(iced::widget::text::LineHeight::Absolute(iced::Pixels(
                    (f64::from(paragraph.line_height().to_absolute(size).0) * total_scale) as f32,
                ))),
                baseline: first_baseline(
                    paragraph.buffer(),
                    *position,
                    *text_transformation,
                    transformation,
                ),
            })
        }
        Text::Editor {
            editor,
            position,
            color,
            transformation: text_transformation,
            ..
        } => {
            let editor = editor.upgrade()?;
            let metrics = editor.buffer().metrics();
            let total_scale = scale * f64::from(text_transformation.scale_factor());
            Some(TextPaint {
                content: Some(buffer_text(editor.buffer())),
                bounds,
                color: *color,
                size: Some(f64::from(metrics.font_size) * total_scale),
                font: None,
                line_height: Some(iced::widget::text::LineHeight::Absolute(iced::Pixels(
                    (f64::from(metrics.line_height) * total_scale) as f32,
                ))),
                baseline: first_baseline(
                    editor.buffer(),
                    *position,
                    *text_transformation,
                    transformation,
                ),
            })
        }
        Text::Cached {
            content,
            color,
            size,
            font,
            line_height,
            ..
        } => Some(TextPaint {
            content: Some(content.clone()),
            bounds,
            color: *color,
            size: Some(f64::from(size.0) * scale),
            font: Some(*font),
            line_height: Some(iced::widget::text::LineHeight::Absolute(iced::Pixels(
                (f64::from(line_height.0) * scale) as f32,
            ))),
            baseline: None,
        }),
        Text::Raw {
            raw,
            transformation: text_transformation,
        } => {
            let buffer = raw.buffer.upgrade()?;
            let metrics = buffer.metrics();
            let scale = scale * f64::from(text_transformation.scale_factor());
            Some(TextPaint {
                content: Some(buffer_text(&buffer)),
                bounds,
                color: raw.color,
                size: Some(f64::from(metrics.font_size) * scale),
                font: None,
                line_height: Some(iced::widget::text::LineHeight::Absolute(iced::Pixels(
                    (f64::from(metrics.line_height) * scale) as f32,
                ))),
                baseline: first_baseline(
                    &buffer,
                    raw.position,
                    *text_transformation,
                    transformation,
                ),
            })
        }
    }
}

fn first_baseline(
    buffer: &iced_tiny_skia::graphics::text::cosmic_text::Buffer,
    position: Point,
    text_transformation: iced::Transformation,
    group_transformation: iced::Transformation,
) -> Option<f64> {
    let run = buffer.layout_runs().next()?;
    let baseline = Point::new(position.x, position.y + run.line_y)
        * text_transformation
        * group_transformation;
    Some(f64::from(baseline.y))
}

fn rectangle_eq(left: Rectangle, right: Rectangle) -> bool {
    const EPSILON: f32 = 0.001;
    (left.x - right.x).abs() <= EPSILON
        && (left.y - right.y).abs() <= EPSILON
        && (left.width - right.width).abs() <= EPSILON
        && (left.height - right.height).abs() <= EPSILON
}

fn rectangle_pixel_aligned(bounds: Rectangle, scale_factor: f32) -> bool {
    let aligned = |value: f32| {
        let physical = value * scale_factor;
        physical.is_finite() && (physical - physical.round()).abs() <= 0.001
    };
    aligned(bounds.x)
        && aligned(bounds.y)
        && aligned(bounds.x + bounds.width)
        && aligned(bounds.y + bounds.height)
}

fn valid_dimension(value: f32) -> bool {
    value.is_finite() && value > 0.0
}

fn failure_origin(test_name: &str, source: Option<Location>) -> String {
    source.map_or_else(
        || format!("test `{test_name}` during boot"),
        |source| {
            format!(
                "{source}: test `{test_name}`\nstatement: {}",
                source.statement
            )
        },
    )
}

fn with_panic_context<T>(
    test_name: &str,
    source: Option<Location>,
    operation: impl FnOnce() -> T,
) -> T {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation)) {
        Ok(value) => value,
        Err(payload) => resume_panic_with_context(payload, test_name, source),
    }
}

fn resume_panic_with_context(
    payload: Box<dyn Any + Send>,
    test_name: &str,
    source: Option<Location>,
) -> ! {
    let origin = failure_origin(test_name, source);
    let prefix = source.map_or_else(
        || origin.clone(),
        |source| format!("{source}: test `{test_name}`"),
    );
    let message = payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&'static str>().copied());

    match message {
        Some(message) if message.starts_with(&prefix) => std::panic::resume_unwind(payload),
        Some(message) => std::panic::panic_any(format!("{origin}\nRust panic: {message}")),
        None => std::panic::panic_any(format!("{origin}\nRust panic: non-string payload")),
    }
}

struct KnownIds<Message>(PhantomData<fn() -> Message>);

impl<Message> KnownIds<Message> {
    fn new() -> Self {
        Self(PhantomData)
    }
}

impl<Message: 'static> Selector for KnownIds<Message> {
    type Output = String;

    fn select(&mut self, candidate: Candidate<'_>) -> Option<Self::Output> {
        if let Candidate::Custom { state, .. } = candidate.clone()
            && let Some(state) = state.downcast_ref::<SemanticState<Message>>()
        {
            return state.semantics.logical_id.clone();
        }

        candidate.id().and_then(readable_widget_id)
    }

    fn description(&self) -> String {
        "all widget ids".to_owned()
    }
}

struct FocusedIds<Message>(PhantomData<fn() -> Message>);

impl<Message> FocusedIds<Message> {
    fn new() -> Self {
        Self(PhantomData)
    }
}

impl<Message: 'static> Selector for FocusedIds<Message> {
    type Output = widget::Id;

    fn select(&mut self, candidate: Candidate<'_>) -> Option<Self::Output> {
        match candidate {
            Candidate::Focusable { id, state, .. } if state.is_focused() => id.cloned(),
            _ => None,
        }
    }

    fn description(&self) -> String {
        "focused widget ids".to_owned()
    }
}

#[derive(Clone, Copy)]
enum WidgetCapability {
    Focusable,
    Scrollable,
}

impl fmt::Display for WidgetCapability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Focusable => "focusable",
            Self::Scrollable => "scrollable",
        })
    }
}

struct MatchingWidgetIds {
    native_id: widget::Id,
    stable_id: widget::Id,
    capability: WidgetCapability,
}

impl MatchingWidgetIds {
    fn new(logical_id: &str, capability: WidgetCapability) -> Self {
        Self {
            native_id: logical_id.to_owned().into(),
            stable_id: StableId::new(logical_id).widget_id(),
            capability,
        }
    }

    fn matches(&self, id: Option<&widget::Id>) -> bool {
        id.is_some_and(|id| id == &self.native_id || id == &self.stable_id)
    }
}

impl Selector for MatchingWidgetIds {
    type Output = widget::Id;

    fn select(&mut self, candidate: Candidate<'_>) -> Option<Self::Output> {
        let id = match (&self.capability, candidate) {
            (WidgetCapability::Focusable, Candidate::Focusable { id, .. })
            | (WidgetCapability::Scrollable, Candidate::Scrollable { id, .. }) => id,
            _ => return None,
        };
        self.matches(id).then(|| id.cloned()).flatten()
    }

    fn description(&self) -> String {
        format!("{} widget id", self.capability)
    }
}

struct TextInputIds;

impl Selector for TextInputIds {
    type Output = widget::Id;

    fn select(&mut self, candidate: Candidate<'_>) -> Option<Self::Output> {
        match candidate {
            Candidate::TextInput { id, .. } => id.cloned(),
            _ => None,
        }
    }

    fn description(&self) -> String {
        "text-input widget ids".to_owned()
    }
}

fn readable_widget_id(id: &widget::Id) -> Option<String> {
    let debug = format!("{id:?}");
    let value = debug
        .strip_prefix("Id(Custom(")?
        .strip_suffix("))")?
        .strip_prefix('"')?
        .strip_suffix('"')?;
    (!value.starts_with("__ice_accessibility/")).then(|| value.to_owned())
}

fn internal_auto_id(id: &str) -> bool {
    let segment = id.rsplit('/').next().unwrap_or(id);
    segment.starts_with('@')
}

fn known_ids_display(ids: &[String]) -> String {
    if ids.is_empty() {
        "<none>".to_owned()
    } else {
        ids.join(", ")
    }
}

fn iced_key(key: &Key) -> keyboard::Key {
    match key {
        Key::Named(name) => keyboard::Key::Named(*name),
        Key::Character(value) => keyboard::Key::Character(value.clone().into()),
        Key::Unidentified => keyboard::Key::Unidentified,
    }
}

fn held_key_identity(key: &Key, metadata: &KeyMetadata) -> HeldKeyIdentity {
    let physical_key = metadata.physical_key.filter(|physical_key| {
        !matches!(
            physical_key,
            keyboard::key::Physical::Unidentified(keyboard::key::NativeCode::Unidentified)
        )
    });
    physical_key.map_or_else(
        || HeldKeyIdentity::Logical {
            key: key.clone(),
            location: metadata.location,
        },
        HeldKeyIdentity::Physical,
    )
}

fn unidentified_physical_key() -> keyboard::key::Physical {
    keyboard::key::Physical::Unidentified(keyboard::key::NativeCode::Unidentified)
}

fn theme_mode(mode: theme::Mode) -> ThemeMode {
    match mode {
        theme::Mode::None => ThemeMode::None,
        theme::Mode::Light => ThemeMode::Light,
        theme::Mode::Dark => ThemeMode::Dark,
    }
}

fn safe_path_component(value: &str) -> String {
    let value = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    if value.is_empty() {
        "test".to_owned()
    } else {
        value
    }
}

fn theme_mode_name(mode: ThemeMode) -> &'static str {
    match mode {
        ThemeMode::None => "none",
        ThemeMode::Light => "light",
        ThemeMode::Dark => "dark",
    }
}

fn platform_name(platform: Platform) -> &'static str {
    match platform {
        Platform::Linux => "linux",
        Platform::Windows => "windows",
        Platform::Macos => "macos",
        Platform::Wasm => "wasm",
    }
}

fn target_manifest(target: &Target) -> serde_json::Value {
    let accessibility = target.accessibility.as_ref().map(|data| {
        serde_json::json!({
            "role": accessibility_role_name(data.role),
            "name": data.name,
            "description": data.description,
            "value": data.value,
            "checked": data.checked,
            "expanded": data.expanded,
            "disabled": data.disabled,
            "focused": data.focused,
            "actions": {
                "click": data.supports_activate,
                "focus": data.supports_focus,
            },
        })
    });
    serde_json::json!({
        "id": target.id,
        "kind": target.kind,
        "source": target.render_source.map(|source| serde_json::json!({
            "path": manifest_source_path(source.path),
            "line": source.line,
            "column": source.column,
        })),
        "geometry": {
            "x": target.x(),
            "y": target.y(),
            "width": target.width(),
            "height": target.height(),
            "left": target.left(),
            "top": target.top(),
            "right": target.right(),
            "bottom": target.bottom(),
            "center_x": target.center_x(),
            "center_y": target.center_y(),
            "pixel_aligned": target.pixel_aligned(),
        },
        "visible": {
            "present": target.visible.is_some(),
            "x": target.visible.map(|bounds| f64::from(bounds.x)),
            "y": target.visible.map(|bounds| f64::from(bounds.y)),
            "width": target.visible.map(|bounds| f64::from(bounds.width)),
            "height": target.visible.map(|bounds| f64::from(bounds.height)),
        },
        "content": {
            "x": target.content.map(|bounds| f64::from(bounds.x)),
            "y": target.content.map(|bounds| f64::from(bounds.y)),
            "width": target.content.map(|bounds| f64::from(bounds.width)),
            "height": target.content.map(|bounds| f64::from(bounds.height)),
        },
        "translation": {
            "x": target.translation.map(|translation| f64::from(translation.x)),
            "y": target.translation.map(|translation| f64::from(translation.y)),
        },
        "scroll": {
            "x": target.translation.map(|translation| f64::from(translation.x)),
            "y": target.translation.map(|translation| f64::from(translation.y)),
        },
        "value": target.value,
        "focused": target.focused(),
        "accessibility": accessibility,
        "paint": {
            "available": target.paint_error.is_none(),
            "unavailable_reason": target.paint_error,
            "surfaces": target.surfaces.iter().map(surface_manifest).collect::<Vec<_>>(),
            "texts": target.texts.iter().map(text_manifest).collect::<Vec<_>>(),
            "images": target.images.iter().map(|image| rectangle_manifest(image.bounds)).collect::<Vec<_>>(),
        },
    })
}

fn manifest_source_path(path: &str) -> String {
    let path = Path::new(path);
    let root = std::env::var_os("ICE_AGENT_INSPECT_ROOT").map(PathBuf::from);
    normalized_source_path(path, root.as_deref())
}

fn normalized_source_path(path: &Path, root: Option<&Path>) -> String {
    root.and_then(|root| path.strip_prefix(root).ok())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn surface_manifest(surface: &SurfacePaint) -> serde_json::Value {
    let background = match surface.background {
        Background::Color(color) => serde_json::json!({
            "kind": "color",
            "color": color_manifest(color),
        }),
        Background::Gradient(iced::gradient::Gradient::Linear(linear)) => serde_json::json!({
            "kind": "linear-gradient",
            "angle_radians": linear.angle.0,
            "stops": linear.stops.iter().flatten().map(|stop| serde_json::json!({
                "offset": stop.offset,
                "color": color_manifest(stop.color),
            })).collect::<Vec<_>>(),
        }),
    };
    serde_json::json!({
        "background": background,
        "border": {
            "color": color_manifest(surface.border.color),
            "width": surface.border.width,
            "radius": {
                "top_left": surface.border.radius.top_left,
                "top_right": surface.border.radius.top_right,
                "bottom_right": surface.border.radius.bottom_right,
                "bottom_left": surface.border.radius.bottom_left,
            },
        },
        "shadow": {
            "color": color_manifest(surface.shadow.color),
            "offset_x": surface.shadow.offset.x,
            "offset_y": surface.shadow.offset.y,
            "blur_radius": surface.shadow.blur_radius,
        },
    })
}

fn text_manifest(text: &TextPaint) -> serde_json::Value {
    let line_height = text.line_height.map(|line_height| match line_height {
        iced::widget::text::LineHeight::Relative(value) => {
            serde_json::json!({ "kind": "relative", "value": value })
        }
        iced::widget::text::LineHeight::Absolute(value) => {
            serde_json::json!({ "kind": "absolute", "value": value.0 })
        }
    });
    serde_json::json!({
        "content": text.content,
        "bounds": rectangle_manifest(text.bounds),
        "color": color_manifest(text.color),
        "size": text.size,
        "font": text.font.map(font_manifest),
        "line_height": line_height,
        "baseline": text.baseline,
    })
}

fn font_manifest(font: Font) -> serde_json::Value {
    let (family_kind, family_name) = match font.family {
        iced::font::Family::Name(name) => ("named", name),
        iced::font::Family::Serif => ("generic", "serif"),
        iced::font::Family::SansSerif => ("generic", "sans-serif"),
        iced::font::Family::Cursive => ("generic", "cursive"),
        iced::font::Family::Fantasy => ("generic", "fantasy"),
        iced::font::Family::Monospace => ("generic", "monospace"),
    };
    let weight = match font.weight {
        iced::font::Weight::Thin => "thin",
        iced::font::Weight::ExtraLight => "extra-light",
        iced::font::Weight::Light => "light",
        iced::font::Weight::Normal => "normal",
        iced::font::Weight::Medium => "medium",
        iced::font::Weight::Semibold => "semibold",
        iced::font::Weight::Bold => "bold",
        iced::font::Weight::ExtraBold => "extra-bold",
        iced::font::Weight::Black => "black",
    };
    let stretch = match font.stretch {
        iced::font::Stretch::UltraCondensed => "ultra-condensed",
        iced::font::Stretch::ExtraCondensed => "extra-condensed",
        iced::font::Stretch::Condensed => "condensed",
        iced::font::Stretch::SemiCondensed => "semi-condensed",
        iced::font::Stretch::Normal => "normal",
        iced::font::Stretch::SemiExpanded => "semi-expanded",
        iced::font::Stretch::Expanded => "expanded",
        iced::font::Stretch::ExtraExpanded => "extra-expanded",
        iced::font::Stretch::UltraExpanded => "ultra-expanded",
    };
    let style = match font.style {
        iced::font::Style::Normal => "normal",
        iced::font::Style::Italic => "italic",
        iced::font::Style::Oblique => "oblique",
    };

    serde_json::json!({
        "family": { "kind": family_kind, "name": family_name },
        "weight": weight,
        "stretch": stretch,
        "style": style,
    })
}

fn rectangle_manifest(bounds: Rectangle) -> serde_json::Value {
    serde_json::json!({
        "x": bounds.x,
        "y": bounds.y,
        "width": bounds.width,
        "height": bounds.height,
    })
}

fn color_manifest(color: Color) -> serde_json::Value {
    serde_json::json!({
        "r": color.r,
        "g": color.g,
        "b": color.b,
        "a": color.a,
    })
}

fn write_png(path: &std::path::Path, rgba: &[u8], width: u32, height: u32) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|error| error.to_string())?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
    writer
        .write_image_data(rgba)
        .map_err(|error| error.to_string())?;
    writer.finish().map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tracked_text {
    use super::*;

    /// Lays out `label` the way tracked text renders: one primitive per
    /// grapheme, `spacing` apart, with spaces leaving a hole because they
    /// paint nothing.
    fn tracked(label: &str, x: f32, baseline: f64, spacing: f32, size: f64) -> Vec<TextPaint> {
        let advance = size as f32 * 0.6;
        let mut painted = Vec::new();
        let mut at = x;
        for grapheme in crate::graphemes(label) {
            if grapheme == " " {
                at += advance * 0.5 + spacing;
                continue;
            }
            painted.push(TextPaint {
                content: Some(grapheme.to_owned()),
                bounds: Rectangle {
                    x: at,
                    y: baseline as f32 - size as f32,
                    width: advance,
                    height: size as f32,
                },
                color: Color::WHITE,
                size: Some(size),
                font: Some(Font::DEFAULT),
                line_height: None,
                baseline: Some(baseline),
            });
            at += advance + spacing;
        }
        painted
    }

    #[test]
    fn a_tracked_label_is_rebuilt_from_its_graphemes() {
        let runs = tracked_runs(&tracked("ORDER", 10.0, 40.0, 1.1, 10.0));
        assert_eq!(runs, vec!["ORDER".to_owned()]);
    }

    #[test]
    fn a_space_paints_nothing_and_is_recovered_from_the_hole_it_leaves() {
        let runs = tracked_runs(&tracked("READ ONLY", 10.0, 40.0, 1.1, 10.0));
        assert_eq!(runs, vec!["READ ONLY".to_owned()]);
    }

    /// The case that makes even spacing the rule rather than the baseline:
    /// two labels in one row share everything but the gap between them.
    #[test]
    fn two_labels_along_a_row_do_not_merge_into_one() {
        let mut row = tracked("MARKET", 10.0, 40.0, 1.1, 10.0);
        row.extend(tracked("LAST", 120.0, 40.0, 1.1, 10.0));
        let runs = tracked_runs(&row);
        assert_eq!(runs, vec!["MARKET".to_owned(), "LAST".to_owned()]);
    }

    #[test]
    fn a_different_line_or_style_is_a_different_run() {
        let mut screen = tracked("TOP", 10.0, 40.0, 1.1, 10.0);
        screen.extend(tracked("LOW", 10.0, 80.0, 1.1, 10.0));
        let mut bigger = tracked("BIG", 10.0, 40.0, 1.1, 13.0);
        for text in &mut bigger {
            text.baseline = Some(40.0);
        }
        screen.extend(bigger);
        let runs = tracked_runs(&screen);
        assert_eq!(runs.len(), 3, "{runs:?}");
        for expected in ["TOP", "LOW", "BIG"] {
            assert!(runs.iter().any(|run| run == expected), "{runs:?}");
        }
    }

    /// A count beside a heading is one grapheme wide, so the run it starts
    /// has a single gap to learn its spacing from — and that gap is the one
    /// crossing to the NEXT label. Reading it as tracking glued the count to
    /// the following label's first letter and broke that label one letter in,
    /// which is what made `FUNDING IN` unfindable beside `POSITIONS 0`.
    #[test]
    fn a_one_glyph_label_does_not_swallow_the_next_label() {
        let mut row = tracked("POSITIONS", 10.0, 40.0, 1.1, 10.0);
        row.extend(tracked("0", 90.0, 40.0, 1.1, 10.0));
        row.extend(tracked("FUNDING IN", 300.0, 40.0, 1.1, 10.0));
        let runs = tracked_runs(&row);
        assert_eq!(
            runs,
            vec![
                "POSITIONS".to_owned(),
                "0".to_owned(),
                "FUNDING IN".to_owned(),
            ]
        );
    }

    /// Ordinary text is one primitive holding the whole string, and joining
    /// two of those into a word that is not on screen would be a lie.
    #[test]
    fn untracked_text_is_never_joined() {
        let mut plain = tracked("AB", 10.0, 40.0, 1.1, 10.0);
        plain[0].content = Some("SPREAD".to_owned());
        plain[1].content = Some("0.3 bps".to_owned());
        assert!(tracked_runs(&plain).is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::Element;
    use iced::widget::{button, column, container, scrollable, text, text_editor, text_input};

    #[derive(Debug, Default)]
    struct State {
        count: usize,
        input: String,
        redraws: usize,
        events: Vec<&'static str>,
        observed_system_theme: Option<theme::Mode>,
    }

    #[derive(Debug, Clone)]
    enum Message {
        Increment,
        Incremented,
        Input(String),
        ObservedKey,
        ObservedRedraw,
        ObservedEvent(&'static str),
        OpenWindow,
        OpenedWindow(window::Id),
        ReadSystemTheme,
        ObservedSystemTheme(theme::Mode),
        HangTask,
        PanicTask,
        PanicUpdate,
    }

    fn boot() -> State {
        State::default()
    }

    fn update(state: &mut State, message: Message) -> Task<Message> {
        match message {
            Message::Increment => Task::perform(async {}, |()| Message::Incremented),
            Message::Incremented => {
                state.count += 1;
                Task::none()
            }
            Message::Input(value) => {
                state.input = value;
                Task::none()
            }
            Message::ObservedKey => {
                state.count += 10;
                Task::none()
            }
            Message::ObservedRedraw => {
                state.redraws += 1;
                Task::none()
            }
            Message::ObservedEvent(event) => {
                state.events.push(event);
                Task::none()
            }
            Message::OpenWindow => {
                let (_, task) = window::open(window::Settings {
                    size: Size::new(180.0, 90.0),
                    ..window::Settings::default()
                });
                task.map(Message::OpenedWindow)
            }
            Message::OpenedWindow(_id) => Task::none(),
            Message::ReadSystemTheme => iced::system::theme().map(Message::ObservedSystemTheme),
            Message::ObservedSystemTheme(mode) => {
                state.observed_system_theme = Some(mode);
                Task::none()
            }
            Message::HangTask => Task::perform(std::future::pending(), |()| Message::Incremented),
            Message::PanicTask => Task::perform(
                async {
                    panic!("real task panic");
                },
                |()| Message::Incremented,
            ),
            Message::PanicUpdate => panic!("real update panic"),
        }
    }

    struct PaintAndRedrawProbe;

    std::thread_local! {
        static PROBE_REDRAWS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
        static PROBE_REDRAW_TIMES: std::cell::RefCell<Vec<Instant>> = const { std::cell::RefCell::new(Vec::new()) };
    }

    impl<Theme, Renderer> iced::advanced::Widget<Message, Theme, Renderer> for PaintAndRedrawProbe
    where
        Renderer: iced::advanced::text::Renderer<Font = Font>,
    {
        fn size(&self) -> Size<iced::Length> {
            Size::new(iced::Length::Fixed(100.0), iced::Length::Fixed(20.0))
        }

        fn layout(
            &mut self,
            _tree: &mut iced::advanced::widget::Tree,
            _renderer: &Renderer,
            limits: &iced::advanced::layout::Limits,
        ) -> iced::advanced::layout::Node {
            iced::advanced::layout::atomic(
                limits,
                iced::Length::Fixed(100.0),
                iced::Length::Fixed(20.0),
            )
        }

        fn draw(
            &self,
            _tree: &iced::advanced::widget::Tree,
            renderer: &mut Renderer,
            _theme: &Theme,
            _style: &iced::advanced::renderer::Style,
            layout: iced::advanced::Layout<'_>,
            _cursor: mouse::Cursor,
            viewport: &Rectangle,
        ) {
            renderer.fill_text(
                iced::advanced::text::Text {
                    content: "paint-only".to_owned(),
                    bounds: layout.bounds().size(),
                    size: iced::Pixels(14.0),
                    line_height: iced::advanced::text::LineHeight::default(),
                    font: Font::DEFAULT,
                    align_x: iced::advanced::text::Alignment::Left,
                    align_y: iced::alignment::Vertical::Top,
                    shaping: iced::advanced::text::Shaping::Basic,
                    wrapping: iced::advanced::text::Wrapping::None,
                },
                layout.position(),
                Color::WHITE,
                *viewport,
            );
        }

        fn update(
            &mut self,
            _tree: &mut iced::advanced::widget::Tree,
            event: &iced::Event,
            _layout: iced::advanced::Layout<'_>,
            _cursor: mouse::Cursor,
            _renderer: &Renderer,
            _clipboard: &mut dyn iced::advanced::Clipboard,
            shell: &mut iced::advanced::Shell<'_, Message>,
            _viewport: &Rectangle,
        ) {
            if let iced::Event::Window(window::Event::RedrawRequested(time)) = event {
                PROBE_REDRAWS.with(|redraws| redraws.set(redraws.get() + 1));
                PROBE_REDRAW_TIMES.with(|times| times.borrow_mut().push(*time));
                shell.publish(Message::ObservedRedraw);
            }
        }
    }

    fn view(state: &State) -> Element<'_, Message> {
        let source = push_render_source(HERE);
        let view = container(
            column![
                crate::accessible(
                    text(state.count),
                    StableId::new("App/root/count"),
                    crate::Role::Label,
                )
                .logical_id("App/root/count")
                .value(state.count.to_string()),
                crate::accessible(
                    button("Increment")
                        .on_press(Message::Increment)
                        .style(|_, status| button::Style {
                            background: Some(match status {
                                button::Status::Disabled => Color::TRANSPARENT.into(),
                                _ => Color::from_rgb8(51, 102, 255).into(),
                            }),
                            border: Border {
                                radius: 6.0.into(),
                                ..Border::default()
                            },
                            ..button::Style::default()
                        }),
                    StableId::new("App/root/increment"),
                    crate::Role::Button,
                )
                .logical_id("App/root/increment")
                .on_activate(Message::Increment),
                text_input("", &state.input)
                    .id("App/root/input")
                    .on_input(Message::Input),
                crate::accessible(
                    scrollable(container(text("Long content")).height(200))
                        .id("App/root/scroll")
                        .height(50),
                    StableId::new("App/root/scroll"),
                    crate::Role::GenericContainer,
                )
                .logical_id("App/root/scroll"),
                Element::new(PaintAndRedrawProbe),
            ]
            .spacing(8),
        )
        .id("App/root")
        .width(240)
        .padding(12)
        .style(|_| container::Style {
            background: Some(Color::from_rgb8(17, 17, 17).into()),
            border: Border {
                color: Color::from_rgb8(51, 102, 255),
                width: 1.0,
                radius: 6.0.into(),
            },
            ..container::Style::default()
        })
        .into();
        drop(source);
        view
    }

    const HERE: Location = Location::new("test.ice", 1, 1, "test statement");

    fn panic_message(f: impl FnOnce()) -> String {
        let payload = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
            .expect_err("operation must panic");
        payload
            .downcast::<String>()
            .map(|message| *message)
            .or_else(|payload| {
                payload
                    .downcast::<&'static str>()
                    .map(|message| (*message).to_owned())
            })
            .unwrap_or_default()
    }

    fn subscription(_state: &State) -> iced::Subscription<Message> {
        iced::event::listen_with(|event, _status, _window| {
            let observed = matches!(
                event,
                iced::Event::Keyboard(keyboard::Event::KeyPressed { .. })
            );
            observed.then(|| {
                std::thread::sleep(Duration::from_millis(10));
                Message::ObservedKey
            })
        })
    }

    fn modified_key_subscription(_state: &State) -> iced::Subscription<Message> {
        iced::event::listen_with(|event, _status, _window| {
            matches!(
                event,
                iced::Event::Keyboard(keyboard::Event::KeyPressed { modifiers, .. })
                    if modifiers.shift() && modifiers.control()
            )
            .then_some(Message::ObservedKey)
        })
    }

    fn action_subscription(_state: &State) -> iced::Subscription<Message> {
        iced::event::listen_with(|event, _status, _window| {
            let kind = match event {
                iced::Event::Mouse(mouse::Event::CursorMoved { .. }) => Some("pointer"),
                iced::Event::Mouse(
                    mouse::Event::ButtonPressed(_) | mouse::Event::ButtonReleased(_),
                ) => Some("mouse-button"),
                iced::Event::Mouse(mouse::Event::WheelScrolled { .. }) => Some("wheel"),
                iced::Event::Keyboard(keyboard::Event::KeyPressed { .. }) => Some("key"),
                iced::Event::Touch(touch::Event::FingerLost { .. }) => Some("touch-cancel"),
                iced::Event::Touch(_) => Some("touch"),
                iced::Event::InputMethod(_) => Some("ime"),
                iced::Event::Window(window::Event::Opened { .. }) => Some("window-opened"),
                iced::Event::Window(window::Event::Closed) => Some("window-closed"),
                iced::Event::Window(window::Event::Moved(_)) => Some("window-moved"),
                iced::Event::Window(window::Event::Resized(_)) => Some("window-resized"),
                iced::Event::Window(window::Event::Rescaled(_)) => Some("window-rescaled"),
                iced::Event::Window(window::Event::Focused | window::Event::Unfocused) => {
                    Some("window-focus")
                }
                iced::Event::Window(window::Event::CloseRequested) => Some("close-requested"),
                iced::Event::Window(window::Event::RedrawRequested(_)) => Some("redraw"),
                iced::Event::Window(window::Event::FileHovered(_)) => Some("file-hover"),
                iced::Event::Window(window::Event::FileDropped(_)) => Some("file-drop"),
                iced::Event::Window(window::Event::FilesHoveredLeft) => Some("file-leave"),
                _ => None,
            };
            kind.map(Message::ObservedEvent)
        })
    }

    fn every_subscription(_state: &State) -> iced::Subscription<Message> {
        every(Duration::from_millis(250)).map(|_| Message::Incremented)
    }

    fn panicking_subscription(_state: &State) -> iced::Subscription<Message> {
        iced::Subscription::run(|| {
            iced_test::futures::futures::stream::once(async {
                panic!("real subscription panic");
            })
        })
    }

    // `()` is iced's null renderer, and `iced_core` only implements the
    // renderer traits for it under `debug_assertions`. The custom-renderer
    // diagnostics below are the only tests that stand a driver up against one,
    // so they carry the same gate the implementation does.
    #[cfg(debug_assertions)]
    fn null_view(_state: &State) -> Element<'_, Message, iced::Theme, ()> {
        container(iced::widget::Space::new()).id("Null/root").into()
    }

    fn duplicate_view(_state: &State) -> Element<'_, Message> {
        column![
            crate::accessible(
                text("First"),
                StableId::new("Duplicate/item"),
                crate::Role::Label,
            )
            .logical_id("Duplicate/item"),
            crate::accessible(
                text("Second"),
                StableId::new("Duplicate/item"),
                crate::Role::Label,
            )
            .logical_id("Duplicate/item"),
        ]
        .into()
    }

    fn password_view(state: &State) -> Element<'_, Message> {
        crate::accessible(
            text_input("", &state.input)
                .id("Password/field")
                .secure(true),
            StableId::new("Password/field"),
            crate::Role::PasswordInput,
        )
        .logical_id("Password/field")
        .value_maybe(None)
        .into()
    }

    fn theme_probe_view(_state: &State) -> Element<'_, Message> {
        container(iced::widget::Space::new())
            .id("Theme/root")
            .width(100)
            .height(40)
            .style(|theme: &iced::Theme| container::Style {
                background: Some(
                    if matches!(theme, iced::Theme::Dark) {
                        Color::from_rgb8(1, 2, 3)
                    } else {
                        Color::from_rgb8(250, 251, 252)
                    }
                    .into(),
                ),
                ..container::Style::default()
            })
            .into()
    }

    /// An Ice `layer` lowers to a translated `float`, which draws nothing in
    /// place and everything through iced's overlay instead — exactly the shape
    /// the generator emits for an open `overlay`.
    fn layered_view(_state: &State) -> Element<'_, Message> {
        iced::widget::Stack::new()
            .width(iced::Fill)
            .height(iced::Fill)
            .push(text("beneath the layer"))
            .push(
                iced::widget::float(container(text("inside the layer")).id("Layer/panel"))
                    .translate(|_, _| iced::Vector::new(f32::EPSILON, 0.0)),
            )
            .into()
    }

    fn stable_scroll_view(_state: &State) -> Element<'_, Message> {
        scrollable(container(text("Stable content")).height(200))
            .id(StableId::new("Stable/scroll").widget_id())
            .height(50)
            .into()
    }

    fn scrolled_card_view(_state: &State) -> Element<'_, Message> {
        scrollable(column![
            container(text("top")).height(400),
            container(text("deep card")).id("Scrolled/card").height(40),
            container(text("tail")).height(400),
        ])
        .id("Scrolled/scroll")
        .height(120)
        .into()
    }

    fn duplicate_scroll_view(_state: &State) -> Element<'_, Message> {
        column![
            scrollable(container(text("First")).height(200))
                .id("Duplicate/scroll")
                .height(50),
            scrollable(container(text("Second")).height(200))
                .id("Duplicate/scroll")
                .height(50),
        ]
        .into()
    }

    fn accessible_input_view(state: &State) -> Element<'_, Message> {
        crate::accessible(
            text_input("", &state.input)
                .id("Accessible/input")
                .on_input(Message::Input),
            StableId::new("Accessible/input"),
            crate::Role::TextInput,
        )
        .logical_id("Accessible/input")
        .value(state.input.clone())
        .into()
    }

    fn identified_extern_view(_state: &State) -> Element<'_, Message> {
        let source = push_render_source(HERE);
        let native: Element<'_, Option<()>> = crate::accessible(
            container(text("Chart")).width(120).height(80),
            StableId::new("native-chart"),
            crate::Role::Image,
        )
        .logical_id("native-chart")
        .label("Market chart")
        .into();
        let native = native.map(|_| Message::ObservedKey);
        let identified = container(native).id("App/chart");
        drop(source);
        sourced(identified, HERE)
    }

    #[derive(Default)]
    struct EditorState {
        content: text_editor::Content,
    }

    #[derive(Debug, Clone)]
    enum EditorMessage {
        Edit(text_editor::Action),
    }

    fn editor_update(state: &mut EditorState, message: EditorMessage) -> Task<EditorMessage> {
        match message {
            EditorMessage::Edit(action) => state.content.perform(action),
        }
        Task::none()
    }

    fn editor_view(state: &EditorState) -> Element<'_, EditorMessage> {
        text_editor(&state.content)
            .id("Editor/root")
            .on_action(EditorMessage::Edit)
            .into()
    }

    #[test]
    fn drives_real_updates_and_keeps_widget_state() {
        let mut driver = Driver::new(
            iced::application::<State, Message, iced::Theme, iced::Renderer>(boot, update, view),
            Config::new("runtime").viewport(320.0, 240.0),
        );

        driver.check_exists("App/root", true, HERE);
        driver.check_exists("App/missing", false, HERE);
        driver.check_text("paint-only", Some("App/root"), false, HERE);
        assert_eq!(driver.state().redraws, 1);
        driver.check_text("0", Some("App/root/count"), false, HERE);
        driver.check_text("missing", None, true, HERE);
        assert_eq!(driver.target("App/root", HERE).width(), 240.0);
        driver.move_to("App/root/increment", HERE);
        driver.press_with("App/root/increment", MouseButton::Left, HERE);
        driver.release_button(MouseButton::Left, HERE);
        assert_eq!(driver.state().count, 1);
        driver.click_with("App/root/increment", MouseButton::Left, 1, HERE);
        assert_eq!(driver.state().count, 2);
        driver.click_with("App/root/input", MouseButton::Left, 1, HERE);
        driver.typewrite("iced", HERE);
        assert_eq!(driver.target("App/root/input", HERE).value(), "iced");
        driver.key(Key::named(keyboard::key::Named::Escape), HERE);
        driver.check_text("2", None, false, HERE);
        driver.resize(640.0, 480.0, HERE);
        assert_eq!(driver.viewport(), Size::new(640.0, 480.0));
        let scroll = driver.target("App/root/scroll", HERE);
        assert!(scroll.content_height() >= 200.0);
        assert_eq!(scroll.scroll_y(), 0.0);
    }

    #[test]
    fn action_tracing_records_ordered_phases_and_is_inert_when_disabled() {
        let program =
            || iced::application::<State, Message, iced::Theme, iced::Renderer>(boot, update, view);
        let config = Config::new("trace_order").viewport(320.0, 240.0);
        let mut ordinary = Driver::new(program(), config.clone());
        ordinary.perform_action(
            Action::Click {
                target: "App/root/increment".into(),
                button: MouseButton::Left,
                count: 1,
            },
            HERE,
        );
        assert_eq!(ordinary.state().count, 1);
        assert!(ordinary.trace.is_none());
        assert_eq!(ordinary.action_index, 0);

        let mut traced = Driver::new(program(), config.clone());
        traced.enable_trace(
            &config,
            ui_lang_template::trace::Configuration {
                mode: ui_lang_template::trace::Mode::Replay,
                test: None,
                warmup: 0,
                repeat: 1,
                steps: Some(1),
                confirmations: 1,
                deadline_ms: None,
                max_to_median_ratio: None,
                generator_version: None,
            },
            None,
        );
        traced.perform_action(
            Action::Click {
                target: "App/root/increment".into(),
                button: MouseButton::Left,
                count: 1,
            },
            HERE,
        );
        assert_eq!(traced.state().count, ordinary.state().count);
        let artifact = traced.take_trace();
        assert_eq!(artifact.actions.len(), 1);
        assert_eq!(
            artifact.actions[0].target.as_deref(),
            Some("App/root/increment")
        );
        assert!(artifact.actions[0].target_source.is_some());
        assert_eq!(artifact.samples.last().unwrap().phase, Phase::Action);
        let update = artifact
            .samples
            .iter()
            .position(|sample| sample.phase == Phase::ProgramUpdate)
            .unwrap();
        let final_settle = artifact
            .samples
            .iter()
            .rposition(|sample| sample.phase == Phase::TaskSettle)
            .unwrap();
        assert!(update < final_settle);
    }

    #[derive(Debug, Default)]
    struct CliffState {
        armed: bool,
        hits: usize,
    }

    #[derive(Debug, Clone)]
    enum CliffMessage {
        Arm,
        Hit,
    }

    static CLIFF_BOOTS: AtomicUsize = AtomicUsize::new(0);

    fn cliff_boot() -> CliffState {
        CLIFF_BOOTS.fetch_add(1, Ordering::Relaxed);
        CliffState::default()
    }

    fn cliff_update(state: &mut CliffState, message: CliffMessage) -> Task<CliffMessage> {
        match message {
            CliffMessage::Arm => state.armed = true,
            CliffMessage::Hit => {
                if state.armed {
                    std::thread::sleep(Duration::from_millis(60));
                }
                state.hits += 1;
            }
        }
        Task::none()
    }

    fn cliff_controls<'a>() -> Element<'a, CliffMessage> {
        column![
            crate::accessible(
                button("Arm").on_press(CliffMessage::Arm),
                StableId::new("Cliff/arm"),
                crate::Role::Button,
            )
            .logical_id("Cliff/arm")
            .on_activate(CliffMessage::Arm),
            crate::accessible(
                button("Cliff").on_press(CliffMessage::Hit),
                StableId::new("Cliff/slow"),
                crate::Role::Button,
            )
            .logical_id("Cliff/slow")
            .on_activate(CliffMessage::Hit),
        ]
        .into()
    }

    fn cliff_view(_state: &CliffState) -> Element<'_, CliffMessage> {
        cliff_controls()
    }

    #[test]
    fn seeded_campaign_confirms_replays_and_reduces_a_stateful_latency_cliff() {
        const SEED: u64 = 2;
        const STEPS: usize = 21;
        let program = || {
            iced::application::<CliffState, CliffMessage, iced::Theme, iced::Renderer>(
                cliff_boot,
                cliff_update,
                cliff_view,
            )
        };
        let config = Config::new("seeded_cliff")
            .source(HERE)
            .viewport(320.0, 240.0)
            .artifact_dir(
                std::env::temp_dir().join(format!("ice-trace-cliff-{}-{SEED}", std::process::id())),
            );
        let artifact_dir = config.artifact_dir.clone().unwrap();
        let artifact = trace::run_campaign(
            program,
            config.clone(),
            trace::Campaign {
                mode: ui_lang_template::trace::Mode::Fuzz,
                seed: Some(SEED),
                steps: Some(STEPS),
                confirmations: 2,
                deadline_ms: Some(30.0),
                max_to_median_ratio: None,
                replay: None,
            },
        );
        artifact.validate().unwrap();
        let finding = artifact
            .finding
            .as_ref()
            .expect("confirmed latency finding");
        assert_eq!(finding.kind, ui_lang_template::trace::FindingKind::Latency);
        assert_eq!(finding.confirmed_runs, 2);
        let reduction = artifact
            .reduction
            .as_ref()
            .expect("the finding sequence must be minimized");
        assert!(reduction.minimized_actions.len() < artifact.actions.len());
        let reduced_targets = reduction
            .minimized_actions
            .iter()
            .filter_map(|action| action.target.as_deref())
            .collect::<Vec<_>>();
        assert_eq!(reduced_targets, ["Cliff/arm", "Cliff/slow"]);
        let worst = artifact
            .worst_states
            .first()
            .expect("confirmed finding keeps untimed visual evidence");
        assert!(Path::new(&worst.png).is_file());
        assert!(Path::new(&worst.manifest).is_file());

        let boots_before_replay = CLIFF_BOOTS.load(Ordering::Relaxed);
        let replay = trace::run_campaign(
            program,
            config,
            trace::Campaign {
                mode: ui_lang_template::trace::Mode::Replay,
                seed: artifact.seed,
                steps: Some(artifact.actions.len()),
                confirmations: 1,
                deadline_ms: Some(30.0),
                max_to_median_ratio: None,
                replay: Some(artifact.clone()),
            },
        );
        assert!(CLIFF_BOOTS.load(Ordering::Relaxed) > boots_before_replay);
        assert_eq!(replay.actions, artifact.actions);
        assert_eq!(
            replay.finding.as_ref().map(|finding| &finding.fingerprint),
            Some(&finding.fingerprint)
        );
        std::fs::remove_dir_all(artifact_dir).unwrap();
    }

    #[derive(Debug)]
    struct FlakyCliffState {
        armed: bool,
        slow: bool,
    }

    fn flaky_cliff_update(
        state: &mut FlakyCliffState,
        message: CliffMessage,
    ) -> Task<CliffMessage> {
        match message {
            CliffMessage::Arm => state.armed = true,
            CliffMessage::Hit if state.armed && state.slow => {
                std::thread::sleep(Duration::from_millis(60));
            }
            CliffMessage::Hit => {}
        }
        Task::none()
    }

    fn flaky_cliff_view(_state: &FlakyCliffState) -> Element<'_, CliffMessage> {
        cliff_controls()
    }

    #[test]
    fn confirmation_discards_a_one_off_latency_candidate() {
        let boots = Arc::new(AtomicUsize::new(0));
        let program = || {
            let boots = Arc::clone(&boots);
            iced::application::<FlakyCliffState, CliffMessage, iced::Theme, iced::Renderer>(
                move || FlakyCliffState {
                    armed: false,
                    slow: boots.fetch_add(1, Ordering::Relaxed) == 0,
                },
                flaky_cliff_update,
                flaky_cliff_view,
            )
        };
        let artifact = trace::run_campaign(
            program,
            Config::new("flaky_cliff").viewport(320.0, 240.0),
            trace::Campaign {
                mode: ui_lang_template::trace::Mode::Fuzz,
                seed: Some(2),
                steps: Some(21),
                confirmations: 2,
                deadline_ms: Some(30.0),
                max_to_median_ratio: None,
                replay: None,
            },
        );
        assert!(boots.load(Ordering::Relaxed) >= 2);
        assert!(artifact.finding.is_none());
        assert!(artifact.reduction.is_none());
        assert!(artifact.worst_states.is_empty());
    }

    #[test]
    #[ignore = "trace overhead contract run explicitly by the performance CI name filter"]
    fn performance_contract_interaction_trace_overhead() {
        // A longer sample amortizes scheduler jitter while retaining the 1.5x ceiling.
        const ACTIONS: usize = 300;
        let program =
            || iced::application::<State, Message, iced::Theme, iced::Renderer>(boot, update, view);
        let action = || Action::Redraw;
        let config = Config::new("trace_overhead").viewport(320.0, 240.0);
        let mut ordinary = Driver::new(program(), config.clone());
        let started = Instant::now();
        for _ in 0..ACTIONS {
            ordinary.perform_action(action(), HERE);
        }
        let ordinary_elapsed = started.elapsed();

        let mut traced = Driver::new(program(), config.clone());
        traced.enable_trace(
            &config,
            ui_lang_template::trace::Configuration {
                mode: ui_lang_template::trace::Mode::Replay,
                test: None,
                warmup: 0,
                repeat: 1,
                steps: Some(ACTIONS),
                confirmations: 1,
                deadline_ms: None,
                max_to_median_ratio: None,
                generator_version: None,
            },
            None,
        );
        let started = Instant::now();
        for _ in 0..ACTIONS {
            traced.perform_action(action(), HERE);
        }
        let traced_elapsed = started.elapsed();
        let artifact = traced.take_trace();
        assert_eq!(
            ordinary.action_index, 0,
            "disabled tracing retains no action state"
        );
        assert_eq!(artifact.actions.len(), ACTIONS);
        assert!(
            traced_elapsed <= ordinary_elapsed.mul_f64(1.5),
            "enabled tracing exceeded its documented 50% ceiling: ordinary={ordinary_elapsed:?}, traced={traced_elapsed:?}"
        );
    }

    /// What is drawn on a layer is on screen, so a question about what is on
    /// screen has to reach it. Both sides are asserted: a modal that answered
    /// "present" to everything would be no more use than one that answered
    /// "missing" to everything, and it is the second that let `expect no text`
    /// inside a modal pass for ink that was plainly there.
    #[test]
    fn text_drawn_on_a_layer_is_visible_to_the_text_index() {
        let mut driver = Driver::new(
            iced::application::<State, Message, iced::Theme, iced::Renderer>(
                boot,
                update,
                layered_view,
            ),
            Config::new("layered_text").viewport(320.0, 240.0),
        );

        driver.check_text("beneath the layer", None, false, HERE);
        driver.check_text("inside the layer", None, false, HERE);
        driver.check_text("nowhere on this screen", None, true, HERE);
        driver.check_text("inside the layer", Some("Layer/panel"), false, HERE);
    }

    #[test]
    fn targeted_widget_operations_require_the_capability_they_invoke() {
        let mut driver = Driver::new(
            iced::application::<State, Message, iced::Theme, iced::Renderer>(boot, update, view),
            Config::new("target_capabilities").viewport(320.0, 240.0),
        );
        let failure = panic_message(|| driver.focus("App/root", HERE));
        assert!(
            failure.contains("expected: a focusable target"),
            "{failure}"
        );
        PROBE_REDRAWS.with(|redraws| redraws.set(0));
        driver.focus("App/root/input", HERE);
        PROBE_REDRAWS.with(|redraws| assert_eq!(redraws.get(), 0));

        let mut editor = Driver::new(
            iced::application::<EditorState, EditorMessage, iced::Theme, iced::Renderer>(
                EditorState::default,
                editor_update,
                editor_view,
            ),
            Config::new("editor_selection").viewport(320.0, 240.0),
        );
        editor.focus("Editor/root", HERE);
        assert!(editor.target("Editor/root", HERE).focused());
        for action in [
            Action::Select { start: 0, end: 0 },
            Action::SelectAll,
            Action::Cursor(0),
            Action::CursorFront,
            Action::CursorEnd,
            Action::Clear,
            Action::Replace("replacement".to_owned()),
        ] {
            let failure = panic_message(|| {
                editor.perform_action(action, HERE);
            });
            assert!(
                failure.contains("expected: exactly one focused text input with an id"),
                "{failure}"
            );
        }

        let mut input = Driver::new(
            iced::application::<State, Message, iced::Theme, iced::Renderer>(
                boot,
                update,
                accessible_input_view,
            ),
            Config::new("accessible_input_selection").viewport(320.0, 240.0),
        );
        input.focus("Accessible/input", HERE);
        input.select_all(HERE);
        input.cursor_front(HERE);
        assert!(input.target("Accessible/input", HERE).focused());
    }

    #[test]
    fn targeted_scroll_uses_the_widget_id_that_validation_matched() {
        let mut driver = Driver::new(
            iced::application::<State, Message, iced::Theme, iced::Renderer>(
                boot,
                update,
                stable_scroll_view,
            ),
            Config::new("stable_scroll").viewport(320.0, 240.0),
        );
        driver.scroll_by("Stable/scroll", 0.0, 24.0, HERE);
        assert!(driver.target("Stable/scroll", HERE).scroll_y() > 0.0);
        driver.snap("Stable/scroll", 0.0, 1.0, HERE);
        assert!(driver.target("Stable/scroll", HERE).scroll_y() > 0.0);

        let mut duplicate = Driver::new(
            iced::application::<State, Message, iced::Theme, iced::Renderer>(
                boot,
                update,
                duplicate_scroll_view,
            ),
            Config::new("duplicate_scroll").viewport(320.0, 240.0),
        );
        let failure = panic_message(|| {
            duplicate.scroll_by("Duplicate/scroll", 0.0, 24.0, HERE);
        });
        assert!(failure.contains("target lookup is ambiguous"), "{failure}");
    }

    /// A card scrolled into view is searched where it now is. The region for
    /// `within` used to be the target's layout bounds, which is where it sits
    /// unscrolled, so ink plainly on screen inside a scrolled target answered
    /// "missing" — and `expect no text` then passed for text in plain view.
    #[test]
    fn text_within_a_scrolled_target_is_searched_where_the_target_now_is() {
        let mut driver = Driver::new(
            iced::application::<State, Message, iced::Theme, iced::Renderer>(
                boot,
                update,
                scrolled_card_view,
            ),
            Config::new("scrolled_card").viewport(320.0, 240.0),
        );
        // Off the bottom of a 120px window: the card is laid out at y=400.
        driver.check_text("deep card", Some("Scrolled/card"), true, HERE);
        driver.scroll_to("Scrolled/scroll", 0.0, 400.0, HERE);
        assert!(driver.target("Scrolled/scroll", HERE).scroll_y() > 0.0);
        driver.check_text("deep card", Some("Scrolled/card"), false, HERE);
        let missing =
            panic_message(|| driver.check_text("deep card", Some("Scrolled/card"), true, HERE));
        assert!(missing.contains("visible: Some("), "{missing}");
    }

    #[test]
    fn tap_allocates_around_retained_multitouch_contacts() {
        let mut driver = Driver::new(
            iced::application::<State, Message, iced::Theme, iced::Renderer>(boot, update, view),
            Config::new("tap_touch_ids").viewport(320.0, 240.0),
        );
        let position = driver.target("App/root", HERE).bounds.center();
        driver.touch(TouchPhase::Down, 0, position.x, position.y, HERE);
        driver.tap("App/root/increment", 2, HERE);
        assert_eq!(driver.touches, HashMap::from([(0, position)]));
        driver.touch(TouchPhase::Cancel, 0, position.x, position.y, HERE);
        assert!(driver.touches.is_empty());
    }

    #[test]
    fn semantic_actions_share_one_driver_boundary() {
        let mut driver = Driver::new(
            iced::application::<State, Message, iced::Theme, iced::Renderer>(boot, update, view),
            Config::new("semantic_actions").viewport(320.0, 240.0),
        );

        driver.perform_action(
            Action::Click {
                target: "App/root/increment".to_owned(),
                button: MouseButton::Left,
                count: 2,
            },
            HERE,
        );
        assert_eq!(driver.state().count, 2);

        driver.perform_action(Action::Focus("App/root/input".to_owned()), HERE);
        driver.perform_action(Action::Type("abcdef".to_owned()), HERE);
        driver.perform_action(Action::Select { start: 1, end: 5 }, HERE);
        driver.perform_action(Action::Type("X".to_owned()), HERE);
        assert_eq!(driver.state().input, "aXf");

        driver.perform_action(
            Action::ScrollTo {
                target: "App/root/scroll".to_owned(),
                x: 0.0,
                y: 40.0,
            },
            HERE,
        );
        let scroll = driver.target("App/root/scroll", HERE);
        assert!(scroll.scroll_y() > 0.0, "{scroll:?}");

        driver.perform_action(Action::Leave, HERE);
        assert!(!driver.cursor_inside);
        driver.perform_action(Action::MoveToPoint(Point::new(5.0, 5.0)), HERE);
        assert!(driver.cursor_inside);
    }

    #[test]
    fn perform_action_drives_pointer_editing_ime_and_touch_contracts() {
        let program =
            iced::application::<State, Message, iced::Theme, iced::Renderer>(boot, update, view)
                .subscription(action_subscription);
        let mut driver = Driver::new(
            program,
            Config::new("action_input_matrix").viewport(320.0, 240.0),
        );

        let button = driver.target("App/root/increment", HERE);
        let button_center = Point::new(button.bounds.center_x(), button.bounds.center_y());
        driver.perform_action(Action::MoveTo("App/root/input".to_owned()), HERE);
        driver.perform_action(Action::MoveToPoint(Point::new(8.0, 8.0)), HERE);
        driver.perform_action(
            Action::ClickAt {
                position: button_center,
                button: MouseButton::Left,
                count: 1,
            },
            HERE,
        );
        assert_eq!(driver.state().count, 1);
        driver.perform_action(
            Action::Press {
                target: "App/root/increment".to_owned(),
                button: MouseButton::Right,
            },
            HERE,
        );
        assert!(driver.pressed_mouse.contains(&MouseButton::Right));
        for action in [
            Action::Press {
                target: "App/root/increment".to_owned(),
                button: MouseButton::Right,
            },
            Action::Click {
                target: "App/root/increment".to_owned(),
                button: MouseButton::Right,
                count: 1,
            },
        ] {
            let duplicate = panic_message(|| {
                driver.perform_action(action.clone(), HERE);
            });
            assert!(
                duplicate.contains("expected: a pointer button that is not already pressed"),
                "{duplicate}"
            );
            assert!(driver.pressed_mouse.contains(&MouseButton::Right));
        }
        driver.perform_action(Action::Release(MouseButton::Right), HERE);
        assert!(driver.pressed_mouse.is_empty());
        driver.perform_action(Action::Wheel(WheelDelta::Lines { x: 0.0, y: -2.0 }), HERE);
        driver.perform_action(Action::Wheel(WheelDelta::Pixels { x: 0.0, y: -12.0 }), HERE);
        driver.perform_action(
            Action::ScrollBy {
                target: "App/root/scroll".to_owned(),
                x: 0.0,
                y: 24.0,
            },
            HERE,
        );
        assert!(driver.target("App/root/scroll", HERE).scroll_y() > 0.0);
        driver.perform_action(
            Action::Snap {
                target: "App/root/scroll".to_owned(),
                x: 0.0,
                y: 0.5,
            },
            HERE,
        );
        driver.perform_action(Action::SnapEnd("App/root/scroll".to_owned()), HERE);
        driver.perform_action(
            Action::Drag {
                from: "App/root".to_owned(),
                to: "App/root/input".to_owned(),
            },
            HERE,
        );
        driver.perform_action(
            Action::Press {
                target: "App/root".to_owned(),
                button: MouseButton::Left,
            },
            HERE,
        );
        for action in [
            Action::Press {
                target: "App/root".to_owned(),
                button: MouseButton::Left,
            },
            Action::Click {
                target: "App/root/increment".to_owned(),
                button: MouseButton::Left,
                count: 1,
            },
            Action::Drag {
                from: "App/root/increment".to_owned(),
                to: "App/root/input".to_owned(),
            },
        ] {
            let duplicate = panic_message(|| {
                driver.perform_action(action.clone(), HERE);
            });
            assert!(
                duplicate.contains("expected: a pointer button that is not already pressed"),
                "{duplicate}"
            );
            assert!(driver.pressed_mouse.contains(&MouseButton::Left));
        }
        driver.perform_action(Action::DropAt("App/root/input".to_owned()), HERE);
        assert!(driver.pressed_mouse.is_empty());

        driver.perform_action(Action::Focus("App/root/input".to_owned()), HERE);
        assert!(driver.target("App/root/input", HERE).focused());
        driver.perform_action(Action::FocusNext, HERE);
        driver.perform_action(Action::FocusPrevious, HERE);
        driver.perform_action(Action::Blur, HERE);
        assert!(!driver.target("App/root/input", HERE).focused());
        driver.perform_action(Action::Focus("App/root/input".to_owned()), HERE);
        driver.perform_action(Action::Type("abcd".to_owned()), HERE);
        driver.perform_action(Action::SelectAll, HERE);
        driver.perform_action(Action::Type("q".to_owned()), HERE);
        driver.perform_action(Action::Replace("hello".to_owned()), HERE);
        driver.perform_action(Action::Select { start: 1, end: 4 }, HERE);
        driver.perform_action(Action::Type("X".to_owned()), HERE);
        driver.perform_action(Action::CursorFront, HERE);
        driver.perform_action(Action::Type("<".to_owned()), HERE);
        driver.perform_action(Action::CursorEnd, HERE);
        driver.perform_action(Action::Type(">".to_owned()), HERE);
        driver.perform_action(Action::Cursor(1), HERE);
        driver.perform_action(Action::Type("!".to_owned()), HERE);
        assert_eq!(driver.state().input, "<!hXo>");
        driver.perform_action(Action::Clear, HERE);
        assert!(driver.state().input.is_empty());

        driver.perform_action(Action::Composition(CompositionPhase::Start), HERE);
        driver.perform_action(
            Action::Composition(CompositionPhase::Update {
                text: "한글".to_owned(),
                selection: Some(0..3),
            }),
            HERE,
        );
        driver.perform_action(
            Action::Composition(CompositionPhase::Commit("한".to_owned())),
            HERE,
        );
        assert!(driver.ime_open);
        driver.perform_action(Action::Composition(CompositionPhase::Cancel), HERE);
        assert!(!driver.ime_open);

        driver.perform_action(
            Action::Touch {
                phase: TouchPhase::Down,
                id: 7,
                position: button_center,
            },
            HERE,
        );
        driver.perform_action(
            Action::Touch {
                phase: TouchPhase::Move,
                id: 7,
                position: Point::new(button_center.x + 1.0, button_center.y + 1.0),
            },
            HERE,
        );
        driver.perform_action(
            Action::Touch {
                phase: TouchPhase::Up,
                id: 7,
                position: button_center,
            },
            HERE,
        );
        driver.perform_action(
            Action::Touch {
                phase: TouchPhase::Down,
                id: 8,
                position: button_center,
            },
            HERE,
        );
        driver.perform_action(
            Action::Touch {
                phase: TouchPhase::Cancel,
                id: 8,
                position: button_center,
            },
            HERE,
        );
        driver.perform_action(
            Action::Tap {
                target: "App/root/increment".to_owned(),
                count: 2,
            },
            HERE,
        );
        assert!(driver.touches.is_empty());
        driver.perform_action(Action::Leave, HERE);
        assert!(!driver.cursor_inside);

        for event in [
            "pointer",
            "mouse-button",
            "wheel",
            "key",
            "ime",
            "touch",
            "touch-cancel",
        ] {
            assert!(driver.state().events.contains(&event), "missing {event}");
        }
    }

    #[test]
    fn perform_action_drives_window_system_file_and_monotonic_time_contracts() {
        PROBE_REDRAW_TIMES.with(|times| times.borrow_mut().clear());
        let program =
            iced::application::<State, Message, iced::Theme, iced::Renderer>(boot, update, view)
                .subscription(action_subscription);
        let mut driver = Driver::new(
            program,
            Config::new("action_environment_matrix").viewport(320.0, 240.0),
        );

        driver.perform_action(Action::WindowMove(Point::new(12.0, 34.0)), HERE);
        assert_eq!(driver.window_position, Some(Point::new(12.0, 34.0)));
        driver.perform_action(Action::Resize(Size::new(300.0, 180.0)), HERE);
        assert_eq!(driver.viewport(), Size::new(300.0, 180.0));
        driver.perform_action(Action::Rescale(1.5), HERE);
        assert_eq!(driver.scale_factor(), 1.5);
        driver.perform_action(Action::WindowFocus(false), HERE);
        assert!(!driver.window_focused);
        driver.perform_action(Action::WindowFocus(true), HERE);
        assert!(driver.window_focused);
        driver.perform_action(Action::WindowOpened, HERE);
        driver.perform_action(Action::WindowClosed, HERE);
        driver.perform_action(Action::CloseRequested, HERE);

        driver.perform_action(Action::SystemTheme(ThemeMode::Dark), HERE);
        assert_eq!(driver.system_theme, ThemeMode::Dark);
        driver.perform_action(Action::FileHover(PathBuf::from("hover.txt")), HERE);
        driver.perform_action(Action::FileDrop(PathBuf::from("drop.txt")), HERE);
        driver.perform_action(Action::FileLeave, HERE);

        for action in [
            Action::Wait(Duration::ZERO),
            Action::Advance(Duration::ZERO),
        ] {
            let zero = panic_message(|| {
                driver.perform_action(action.clone(), HERE);
            });
            assert!(zero.contains("expected: a positive duration"), "{zero}");
        }
        let overflow = panic_message(|| {
            driver.perform_action(Action::Advance(Duration::MAX), HERE);
        });
        assert!(
            overflow.contains("expected: a duration within the platform Instant range"),
            "{overflow}"
        );
        driver.perform_action(Action::Advance(Duration::from_millis(20)), HERE);
        driver.perform_action(Action::Wait(Duration::from_millis(1)), HERE);
        driver.perform_action(Action::Idle, HERE);
        driver.perform_action(Action::Redraw, HERE);
        let _ = driver.target("App/root", HERE);
        PROBE_REDRAW_TIMES.with(|times| {
            let times = times.borrow();
            assert!(
                times.len() >= 4,
                "expected action and paint redraw timestamps"
            );
            assert!(times.windows(2).all(|pair| pair[0] <= pair[1]), "{times:?}");
        });
        assert!(driver.state().redraws >= 4);

        for event in [
            "window-moved",
            "window-resized",
            "window-rescaled",
            "window-focus",
            "window-opened",
            "window-closed",
            "close-requested",
            "file-hover",
            "file-drop",
            "file-leave",
        ] {
            assert!(driver.state().events.contains(&event), "missing {event}");
        }

        driver.perform_action(
            Action::ScrollTo {
                target: "App/root/scroll".to_owned(),
                x: 0.0,
                y: 40.0,
            },
            HERE,
        );
        driver.perform_action(Action::MoveToPoint(Point::new(10.0, 10.0)), HERE);
        driver.perform_action(
            Action::Press {
                target: "App/root".to_owned(),
                button: MouseButton::Right,
            },
            HERE,
        );
        driver.perform_action(Action::Focus("App/root/input".to_owned()), HERE);
        driver.perform_action(Action::Type("persisted".to_owned()), HERE);
        driver.perform_action(
            Action::Modifiers(Modifiers::new(true, false, false, false)),
            HERE,
        );
        driver.perform_action(
            Action::KeyDown {
                key: Key::named(keyboard::key::Named::Escape),
                metadata: KeyMetadata::default(),
            },
            HERE,
        );
        driver.perform_action(
            Action::Touch {
                phase: TouchPhase::Down,
                id: 99,
                position: Point::new(10.0, 10.0),
            },
            HERE,
        );
        driver.perform_action(Action::Composition(CompositionPhase::Start), HERE);
        driver.perform_action(Action::WindowFocus(false), HERE);
        assert!(driver.target("App/root/input", HERE).focused());
        assert!(driver.target("App/root/scroll", HERE).scroll_y() > 0.0);
        assert!(driver.cursor_inside);
        assert!(!driver.pressed_mouse.is_empty());
        assert!(!driver.pressed_keys.is_empty());
        assert!(!driver.touches.is_empty());
        assert!(driver.ime_open);
        let old_window = driver.window();
        let old_time = driver.logical_time;

        driver.dispatch(Message::OpenWindow, HERE);
        assert_ne!(driver.window(), old_window);
        assert_eq!(driver.viewport(), Size::new(180.0, 90.0));
        assert_eq!(driver.window_position, None);
        assert!(driver.window_focused);
        assert!(matches!(driver.cursor, mouse::Cursor::Unavailable));
        assert!(!driver.cursor_inside);
        assert!(driver.pressed_mouse.is_empty());
        assert_eq!(driver.modifiers, Modifiers::NONE);
        assert!(driver.pressed_keys.is_empty());
        assert!(driver.touches.is_empty());
        assert!(!driver.ime_open);
        assert_eq!(driver.logical_time, old_time);
        assert_eq!(driver.state().input, "persisted");
        assert!(!driver.target("App/root/input", HERE).focused());
        assert_eq!(driver.target("App/root/scroll", HERE).scroll_y(), 0.0);
        let screenshot = driver.screenshot_at(Some(HERE));
        assert_eq!(screenshot.size, Size::new(270, 135));
    }

    #[test]
    fn accessibility_actions_and_expectations_use_live_semantics() {
        let mut driver = Driver::new(
            iced::application::<State, Message, iced::Theme, iced::Renderer>(boot, update, view),
            Config::new("accessibility_actions").viewport(320.0, 240.0),
        );

        driver.check_accessibility_str(
            "App/root/increment",
            AccessibilityProperty::Role,
            "button",
            HERE,
        );
        driver.check_accessibility_action(
            "App/root/increment",
            AccessibilityAction::Click,
            true,
            HERE,
        );
        driver.accessibility_activate("App/root/increment", HERE);
        assert_eq!(driver.state().count, 1);
        driver.accessibility_focus("App/root/increment", HERE);
        driver.check_accessibility_bool(
            "App/root/increment",
            AccessibilityProperty::Focused,
            true,
            HERE,
        );
    }

    #[test]
    fn identified_extern_target_retains_descendant_semantics() {
        let mut driver = Driver::new(
            iced::application::<State, Message, iced::Theme, iced::Renderer>(
                boot,
                update,
                identified_extern_view,
            ),
            Config::new("identified_extern_semantics").viewport(320.0, 240.0),
        );

        driver.check_accessibility_str("App/chart", AccessibilityProperty::Role, "image", HERE);
        driver.check_accessibility_str(
            "App/chart",
            AccessibilityProperty::Name,
            "Market chart",
            HERE,
        );
    }

    #[test]
    fn named_capture_writes_png_and_structured_manifest() {
        let artifact_dir = std::env::temp_dir().join(format!(
            "ice-test-capture-{}-{}",
            std::process::id(),
            StableId::new("capture-test").node_id().0
        ));
        let mut driver = Driver::new(
            iced::application::<State, Message, iced::Theme, iced::Renderer>(boot, update, view),
            Config::new("capture")
                .viewport(160.0, 120.0)
                .scale_factor(2.0)
                .theme(ThemeMode::Dark)
                .locale("ko-KR")
                .platform(Platform::Linux)
                .reduced_motion(true)
                .artifact_dir(artifact_dir.clone()),
        );

        let before = (
            driver.state().count,
            driver.state().input.clone(),
            driver.state().redraws,
        );
        let capture = driver.capture("primitive_matrix", HERE);
        assert_eq!(
            (
                driver.state().count,
                driver.state().input.clone(),
                driver.state().redraws,
            ),
            before,
            "capture must not update application state"
        );
        assert_eq!(
            driver.state().redraws,
            0,
            "capture must not mutate app state"
        );
        assert_eq!((capture.width, capture.height), (320, 240));
        assert_eq!(
            capture.rgba.len(),
            capture.width as usize * capture.height as usize * 4,
            "capture must contain one RGBA8 texel per physical pixel"
        );
        assert!(capture.png_path.is_file());
        assert!(capture.metadata_path.is_file());
        let metadata: serde_json::Value = serde_json::from_slice(
            &std::fs::read(&capture.metadata_path).expect("capture manifest"),
        )
        .expect("valid capture manifest");
        assert_eq!(metadata["schema_version"], 2);
        assert_eq!(metadata["scale_factor"], 2.0);
        assert_eq!(metadata["png"], "primitive_matrix.png");
        assert_eq!(metadata["clock"]["supports_virtual_redraw_advance"], true);
        assert!(metadata["clock"].get("redraw_time_is_virtual").is_none());
        assert_eq!(metadata["configured_theme"], "dark");
        assert_eq!(metadata["resolved_theme"]["mode"], "dark");
        assert_eq!(metadata["resolved_theme"]["name"], "Dark");
        assert_eq!(metadata["system_theme"], "none");
        assert_eq!(metadata["locale"], "ko-KR");
        assert!(
            metadata["targets"]
                .as_array()
                .is_some_and(|targets| !targets.is_empty())
        );
        assert!(
            metadata["targets"].as_array().is_some_and(|targets| {
                targets.iter().all(|target| {
                    target["id"]
                        .as_str()
                        .is_some_and(|id| !internal_auto_id(id))
                })
            }),
            "capture targets must contain only stable, addressable IDs"
        );
        assert!(metadata["targets"].as_array().is_some_and(|targets| {
            targets.iter().any(|target| {
                target["source"]["path"] == HERE.path
                    && target["source"]["line"] == HERE.line
                    && target["source"]["column"] == HERE.column
            })
        }));
        assert!(
            metadata["targets"].as_array().is_some_and(|targets| {
                targets.iter().any(|target| {
                    target["paint"]["texts"].as_array().is_some_and(|texts| {
                        texts.iter().any(|text| {
                            text["font"]["family"]["name"].is_string()
                                && text["font"]["weight"].is_string()
                                && text["font"]["stretch"].is_string()
                                && text["font"]["style"].is_string()
                        })
                    })
                })
            }),
            "capture text fonts must use the structured conformance schema"
        );
        let increment = metadata["targets"]
            .as_array()
            .and_then(|targets| {
                targets
                    .iter()
                    .find(|target| target["id"] == "App/root/increment")
            })
            .expect("capture includes the increment button");
        let active_background = &increment["paint"]["surfaces"][0]["background"]["color"];
        assert_eq!(active_background["a"], 1.0);
        assert!(
            (active_background["r"].as_f64().expect("red channel") - 0.2).abs() < 1.0e-6,
            "capture must update native widget status before drawing: {active_background}"
        );

        std::fs::remove_dir_all(&artifact_dir).expect("remove test capture directory");
    }

    #[test]
    fn manifest_source_paths_are_stable_relative_to_the_inspection_root() {
        assert_eq!(
            normalized_source_path(
                Path::new("workspace/examples/app/src/ui/app.ice"),
                Some(Path::new("workspace")),
            ),
            "examples/app/src/ui/app.ice"
        );
    }

    #[test]
    fn theme_override_controls_the_rendered_theme_instead_of_the_program_callback() {
        let program = iced::application::<State, Message, iced::Theme, iced::Renderer>(
            || {
                (
                    State::default(),
                    iced::system::theme().map(Message::ObservedSystemTheme),
                )
            },
            update,
            theme_probe_view,
        )
        .theme(|_state: &State| iced::Theme::Light);
        let mut driver = Driver::new(
            program,
            Config::new("theme_override")
                .viewport(120.0, 60.0)
                .theme(ThemeMode::Dark)
                .system_theme(ThemeMode::Light),
        );

        assert_eq!(driver.system_theme, ThemeMode::Light);
        assert_eq!(
            driver.state().observed_system_theme,
            Some(theme::Mode::Light)
        );
        assert!(matches!(driver.theme(), iced::Theme::Dark));
        assert_eq!(
            driver.target("Theme/root", HERE).background(),
            Background::Color(Color::from_rgb8(1, 2, 3))
        );
        driver.perform_action(Action::SystemTheme(ThemeMode::None), HERE);
        driver.dispatch(Message::ReadSystemTheme, HERE);
        assert_eq!(driver.system_theme, ThemeMode::None);
        assert_eq!(
            driver.state().observed_system_theme,
            Some(theme::Mode::None)
        );
        assert!(matches!(driver.theme(), iced::Theme::Dark));
    }

    #[test]
    fn screenshots_reject_unrenderable_physical_sizes_before_renderer_allocation() {
        let mut configured = Driver::new(
            iced::application::<State, Message, iced::Theme, iced::Renderer>(boot, update, view),
            Config::new("configured_scale_overflow")
                .viewport(320.0, 240.0)
                .scale_factor(f32::MAX),
        );
        let configured_failure = panic_message(|| _ = configured.screenshot_at(Some(HERE)));
        assert!(
            configured_failure
                .contains("expected: rounded physical width and height in 1..=u32::MAX"),
            "{configured_failure}"
        );
        assert!(configured_failure.contains("test.ice:1:1"));

        let mut rescaled = Driver::new(
            iced::application::<State, Message, iced::Theme, iced::Renderer>(boot, update, view),
            Config::new("rescaled_overflow").viewport(320.0, 240.0),
        );
        rescaled.perform_action(Action::Rescale(f32::MAX), HERE);
        let rescaled_failure = panic_message(|| _ = rescaled.screenshot_at(Some(HERE)));
        assert!(
            rescaled_failure
                .contains("expected: rounded physical width and height in 1..=u32::MAX"),
            "{rescaled_failure}"
        );

        let mut rounded_to_zero = Driver::new(
            iced::application::<State, Message, iced::Theme, iced::Renderer>(boot, update, view),
            Config::new("rounded_to_zero").viewport(0.25, 0.25),
        );
        let zero_failure = panic_message(|| _ = rounded_to_zero.screenshot_at(Some(HERE)));
        assert!(
            zero_failure.contains("rounds to (0.0, 0.0)"),
            "{zero_failure}"
        );

        let mut over_budget = Driver::new(
            iced::application::<State, Message, iced::Theme, iced::Renderer>(boot, update, view),
            Config::new("screenshot_budget").viewport(4097.0, 4097.0),
        );
        let budget_failure = panic_message(|| _ = over_budget.screenshot_at(Some(HERE)));
        assert!(
            budget_failure.contains("expected: at most 16777216 physical pixels"),
            "{budget_failure}"
        );
    }

    #[test]
    fn validates_ime_selection_ranges_and_state_order() {
        let mut driver = Driver::new(
            iced::application::<State, Message, iced::Theme, iced::Renderer>(boot, update, view),
            Config::new("ime_validation").viewport(320.0, 240.0),
        );

        let closed = panic_message(|| {
            driver.composition(
                CompositionPhase::Update {
                    text: "한글".to_owned(),
                    selection: Some(0..3),
                },
                HERE,
            );
        });
        for expected in [
            "test.ice:1:1",
            "test `ime_validation` IME update failed",
            "statement: test statement",
            "expected: an open composition",
        ] {
            assert!(closed.contains(expected), "missing {expected:?}: {closed}");
        }

        driver.composition(CompositionPhase::Start, HERE);
        for selection in [0..7, 1..3, Range { start: 3, end: 0 }] {
            let invalid = panic_message(|| {
                driver.composition(
                    CompositionPhase::Update {
                        text: "한글".to_owned(),
                        selection: Some(selection.clone()),
                    },
                    HERE,
                );
            });
            for expected in [
                "test.ice:1:1",
                "test `ime_validation` IME update failed",
                "statement: test statement",
                "expected: an ordered UTF-8 byte range within the composition text at character boundaries",
            ] {
                assert!(
                    invalid.contains(expected),
                    "missing {expected:?}: {invalid}"
                );
            }
            assert!(
                driver.ime_open,
                "invalid preedit must not close the composition"
            );
        }

        driver.composition(
            CompositionPhase::Update {
                text: "한글".to_owned(),
                selection: Some(0..3),
            },
            HERE,
        );
        driver.composition(CompositionPhase::Cancel, HERE);
        assert!(!driver.ime_open);
    }

    #[test]
    fn rejects_key_release_only_metadata_at_the_action_boundary() {
        let mut driver = Driver::new(
            iced::application::<State, Message, iced::Theme, iced::Renderer>(boot, update, view),
            Config::new("key_up_validation").viewport(320.0, 240.0),
        );

        for metadata in [
            KeyMetadata {
                text: Some("x".to_owned()),
                ..KeyMetadata::default()
            },
            KeyMetadata {
                repeat: true,
                ..KeyMetadata::default()
            },
        ] {
            let invalid = panic_message(|| {
                driver.perform_action(
                    Action::KeyUp {
                        key: Key::character("x"),
                        metadata: metadata.clone(),
                    },
                    HERE,
                );
            });
            for expected in [
                "test.ice:1:1",
                "test `key_up_validation` key up failed",
                "statement: test statement",
                "expected: release metadata without produced text or repeat",
            ] {
                assert!(
                    invalid.contains(expected),
                    "missing {expected:?}: {invalid}"
                );
            }
        }
        assert_eq!(driver.state().count, 0);
    }

    #[test]
    fn keyboard_actions_preserve_modifiers_and_validate_held_state() {
        let program =
            iced::application::<State, Message, iced::Theme, iced::Renderer>(boot, update, view)
                .subscription(modified_key_subscription);
        let mut driver = Driver::new(
            program,
            Config::new("keyboard_state").viewport(320.0, 240.0),
        );
        let held_modifiers = Modifiers::new(true, true, false, false);

        driver.perform_action(Action::Modifiers(held_modifiers), HERE);
        driver.perform_action(Action::Type("x".to_owned()), HERE);
        assert_eq!(driver.state().count, 10);
        assert_eq!(driver.modifiers, held_modifiers);
        assert!(driver.pressed_keys.is_empty());

        driver.perform_action(
            Action::Chord {
                modifiers: held_modifiers,
                key: Key::character("p"),
            },
            HERE,
        );
        assert_eq!(driver.state().count, 20);
        assert_eq!(driver.modifiers, held_modifiers);
        driver.perform_action(
            Action::Repeat {
                key: Key::named(keyboard::key::Named::Escape),
                count: 3,
            },
            HERE,
        );
        assert_eq!(driver.state().count, 50);
        assert!(driver.pressed_keys.is_empty());

        let key = Key::character("y");
        driver.perform_action(
            Action::KeyDown {
                key: key.clone(),
                metadata: KeyMetadata::default(),
            },
            HERE,
        );
        assert_eq!(driver.state().count, 60);
        let duplicate = panic_message(|| {
            driver.perform_action(
                Action::KeyDown {
                    key: key.clone(),
                    metadata: KeyMetadata::default(),
                },
                HERE,
            );
        });
        assert!(
            duplicate.contains("expected: a key that is not already pressed"),
            "{duplicate}"
        );
        assert!(driver.pressed_keys.values().any(|held| held.key == key));
        driver.perform_action(
            Action::KeyDown {
                key: key.clone(),
                metadata: KeyMetadata {
                    repeat: true,
                    ..KeyMetadata::default()
                },
            },
            HERE,
        );
        assert_eq!(driver.state().count, 70);
        driver.perform_action(
            Action::KeyUp {
                key: key.clone(),
                metadata: KeyMetadata::default(),
            },
            HERE,
        );
        assert!(driver.pressed_keys.is_empty());

        let repeat_without_press = panic_message(|| {
            driver.perform_action(
                Action::KeyDown {
                    key: key.clone(),
                    metadata: KeyMetadata {
                        repeat: true,
                        ..KeyMetadata::default()
                    },
                },
                HERE,
            );
        });
        assert!(
            repeat_without_press.contains("expected: a key that is already pressed"),
            "{repeat_without_press}"
        );
        let release_without_press = panic_message(|| {
            driver.perform_action(
                Action::KeyUp {
                    key,
                    metadata: KeyMetadata::default(),
                },
                HERE,
            );
        });
        assert!(
            release_without_press.contains("expected: a key that is currently pressed"),
            "{release_without_press}"
        );
    }

    #[test]
    fn exact_keyboard_actions_track_physical_or_logical_location_identity() {
        let mut driver = Driver::new(
            iced::application::<State, Message, iced::Theme, iced::Renderer>(boot, update, view),
            Config::new("exact_keyboard_identity").viewport(320.0, 240.0),
        );
        let shift = Key::named(keyboard::key::Named::Shift);
        let left = KeyMetadata {
            location: KeyLocation::Left,
            ..KeyMetadata::default()
        };
        let right = KeyMetadata {
            location: KeyLocation::Right,
            ..KeyMetadata::default()
        };
        driver.key_down_with(shift.clone(), left.clone(), HERE);
        driver.key_down_with(shift.clone(), right.clone(), HERE);
        assert_eq!(driver.pressed_keys.len(), 2);

        for metadata in [
            KeyMetadata::default(),
            KeyMetadata {
                location: KeyLocation::Numpad,
                repeat: true,
                ..KeyMetadata::default()
            },
        ] {
            let mismatch = panic_message(|| {
                if metadata.repeat {
                    driver.key_down_with(shift.clone(), metadata.clone(), HERE);
                } else {
                    driver.key_up_with(shift.clone(), metadata.clone(), HERE);
                }
            });
            assert!(mismatch.contains("expected: a key that is"), "{mismatch}");
            assert_eq!(driver.pressed_keys.len(), 2);
        }
        driver.key_up_with(shift.clone(), left, HERE);
        driver.key_up_with(shift, right, HERE);
        assert!(driver.pressed_keys.is_empty());

        let physical_a = KeyMetadata {
            physical_key: Some(keyboard::key::Physical::Code(keyboard::key::Code::KeyA)),
            location: KeyLocation::Left,
            ..KeyMetadata::default()
        };
        driver.key_down_with(Key::character("a"), physical_a.clone(), HERE);
        let wrong_repeat_location = panic_message(|| {
            driver.key_down_with(
                Key::character("A"),
                KeyMetadata {
                    physical_key: physical_a.physical_key,
                    location: KeyLocation::Right,
                    repeat: true,
                    ..KeyMetadata::default()
                },
                HERE,
            );
        });
        assert!(
            wrong_repeat_location.contains("expected: the same key location as the initial press"),
            "{wrong_repeat_location}"
        );
        driver.key_down_with(
            Key::character("A"),
            KeyMetadata {
                physical_key: physical_a.physical_key,
                location: KeyLocation::Left,
                repeat: true,
                ..KeyMetadata::default()
            },
            HERE,
        );
        let wrong_physical = panic_message(|| {
            driver.key_up_with(
                Key::character("a"),
                KeyMetadata {
                    physical_key: Some(keyboard::key::Physical::Code(keyboard::key::Code::KeyB)),
                    ..KeyMetadata::default()
                },
                HERE,
            );
        });
        assert!(
            wrong_physical.contains("expected: a key that is currently pressed"),
            "{wrong_physical}"
        );
        let wrong_release_location = panic_message(|| {
            driver.key_up_with(
                Key::character("q"),
                KeyMetadata {
                    physical_key: physical_a.physical_key,
                    location: KeyLocation::Numpad,
                    ..KeyMetadata::default()
                },
                HERE,
            );
        });
        assert!(
            wrong_release_location.contains("expected: the same key location as the initial press"),
            "{wrong_release_location}"
        );
        driver.key_up_with(
            Key::character("q"),
            KeyMetadata {
                physical_key: physical_a.physical_key,
                location: KeyLocation::Left,
                ..KeyMetadata::default()
            },
            HERE,
        );
        assert!(driver.pressed_keys.is_empty());

        let native = |code| KeyMetadata {
            physical_key: Some(keyboard::key::Physical::Unidentified(
                keyboard::key::NativeCode::Xkb(code),
            )),
            ..KeyMetadata::default()
        };
        driver.key_down_with(Key::Unidentified, native(41), HERE);
        driver.key_down_with(Key::Unidentified, native(42), HERE);
        assert_eq!(driver.pressed_keys.len(), 2);
        driver.key_up_with(Key::Unidentified, native(41), HERE);
        driver.key_up_with(Key::Unidentified, native(42), HERE);
        assert!(driver.pressed_keys.is_empty());
    }

    #[test]
    fn keyboard_actions_reject_empty_primary_and_modified_character_keys() {
        let program =
            iced::application::<State, Message, iced::Theme, iced::Renderer>(boot, update, view);
        let mut driver = Driver::new(
            program,
            Config::new("empty_character_key").viewport(320.0, 240.0),
        );
        driver.perform_action(
            Action::Modifiers(Modifiers::new(false, true, false, false)),
            HERE,
        );

        for metadata in [
            KeyMetadata::default(),
            KeyMetadata {
                text: Some("x".to_owned()),
                ..KeyMetadata::default()
            },
        ] {
            let invalid = panic_message(|| {
                driver.perform_action(
                    Action::KeyDown {
                        key: Key::character(""),
                        metadata: metadata.clone(),
                    },
                    HERE,
                );
            });
            assert!(
                invalid.contains("expected: a non-empty character value for the logical key"),
                "{invalid}"
            );
        }

        let key = Key::character("x");
        let invalid_modified_down = panic_message(|| {
            driver.perform_action(
                Action::KeyDown {
                    key: key.clone(),
                    metadata: KeyMetadata {
                        modified_key: Some(Key::character("")),
                        ..KeyMetadata::default()
                    },
                },
                HERE,
            );
        });
        assert!(
            invalid_modified_down
                .contains("expected: a non-empty character value for the modified key"),
            "{invalid_modified_down}"
        );

        driver.perform_action(
            Action::KeyDown {
                key: key.clone(),
                metadata: KeyMetadata::default(),
            },
            HERE,
        );
        let invalid_modified_up = panic_message(|| {
            driver.perform_action(
                Action::KeyUp {
                    key: key.clone(),
                    metadata: KeyMetadata {
                        modified_key: Some(Key::character("")),
                        ..KeyMetadata::default()
                    },
                },
                HERE,
            );
        });
        assert!(
            invalid_modified_up
                .contains("expected: a non-empty character value for the modified key"),
            "{invalid_modified_up}"
        );
        assert!(driver.pressed_keys.values().any(|held| held.key == key));
        driver.perform_action(
            Action::KeyUp {
                key,
                metadata: KeyMetadata::default(),
            },
            HERE,
        );
        assert!(driver.pressed_keys.is_empty());
    }

    #[test]
    fn role_names_keep_acronym_boundaries_stable() {
        assert_eq!(
            camel_to_kebab("PDFActionableHighlight"),
            "pdf-actionable-highlight"
        );
        assert_eq!(camel_to_kebab("ImeCandidate"), "ime-candidate");
        assert_eq!(camel_to_kebab("ListItem"), "list-item");
        assert!(internal_auto_id("Conformance/@layout:942"));
        assert!(internal_auto_id("Conformance/@for:42(0)"));
        assert!(!internal_auto_id("Conformance/@for:42(0)/button"));
    }

    #[test]
    fn pixel_alignment_uses_physical_edges_at_the_configured_scale() {
        let half_pixel = Rectangle::new(Point::new(0.5, 0.5), Size::new(1.0, 1.0));
        assert!(!rectangle_pixel_aligned(half_pixel, 1.0));
        assert!(rectangle_pixel_aligned(half_pixel, 2.0));
        assert!(!rectangle_pixel_aligned(
            Rectangle::new(Point::new(f32::NAN, 0.0), Size::new(1.0, 1.0)),
            1.0,
        ));
        assert!(!rectangle_pixel_aligned(half_pixel, f32::INFINITY));
    }

    #[test]
    fn paint_inspection_delivers_one_redraw_event() {
        PROBE_REDRAWS.with(|redraws| redraws.set(0));
        let mut driver = Driver::new(
            iced::application::<State, Message, iced::Theme, iced::Renderer>(boot, update, view),
            Config::new("single_redraw").viewport(320.0, 240.0),
        );

        let _ = driver.target("App/root", HERE);
        PROBE_REDRAWS.with(|redraws| assert_eq!(redraws.get(), 1));
    }

    #[test]
    fn semantic_target_merging_never_exposes_password_text() {
        let mut driver = Driver::new(
            iced::application::<State, Message, iced::Theme, iced::Renderer>(
                || {
                    (
                        State {
                            input: "secret".to_owned(),
                            ..State::default()
                        },
                        Task::none(),
                    )
                },
                update,
                password_view,
            ),
            Config::new("password").viewport(320.0, 240.0),
        );

        let target = driver.target("Password/field", HERE);
        let failure = panic_message(|| {
            let _ = target.value();
        });
        assert!(failure.contains("rendered text content"), "{failure}");
        assert!(!failure.contains("secret"), "{failure}");
    }

    #[test]
    fn settles_boot_presets_and_event_subscriptions() {
        let program = iced::application::<State, Message, iced::Theme, iced::Renderer>(
            || (State::default(), Task::done(Message::Incremented)),
            update,
            view,
        )
        .subscription(subscription)
        .presets([iced::Preset::new("seeded", || {
            (
                State {
                    count: 4,
                    input: String::new(),
                    redraws: 0,
                    events: Vec::new(),
                    observed_system_theme: None,
                },
                Task::done(Message::Incremented),
            )
        })]);
        let mut driver = Driver::new(
            program,
            Config::new("settling")
                .preset("seeded")
                .viewport(320.0, 240.0),
        );

        assert_eq!(driver.state().count, 5);
        driver.key(Key::named(keyboard::key::Named::Escape), HERE);
        assert_eq!(driver.state().count, 15);
    }

    /// An `every` belongs to the test's logical clock, not to the machine's.
    /// Wall time passing without a scripted `advance`/`wait` must deliver no
    /// tick — that load-dependent tick mid-assertion is exactly the flake —
    /// and a scripted step must deliver exactly the periods it crosses.
    #[test]
    fn every_ticks_off_the_logical_clock_not_the_wall_clock() {
        let mut driver = Driver::new(
            iced::application::<State, Message, iced::Theme, iced::Renderer>(boot, update, view)
                .subscription(every_subscription),
            Config::new("every_logical_clock").viewport(320.0, 240.0),
        );
        assert_eq!(driver.state().count, 0);

        // Real time crosses the 250ms period twice over; reading the screen
        // afterwards still delivers no tick, because neither the sleep nor the
        // redraw moves the logical clock.
        std::thread::sleep(Duration::from_millis(600));
        driver.redraw(HERE);
        assert_eq!(driver.state().count, 0);

        driver.advance(Duration::from_millis(250), HERE);
        assert_eq!(driver.state().count, 1);

        // One step across four periods catches up with all four ticks.
        driver.advance(Duration::from_secs(1), HERE);
        assert_eq!(driver.state().count, 5);

        // `wait` sleeps real time but steps the logical clock by exactly the
        // requested duration: 1250ms → 1550ms crosses the 250ms grid once,
        // however long the sleep really took.
        driver.wait(Duration::from_millis(300), HERE);
        assert_eq!(driver.state().count, 6);
    }

    #[test]
    fn inspects_structured_tiny_skia_paint() {
        let mut driver = Driver::new(
            iced::application::<State, Message, iced::Theme, iced::Renderer>(boot, update, view),
            Config::new("paint").viewport(320.0, 240.0),
        );
        let root = driver.target("App/root", HERE);

        assert_eq!(
            root.background(),
            Background::Color(Color::from_rgb8(17, 17, 17))
        );
        assert_eq!(root.border().color, Color::from_rgb8(51, 102, 255));
        assert_eq!(root.border().width, 1.0);

        let increment = driver.target("App/root/increment", HERE);
        assert_eq!(
            increment.background(),
            Background::Color(Color::from_rgb8(51, 102, 255))
        );
        assert_eq!(increment.border().radius, 6.0.into());

        let count = driver.target("App/root/count", HERE);
        assert!(count.text_size() > 0.0);
        assert!(count.text_color().a > 0.0);
        assert!(matches!(
            count.line_height(),
            iced::widget::text::LineHeight::Absolute(value) if value.0 > 0.0
        ));
        let _ = count.font();
    }

    #[test]
    fn rejects_invalid_viewports_and_resizes() {
        let invalid = panic_message(|| {
            Driver::new(
                iced::application::<State, Message, iced::Theme, iced::Renderer>(
                    boot, update, view,
                ),
                Config::new("invalid").source(HERE).viewport(0.0, 240.0),
            );
        });
        assert!(invalid.contains("test.ice:1:1"), "{invalid}");
        assert!(invalid.contains("test `invalid`"), "{invalid}");
        assert!(invalid.contains("statement: test statement"), "{invalid}");
        assert!(invalid.contains("expected:"), "{invalid}");
        assert!(invalid.contains("actual:"), "{invalid}");

        let locale = panic_message(|| {
            Driver::new(
                iced::application::<State, Message, iced::Theme, iced::Renderer>(
                    boot, update, view,
                ),
                Config::new("invalid_locale").source(HERE).locale(""),
            );
        });
        assert!(locale.contains("expected: a non-empty locale"), "{locale}");
        assert!(locale.contains("test.ice:1:1"), "{locale}");

        let artifact = panic_message(|| {
            Driver::new(
                iced::application::<State, Message, iced::Theme, iced::Renderer>(
                    boot, update, view,
                ),
                Config::new("invalid_artifact")
                    .source(HERE)
                    .artifact_dir(PathBuf::new()),
            );
        });
        assert!(
            artifact.contains("expected: a non-empty artifact directory"),
            "{artifact}"
        );

        let mut driver = Driver::new(
            iced::application::<State, Message, iced::Theme, iced::Renderer>(boot, update, view),
            Config::new("resize").viewport(320.0, 240.0),
        );
        let message = panic_message(|| driver.resize(f32::NAN, 100.0, HERE));
        assert!(message.contains("test.ice:1:1"));
        assert!(message.contains("test statement"));
    }

    #[test]
    fn reports_source_mapped_failure_context_and_logical_nearby_ids() {
        let mut driver = Driver::new(
            iced::application::<State, Message, iced::Theme, iced::Renderer>(boot, update, view),
            Config::new("failure_contract").viewport(320.0, 240.0),
        );

        let missing = panic_message(|| {
            step("failure_contract", HERE, || {
                driver.check_exists("App/missing", true, HERE);
            });
        });
        for expected in [
            "test.ice:1:1",
            "test `failure_contract`",
            "statement: test statement",
            "selector: App/missing",
            "expected: present",
            "actual: missing",
            "bounds: unavailable",
            "App/root/count",
        ] {
            assert!(
                missing.contains(expected),
                "missing {expected:?}: {missing}"
            );
        }
        assert_eq!(missing.matches("test.ice:1:1").count(), 1, "{missing}");
        assert_eq!(
            missing.matches("statement: test statement").count(),
            1,
            "{missing}"
        );
        assert!(!missing.contains("Rust panic:"), "{missing}");
        assert!(!missing.contains("__ice_accessibility/"), "{missing}");

        let target = driver.target("App/root", HERE);
        let unavailable = panic_message(|| _ = target.value());
        for expected in [
            "test.ice:1:1",
            "test `failure_contract`",
            "statement: test statement",
            "selector: App/root",
            "expected: rendered text content",
            "actual: unavailable",
            "bounds: Rectangle",
        ] {
            assert!(
                unavailable.contains(expected),
                "missing {expected:?}: {unavailable}"
            );
        }

        let text = panic_message(|| driver.check_text("absent", Some("App/root"), false, HERE));
        for expected in [
            "test `failure_contract`",
            "selector: visible text \"absent\" within App/root",
            "expected: present",
            "actual: missing",
            "bounds: Rectangle",
        ] {
            assert!(text.contains(expected), "missing {expected:?}: {text}");
        }
    }

    #[cfg(debug_assertions)]
    #[test]
    fn reports_custom_renderer_paint_and_text_as_unavailable() {
        let mut driver = Driver::new(
            iced::application::<State, Message, iced::Theme, ()>(boot, update, null_view),
            Config::new("custom_renderer").viewport(320.0, 240.0),
        );

        let target = driver.target("Null/root", HERE);
        let paint = panic_message(|| _ = target.background());
        assert!(paint.contains("test `custom_renderer`"), "{paint}");
        assert!(paint.contains("selector: Null/root"), "{paint}");
        assert!(
            paint.contains("structured tiny-skia surface paint"),
            "{paint}"
        );
        assert!(paint.contains("custom renderer"), "{paint}");
        assert!(paint.contains("bounds: Rectangle"), "{paint}");
        for unavailable in [
            panic_message(|| _ = target.surface_count()),
            panic_message(|| _ = target.text_count()),
            panic_message(|| _ = target.image_count()),
        ] {
            assert!(
                unavailable.contains("expected: structured tiny-skia paint"),
                "{unavailable}"
            );
            assert!(unavailable.contains("custom renderer"), "{unavailable}");
        }
        assert!(target.pixel_aligned());

        let screenshot = panic_message(|| _ = driver.screenshot_at(Some(HERE)));
        assert!(
            screenshot.contains("expected: 307200 RGBA8 bytes"),
            "{screenshot}"
        );
        assert!(
            screenshot.contains("actual: 0 bytes returned by the headless renderer"),
            "{screenshot}"
        );

        let text = panic_message(|| driver.check_text("anything", None, false, HERE));
        assert!(text.contains("test `custom_renderer`"), "{text}");
        assert!(text.contains("visible text \"anything\""), "{text}");
        assert!(text.contains("complete rendered-text search"), "{text}");
        assert!(text.contains("custom renderer"), "{text}");
    }

    #[test]
    fn rejects_ambiguous_dynamic_ids_without_guessing() {
        let mut driver = Driver::new(
            iced::application::<State, Message, iced::Theme, iced::Renderer>(
                boot,
                update,
                duplicate_view,
            ),
            Config::new("duplicate_ids").viewport(320.0, 240.0),
        );

        let message = panic_message(|| _ = driver.target("Duplicate/item", HERE));
        for expected in [
            "test.ice:1:1",
            "test `duplicate_ids`",
            "statement: test statement",
            "selector: Duplicate/item",
            "expected: exactly 1 candidate",
            "actual: 2 candidates",
            "candidate bounds: [1: Rectangle",
            "2: Rectangle",
            "known runtime ids: Duplicate/item",
        ] {
            assert!(
                message.contains(expected),
                "missing {expected:?}: {message}"
            );
        }
    }

    #[test]
    fn propagates_real_task_panics_instead_of_timing_out() {
        let mut driver = Driver::new(
            iced::application::<State, Message, iced::Theme, iced::Renderer>(boot, update, view),
            Config::new("task_panic")
                .source(HERE)
                .timeout(Duration::from_millis(50))
                .viewport(320.0, 240.0),
        );

        let message = panic_message(|| driver.dispatch(Message::PanicTask, HERE));
        for expected in [
            "test.ice:1:1",
            "test `task_panic`",
            "statement: test statement",
            "real task panic",
        ] {
            assert!(
                message.contains(expected),
                "missing {expected:?}: {message}"
            );
        }
        assert!(!message.contains("quiescence"), "{message}");
    }

    #[test]
    fn adds_source_context_to_boot_update_and_sync_panics() {
        let boot_failure = panic_message(|| {
            Driver::new(
                iced::application::<State, Message, iced::Theme, iced::Renderer>(
                    || -> (State, Task<Message>) { panic!("real boot panic") },
                    update,
                    view,
                ),
                Config::new("boot_panic")
                    .source(HERE)
                    .viewport(320.0, 240.0),
            );
        });
        for expected in [
            "test.ice:1:1",
            "test `boot_panic`",
            "statement: test statement",
            "real boot panic",
        ] {
            assert!(
                boot_failure.contains(expected),
                "missing {expected:?}: {boot_failure}"
            );
        }

        let mut driver = Driver::new(
            iced::application::<State, Message, iced::Theme, iced::Renderer>(boot, update, view),
            Config::new("update_panic").viewport(320.0, 240.0),
        );
        let update_failure = panic_message(|| driver.dispatch(Message::PanicUpdate, HERE));
        for expected in [
            "test.ice:1:1",
            "test `update_panic`",
            "statement: test statement",
            "real update panic",
        ] {
            assert!(
                update_failure.contains(expected),
                "missing {expected:?}: {update_failure}"
            );
        }

        let sync_failure = panic_message(|| {
            step("sync_panic", HERE, || panic!("real sync panic"));
        });
        for expected in [
            "test.ice:1:1",
            "test `sync_panic`",
            "statement: test statement",
            "real sync panic",
        ] {
            assert!(
                sync_failure.contains(expected),
                "missing {expected:?}: {sync_failure}"
            );
        }

        let opaque_failure = panic_message(|| {
            step("opaque_panic", HERE, || std::panic::panic_any(7_u8));
        });
        assert!(opaque_failure.contains("test `opaque_panic`"));
        assert!(opaque_failure.contains("Rust panic: non-string payload"));
    }

    #[test]
    fn panic_hook_displays_source_context_for_sync_update_and_task_panics() {
        const CHILD: &str = "UI_LANG_RUNTIME_PANIC_CONTEXT_CHILD";
        const TEST: &str =
            "testing::tests::panic_hook_displays_source_context_for_sync_update_and_task_panics";

        if let Ok(kind) = std::env::var(CHILD) {
            std::panic::set_hook(Box::new(|info| {
                let message = info
                    .payload()
                    .downcast_ref::<String>()
                    .map(String::as_str)
                    .or_else(|| info.payload().downcast_ref::<&'static str>().copied())
                    .unwrap_or("non-string payload");
                eprintln!("ICE_CONTEXT_HOOK: {message}");
            }));

            match kind.as_str() {
                "sync" => step("sync_hook", HERE, || panic!("raw sync panic")),
                "update" => {
                    let mut driver = Driver::new(
                        iced::application::<State, Message, iced::Theme, iced::Renderer>(
                            boot, update, view,
                        ),
                        Config::new("update_hook").viewport(320.0, 240.0),
                    );
                    driver.dispatch(Message::PanicUpdate, HERE);
                }
                "task" => {
                    let mut driver = Driver::new(
                        iced::application::<State, Message, iced::Theme, iced::Renderer>(
                            boot, update, view,
                        ),
                        Config::new("task_hook")
                            .timeout(Duration::from_millis(50))
                            .viewport(320.0, 240.0),
                    );
                    driver.dispatch(Message::PanicTask, HERE);
                }
                _ => panic!("unknown panic-context child `{kind}`"),
            }
            return;
        }

        let executable = std::env::current_exe().expect("current test executable");
        for (kind, test_name, raw) in [
            ("sync", "sync_hook", "raw sync panic"),
            ("update", "update_hook", "real update panic"),
            ("task", "task_hook", "real task panic"),
        ] {
            let output = std::process::Command::new(&executable)
                .args(["--exact", TEST, "--nocapture"])
                .env(CHILD, kind)
                .output()
                .expect("panic-context child process");
            assert!(!output.status.success(), "{kind} child unexpectedly passed");

            let displayed = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            let expected = format!(
                "ICE_CONTEXT_HOOK: test.ice:1:1: test `{test_name}`\nstatement: test statement\nRust panic: {raw}"
            );
            assert!(
                displayed.contains(&expected),
                "missing contextual hook output {expected:?}:\n{displayed}"
            );
        }
    }

    #[test]
    fn reports_hanging_tasks_at_the_triggering_statement() {
        let mut driver = Driver::new(
            iced::application::<State, Message, iced::Theme, iced::Renderer>(boot, update, view),
            Config::new("task_timeout")
                .source(HERE)
                .timeout(Duration::from_millis(10))
                .viewport(320.0, 240.0),
        );

        let message = panic_message(|| driver.dispatch(Message::HangTask, HERE));
        for expected in [
            "test.ice:1:1",
            "test `task_timeout`",
            "statement: test statement",
            "expected: quiescence within 10ms",
            "actual: 1 task stream(s) still pending",
        ] {
            assert!(
                message.contains(expected),
                "missing {expected:?}: {message}"
            );
        }
    }

    #[test]
    fn propagates_real_subscription_panics_instead_of_timing_out() {
        let message = panic_message(|| {
            Driver::new(
                iced::application::<State, Message, iced::Theme, iced::Renderer>(
                    boot, update, view,
                )
                .subscription(panicking_subscription),
                Config::new("subscription_panic")
                    .source(HERE)
                    .timeout(Duration::from_millis(50))
                    .viewport(320.0, 240.0),
            );
        });
        for expected in [
            "test.ice:1:1",
            "test `subscription_panic`",
            "statement: test statement",
            "real subscription panic",
        ] {
            assert!(
                message.contains(expected),
                "missing {expected:?}: {message}"
            );
        }
        assert!(!message.contains("quiescence"), "{message}");
    }

    /// A menu is one flat table across every submenu, so a text that names two
    /// rows names neither. Choosing the earlier one would run the wrong
    /// handler while the test passed, which is exactly the failure the
    /// row-to-handler table exists to stop — and grouping is what makes two
    /// rows share a word.
    #[test]
    fn a_text_carried_by_two_rows_names_every_one_of_them() {
        let tray = crate::tray::TraySnapshot {
            items: vec![
                "Positions".to_owned(),
                "BTC  Close".to_owned(),
                String::new(),
                "ETH  Close".to_owned(),
            ],
            ..crate::tray::TraySnapshot::default()
        };
        assert_eq!(
            tray_rows_containing(&tray, "Close"),
            vec![1, 3],
            "both rows carry the text, and a caller that must pick one has to be told so"
        );
        assert_eq!(
            tray_rows_containing(&tray, "BTC  Close"),
            vec![1],
            "the fragment that names one row still names one row"
        );
        assert!(
            tray_rows_containing(&tray, "Close").len() > 1,
            "an empty separator slot never matches, so row 2 is not among them"
        );
    }
}
