//! Native window lifecycle shared by the iced shell.

use iced::{Point, Size, window};

pub const MAIN_SIZE: Size = Size::new(1280.0, 800.0);
pub const MAIN_MIN_SIZE: Size = Size::new(900.0, 600.0);
pub const HUDDLE_SIZE: Size = Size::new(380.0, 300.0);
pub const HUDDLE_MIN_SIZE: Size = Size::new(300.0, 220.0);
pub const TRAY_SIZE: Size = Size::new(430.0, 460.0);

#[derive(Debug, Default)]
pub struct State {
    pub main: Option<window::Id>,
    pub huddle: Option<window::Id>,
    pub tray: Option<window::Id>,
    pub main_focused: bool,
    pub tray_hidden_at_ms: u64,
}

impl State {
    pub fn kind(&self, id: window::Id) -> Kind {
        if self.huddle == Some(id) {
            Kind::Huddle
        } else if self.tray == Some(id) {
            Kind::Tray
        } else {
            Kind::Main
        }
    }

    pub fn closed(&mut self, id: window::Id) -> Kind {
        let kind = self.kind(id);
        match kind {
            Kind::Main => {
                self.main = None;
                self.main_focused = false;
            }
            Kind::Huddle => self.huddle = None,
            Kind::Tray => self.tray = None,
        }
        kind
    }

    pub fn mark_tray_hidden(&mut self) {
        self.tray_hidden_at_ms = now_ms();
    }

    pub fn tray_was_just_hidden(&self) -> bool {
        now_ms().saturating_sub(self.tray_hidden_at_ms) < 250
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Main,
    Huddle,
    Tray,
}

pub fn main_settings() -> window::Settings {
    window::Settings {
        size: MAIN_SIZE,
        min_size: Some(MAIN_MIN_SIZE),
        // Keep Ducktape's titlebar layout, but let AppKit own the real
        // traffic-light controls and their accessibility semantics.
        decorations: cfg!(target_os = "macos"),
        #[cfg(target_os = "macos")]
        platform_specific: window::settings::PlatformSpecific {
            title_hidden: true,
            titlebar_transparent: true,
            fullsize_content_view: true,
        },
        // The daemon owns close semantics so macOS can keep running in the menu bar.
        exit_on_close_request: false,
        ..window::Settings::default()
    }
}

pub fn huddle_settings() -> window::Settings {
    window::Settings {
        size: HUDDLE_SIZE,
        min_size: Some(HUDDLE_MIN_SIZE),
        decorations: true,
        maximized: false,
        minimizable: false,
        level: window::Level::AlwaysOnTop,
        exit_on_close_request: false,
        ..window::Settings::default()
    }
}

pub fn tray_settings(position: Point) -> window::Settings {
    window::Settings {
        size: TRAY_SIZE,
        position: window::Position::Specific(position),
        decorations: false,
        transparent: true,
        blur: true,
        resizable: false,
        minimizable: false,
        level: window::Level::AlwaysOnTop,
        exit_on_close_request: false,
        ..window::Settings::default()
    }
}

pub fn title(state: &State, id: window::Id, unread: u32) -> String {
    match state.kind(id) {
        Kind::Main if cfg!(target_os = "linux") && unread > 0 => {
            format!("({unread}) Ducktape")
        }
        Kind::Main => "Ducktape".into(),
        Kind::Huddle => "Huddle".into(),
        Kind::Tray => "Ducktape".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_window_only_clears_its_own_slot() {
        let main = window::Id::unique();
        let huddle = window::Id::unique();
        let mut state = State {
            main: Some(main),
            huddle: Some(huddle),
            tray: None,
            main_focused: true,
            tray_hidden_at_ms: 0,
        };
        assert_eq!(state.closed(huddle), Kind::Huddle);
        assert_eq!(state.main, Some(main));
        assert!(state.huddle.is_none());
        assert!(state.main_focused);
    }
}
