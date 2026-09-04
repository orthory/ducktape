//! A module's VIEW, run as wasm inside the app.
//!
//! A module ships its screen as `<id>.view.wasm` beside its
//! `<id>.component.wasm` in the modules dir (`workspace_config::modules_dir`),
//! an ordinary Ice application built on `ui-lang-guest`. The app instantiates
//! it in a fuel and memory budget, feeds it the events its widget sees in the
//! guest's own coordinates, and replays the frame it draws — quads as quads,
//! text as one shaped line each — through the app's own renderer. What the
//! guest cannot do alone it asks for as a request the widget answers:
//!
//! | kind | payload | answer |
//! |---|---|---|
//! | `host.theme` (stream) | – | `light` or `dark` at once and on every change |
//! | `host.refresh` (stream) | – | one empty item each time the host's data generation moves |
//! | `host.log` | text | nothing; the line lands in the app's log |
//! | `query.<module>` | a JSON query | the module's JSON reply, over `/v1/query` |
//!
//! A query is the node's to answer, over HTTP, so it never runs on the window
//! thread: the guest's requests queue on the surface, the widget says it was
//! asked, and [`serve_wasm_view`] — an Ice task — carries them to the node and
//! back into the guest's inbox.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use iced::advanced::text::{self as core_text, LineHeight, Paragraph as _, Shaping, Wrapping};
use iced::advanced::widget::Tree;
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, renderer};
use iced::time::Instant;
use iced::{
    Border, Color, Element, Event, Length, Pixels, Point, Rectangle, Shadow, Size, Vector, keyboard,
};
use ui_lang_wire as wire;
use wasmtime::{
    Config, Engine, Linker, Module, OptLevel, Store, StoreLimits, StoreLimitsBuilder, TypedFunc,
};

use super::rpc_client;

const TARGET: &str = "ducktape::wasm_view";

// ---------- the budget ----------

/// Roughly one unit per instruction: what one tick may run.
const FUEL_PER_TICK: u64 = 200_000_000;
const MEMORY_LIMIT: usize = 64 << 20;
const MAX_FRAME_BYTES: usize = 8 << 20;
const MAX_REQUESTS_PER_TICK: usize = 256;
const MAX_PAYLOAD_BYTES: usize = 1 << 20;
const MAX_REPLY_BYTES_PER_TICK: usize = 4 << 20;
const MAX_FAULT_BYTES: usize = 1024;
const MAX_LOG_BYTES: usize = 1024;
const MAX_SUBSCRIPTIONS: usize = 16;
const MAX_QUERIES: usize = 64;

// ---------- the Ice-facing surface ----------

/// The handle the view holds. Identity is the instance: two surfaces compare
/// equal only when they are the same guest.
#[derive(Clone, Debug)]
pub struct WasmSurface(Arc<Mutex<Guest>>);

impl PartialEq for WasmSurface {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for WasmSurface {}

#[cfg(test)]
impl WasmSurface {
    /// A surface over a module file the test names — the committed artifact
    /// rather than the staged one, since a test binary has no modules dir.
    pub(crate) fn load_for_probe(id: &str, path: &std::path::Path) -> Result<Self, String> {
        Guest::load(id, path).map(|guest| Self(Arc::new(Mutex::new(guest))))
    }

    /// How many queries the guest has waiting on the node.
    pub(crate) fn queued_queries(&self) -> usize {
        self.0.lock().expect("guest lock").queries.len()
    }
}

/// What the widget asks of the app, which only an Ice task can do: carry the
/// guest's queries to the node, or load its module again after a trap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WasmViewEvent(Verb);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Verb {
    Ask,
    Restart,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WasmViewError {
    pub message: String,
}

/// The module's view, if it ships one: `None` when the modules dir holds no
/// `<id>.view.wasm`, and the screen is the app's own. Runs on the executor —
/// a cold load is a cranelift run, a second the window thread cannot spend.
pub async fn load_wasm_view(id: String) -> Result<Option<WasmSurface>, WasmViewError> {
    let dir = workspace_config::modules_dir().map_err(|message| WasmViewError { message })?;
    let path = dir.join(format!("{id}.view.wasm"));
    if !path.is_file() {
        tracing::debug!(target: TARGET, module = %id, event = "view_absent");
        return Ok(None);
    }
    let guest = Guest::load(&id, &path).map_err(|message| WasmViewError { message })?;
    Ok(Some(WasmSurface(Arc::new(Mutex::new(guest)))))
}

/// Does what the widget cannot: answers the queries the guest queued through
/// the node, or reloads a trapped module into the same handle. Either way the
/// surface comes back the same, so the handler that hears it can redraw.
pub async fn serve_wasm_view(
    surface: Option<WasmSurface>,
    event: WasmViewEvent,
    rpc: String,
) -> Result<Option<WasmSurface>, WasmViewError> {
    // The widget only exists under a loaded surface, so an event with none
    // is a view that was dropped meanwhile: nothing to serve.
    let Some(surface) = surface else {
        return Ok(None);
    };
    match event.0 {
        Verb::Ask => answer_queries(&surface, &rpc).await,
        Verb::Restart => restart(&surface),
    }
    .map_err(|message| WasmViewError { message })?;
    Ok(Some(surface))
}

async fn answer_queries(surface: &WasmSurface, rpc: &str) -> Result<(), String> {
    // Taken under the lock and answered outside it: a query is a round trip
    // to the node, and the window thread ticks the guest meanwhile.
    let queries = std::mem::take(&mut surface.0.lock().expect("guest lock").queries);
    if queries.is_empty() {
        return Ok(());
    }
    let client = rpc_client(rpc)?;
    let mut answers = Vec::with_capacity(queries.len());
    for query in queries {
        let result = match serde_json::from_slice::<serde_json::Value>(&query.payload) {
            Ok(value) => client
                .query::<serde_json::Value, serde_json::Value>(&query.module, &value)
                .await
                .map(|reply| reply.to_string().into_bytes())
                .map_err(|error| error.to_string()),
            Err(error) => Err(format!("a query that is not JSON: {error}")),
        };
        tracing::debug!(
            target: TARGET,
            module = %query.module,
            ok = result.is_ok(),
            event = "query_answered"
        );
        answers.push(one_shot(query.id, result));
    }
    surface.0.lock().expect("guest lock").inbox.extend(answers);
    Ok(())
}

/// Reloads a faulted guest's module and swaps the fresh instance into the
/// handle the widget already holds. A guest that is not faulted is left as it
/// is: the Restart button stays live through the load, and a second press
/// must not drop a running instance.
fn restart(surface: &WasmSurface) -> Result<(), String> {
    let (id, path) = {
        let guest = surface.0.lock().expect("guest lock");
        if guest.fault.is_none() {
            return Ok(());
        }
        (guest.id.clone(), guest.path.clone())
    };
    let fresh = Guest::load(&id, &path);
    let mut guest = surface.0.lock().expect("guest lock");
    match fresh {
        Ok(mut fresh) => {
            fresh.size = guest.size;
            fresh.dark = guest.dark;
            fresh.generation = guest.generation;
            fresh.pending.push(wire::Event::Resized {
                width: guest.size.width,
                height: guest.size.height,
            });
            *guest = fresh;
            Ok(())
        }
        Err(message) => {
            guest.fault = Some(message.clone());
            Err(message)
        }
    }
}

// ---------- the guest ----------

/// Where the guest's panic hook left its message, and how long it is.
type PanicText = Option<(TypedFunc<(), u32>, TypedFunc<(), u32>)>;

/// A `query.<module>` the guest made, waiting for the node.
#[derive(Debug)]
struct Query {
    id: u64,
    module: String,
    payload: Vec<u8>,
}

struct Guest {
    id: String,
    path: std::path::PathBuf,
    /// Identity for the keyboard focus: an instance's for as long as the app
    /// runs, unlike the `Arc`'s address, which the allocator hands to the next
    /// instance of the same size.
    serial: u64,
    store: Store<StoreLimits>,
    memory: wasmtime::Memory,
    input_ptr: TypedFunc<u32, u32>,
    tick: TypedFunc<u32, u32>,
    output_ptr: TypedFunc<(), u32>,
    panic_text: PanicText,
    size: Size,
    pending: Vec<wire::Event>,
    frame: wire::Frame,
    /// Answers ready for the next tick: subscription items and query replies.
    inbox: Vec<wire::Event>,
    /// Queries the guest made that the node has not answered yet.
    queries: Vec<Query>,
    /// Set by a tick that queued a query; the widget reads it once and asks.
    asked: bool,
    dark: Option<bool>,
    theme_subscriptions: Vec<u64>,
    generation: Option<i64>,
    refresh_subscriptions: Vec<u64>,
    /// The trap that ended the module, if one did. A faulted guest never
    /// ticks again.
    fault: Option<String>,
    reply_bytes: usize,
    fuel_used: u64,
    tick_time: Duration,
}

impl std::fmt::Debug for Guest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Guest")
            .field("module", &self.id)
            .field("size", &self.size)
            .field("fault", &self.fault)
            .finish_non_exhaustive()
    }
}

impl Drop for Guest {
    fn drop(&mut self) {
        release_focus(self.serial);
    }
}

/// One engine for every view: fuel is metered, and the compile is cranelift
/// at speed, so a tick costs what the module does and not what wasmtime does.
fn engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(|| {
        let mut config = Config::new();
        config.cranelift_opt_level(OptLevel::Speed);
        config.consume_fuel(true);
        Engine::new(&config).expect("wasmtime engine")
    })
}

impl Guest {
    fn load(id: &str, path: &std::path::Path) -> Result<Self, String> {
        let engine = engine();
        let started = Instant::now();
        let module = Module::from_file(engine, path)
            .map_err(|error| format!("{}: {}", path.display(), first_line(&error)))?;
        // Tables are allocated eagerly at their declared minimum, before any
        // fuel or memory limit is consulted.
        let limits = StoreLimitsBuilder::new()
            .memory_size(MEMORY_LIMIT)
            .memories(1)
            .instances(1)
            .tables(4)
            .table_elements(1 << 20)
            .trap_on_grow_failure(true)
            .build();
        let mut store = Store::new(engine, limits);
        store.limiter(|limits| limits);
        store
            .set_fuel(FUEL_PER_TICK)
            .map_err(|error| error.to_string())?;
        // The guest links web_time's wasm-bindgen shims for `Instant::now`;
        // nothing on the frame path calls them, so they answer zero.
        let mut linker = Linker::new(engine);
        linker
            .define_unknown_imports_as_default_values(&mut store, &module)
            .map_err(|error| error.to_string())?;
        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|error| first_line(&error))?;
        let export = |name: &str| format!("{}: missing export `{name}`", path.display());
        let init = instance
            .get_typed_func::<(), ()>(&mut store, "init")
            .map_err(|_| export("init"))?;
        let input_ptr = instance
            .get_typed_func::<u32, u32>(&mut store, "input_ptr")
            .map_err(|_| export("input_ptr"))?;
        let tick = instance
            .get_typed_func::<u32, u32>(&mut store, "tick")
            .map_err(|_| export("tick"))?;
        let output_ptr = instance
            .get_typed_func::<(), u32>(&mut store, "output_ptr")
            .map_err(|_| export("output_ptr"))?;
        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| export("memory"))?;
        let panic_text = instance
            .get_typed_func::<(), u32>(&mut store, "panic_ptr")
            .ok()
            .zip(
                instance
                    .get_typed_func::<(), u32>(&mut store, "panic_len")
                    .ok(),
            );
        // `on mount` runs in here, so a panic in the app's boot has the same
        // message parked as a panic in any later tick.
        if let Err(error) = init.call(&mut store, ()) {
            let trap = format!("init trapped: {}", first_line(&error));
            return Err(panic_message(&mut store, &memory, &panic_text).unwrap_or(trap));
        }
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        tracing::info!(
            target: TARGET,
            module = %id,
            took_ms = started.elapsed().as_millis() as u64,
            event = "view_loaded"
        );
        Ok(Self {
            id: id.to_string(),
            path: path.to_path_buf(),
            serial: SERIAL.fetch_add(1, Ordering::Relaxed),
            store,
            memory,
            input_ptr,
            tick,
            output_ptr,
            panic_text,
            size: Size::ZERO,
            pending: Vec::new(),
            frame: wire::Frame::default(),
            inbox: Vec::new(),
            queries: Vec::new(),
            asked: false,
            dark: None,
            theme_subscriptions: Vec::new(),
            generation: None,
            refresh_subscriptions: Vec::new(),
            fault: None,
            reply_bytes: 0,
            fuel_used: 0,
            tick_time: Duration::ZERO,
        })
    }

    /// The app's colour mode as the widget has it; a change reaches every
    /// theme subscription the guest holds.
    fn set_theme(&mut self, dark: bool) {
        if self.dark == Some(dark) {
            return;
        }
        self.dark = Some(dark);
        for id in self.theme_subscriptions.clone() {
            self.inbox.push(theme_item(id, dark));
        }
    }

    /// The host's data generation as the widget has it; a move reaches every
    /// refresh subscription. The first value is the one the guest was born
    /// under: it queried on mount and owes nobody a second read.
    fn set_generation(&mut self, generation: i64) {
        let moved = self.generation.is_some_and(|known| known != generation);
        self.generation = Some(generation);
        if !moved {
            return;
        }
        for id in self.refresh_subscriptions.clone() {
            self.inbox.push(stream_item(id, Vec::new()));
        }
    }

    /// One redraw: deliver the inbox, tick, answer the new requests, and say
    /// whether the guest wants another frame. A guest with nothing to deliver
    /// and no wish of its own to draw is not ticked at all.
    fn redraw(&mut self, now: Instant) -> Option<Instant> {
        if self.fault.is_some() {
            return None;
        }
        if self.quiet(now) {
            return self.next_wake();
        }
        self.pending.append(&mut self.inbox);
        self.pending.push(wire::Event::Redraw {
            elapsed_ms: uptime_ms(now),
        });
        self.tick();
        self.reply_bytes = 0;
        for (nth, request) in std::mem::take(&mut self.frame.requests)
            .into_iter()
            .enumerate()
        {
            match nth < MAX_REQUESTS_PER_TICK {
                true => self.answer(request),
                false => self.reply(request.id, Err("too many requests this tick".into())),
            }
        }
        // After the requests, never before: a request made and dropped inside
        // one tick sits in both lists of the same frame.
        for id in std::mem::take(&mut self.frame.cancels) {
            self.cancel(id);
        }
        self.next_wake()
    }

    /// Nothing waiting, and the guest's own widgets did not ask for a frame.
    fn quiet(&self, now: Instant) -> bool {
        self.pending.is_empty() && self.inbox.is_empty() && !self.wants_frame(now)
    }

    fn wants_frame(&self, now: Instant) -> bool {
        match self.frame.redraw {
            wire::Redraw::Wait => false,
            wire::Redraw::NextFrame => true,
            wire::Redraw::At(ms) => uptime_ms(now) >= ms,
        }
    }

    fn next_wake(&self) -> Option<Instant> {
        let inbox_waiting = !self.inbox.is_empty();
        if inbox_waiting {
            return Some(Instant::now());
        }
        match self.frame.redraw {
            wire::Redraw::Wait => None,
            wire::Redraw::NextFrame => Some(Instant::now()),
            wire::Redraw::At(ms) => Some(at_uptime_ms(ms)),
        }
    }

    fn cancel(&mut self, id: u64) {
        self.inbox
            .retain(|event| !matches!(event, wire::Event::Response { id: due, .. } if *due == id));
        self.queries.retain(|query| query.id != id);
        self.theme_subscriptions.retain(|theme| *theme != id);
        self.refresh_subscriptions.retain(|refresh| *refresh != id);
    }

    /// Routes one request to what answers it. A refusal is an ordinary `Err`.
    fn answer(&mut self, request: wire::Request) {
        let wire::Request { id, kind, payload } = request;
        if payload.len() > MAX_PAYLOAD_BYTES {
            let message = format!("`{kind}` carries more than {MAX_PAYLOAD_BYTES} bytes");
            self.reply(id, Err(message));
            return;
        }
        self.reply_bytes += payload.len();
        if let Some(message) = self.over_budget() {
            self.reply(id, Err(message));
            return;
        }
        let (capability, operation) = kind.split_once('.').unwrap_or((kind.as_str(), ""));
        match (capability, operation) {
            ("host", "log") => {
                let line = String::from_utf8_lossy(&payload[..payload.len().min(MAX_LOG_BYTES)])
                    .replace(|c: char| c.is_control(), " ");
                tracing::debug!(target: TARGET, module = %self.id, line = %line, event = "guest_log");
                self.reply(id, Ok(Vec::new()));
            }
            ("host", "theme") if self.theme_subscriptions.len() >= MAX_SUBSCRIPTIONS => {
                let message = format!("more than {MAX_SUBSCRIPTIONS} theme subscriptions");
                self.reply(id, Err(message));
            }
            // The current mode at once, then every change. The widget sets
            // the mode before the first tick, so there is always one to send.
            ("host", "theme") => {
                self.theme_subscriptions.push(id);
                if let Some(dark) = self.dark {
                    self.inbox.push(theme_item(id, dark));
                }
            }
            ("host", "refresh") if self.refresh_subscriptions.len() >= MAX_SUBSCRIPTIONS => {
                let message = format!("more than {MAX_SUBSCRIPTIONS} refresh subscriptions");
                self.reply(id, Err(message));
            }
            ("host", "refresh") => self.refresh_subscriptions.push(id),
            ("query", _) if self.queries.len() >= MAX_QUERIES => {
                let message = format!("more than {MAX_QUERIES} queries waiting on the node");
                self.reply(id, Err(message));
            }
            ("query", module) if !module.is_empty() => {
                self.queries.push(Query {
                    id,
                    module: module.to_string(),
                    payload,
                });
                self.asked = true;
            }
            _ => self.reply(id, Err(format!("unknown request `{kind}`"))),
        }
    }

    fn over_budget(&self) -> Option<String> {
        (self.reply_bytes > MAX_REPLY_BYTES_PER_TICK)
            .then(|| format!("more than {MAX_REPLY_BYTES_PER_TICK} bytes this tick"))
    }

    fn reply(&mut self, id: u64, result: Result<Vec<u8>, String>) {
        self.reply_bytes += match &result {
            Ok(bytes) => bytes.len(),
            Err(message) => message.len(),
        };
        let result = match self.over_budget() {
            Some(message) => Err(message),
            None => result,
        };
        self.inbox.push(one_shot(id, result));
    }

    /// One call into the module with the pending events, inside the fuel
    /// budget. A trap ends the view; the reason stays for the widget to show.
    fn tick(&mut self) {
        let events = std::mem::take(&mut self.pending);
        let bytes = wire::encode(&events);
        let started = Instant::now();
        let _ = self.store.set_fuel(FUEL_PER_TICK);
        let outcome = self.tick_inner(&bytes);
        self.tick_time = started.elapsed();
        self.fuel_used = FUEL_PER_TICK.saturating_sub(self.store.get_fuel().unwrap_or(0));
        match outcome {
            Ok(mut frame) => {
                if frame.unchanged {
                    frame.layers = std::mem::take(&mut self.frame.layers);
                }
                self.frame = frame;
            }
            Err(error) => {
                // With `panic = "abort"` a panic is a bare `unreachable`, so
                // the reason is in the module's buffer or nowhere.
                let trap = first_line(&error);
                let reason =
                    panic_message(&mut self.store, &self.memory, &self.panic_text).unwrap_or(trap);
                tracing::warn!(target: TARGET, module = %self.id, reason = %reason, event = "view_trapped");
                self.fault = Some(reason);
            }
        }
    }

    fn tick_inner(&mut self, bytes: &[u8]) -> wasmtime::Result<wire::Frame> {
        let ptr = self.input_ptr.call(&mut self.store, bytes.len() as u32)? as usize;
        // A guest chooses these offsets and lengths; the host must not index
        // its own memory on the guest's word.
        let input = window_mut(self.memory.data_mut(&mut self.store), ptr, bytes.len())?;
        input.copy_from_slice(bytes);
        let len = self.tick.call(&mut self.store, bytes.len() as u32)? as usize;
        if len > MAX_FRAME_BYTES {
            return Err(wasmtime::Error::msg("frame too large"));
        }
        let ptr = self.output_ptr.call(&mut self.store, ())? as usize;
        let frame = window(self.memory.data(&self.store), ptr, len)?;
        let mut frame: wire::Frame = wire::decode(frame).map_err(wasmtime::Error::msg)?;
        frame.requests.truncate(2 * MAX_REQUESTS_PER_TICK);
        // A frame that says it changed nothing must not carry layers the
        // host would then draw unsanitized; it is treated as what it claims.
        if frame.unchanged {
            frame.layers.clear();
        } else {
            // The renderer panics on values a frame may carry and allocates
            // by the sizes it is given; the guest chose every one.
            wire::sanitize(&mut frame, [self.size.width, self.size.height]);
        }
        Ok(frame)
    }
}

fn panic_message(
    store: &mut Store<StoreLimits>,
    memory: &wasmtime::Memory,
    panic_text: &PanicText,
) -> Option<String> {
    let (ptr, len) = panic_text.clone()?;
    let _ = store.set_fuel(FUEL_PER_TICK);
    let ptr = ptr.call(&mut *store, ()).ok()? as usize;
    let len = (len.call(&mut *store, ()).ok()? as usize).min(MAX_FAULT_BYTES);
    let text = window(memory.data(&*store), ptr, len).ok()?;
    let text = String::from_utf8_lossy(text);
    let text = text.lines().next().unwrap_or_default().to_string();
    (!text.is_empty()).then_some(text)
}

fn window(memory: &[u8], ptr: usize, len: usize) -> wasmtime::Result<&[u8]> {
    ptr.checked_add(len)
        .and_then(|end| memory.get(ptr..end))
        .ok_or_else(|| wasmtime::Error::msg("a buffer outside the guest's memory"))
}

fn window_mut(memory: &mut [u8], ptr: usize, len: usize) -> wasmtime::Result<&mut [u8]> {
    ptr.checked_add(len)
        .and_then(|end| memory.get_mut(ptr..end))
        .ok_or_else(|| wasmtime::Error::msg("a buffer outside the guest's memory"))
}

fn one_shot(id: u64, result: Result<Vec<u8>, String>) -> wire::Event {
    wire::Event::Response {
        id,
        result,
        done: true,
    }
}

fn stream_item(id: u64, bytes: Vec<u8>) -> wire::Event {
    wire::Event::Response {
        id,
        result: Ok(bytes),
        done: false,
    }
}

fn theme_item(id: u64, dark: bool) -> wire::Event {
    let mode: &[u8] = if dark { b"dark" } else { b"light" };
    stream_item(id, mode.to_vec())
}

/// The trap itself, not the "error while executing" wrapper and wasm
/// backtrace wasmtime prints around it.
fn first_line(error: &wasmtime::Error) -> String {
    error
        .root_cause()
        .to_string()
        .lines()
        .next()
        .unwrap_or("trap")
        .to_string()
}

/// The guest's clock is the app's uptime: `Instant::now()` inside a module
/// answers zero, and the uptime on every `Redraw` added to zero is a
/// monotonic clock.
fn started() -> Instant {
    static STARTED: OnceLock<Instant> = OnceLock::new();
    *STARTED.get_or_init(Instant::now)
}

fn uptime_ms(now: Instant) -> u64 {
    now.saturating_duration_since(started()).as_millis() as u64
}

fn at_uptime_ms(ms: u64) -> Instant {
    started() + Duration::from_millis(ms)
}

// ---------- the widget ----------

/// The module's view in the app. `dark` is the app's colour mode and
/// `generation` its data generation for the module; both reach the guest as
/// stream items. `wake` is only the reason the view was rebuilt after a
/// served event: the redraw it costs is what delivers the guest's inbox.
pub fn wasm_view(
    surface: &WasmSurface,
    dark: bool,
    generation: i64,
    _wake: i64,
) -> Element<'static, WasmViewEvent> {
    Element::new(WasmView {
        guest: surface.0.clone(),
        dark,
        generation,
    })
}

struct WasmView {
    guest: Arc<Mutex<Guest>>,
    dark: bool,
    generation: i64,
}

impl<Theme, Renderer> Widget<WasmViewEvent, Theme, Renderer> for WasmView
where
    Renderer: core_text::Renderer<Font = iced::Font>,
{
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(limits.max())
    }

    fn update(
        &mut self,
        _tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, WasmViewEvent>,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let mut guest = self.guest.lock().expect("guest lock");
        if guest.fault.is_some() {
            // A module the host ended receives nothing more; the only live
            // thing left in its place is Restart, and an Ice task runs it.
            if matches!(event, Event::Mouse(mouse::Event::ButtonPressed(_))) {
                focus(guest.serial, cursor.is_over(bounds));
                if cursor.is_over(restart_button(bounds)) {
                    shell.publish(WasmViewEvent(Verb::Restart));
                    shell.capture_event();
                }
            }
            return;
        }
        if guest.size != bounds.size() {
            guest.size = bounds.size();
            guest.pending.push(wire::Event::Resized {
                width: bounds.width,
                height: bounds.height,
            });
        }
        // Before any tick, so a guest subscribing in its `on mount` is
        // answered with the mode and generation the app already has.
        guest.set_theme(self.dark);
        guest.set_generation(self.generation);
        match event {
            Event::Mouse(event) => {
                let translated = match event {
                    // The widget may sit inside a scrollable: `cursor` is
                    // already translated into this layout's space, while the
                    // event's position is the raw window position.
                    mouse::Event::CursorMoved { position } => {
                        let position = cursor.position().unwrap_or(*position);
                        Some(wire::Event::CursorMoved {
                            x: position.x - bounds.x,
                            y: position.y - bounds.y,
                        })
                    }
                    mouse::Event::CursorLeft => Some(wire::Event::CursorLeft),
                    mouse::Event::CursorEntered => Some(wire::Event::CursorEntered),
                    mouse::Event::ButtonPressed(button) => {
                        wire_button(*button).map(wire::Event::ButtonPressed)
                    }
                    mouse::Event::ButtonReleased(button) => {
                        wire_button(*button).map(wire::Event::ButtonReleased)
                    }
                    mouse::Event::WheelScrolled { delta } => Some(match delta {
                        mouse::ScrollDelta::Lines { x, y } => {
                            wire::Event::WheelLines { x: *x, y: *y }
                        }
                        mouse::ScrollDelta::Pixels { x, y } => {
                            wire::Event::WheelPixels { x: *x, y: *y }
                        }
                    }),
                };
                // A press that lands on the guest is the guest's, and takes
                // the keyboard with it.
                if matches!(event, mouse::Event::ButtonPressed(_)) {
                    let over = cursor.is_over(bounds);
                    focus(guest.serial, over);
                    if over {
                        shell.capture_event();
                    }
                }
                if let Some(translated) = translated {
                    guest.pending.push(translated);
                    shell.request_redraw();
                }
            }
            Event::Keyboard(event) => {
                let translated = match event {
                    // State, not input: a guest that loses focus while Shift
                    // is down would never hear it come up.
                    keyboard::Event::ModifiersChanged(modifiers) => {
                        wire::Event::ModifiersChanged(modifiers.bits())
                    }
                    _ if !focused(guest.serial) => return,
                    keyboard::Event::KeyPressed {
                        key,
                        modifiers,
                        text,
                        ..
                    } => wire::Event::KeyPressed {
                        key: wire_key(key),
                        modifiers: modifiers.bits(),
                        text: text.as_ref().map(|text| text.to_string()),
                    },
                    keyboard::Event::KeyReleased { key, modifiers, .. } => {
                        wire::Event::KeyReleased {
                            key: wire_key(key),
                            modifiers: modifiers.bits(),
                        }
                    }
                };
                // A key the focused guest takes is taken: the window's Tab
                // traversal would otherwise read the guest's typing as
                // nobody's. Modifier state is not a key.
                if !matches!(event, keyboard::Event::ModifiersChanged(_)) {
                    shell.capture_event();
                }
                guest.pending.push(translated);
                shell.request_redraw();
            }
            Event::Window(iced::window::Event::RedrawRequested(now)) => {
                if let Some(at) = guest.redraw(*now) {
                    shell.request_redraw_at(at);
                }
                if std::mem::take(&mut guest.asked) {
                    shell.publish(WasmViewEvent(Verb::Ask));
                }
            }
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if !cursor.is_over(layout.bounds()) {
            return mouse::Interaction::None;
        }
        let guest = self.guest.lock().expect("guest lock");
        if guest.fault.is_some() {
            return mouse::Interaction::None;
        }
        match guest.frame.interaction {
            1 => mouse::Interaction::Pointer,
            2 => mouse::Interaction::Text,
            3 => mouse::Interaction::Grab,
            _ => mouse::Interaction::Idle,
        }
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let guest = self.guest.lock().expect("guest lock");
        if let Some(fault) = &guest.fault {
            renderer.with_layer(bounds, |renderer| {
                draw_fault(renderer, bounds, fault, self.dark)
            });
            return;
        }
        renderer.with_layer(bounds, |renderer| {
            renderer.with_translation(Vector::new(bounds.x, bounds.y), |renderer| {
                for layer in &guest.frame.layers {
                    renderer
                        .with_layer(rect(layer.bounds), |renderer| replay_layer(renderer, layer));
                }
            });
        });
        // Its own layer, pushed after the frame's: anything added to a parent
        // layer after a child was pushed is drawn beneath that child.
        let status = format!(
            "wasm · {:.0}k fuel · {:.2} ms",
            guest.fuel_used as f64 / 1000.0,
            guest.tick_time.as_secs_f64() * 1000.0,
        );
        renderer.with_layer(bounds, |renderer| {
            small_text(
                renderer,
                status,
                Point::new(bounds.x + 12.0, bounds.y + bounds.height - 24.0),
                muted(self.dark),
                bounds,
            );
        });
    }
}

fn muted(dark: bool) -> Color {
    match dark {
        true => Color::from_rgba(0.62, 0.68, 0.76, 0.9),
        false => Color::from_rgba(0.40, 0.44, 0.52, 0.9),
    }
}

/// The host ended this module. What remains is the reason, in its place.
fn draw_fault<Renderer>(renderer: &mut Renderer, bounds: Rectangle, fault: &str, dark: bool)
where
    Renderer: core_text::Renderer<Font = iced::Font>,
{
    renderer.fill_quad(
        renderer::Quad {
            bounds,
            ..renderer::Quad::default()
        },
        Color::from_rgba(0.79, 0.20, 0.27, 0.06),
    );
    let left = bounds.x + 24.0;
    let middle = bounds.y + bounds.height / 2.0;
    small_text(
        renderer,
        "The app ended this module's view.".to_string(),
        Point::new(left, middle - 20.0),
        Color::from_rgb(0.79, 0.20, 0.27),
        bounds,
    );
    small_text(
        renderer,
        fault.to_string(),
        Point::new(left, middle + 4.0),
        muted(dark),
        bounds,
    );
    let button = restart_button(bounds);
    renderer.fill_quad(
        renderer::Quad {
            bounds: button,
            border: Border {
                color: Color::from_rgb(0.79, 0.20, 0.27),
                width: 1.0,
                radius: 6.0.into(),
            },
            ..renderer::Quad::default()
        },
        Color::TRANSPARENT,
    );
    small_text(
        renderer,
        "Restart".to_string(),
        Point::new(button.x + 14.0, button.center_y() - 8.0),
        Color::from_rgb(0.79, 0.20, 0.27),
        bounds,
    );
}

/// Drawn there, hit-tested there: the host draws this plate, so it also does
/// its own hit-testing.
fn restart_button(bounds: Rectangle) -> Rectangle {
    Rectangle {
        x: bounds.x + 24.0,
        y: bounds.y + bounds.height / 2.0 + 28.0,
        width: 84.0,
        height: 26.0,
    }
}

/// The one guest the keyboard goes to, by its serial.
static FOCUS: Mutex<Option<u64>> = Mutex::new(None);

fn focus(serial: u64, over: bool) {
    let mut focus = FOCUS.lock().expect("focus");
    match over {
        true => *focus = Some(serial),
        false if *focus == Some(serial) => *focus = None,
        false => {}
    }
}

fn focused(serial: u64) -> bool {
    *FOCUS.lock().expect("focus") == Some(serial)
}

fn release_focus(serial: u64) {
    focus(serial, false);
}

/// One 16px line of host text, anchored at its top-left corner.
fn small_text<Renderer>(
    renderer: &mut Renderer,
    content: String,
    top_left: Point,
    color: Color,
    clip: Rectangle,
) where
    Renderer: core_text::Renderer<Font = iced::Font>,
{
    renderer.fill_text(
        core_text::Text {
            content,
            bounds: Size::new(clip.width - 48.0, 16.0),
            size: Pixels(11.0),
            line_height: LineHeight::Absolute(Pixels(16.0)),
            font: iced::Font::DEFAULT,
            align_x: core_text::Alignment::Left,
            align_y: iced::alignment::Vertical::Top,
            shaping: Shaping::Advanced,
            wrapping: Wrapping::None,
        },
        top_left,
        color,
        clip,
    );
}

fn replay_layer<Renderer>(renderer: &mut Renderer, layer: &wire::Layer)
where
    Renderer: core_text::Renderer<Font = iced::Font>,
{
    for quad in &layer.quads {
        renderer.fill_quad(
            renderer::Quad {
                bounds: rect(quad.bounds),
                border: Border {
                    color: color(quad.border_color),
                    width: quad.border_width,
                    radius: iced::border::Radius {
                        top_left: quad.radius[0],
                        top_right: quad.radius[1],
                        bottom_right: quad.radius[2],
                        bottom_left: quad.radius[3],
                    },
                },
                shadow: Shadow {
                    color: color(quad.shadow_color),
                    offset: Vector::new(quad.shadow_offset[0], quad.shadow_offset[1]),
                    blur_radius: quad.shadow_blur,
                },
                snap: quad.snap,
            },
            color(quad.background),
        );
    }
    for text in &layer.texts {
        // The recorded anchor is honoured by moving the top-left corner, not
        // by asking the renderer to align: an aligned text is damaged over
        // the whole width it was given.
        let bounds = Size::new(f32::INFINITY, text.line_height);
        let shaped = core_text::Text {
            content: text.content.as_str(),
            bounds,
            size: Pixels(text.size),
            line_height: LineHeight::Absolute(Pixels(text.line_height)),
            font: font(&text.font),
            align_x: core_text::Alignment::Left,
            align_y: iced::alignment::Vertical::Top,
            shaping: Shaping::Advanced,
            wrapping: Wrapping::None,
        };
        let x = match text.anchor.x {
            wire::AlignX::Left => text.x,
            wire::AlignX::Center | wire::AlignX::Right => {
                let width = Renderer::Paragraph::with_text(shaped).min_width();
                match text.anchor.x {
                    wire::AlignX::Center => text.x - width / 2.0,
                    _ => text.x - width,
                }
            }
        };
        let y = match text.anchor.y {
            wire::AlignY::Top => text.y,
            wire::AlignY::Center => text.y - text.line_height / 2.0,
            wire::AlignY::Bottom => text.y - text.line_height,
        };
        let clip = rect(text.clip);
        renderer.fill_text(
            core_text::Text {
                content: text.content.clone(),
                // Finite on purpose: an infinite width through the layer's
                // transformation is a NaN, and a NaN is never equal to
                // itself, so every text would count as changed every frame.
                bounds: Size::new((clip.x + clip.width - x).max(0.0), text.line_height),
                size: Pixels(text.size),
                line_height: LineHeight::Absolute(Pixels(text.line_height)),
                font: font(&text.font),
                align_x: core_text::Alignment::Left,
                align_y: iced::alignment::Vertical::Top,
                shaping: Shaping::Advanced,
                wrapping: Wrapping::None,
            },
            Point::new(x, y),
            color(text.color),
            clip,
        );
    }
}

fn rect(r: wire::Rect) -> Rectangle {
    Rectangle {
        x: r[0],
        y: r[1],
        width: r[2],
        height: r[3],
    }
}

fn color(c: wire::Rgba) -> Color {
    Color {
        r: c[0],
        g: c[1],
        b: c[2],
        a: c[3],
    }
}

fn wire_button(button: mouse::Button) -> Option<wire::Button> {
    match button {
        mouse::Button::Left => Some(wire::Button::Left),
        mouse::Button::Right => Some(wire::Button::Right),
        mouse::Button::Middle => Some(wire::Button::Middle),
        _ => None,
    }
}

fn wire_key(key: &keyboard::Key) -> wire::Key {
    use keyboard::key::Named;
    match key {
        keyboard::Key::Character(text) => wire::Key::Character(text.to_string()),
        keyboard::Key::Named(named) => match named {
            Named::Enter => wire::Key::Enter,
            Named::Tab => wire::Key::Tab,
            Named::Space => wire::Key::Space,
            Named::Backspace => wire::Key::Backspace,
            Named::Delete => wire::Key::Delete,
            Named::Escape => wire::Key::Escape,
            Named::ArrowUp => wire::Key::ArrowUp,
            Named::ArrowDown => wire::Key::ArrowDown,
            Named::ArrowLeft => wire::Key::ArrowLeft,
            Named::ArrowRight => wire::Key::ArrowRight,
            Named::Home => wire::Key::Home,
            Named::End => wire::Key::End,
            Named::PageUp => wire::Key::PageUp,
            Named::PageDown => wire::Key::PageDown,
            Named::Shift => wire::Key::Shift,
            Named::Control => wire::Key::Control,
            Named::Alt => wire::Key::Alt,
            Named::Super => wire::Key::Super,
            _ => wire::Key::Unidentified,
        },
        keyboard::Key::Unidentified => wire::Key::Unidentified,
    }
}

#[cfg(test)]
mod tests {
    //! The committed governance view through this host: the wire both sides
    //! were built on agrees, the module boots inside the budget, its first
    //! frame queues the query the widget must carry, and the module's reply
    //! comes back as rows in the next frame.

    use super::*;

    /// The artifact `make wasm-views` commits; a test pins bytes on purpose.
    const VIEW: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../crates/modules/system/governance/view.wasm"
    );

    fn texts(frame: &wire::Frame) -> Vec<&str> {
        frame
            .layers
            .iter()
            .flat_map(|layer| layer.texts.iter().map(|text| text.content.as_str()))
            .collect()
    }

    fn booted() -> Guest {
        let mut guest =
            Guest::load("governance", std::path::Path::new(VIEW)).expect("the view loads");
        guest.size = Size::new(960.0, 640.0);
        guest.pending.push(wire::Event::Resized {
            width: 960.0,
            height: 640.0,
        });
        guest.set_theme(false);
        guest.set_generation(0);
        guest.redraw(Instant::now());
        guest
    }

    #[test]
    fn the_committed_view_boots_and_asks_the_node_for_the_proposals() {
        let guest = booted();
        assert_eq!(guest.fault, None);
        assert!(guest.asked, "the first frame queues a query");
        let [query] = guest.queries.as_slice() else {
            panic!("one query at boot, got {:?}", guest.queries);
        };
        assert_eq!(query.module, "governance");
        assert_eq!(query.payload, b"\"proposals\"");
        assert_eq!(guest.theme_subscriptions.len(), 1, "the mode is subscribed");
        assert_eq!(
            guest.refresh_subscriptions.len(),
            1,
            "refreshes are subscribed"
        );
        assert!(
            texts(&guest.frame).contains(&"Approvals"),
            "{:?}",
            texts(&guest.frame)
        );
    }

    #[test]
    fn the_nodes_reply_lands_as_rows_and_a_moved_generation_asks_again() {
        let mut guest = booted();
        let query = guest.queries.remove(0);
        let reply = br#"{"proposals":[{"proposal_id":"p1","status":"open",
            "action":{"signal":{"text":"Upgrade the forge"}},"votes":[["a",true]],
            "deadline":120,"electorate":["a","b"],
            "voting_rule":{"threshold":{"required_yes":2}}}]}"#;
        guest.inbox.push(one_shot(query.id, Ok(reply.to_vec())));
        guest.redraw(Instant::now());
        assert_eq!(guest.fault, None);
        let shown = texts(&guest.frame);
        assert!(shown.contains(&"Upgrade the forge"), "{shown:?}");
        assert!(shown.contains(&"1 open · 0 settled"), "{shown:?}");
        assert!(guest.queries.is_empty(), "{:?}", guest.queries);

        guest.set_generation(1);
        guest.redraw(Instant::now());
        assert_eq!(
            guest.queries.len(),
            1,
            "a moved generation is one more query"
        );
    }

    /// The whole round trip under a real node: the guest's query goes out
    /// through the widget's Ask, `serve_wasm_view` carries it over `/v1/query`
    /// to a simnode whose governance module answers, and the next frame draws
    /// what it said — an empty register on a fresh network.
    #[tokio::test(flavor = "current_thread")]
    async fn the_query_round_trips_through_a_live_node() {
        let storage = tempfile::tempdir().unwrap();
        let sim = simnode::boot(
            storage.path(),
            "127.0.0.1:0".parse().unwrap(),
            simnode::SimOpts {
                auto: true,
                valset_keys: vec![vec![0x11; 32]],
                ..Default::default()
            },
        )
        .unwrap();
        let origin = format!("http://{}", sim.addr());
        let surface = WasmSurface(Arc::new(Mutex::new(booted())));
        let served = serve_wasm_view(Some(surface.clone()), WasmViewEvent(Verb::Ask), origin)
            .await
            .expect("the node answers the query");
        assert_eq!(served, Some(surface.clone()));
        let mut guest = surface.0.lock().expect("guest lock");
        assert!(guest.queries.is_empty(), "{:?}", guest.queries);
        guest.redraw(Instant::now());
        assert_eq!(guest.fault, None);
        let shown = texts(&guest.frame);
        assert!(shown.contains(&"0 open · 0 settled"), "{shown:?}");
        assert!(shown.contains(&"Nothing to decide."), "{shown:?}");
        drop(guest);
        sim.shutdown();
    }
}

thread_local! {
    /// iced fonts name families by `&'static str`; each distinct family the
    /// guest names is interned once rather than leaked per frame.
    static FAMILIES: RefCell<HashMap<String, &'static str>> = RefCell::new(HashMap::new());
}

/// Interning leaks, so the guest may not name families forever.
const MAX_FAMILIES: usize = 64;

fn font(font: &wire::Font) -> iced::Font {
    use iced::font::{Family, Style, Weight};
    let family = match &font.family {
        Some(name) => FAMILIES.with(|families| {
            let mut families = families.borrow_mut();
            if let Some(interned) = families.get(name.as_str()) {
                return Family::Name(interned);
            }
            if families.len() >= MAX_FAMILIES {
                return Family::SansSerif;
            }
            let interned: &'static str = Box::leak(name.clone().into_boxed_str());
            families.insert(name.clone(), interned);
            Family::Name(interned)
        }),
        None if font.monospace => Family::Monospace,
        None => Family::SansSerif,
    };
    let weight = match font.weight {
        0..=149 => Weight::Thin,
        150..=249 => Weight::ExtraLight,
        250..=349 => Weight::Light,
        350..=449 => Weight::Normal,
        450..=549 => Weight::Medium,
        550..=649 => Weight::Semibold,
        650..=749 => Weight::Bold,
        750..=849 => Weight::ExtraBold,
        _ => Weight::Black,
    };
    iced::Font {
        family,
        weight,
        style: if font.italic {
            Style::Italic
        } else {
            Style::Normal
        },
        ..iced::Font::DEFAULT
    }
}
