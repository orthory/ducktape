//! Dev-only wiring between the shell and the iced-agent-plugin bridge.
//!
//! Compiled only under `all(feature = "agent", debug_assertions)` (gated at
//! the `mod` site). Owns the bridge handle, the 150 ms snapshot loop, the
//! curated intent mapping, and the AccessKit action routing — `shell.rs`
//! carries only thin hooks into this module.

use std::sync::{Mutex, OnceLock};

use iced_agent_plugin::{
    AgentHandle, Collector, Intent, LogsHandle, Role, UiCommand, sem, to_accesskit,
};

use super::*;

/// Set by `crate::run()` when it installs the ring layer, read at boot.
pub(crate) static LOGS: OnceLock<LogsHandle> = OnceLock::new();

pub(super) struct Runtime {
    handle: AgentHandle,
}

pub(super) fn boot() -> Runtime {
    let logs = LOGS
        .get()
        .cloned()
        .unwrap_or_else(|| iced_agent_plugin::ring_layer().1);
    #[cfg(feature = "cef-browser")]
    let cdp = crate::browser::agent_cdp_port().map(|port| format!("http://127.0.0.1:{port}"));
    #[cfg(not(feature = "cef-browser"))]
    let cdp = None;
    let handle = AgentHandle::boot_with_cdp("com.ducktape.app", logs, cdp);

    // Route AccessKit ActionRequests (screen-reader clicks) through the same
    // synthetic-input path the bridge uses.
    if let Some(rx) = iced_winit::agent::take_action_rx() {
        let snapshot = handle.snapshot_slot();
        std::thread::spawn(move || {
            while let Ok((window, request)) = rx.recv() {
                if request.action == iced_winit::accesskit::Action::Click {
                    click_node(&snapshot, window, request.target_node.0);
                }
            }
        });
    }

    Runtime { handle }
}

pub(super) fn window_opened(state: &Shell, name: &str, id: window::Id) {
    if let Some(agent) = state.agent.as_ref() {
        let _ = agent
            .handle
            .window_map()
            .lock()
            .unwrap()
            .insert(name.to_owned(), id);
    }
}

/// One agent beat: publish the previous collection, refresh the curated state
/// projection, drain bridge commands, and schedule the next tree collection.
pub(super) fn tick(state: &mut Shell) -> Task<Message> {
    let Some(agent) = state.agent.as_ref() else {
        return Task::none();
    };
    let handle = &agent.handle;

    // Publish the snapshots collected on the previous beat.
    {
        let map = handle.window_map().lock().unwrap().clone();
        let snapshots = handle.snapshot_slot().lock().unwrap().clone();
        for snapshot in &snapshots {
            if let Some(id) = map.get(&snapshot.window_name) {
                iced_winit::agent::set_tree(*id, to_accesskit(snapshot));
            }
        }
    }

    // Curated state projection: reviewed fields only, never key material.
    *handle.state_slot().lock().unwrap() = serde_json::json!({
        "screen": format!("{:?}", state.screen()).to_lowercase(),
        "section": format!("{:?}", state.section).to_lowercase(),
        "history_len": state.history.len(),
        "has_workspace": state.active_workspace.is_some(),
        "unread": state.notifications.unread,
        "search_open": state.search.open,
        "quitting": state.quitting,
    });

    let mut tasks = vec![];
    for command in handle.drain_ui() {
        match command {
            UiCommand::Intent(intent) => tasks.push(apply_intent(intent)),
            UiCommand::Shot { window, reply } => {
                let windows = handle.window_map();
                let id = windows
                    .lock()
                    .unwrap()
                    .get(&window)
                    .copied()
                    .or(state.desktop.main);
                if let Some(id) = id {
                    let reply = Mutex::new(Some(reply));
                    tasks.push(iced::window::screenshot(id).then(move |shot| {
                        if let Some(reply) = reply.lock().unwrap().take() {
                            let png = encode_png(&shot);
                            // A dropped reply surfaces as a bridge error;
                            // never answer success with an empty image.
                            if !png.is_empty() {
                                let _ = reply.send(png);
                            }
                        }
                        Task::none()
                    }));
                }
            }
        }
    }

    // Collect a fresh tree; it publishes on the next beat (150 ms staleness
    // is well under interactive tolerances and avoids task chaining).
    tasks.push(
        iced::advanced::widget::operate(Collector::new(handle.snapshot_slot())).discard(),
    );
    Task::batch(tasks)
}

/// Wrap a window's whole content in its root `sem` node; the node name is the
/// window key every tool's `window` parameter resolves against.
pub(super) fn root(kind: desktop::Kind, content: Element<'_, Message>) -> Element<'_, Message> {
    let name = match kind {
        desktop::Kind::Main => "main",
        desktop::Kind::Huddle => "huddle",
        desktop::Kind::Tray => "tray",
    };
    sem(Role::Window, name, content)
}

fn apply_intent(intent: Intent) -> Task<Message> {
    match intent {
        Intent::Section { name } => match name.to_lowercase().as_str() {
            "user" => Task::done(Message::Section(Section::User)),
            "operator" => Task::done(Message::Section(Section::Operator)),
            _ => Task::none(),
        },
        Intent::Navigate { url } => screen_by_name(&url)
            .map(|screen| Task::done(Message::Navigate(screen)))
            .unwrap_or_else(Task::none),
        Intent::ToggleTheme => Task::done(Message::ToggleTheme),
        // v1 opens the palette; the query is typed via synthetic input.
        Intent::Search { query: _ } => Task::done(Message::ToggleSearch),
    }
}

fn screen_by_name(name: &str) -> Option<Screen> {
    let name = name.trim().trim_start_matches("duck://");
    Some(match name.to_lowercase().as_str() {
        "home" => Screen::Home,
        "chat" => Screen::Chat,
        "pages" => Screen::Pages,
        "files" => Screen::Files,
        "browser" => Screen::Browser,
        "forge" => Screen::Forge,
        "agents" => Screen::Agents,
        "members" => Screen::Members,
        "governance" => Screen::Governance,
        "explorer" => Screen::Explorer,
        "node" => Screen::Node,
        "gateway" => Screen::Gateway,
        "modules" => Screen::Modules,
        "sandbox" => Screen::Sandbox,
        "terminal" => Screen::Terminal,
        "metrics" => Screen::Metrics,
        "settings" => Screen::Settings,
        _ => return None,
    })
}

/// AccessKit Click on `@node_id` → the same synthetic sequence as the bridge.
fn click_node(
    snapshot: &iced_agent_plugin::SnapshotSlot,
    window: window::Id,
    node_id: u64,
) {
    let reference = format!("@{node_id}");
    let bounds = snapshot
        .lock()
        .unwrap()
        .iter()
        .flat_map(|snap| snap.flat.iter())
        .find(|node| node.r#ref == reference)
        .map(|node| node.bounds);
    let Some(bounds) = bounds else { return };
    let position = iced::Point::new(
        bounds.x + bounds.width / 2.0,
        bounds.y + bounds.height / 2.0,
    );
    use iced::mouse;
    iced_winit::agent::inject(
        window,
        iced::Event::Mouse(mouse::Event::CursorMoved { position }),
    );
    iced_winit::agent::inject(
        window,
        iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
    );
    iced_winit::agent::inject(
        window,
        iced::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
    );
}

fn encode_png(shot: &iced::window::Screenshot) -> Vec<u8> {
    use image::ImageEncoder as _;
    let mut out = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut out);
    if encoder
        .write_image(
            shot.rgba.as_ref(),
            shot.size.width,
            shot.size.height,
            image::ExtendedColorType::Rgba8,
        )
        .is_err()
    {
        out.clear();
    }
    out
}
