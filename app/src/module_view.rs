//! Module-owned views. A screen that ships as an `ice:view` component — an
//! Ice application compiled for the `tree` target (`crates/views`) — is
//! loaded FROM A FILE beside the binary (`make views` stages
//! `target/views/<module>_view.wasm`; `DUCKTAPE_VIEWS_DIR` overrides), ticked
//! inside a fuel and time budget, and drawn with the runtime's tree renderer
//! as one widget in the tab that used to hold the native screen.
//!
//! The boundary is the screen component's own contract. Its props go in as
//! JSON, one item per change, on the guest's `<module>.props` subscription;
//! its emits come out as intents the widget hands the app as
//! [`ModuleViewEvent`]s, so every write keeps going through the handler that
//! signs it today. The guest sees no key, no endpoint and no clock — a view
//! that holds none of them cannot leak one — and a view that traps shows why
//! in its place instead of taking the window with it.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer};
use iced::{Element, Event, Length, Rectangle, Size, Vector, widget, window};
use ui_lang_runtime::view_tree::{self, Inputs, Output, Pictures, Surfaces};
use ui_lang_wire as wire;
use wasmtime::component::{Component, Linker, TypedFunc};
use wasmtime::{Config, Engine, OptLevel, Store, StoreContextMut, StoreLimits, StoreLimitsBuilder};

/// What a module view asked the app to do: `kind` is the operation
/// (`vote`, `execute`), `detail` the guest's JSON for it.
#[derive(Clone, Debug, Hash, PartialEq, Default)]
pub struct ModuleViewEvent {
    pub kind: String,
    pub detail: String,
}

/// What one call into a view may spend: about a 60 Hz frame of
/// instructions, and a wall-clock deadline for the time an import takes
/// that fuel cannot see.
const FUEL_PER_TICK: u64 = 100_000_000;
const TICK_DEADLINE: Duration = Duration::from_millis(100);
const EPOCH_TICK: Duration = Duration::from_millis(10);
const MEMORY_LIMIT: usize = 64 << 20;
const MAX_MODULE_BYTES: u64 = 64 << 20;
/// A frame the view sends past this ends it: nothing a screen needs is
/// megabytes, and the host would decode all of it on the window thread.
const MAX_FRAME_BYTES: usize = 8 << 20;
const MAX_REQUESTS_PER_TICK: usize = 256;
const MAX_PAYLOAD_BYTES: usize = 1 << 20;
/// How often a tab polls for a component still loading on its thread.
const LOAD_POLL: Duration = Duration::from_millis(50);

// ---------- the Approvals seat ----------

/// The Approvals tab: the governance register as the app has it, drawn by
/// the `governance` view. Its intents come back as `vote` and `execute`,
/// each with an [`Intent`] in `detail`.
pub fn governance_view(
    dark: bool,
    connected: bool,
    admin: bool,
    answered: bool,
    voting: &str,
    rows: &[crate::backend::ProposalRow],
) -> Element<'static, ModuleViewEvent> {
    let props = serde_json::json!({
        "rows": rows,
        "voting": voting,
        "admin": admin,
        "connected": connected,
        "answered": answered,
        "dark": dark,
    });
    module_view(
        "governance",
        serde_json::to_vec(&props).expect("props encode"),
    )
}

/// A vote or a settle, as the governance view sends it.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
pub struct Intent {
    pub proposal_id: String,
    pub approve: bool,
}

pub fn gov_intent(event: &ModuleViewEvent) -> crate::GovIntent {
    match event.kind.as_str() {
        "execute" => crate::GovIntent::Execute,
        _ => crate::GovIntent::Vote,
    }
}

pub fn gov_event_proposal(event: &ModuleViewEvent) -> String {
    intent(event)
        .map(|intent| intent.proposal_id)
        .unwrap_or_default()
}

pub fn gov_event_approves(event: &ModuleViewEvent) -> bool {
    intent(event).is_some_and(|intent| intent.approve)
}

fn intent(event: &ModuleViewEvent) -> Option<Intent> {
    serde_json::from_str(&event.detail).ok()
}

/// The operations a view may ask of the app, by module. An intent outside
/// the list is refused at the door, never handed to a handler.
fn intents_of(module: &str) -> &'static [&'static str] {
    match module {
        "governance" => &["vote", "execute"],
        _ => &[],
    }
}

// ---------- mounting ----------

/// The widget for one module's view, with `props` as the app has them now.
/// The view is loaded once per process, on its own thread, and the tab
/// shows what stage it is at until then.
fn module_view(module: &'static str, props: Vec<u8>) -> Element<'static, ModuleViewEvent> {
    let mounted = mounted(module);
    let (content, rev) = {
        let mut locked = mounted.lock().expect("module view lock");
        locked.props = Some(props);
        match &mut locked.slot {
            Slot::Loading => return notice("Loading the view…"),
            Slot::Failed(reason) => return notice(reason),
            Slot::Ready(guest) => (guest.render(), guest.frame_rev),
        }
    };
    Element::new(ModuleView {
        mounted,
        rev,
        content,
    })
}

/// What the tab shows while the view is not there to show itself.
fn notice(text: &str) -> Element<'static, ModuleViewEvent> {
    widget::container(widget::text(text.to_owned()).size(13))
        .width(Length::Fill)
        .height(Length::Fill)
        .center(Length::Fill)
        .into()
}

/// One module's view for the life of the process: the instance once it is
/// there, and the props the app last handed it, which it takes on its next
/// redraw whether the instance was ready when they arrived or not.
struct Mounted {
    slot: Slot,
    props: Option<Vec<u8>>,
}

enum Slot {
    Loading,
    Ready(Box<Guest>),
    Failed(String),
}

type Registry = Mutex<HashMap<&'static str, Arc<Mutex<Mounted>>>>;

fn mounted(module: &'static str) -> Arc<Mutex<Mounted>> {
    static MOUNTED: OnceLock<Registry> = OnceLock::new();
    let mut registry = MOUNTED
        .get_or_init(Mutex::default)
        .lock()
        .expect("module views");
    registry
        .entry(module)
        .or_insert_with(|| {
            let mounted = Arc::new(Mutex::new(Mounted {
                slot: Slot::Loading,
                props: None,
            }));
            // A cold cranelift compile is a second or more; the window
            // thread shows "Loading" instead of freezing for it.
            let loading = mounted.clone();
            std::thread::spawn(move || {
                let slot = match Guest::load(module) {
                    Ok(guest) => Slot::Ready(Box::new(guest)),
                    Err(reason) => {
                        tracing::warn!(
                            target: "ducktape::app",
                            module,
                            reason = "module_view_unloadable",
                            error = %reason,
                            "module view not loaded"
                        );
                        Slot::Failed(reason)
                    }
                };
                loading.lock().expect("module view lock").slot = slot;
            });
            mounted
        })
        .clone()
}

/// Where the staged views are: `$DUCKTAPE_VIEWS_DIR`, else `views/` beside
/// the binary or beside its profile directory — the shape
/// `workspace_config::staged_modules_dir` gives the founding set.
fn views_dir() -> Result<PathBuf, String> {
    if let Some(dir) = std::env::var_os("DUCKTAPE_VIEWS_DIR") {
        return Ok(PathBuf::from(dir));
    }
    let exe = std::env::current_exe().map_err(|error| format!("current executable: {error}"))?;
    let exe_dir = exe.parent().ok_or("the executable has no directory")?;
    [Some(exe_dir), exe_dir.parent()]
        .into_iter()
        .flatten()
        .map(|dir| dir.join("views"))
        .find(|dir| dir.is_dir())
        .ok_or_else(|| {
            format!(
                "no views beside {} — `make views` stages them under target/views, or set $DUCKTAPE_VIEWS_DIR",
                exe.display()
            )
        })
}

// ---------- the guest ----------

/// What a view's store holds: its limits, and the message its panic hook
/// handed over before the trap that follows.
struct HostState {
    limits: StoreLimits,
    panic: Option<String>,
}

struct Guest {
    module: &'static str,
    store: Store<HostState>,
    tick: TypedFunc<(Vec<u8>,), (Vec<u8>,)>,
    /// The guest's events for its next tick.
    pending: Vec<wire::Event>,
    /// The last frame, its `root` kept across `unchanged` ticks and patched
    /// in place by a frame that carries patches instead of a tree.
    frame: wire::Frame,
    /// Bumped when `frame.root` changes: the widget rebuilds when it sees a
    /// number it has not rendered.
    frame_rev: u64,
    ticks: u64,
    /// The live text of every input in the tree — the host's, not the guest's.
    inputs: Inputs,
    /// Every picture the guest has sent, by hash: the bytes cross once.
    pictures: Pictures,
    surfaces: Surfaces,
    /// The guest's `<module>.props` subscription, once it asked, and the
    /// props it was last given on it.
    props_subscription: Option<u64>,
    props_sent: Option<Vec<u8>>,
    /// What the guest asked the app to do this redraw.
    intents: Vec<ModuleViewEvent>,
    /// The trap that ended the view, if one did. A faulted guest never ticks again.
    fault: Option<String>,
}

fn engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(|| {
        let mut config = Config::new();
        config.cranelift_opt_level(OptLevel::Speed);
        config.consume_fuel(true);
        config.epoch_interruption(true);
        let engine = Engine::new(&config).expect("wasmtime engine");
        // The clock every tick's deadline is measured against: one thread
        // for the process, never stopped.
        let ticking = engine.clone();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(EPOCH_TICK);
                ticking.increment_epoch();
            }
        });
        engine
    })
}

/// What one call into the view may spend: instructions, and time.
fn arm(store: &mut Store<HostState>) {
    let _ = store.set_fuel(FUEL_PER_TICK);
    let epochs = TICK_DEADLINE.as_nanos().div_ceil(EPOCH_TICK.as_nanos()) as u64;
    store.set_epoch_deadline(epochs);
}

impl Guest {
    fn load(module: &'static str) -> Result<Self, String> {
        let path = views_dir()?.join(format!("{module}_view.wasm"));
        Self::load_from(module, &path)
    }

    fn load_from(module: &'static str, path: &std::path::Path) -> Result<Self, String> {
        let shown = path.display().to_string();
        let metadata = std::fs::metadata(&path).map_err(|error| format!("{shown}: {error}"))?;
        if metadata.len() > MAX_MODULE_BYTES {
            return Err(format!(
                "{shown}: past the {MAX_MODULE_BYTES} byte module limit"
            ));
        }
        let bytes = std::fs::read(&path).map_err(|error| format!("{shown}: {error}"))?;
        let engine = engine();
        let component =
            Component::new(engine, &bytes).map_err(|error| format!("{shown}: {error}"))?;
        // Tables are allocated eagerly at their declared minimum, before any
        // fuel or memory limit is consulted; a component is several core
        // instances — the app, the stub adapters `cargo ice bundle` gave it,
        // the bindings' shims — and one memory.
        let limits = StoreLimitsBuilder::new()
            .memory_size(MEMORY_LIMIT)
            .memories(1)
            .instances(8)
            .tables(4)
            .table_elements(1 << 20)
            .trap_on_grow_failure(true)
            .build();
        let mut store = Store::new(
            engine,
            HostState {
                limits,
                panic: None,
            },
        );
        store.limiter(|state| &mut state.limits);
        store.epoch_deadline_trap();
        // The `ice:view` world's one import is the panic hook's; anything
        // else the component asks for traps if it is ever called.
        let mut linker = Linker::<HostState>::new(engine);
        linker
            .root()
            .func_wrap(
                "panicked",
                |mut store: StoreContextMut<'_, HostState>, (message,): (String,)| {
                    let line = message.lines().next().unwrap_or_default();
                    store.data_mut().panic = Some(line.chars().take(1024).collect());
                    Ok(())
                },
            )
            .map_err(|error| error.to_string())?;
        linker
            .define_unknown_imports_as_traps(&component)
            .map_err(|error| error.to_string())?;
        arm(&mut store);
        let instance = linker
            .instantiate(&mut store, &component)
            .map_err(|error| format!("{shown}: {}", first_line(&error)))?;
        let init = instance
            .get_typed_func::<(), ()>(&mut store, "init")
            .map_err(|error| format!("{shown}: {error}"))?;
        let tick = instance
            .get_typed_func::<(Vec<u8>,), (Vec<u8>,)>(&mut store, "tick")
            .map_err(|error| format!("{shown}: {error}"))?;
        // `on mount` runs in here, with a budget of its own.
        arm(&mut store);
        if let Err(error) = init.call(&mut store, ()) {
            let trap = format!("{shown}: init trapped: {}", first_line(&error));
            return Err(panic_message(&mut store).unwrap_or(trap));
        }
        Ok(Self {
            module,
            store,
            tick,
            pending: Vec::new(),
            frame: wire::Frame::default(),
            frame_rev: 0,
            ticks: 0,
            inputs: Inputs::default(),
            pictures: Pictures::default(),
            surfaces: Surfaces::default(),
            props_subscription: None,
            props_sent: None,
            intents: Vec::new(),
            fault: None,
        })
    }

    /// The tree as the host holds it, rendered — or the reason there is none.
    fn render(&self) -> Element<'static, Output> {
        if let Some(fault) = &self.fault {
            return widget::container(
                widget::column![
                    widget::text("This view was stopped.").size(14),
                    widget::text(fault.clone()).size(12),
                ]
                .spacing(8)
                .align_x(iced::Alignment::Center),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center(Length::Fill)
            .into();
        }
        let root = self.frame.root.clone().unwrap_or_else(wire::Node::empty);
        view_tree::render(&root, &self.inputs, &self.pictures, &self.surfaces)
    }

    /// What the user did to the tree, as the widgets report it: recorded
    /// host-side (an input's text) and queued for the guest's next tick.
    fn deliver(&mut self, output: Output) {
        self.inputs.apply(output, &mut self.pending);
    }

    /// Hands the guest the props the app holds, if they moved since the
    /// guest last saw them. Before the guest has subscribed they wait here.
    fn sync_props(&mut self, props: &Option<Vec<u8>>) {
        let Some(id) = self.props_subscription else {
            return;
        };
        if props.is_none() || *props == self.props_sent {
            return;
        }
        self.props_sent = props.clone();
        self.pending.push(wire::Event::Response {
            id,
            result: Ok(props.clone().unwrap_or_default()),
            done: false,
        });
    }

    /// One redraw: tick if there is anything to deliver — or never was a
    /// first frame — answer the requests, and say whether the guest is due
    /// again at once. A guest with nothing to deliver is left alone: the
    /// tree the host has is the tree it would send.
    fn redraw(&mut self, props: &Option<Vec<u8>>) -> bool {
        if self.fault.is_some() {
            return false;
        }
        self.sync_props(props);
        let quiet = self.ticks > 0 && !self.frame.busy && self.pending.is_empty();
        if quiet {
            return false;
        }
        self.tick();
        self.ticks += 1;
        for (nth, request) in std::mem::take(&mut self.frame.requests)
            .into_iter()
            .enumerate()
        {
            match nth < MAX_REQUESTS_PER_TICK {
                true => self.answer(request, props),
                false => self.refuse(request.id, "too many requests this tick".into()),
            }
        }
        for id in std::mem::take(&mut self.frame.cancels) {
            if self.props_subscription == Some(id) {
                self.props_subscription = None;
            }
        }
        self.fault.is_none() && (self.frame.busy || !self.pending.is_empty())
    }

    /// Routes one request: the props subscription is answered from what the
    /// app holds, an intent the module declares goes to the app, the log
    /// goes to the log, and anything else is refused.
    fn answer(&mut self, request: wire::Request, props: &Option<Vec<u8>>) {
        let wire::Request { id, kind, payload } = request;
        if payload.len() > MAX_PAYLOAD_BYTES {
            self.refuse(
                id,
                format!("`{kind}` carries more than {MAX_PAYLOAD_BYTES} bytes"),
            );
            return;
        }
        let (capability, operation) = kind.split_once('.').unwrap_or((kind.as_str(), ""));
        let own = capability == self.module;
        let declared_intent = own && intents_of(self.module).contains(&operation);
        match (capability, operation) {
            _ if own && operation == "props" => {
                self.props_subscription = Some(id);
                self.props_sent = None;
                self.sync_props(props);
            }
            _ if declared_intent => self.intents.push(ModuleViewEvent {
                kind: operation.to_owned(),
                detail: String::from_utf8_lossy(&payload).into_owned(),
            }),
            ("host", "log") => {
                tracing::debug!(
                    target: "ducktape::app",
                    module = self.module,
                    line = %String::from_utf8_lossy(&payload),
                    "module view log"
                );
                self.reply(id, Ok(Vec::new()));
            }
            _ => self.refuse(id, format!("unknown request `{kind}`")),
        }
    }

    fn refuse(&mut self, id: u64, message: String) {
        self.reply(id, Err(message));
    }

    fn reply(&mut self, id: u64, result: Result<Vec<u8>, String>) {
        self.pending.push(wire::Event::Response {
            id,
            result,
            done: true,
        });
    }

    /// One call into the module with the pending events, inside the budget.
    /// A trap ends the view; the widget shows the message in its place.
    fn tick(&mut self) {
        let events = std::mem::take(&mut self.pending);
        let bytes = wire::encode(&events);
        arm(&mut self.store);
        let outcome = self
            .tick
            .call(&mut self.store, (bytes,))
            .map(|(frame,)| frame)
            .map_err(|error| first_line(&error))
            .and_then(|frame| shape(&frame));
        match outcome {
            Ok(mut frame) => {
                match merge(&mut self.frame.root, &mut frame) {
                    Ok(false) => {}
                    Ok(true) => {
                        self.frame_rev += 1;
                        if let Some(root) = &mut frame.root {
                            self.inputs.adopt(root);
                            self.pictures.adopt(root);
                            // The guest remembers its tree without the
                            // picture bytes; the tree its patches build on
                            // has to be that one.
                            root.for_each_mut(&mut |node| {
                                if let wire::Node::Svg { bytes, .. } = node {
                                    *bytes = None;
                                }
                            });
                        }
                    }
                    // The tab is blank for a tick and the guest hears that
                    // it must send the tree whole.
                    Err(refused) => {
                        tracing::warn!(
                            target: "ducktape::app",
                            module = self.module,
                            reason = "module_view_patch_refused",
                            error = refused,
                            "module view patch refused"
                        );
                        self.frame_rev += 1;
                        self.pending.push(wire::Event::Resync);
                    }
                }
                self.frame = frame;
            }
            Err(trap) => {
                let reason = panic_message(&mut self.store).unwrap_or(trap);
                tracing::warn!(
                    target: "ducktape::app",
                    module = self.module,
                    reason = "module_view_trapped",
                    error = %reason,
                    "module view ended"
                );
                self.fault = Some(reason);
                self.frame_rev += 1;
            }
        }
    }
}

/// Brings the tree the host holds into `frame`: an `unchanged` frame takes
/// it as is, a frame without a tree patches it, a frame with one replaces
/// it. `Ok(true)` is a tree the widget has to rebuild for.
fn merge(held: &mut Option<wire::Node>, frame: &mut wire::Frame) -> Result<bool, &'static str> {
    if frame.unchanged {
        frame.root = held.take();
        return Ok(false);
    }
    if frame.root.is_some() {
        return Ok(true);
    }
    let patches = std::mem::take(&mut frame.patches);
    let mut root = held.take().ok_or("no tree to patch")?;
    wire::apply(&mut root, patches)?;
    frame.root = Some(root);
    Ok(true)
}

/// What the host is willing to take from one tick's bytes: nothing in here
/// is trusted — the length, the counts, the tree.
fn shape(bytes: &[u8]) -> Result<wire::Frame, String> {
    if bytes.len() > MAX_FRAME_BYTES {
        return Err("frame too large".to_string());
    }
    let mut frame: wire::Frame = wire::decode(bytes)?;
    frame.requests.truncate(2 * MAX_REQUESTS_PER_TICK);
    if frame.unchanged {
        frame.root = None;
    }
    if frame.unchanged || frame.root.is_some() {
        frame.patches = Vec::new();
    }
    wire::sanitize(&mut frame);
    Ok(frame)
}

fn panic_message(store: &mut Store<HostState>) -> Option<String> {
    let text = store.data_mut().panic.take()?;
    (!text.is_empty()).then_some(text)
}

/// Why a call failed: the trap itself, not the wrapper and backtrace
/// wasmtime prints around it.
fn first_line(error: &wasmtime::Error) -> String {
    if let Some(wasmtime::Trap::Interrupt) = error.root_cause().downcast_ref::<wasmtime::Trap>() {
        return format!("tick exceeded {} ms", TICK_DEADLINE.as_millis());
    }
    error
        .root_cause()
        .to_string()
        .lines()
        .next()
        .unwrap_or("trap")
        .to_string()
}

// ---------- the widget ----------

/// The tree the guest last sent, rendered with the app's own widgets and
/// wrapped so that every redraw ticks the guest, everything the user does
/// inside goes back as the guest's own events, and a changed tree is
/// re-rendered in place.
struct ModuleView {
    mounted: Arc<Mutex<Mounted>>,
    /// The frame `content` was rendered from.
    rev: u64,
    content: Element<'static, Output, iced::Theme, iced::Renderer>,
}

impl Widget<ModuleViewEvent, iced::Theme, iced::Renderer> for ModuleView {
    fn tag(&self) -> tree::Tag {
        self.content.as_widget().tag()
    }

    fn state(&self) -> tree::State {
        self.content.as_widget().state()
    }

    fn children(&self) -> Vec<Tree> {
        self.content.as_widget().children()
    }

    fn diff(&self, tree: &mut Tree) {
        self.content.as_widget().diff(tree);
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn size_hint(&self) -> Size<Length> {
        self.content.as_widget().size_hint()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content.as_widget_mut().layout(tree, renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(tree, layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, ModuleViewEvent>,
        viewport: &Rectangle,
    ) {
        // The tree's widgets speak `Output`; what they say is the guest's,
        // not the app's, so it is diverted rather than mapped. Everything
        // else the local shell collected carries over to the window's.
        let mut outputs = Vec::new();
        {
            let mut local = Shell::new(&mut outputs);
            self.content.as_widget_mut().update(
                tree, event, layout, cursor, renderer, clipboard, &mut local, viewport,
            );
            if local.is_event_captured() {
                shell.capture_event();
            }
            if local.is_layout_invalid() {
                shell.invalidate_layout();
            }
            if local.are_widgets_invalid() {
                shell.invalidate_widgets();
            }
            match local.redraw_request() {
                window::RedrawRequest::NextFrame => shell.request_redraw(),
                window::RedrawRequest::At(at) => shell.request_redraw_at(at),
                window::RedrawRequest::Wait => {}
            }
            shell.input_method_mut().merge(local.input_method());
        }
        let mut mounted = self.mounted.lock().expect("module view lock");
        let Mounted { slot, props } = &mut *mounted;
        let guest = match slot {
            Slot::Ready(guest) => guest,
            Slot::Loading => {
                shell.request_redraw_at(window::RedrawRequest::At(
                    iced::time::Instant::now() + LOAD_POLL,
                ));
                return;
            }
            Slot::Failed(_) => return,
        };
        if !outputs.is_empty() {
            for output in outputs {
                guest.deliver(output);
            }
            shell.request_redraw();
        }
        let Event::Window(window::Event::RedrawRequested(_)) = event else {
            return;
        };
        if guest.redraw(props) {
            shell.request_redraw();
        }
        for intent in std::mem::take(&mut guest.intents) {
            shell.publish(intent);
        }
        // A new tree is re-rendered here, in place: the app's own view is
        // rebuilt only by its own messages, and a guest's tick is not one.
        if guest.frame_rev != self.rev {
            self.rev = guest.frame_rev;
            self.content = guest.render();
            tree.diff(self.content.as_widget());
            shell.invalidate_layout();
            shell.request_redraw();
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
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
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.content
            .as_widget()
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, ModuleViewEvent, iced::Theme, iced::Renderer>> {
        // An overlay's messages would be the guest's too; the tree carries
        // no widget that opens one.
        let _ = (tree, layout, renderer, viewport, translation);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(kind: &str, detail: &str) -> ModuleViewEvent {
        ModuleViewEvent {
            kind: kind.into(),
            detail: detail.into(),
        }
    }

    #[test]
    fn an_intent_is_read_off_the_guests_json() {
        let vote = event("vote", r#"{"proposal_id":"prop-1","approve":false}"#);
        assert!(matches!(gov_intent(&vote), crate::GovIntent::Vote));
        assert_eq!(gov_event_proposal(&vote), "prop-1");
        assert!(!gov_event_approves(&vote));
        let settle = event("execute", r#"{"proposal_id":"prop-2","approve":true}"#);
        assert!(matches!(gov_intent(&settle), crate::GovIntent::Execute));
        assert_eq!(gov_event_proposal(&settle), "prop-2");
    }

    /// Malformed JSON names no proposal, and the handler's empty-id guard
    /// is what refuses it.
    #[test]
    fn a_malformed_intent_names_no_proposal() {
        let broken = event("vote", "not json");
        assert_eq!(gov_event_proposal(&broken), "");
        assert!(!gov_event_approves(&broken));
    }

    /// Only the operations a module declares reach the app; the props
    /// subscription and the log are the host's, everything else is refused.
    #[test]
    fn only_declared_intents_are_routed() {
        assert_eq!(intents_of("governance"), ["vote", "execute"]);
        assert!(intents_of("chat").is_empty());
    }

    /// Every text in the tree the host holds, in tree order.
    fn texts(guest: &Guest) -> Vec<String> {
        let mut root = guest.frame.root.clone().expect("a tree");
        let mut texts = Vec::new();
        root.for_each_mut(&mut |node| {
            if let wire::Node::Text { content, .. } = node {
                texts.push(content.clone());
            }
        });
        texts
    }

    /// The message index the button labelled `name` would send.
    fn button_message(guest: &Guest, name: &str) -> u32 {
        let mut root = guest.frame.root.clone().expect("a tree");
        let mut message = None;
        root.for_each_mut(&mut |node| {
            if let wire::Node::Button {
                label, on_press, ..
            } = node
                && label.as_deref() == Some(name)
            {
                message = *on_press;
            }
        });
        message.expect("an enabled button")
    }

    /// The bundled component, end to end through the host: it boots on the
    /// offline plate, takes the register the app pushes, and a press on its
    /// card comes back as the intent the handler signs. Needs `make views`;
    /// without the staged component the test says so and does nothing.
    #[test]
    fn the_staged_governance_view_boots_takes_the_register_and_votes() {
        let staged = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../target/views/governance_view.wasm");
        if !staged.is_file() {
            eprintln!("skipped: no {} — run `make views`", staged.display());
            return;
        }
        let mut guest = Guest::load_from("governance", &staged).expect("the view loads");
        let no_props = None;
        assert!(
            !guest.redraw(&no_props),
            "a booted view with no props is quiet"
        );
        assert!(
            guest.props_subscription.is_some(),
            "the view subscribes to its props"
        );
        assert!(
            texts(&guest).iter().any(|text| text == "Not connected"),
            "{:?}",
            texts(&guest)
        );

        let props = Some(
            serde_json::to_vec(&serde_json::json!({
                "rows": [{
                    "id": "prop-1", "action": "add_validator", "detail": "node-7",
                    "proposer": "robin", "status": "open", "deadline": 4200,
                    "approvals": 1, "rejections": 0, "rule": "threshold",
                    "required_yes": 2, "electorate": 4, "open": true, "settled_height": 0
                }],
                "voting": "", "admin": true, "connected": true, "answered": true, "dark": false
            }))
            .expect("props encode"),
        );
        guest.redraw(&props);
        let shown = texts(&guest);
        for expected in [
            "1 pending",
            "prop-1",
            "1 approval · 1 more for quorum",
            "Approve →",
        ] {
            assert!(
                shown.iter().any(|text| text == expected),
                "missing {expected:?} in {shown:?}"
            );
        }
        // The same props again are not delivered again.
        assert!(
            !guest.redraw(&props),
            "unchanged props leave the view quiet"
        );

        guest.deliver(Output::Activate(button_message(&guest, "Approve")));
        guest.redraw(&props);
        assert_eq!(
            guest.intents,
            [ModuleViewEvent {
                kind: "vote".into(),
                detail: r#"{"proposal_id":"prop-1","approve":true}"#.into(),
            }]
        );
        assert!(guest.fault.is_none());
    }

    #[test]
    fn a_frame_that_changed_nothing_keeps_the_held_tree() {
        let held_tree = wire::Node::empty();
        let mut held = Some(held_tree.clone());
        let mut unchanged = wire::Frame {
            unchanged: true,
            ..wire::Frame::default()
        };
        assert_eq!(merge(&mut held, &mut unchanged), Ok(false));
        assert_eq!(unchanged.root, Some(held_tree));
        let mut patched = wire::Frame::default();
        assert_eq!(merge(&mut None, &mut patched), Err("no tree to patch"));
    }
}
