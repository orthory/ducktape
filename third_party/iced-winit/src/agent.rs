//! AGENT SEAM: dev-only agent instrumentation for the iced event loop.
//!
//! Three seams: (1) an AccessKit adapter attached to every window at creation,
//! (2) [`set_tree`] pushing semantic tree updates into that adapter, and
//! (3) [`inject`] feeding synthetic iced-core events into the same runtime
//! path real input takes. Everything is process-global because the event loop
//! owns the windows and the app-side plugin only holds `window::Id`s.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, OnceLock};

use crate::core::window::Id;

/// Per-window adapter + the winit window it belongs to.
///
/// The window is held weakly: a strong clone here would keep the OS window
/// alive past close, so `WindowEvent::Destroyed` (our cleanup signal) would
/// never fire.
struct Slot {
    adapter: accesskit_winit::Adapter,
    window: std::sync::Weak<winit::window::Window>,
    /// Last tree pushed, replayed on activation and dumpable for QA.
    last_tree: Arc<Mutex<Option<accesskit::TreeUpdate>>>,
}

struct Globals {
    slots: Mutex<HashMap<Id, Slot>>,
    winit_to_iced: Mutex<HashMap<winit::window::WindowId, Id>>,
    injected: Mutex<Vec<(Id, crate::core::Event)>>,
    injected_flag: AtomicBool,
    actions_tx: Sender<(Id, accesskit::ActionRequest)>,
    actions_rx: Mutex<Option<Receiver<(Id, accesskit::ActionRequest)>>>,
}

fn globals() -> &'static Globals {
    static G: OnceLock<Globals> = OnceLock::new();
    G.get_or_init(|| {
        let (tx, rx) = channel();
        Globals {
            slots: Mutex::new(HashMap::new()),
            winit_to_iced: Mutex::new(HashMap::new()),
            injected: Mutex::new(Vec::new()),
            injected_flag: AtomicBool::new(false),
            actions_tx: tx,
            actions_rx: Mutex::new(Some(rx)),
        }
    })
}

/// The platform adapters' async plumbing (zbus on Linux) may require an
/// ambient Tokio reactor; the winit event-loop thread has none, so the agent
/// owns a tiny one and enters it around every adapter interaction.
fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .thread_name("iced-agent-a11y")
            .enable_all()
            .build()
            .expect("build agent tokio runtime")
    })
}

/// Replays the last pushed tree when the platform activates accessibility.
struct Activation {
    last_tree: Arc<Mutex<Option<accesskit::TreeUpdate>>>,
}

impl accesskit::ActivationHandler for Activation {
    fn request_initial_tree(&mut self) -> Option<accesskit::TreeUpdate> {
        self.last_tree.lock().unwrap().clone()
    }
}

struct Actions {
    id: Id,
    tx: Sender<(Id, accesskit::ActionRequest)>,
}

impl accesskit::ActionHandler for Actions {
    fn do_action(&mut self, request: accesskit::ActionRequest) {
        let _ = self.tx.send((self.id, request));
    }
}

struct Deactivation;

impl accesskit::DeactivationHandler for Deactivation {
    fn deactivate_accessibility(&mut self) {}
}

/// Seam 1: called by the event loop right after `create_window`, while the
/// window is still invisible (the adapter requires pre-visibility creation).
pub(crate) fn attach(
    event_loop: &winit::event_loop::ActiveEventLoop,
    id: Id,
    window: &Arc<winit::window::Window>,
) {
    let g = globals();
    log::info!("agent: attaching AccessKit adapter to window {id:?}");
    let last_tree = Arc::new(Mutex::new(None));
    let _reactor = runtime().enter();
    let adapter = accesskit_winit::Adapter::with_direct_handlers(
        event_loop,
        window,
        Activation {
            last_tree: Arc::clone(&last_tree),
        },
        Actions {
            id,
            tx: g.actions_tx.clone(),
        },
        Deactivation,
    );
    let _ = g.winit_to_iced.lock().unwrap().insert(window.id(), id);
    let _ = g.slots.lock().unwrap().insert(
        id,
        Slot {
            adapter,
            window: Arc::downgrade(window),
            last_tree,
        },
    );
}

/// Seam 1: forward every winit window event to the window's adapter.
pub(crate) fn process_event(
    winit_id: winit::window::WindowId,
    event: &winit::event::WindowEvent,
) {
    let g = globals();
    let Some(id) = g.winit_to_iced.lock().unwrap().get(&winit_id).copied() else {
        return;
    };
    if let Some(slot) = g.slots.lock().unwrap().get_mut(&id) {
        if let Some(window) = slot.window.upgrade() {
            let _reactor = runtime().enter();
            slot.adapter.process_event(&window, event);
        }
    }
    if matches!(event, winit::event::WindowEvent::Destroyed) {
        let _ = g.slots.lock().unwrap().remove(&id);
        let _ = g.winit_to_iced.lock().unwrap().remove(&winit_id);
    }
}

/// Seam 2: push a semantic tree for a window (app-side plugin calls this).
pub fn set_tree(id: Id, update: accesskit::TreeUpdate) {
    let g = globals();
    if let Some(slot) = g.slots.lock().unwrap().get_mut(&id) {
        *slot.last_tree.lock().unwrap() = Some(update.clone());
        let _reactor = runtime().enter();
        slot.adapter.update_if_active(|| update);
    }
}

/// Seam 2: the plugin takes the (single) ActionRequest receiver at boot.
pub fn take_action_rx() -> Option<Receiver<(Id, accesskit::ActionRequest)>> {
    globals().actions_rx.lock().unwrap().take()
}

/// Dump the last tree pushed for a window (the `iced_a11y` tool).
pub fn last_tree(id: Id) -> Option<accesskit::TreeUpdate> {
    globals()
        .slots
        .lock()
        .unwrap()
        .get(&id)
        .and_then(|slot| slot.last_tree.lock().unwrap().clone())
}

/// Seam 3: queue a synthetic iced-core event; the loop drains it into the
/// same per-window event vector real input lands in, then redraws.
pub fn inject(id: Id, event: crate::core::Event) {
    let g = globals();
    g.injected.lock().unwrap().push((id, event));
    g.injected_flag.store(true, Ordering::Release);
    if let Some(slot) = g.slots.lock().unwrap().get(&id) {
        if let Some(window) = slot.window.upgrade() {
            window.request_redraw();
        }
    }
}

/// Seam 3: drained by the event loop each cycle.
pub(crate) fn drain_injected() -> Vec<(Id, crate::core::Event)> {
    let g = globals();
    if !g.injected_flag.swap(false, Ordering::AcqRel) {
        return Vec::new();
    }
    std::mem::take(&mut *g.injected.lock().unwrap())
}

/// Windows currently alive (the `iced_windows` tool).
pub fn window_ids() -> Vec<Id> {
    globals().slots.lock().unwrap().keys().copied().collect()
}
