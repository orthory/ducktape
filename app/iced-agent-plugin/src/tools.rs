//! Command execution: resolves each [`Cmd`] against the shared bridge context.
//! Read tools serve the stored snapshot/state; drive tools translate targets to
//! synthetic iced events injected through the fork; screenshot/intent tools
//! hand work to the app via the UI-command queue.

use std::time::{Duration, Instant};

use base64::Engine;
use serde_json::{json, Value};

use iced::keyboard::key::{NativeCode, Physical};
use iced::{keyboard, mouse, Event, Point};

use crate::bridge::{Shared, UiCommand};
use crate::collect::FlatNode;
use crate::protocol::{window_or_main, Cmd, Cond, Target};

/// Runs a command against the shared context.
pub async fn execute(cmd: Cmd, shared: &Shared) -> Result<Value, String> {
    match cmd {
        Cmd::Tree { window } => tree(shared, &window),
        Cmd::Find {
            window,
            role,
            name,
            text,
        } => find(shared, &window, role, name, text),
        Cmd::Click { target } => {
            let (id, x, y) = resolve_target(shared, &target)?;
            inject(id, cursor(x, y));
            inject(id, Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)));
            inject(id, Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)));
            Ok(json!({ "clicked": describe(&target) }))
        }
        Cmd::Hover { target } => {
            let (id, x, y) = resolve_target(shared, &target)?;
            inject(id, cursor(x, y));
            Ok(json!({ "hovered": describe(&target) }))
        }
        Cmd::Scroll { target, dx, dy } => {
            let (id, x, y) = resolve_target(shared, &target)?;
            inject(id, cursor(x, y));
            inject(
                id,
                Event::Mouse(mouse::Event::WheelScrolled {
                    delta: mouse::ScrollDelta::Pixels { x: dx, y: dy },
                }),
            );
            Ok(json!({ "scrolled": [dx, dy] }))
        }
        Cmd::Drag { from, to } => {
            let (id, fx, fy) = resolve_target(shared, &from)?;
            let (_to_id, tx, ty) = resolve_target(shared, &to)?;
            inject(id, cursor(fx, fy));
            inject(id, Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)));
            const STEPS: u32 = 8;
            for step in 1..=STEPS {
                let t = step as f32 / STEPS as f32;
                inject(id, cursor(fx + (tx - fx) * t, fy + (ty - fy) * t));
            }
            inject(id, Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)));
            Ok(json!({ "dragged": [describe(&from), describe(&to)] }))
        }
        Cmd::Type { text } => {
            let id = default_window(shared)?;
            for ch in text.chars() {
                let s = ch.to_string();
                let key = keyboard::Key::Character(s.clone().into());
                inject(
                    id,
                    Event::Keyboard(keyboard::Event::KeyPressed {
                        key: key.clone(),
                        modified_key: key.clone(),
                        physical_key: unidentified(),
                        location: keyboard::Location::Standard,
                        modifiers: keyboard::Modifiers::empty(),
                        text: Some(s.into()),
                        repeat: false,
                    }),
                );
                inject(
                    id,
                    Event::Keyboard(keyboard::Event::KeyReleased {
                        key: key.clone(),
                        modified_key: key,
                        physical_key: unidentified(),
                        location: keyboard::Location::Standard,
                        modifiers: keyboard::Modifiers::empty(),
                    }),
                );
            }
            Ok(json!({ "typed": text.chars().count() }))
        }
        Cmd::Press { key, modifiers } => {
            // Named keys (enter/tab/escape/…) or a single character chord
            // (e.g. `press k --mod ctrl` for the search palette).
            let k = match named_key(&key) {
                Some(named) => keyboard::Key::Named(named),
                None if key.chars().count() == 1 => {
                    keyboard::Key::Character(key.to_lowercase().into())
                }
                None => return Err(format!("unknown key '{key}'")),
            };
            let mods = parse_modifiers(&modifiers);
            let id = default_window(shared)?;
            inject(
                id,
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: k.clone(),
                    modified_key: k.clone(),
                    physical_key: unidentified(),
                    location: keyboard::Location::Standard,
                    modifiers: mods,
                    text: None,
                    repeat: false,
                }),
            );
            inject(
                id,
                Event::Keyboard(keyboard::Event::KeyReleased {
                    key: k.clone(),
                    modified_key: k,
                    physical_key: unidentified(),
                    location: keyboard::Location::Standard,
                    modifiers: mods,
                }),
            );
            Ok(json!({ "pressed": key }))
        }
        Cmd::State { path } => {
            let state = shared.state.lock().map_err(|_| "state poisoned")?;
            match &path {
                None => Ok(state.clone()),
                Some(path) => Ok(dig(&state, path).cloned().unwrap_or(Value::Null)),
            }
        }
        Cmd::Intent { intent } => {
            shared.push_ui(UiCommand::Intent(intent));
            Ok(json!({ "queued": true }))
        }
        Cmd::Shot { window } => {
            let window = window_or_main(&window).to_string();
            let (reply, rx) = tokio::sync::oneshot::channel();
            shared.push_ui(UiCommand::Shot { window, reply });
            match tokio::time::timeout(Duration::from_secs(5), rx).await {
                Ok(Ok(png)) => Ok(json!({
                    "png_base64": base64::engine::general_purpose::STANDARD.encode(&png),
                    "bytes": png.len(),
                })),
                Ok(Err(_)) => Err("screenshot reply dropped".into()),
                Err(_) => Err("screenshot timed out".into()),
            }
        }
        Cmd::Logs { clear } => {
            let lines = shared.logs.snapshot();
            if clear {
                shared.logs.clear();
            }
            Ok(json!({ "lines": lines }))
        }
        Cmd::Wait { cond, timeout_ms } => {
            let timeout = timeout_ms.unwrap_or(5000);
            let start = Instant::now();
            loop {
                if eval_cond(shared, &cond) {
                    return Ok(json!({ "pass": true, "waited_ms": start.elapsed().as_millis() }));
                }
                if start.elapsed().as_millis() as u64 >= timeout {
                    return Ok(json!({ "pass": false, "waited_ms": start.elapsed().as_millis() }));
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
        Cmd::Expect { cond } => Ok(json!({ "pass": eval_cond(shared, &cond) })),
        Cmd::Windows => {
            let snaps = shared.snapshot.lock().map_err(|_| "snapshot poisoned")?;
            let windows: Vec<Value> = snaps
                .iter()
                .map(|s| json!({ "name": s.window_name, "bounds": s.nodes.bounds }))
                .collect();
            let mapped: Vec<String> = shared
                .window_map
                .lock()
                .map(|m| m.keys().cloned().collect())
                .unwrap_or_default();
            Ok(json!({ "windows": windows, "mapped": mapped }))
        }
        Cmd::A11y { window } => {
            let want = window_or_main(&window);
            let id = window_id(shared, want)?;
            match iced_winit::agent::last_tree(id) {
                Some(update) => {
                    let nodes: Vec<(u64, String)> = update
                        .nodes
                        .iter()
                        .map(|(nid, node)| (nid.0, format!("{node:?}")))
                        .collect();
                    Ok(json!({ "nodes": nodes, "focus": update.focus.0 }))
                }
                None => Ok(json!({ "nodes": [], "focus": Value::Null })),
            }
        }
    }
}

fn tree(shared: &Shared, window: &Option<String>) -> Result<Value, String> {
    let want = window_or_main(window);
    let snaps = shared.snapshot.lock().map_err(|_| "snapshot poisoned")?;
    match snaps.iter().find(|s| s.window_name == want) {
        Some(snap) => serde_json::to_value(&snap.nodes).map_err(|e| e.to_string()),
        None => Err(format!("window '{want}' has no snapshot yet")),
    }
}

fn find(
    shared: &Shared,
    window: &Option<String>,
    role: Option<crate::protocol::Role>,
    name: Option<String>,
    text: Option<String>,
) -> Result<Value, String> {
    let want = window_or_main(window);
    let snaps = shared.snapshot.lock().map_err(|_| "snapshot poisoned")?;
    let Some(snap) = snaps.iter().find(|s| s.window_name == want) else {
        return Err(format!("window '{want}' has no snapshot yet"));
    };
    let needle = name.or(text).map(|s| s.to_lowercase());
    let matches: Vec<&FlatNode> = snap
        .flat
        .iter()
        .filter(|n| {
            role.is_none_or(|r| r == n.role)
                && needle
                    .as_ref()
                    .is_none_or(|want| n.name.to_lowercase().contains(want))
        })
        .collect();
    Ok(json!({ "matches": matches }))
}

fn eval_cond(shared: &Shared, cond: &Cond) -> bool {
    match cond {
        Cond::Node { role, name, exists } => {
            let found = shared
                .snapshot
                .lock()
                .map(|snaps| {
                    snaps.iter().flat_map(|s| s.flat.iter()).any(|n| {
                        role.is_none_or(|r| r == n.role)
                            && name
                                .as_ref()
                                .is_none_or(|w| n.name.to_lowercase().contains(&w.to_lowercase()))
                    })
                })
                .unwrap_or(false);
            found == *exists
        }
        Cond::StatePath { path, equals } => shared
            .state
            .lock()
            .map(|state| dig(&state, path).map(|v| v == equals).unwrap_or(false))
            .unwrap_or(false),
    }
}

/// Walks a dot-path into a JSON value, indexing arrays by numeric segment.
fn dig<'a>(mut value: &'a Value, path: &str) -> Option<&'a Value> {
    for seg in path.split('.').filter(|s| !s.is_empty()) {
        value = value
            .get(seg)
            .or_else(|| seg.parse::<usize>().ok().and_then(|i| value.get(i)))?;
    }
    Some(value)
}

fn cursor(x: f32, y: f32) -> Event {
    Event::Mouse(mouse::Event::CursorMoved {
        position: Point::new(x, y),
    })
}

fn inject(id: iced::window::Id, event: Event) {
    iced_winit::agent::inject(id, event);
}

fn unidentified() -> Physical {
    Physical::Unidentified(NativeCode::Unidentified)
}

fn describe(target: &Target) -> String {
    match &target.r#ref {
        Some(r) => r.clone(),
        None => format!("({}, {})", target.x.unwrap_or(0.0), target.y.unwrap_or(0.0)),
    }
}

/// Resolves a target to a window id and window-space point. `@ref` looks up the
/// node across all window snapshots (its bounds center); raw x/y targets the
/// main window.
fn resolve_target(shared: &Shared, target: &Target) -> Result<(iced::window::Id, f32, f32), String> {
    if let Some(r) = &target.r#ref {
        let snaps = shared.snapshot.lock().map_err(|_| "snapshot poisoned")?;
        for snap in snaps.iter() {
            if let Some(node) = snap.flat.iter().find(|n| &n.r#ref == r) {
                let (cx, cy) = node.bounds.center();
                let window = snap.window_name.clone();
                drop(snaps);
                return Ok((window_id(shared, &window)?, cx, cy));
            }
        }
        return Err(format!("ref {r} not found in current snapshot"));
    }
    match (target.x, target.y) {
        (Some(x), Some(y)) => Ok((default_window(shared)?, x, y)),
        _ => Err("target needs a ref or both x and y".into()),
    }
}

fn window_id(shared: &Shared, name: &str) -> Result<iced::window::Id, String> {
    shared
        .window_map
        .lock()
        .map_err(|_| "window map poisoned".to_string())?
        .get(name)
        .copied()
        .ok_or_else(|| format!("window '{name}' is not open"))
}

fn default_window(shared: &Shared) -> Result<iced::window::Id, String> {
    let map = shared.window_map.lock().map_err(|_| "window map poisoned")?;
    if let Some(id) = map.get("main") {
        return Ok(*id);
    }
    map.values()
        .next()
        .copied()
        .ok_or_else(|| "no windows are open".to_string())
}

fn named_key(name: &str) -> Option<keyboard::key::Named> {
    use keyboard::key::Named;
    Some(match name.to_ascii_lowercase().as_str() {
        "enter" | "return" => Named::Enter,
        "tab" => Named::Tab,
        "escape" | "esc" => Named::Escape,
        "backspace" => Named::Backspace,
        "delete" | "del" => Named::Delete,
        "space" => Named::Space,
        "up" | "arrowup" => Named::ArrowUp,
        "down" | "arrowdown" => Named::ArrowDown,
        "left" | "arrowleft" => Named::ArrowLeft,
        "right" | "arrowright" => Named::ArrowRight,
        "home" => Named::Home,
        "end" => Named::End,
        "pageup" => Named::PageUp,
        "pagedown" => Named::PageDown,
        _ => return None,
    })
}

fn parse_modifiers(list: &Option<Vec<String>>) -> keyboard::Modifiers {
    let mut mods = keyboard::Modifiers::empty();
    let Some(list) = list else {
        return mods;
    };
    for key in list {
        match key.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => mods |= keyboard::Modifiers::CTRL,
            "shift" => mods |= keyboard::Modifiers::SHIFT,
            "alt" | "option" => mods |= keyboard::Modifiers::ALT,
            "cmd" | "command" | "logo" | "super" | "meta" | "win" => {
                mods |= keyboard::Modifiers::LOGO
            }
            _ => {}
        }
    }
    mods
}
