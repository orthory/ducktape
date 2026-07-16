//! macOS menu-bar integration. Other platforms intentionally have no tray.

#[cfg(target_os = "macos")]
mod imp {
    use std::cell::{Cell, RefCell};
    use std::io::Cursor;

    use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

    const OPEN: &str = "ducktape-open";
    const QUIT: &str = "ducktape-quit";

    thread_local! {
        static TRAY: RefCell<Option<TrayIcon>> = const { RefCell::new(None) };
        static LAST_ACTIVE: Cell<bool> = const { Cell::new(false) };
    }

    #[derive(Debug, Clone, Copy)]
    pub enum Event {
        Toggle { x: f64, y: f64 },
        Open,
        Quit,
    }

    pub fn init() -> Result<(), String> {
        LAST_ACTIVE.set(application_active());
        TRAY.with_borrow_mut(|slot| {
            if slot.is_some() {
                return Ok(());
            }
            let open = MenuItem::with_id(OPEN, "Open Ducktape", true, None);
            let separator = PredefinedMenuItem::separator();
            let quit = MenuItem::with_id(QUIT, "Quit Ducktape", true, None);
            let menu =
                Menu::with_items(&[&open, &separator, &quit]).map_err(|error| error.to_string())?;
            let tray = TrayIconBuilder::new()
                .with_id("ducktape")
                .with_tooltip("Ducktape")
                .with_menu(Box::new(menu))
                .with_menu_on_left_click(false)
                .with_menu_on_right_click(true)
                .with_icon(icon()?)
                .build()
                .map_err(|error| error.to_string())?;
            *slot = Some(tray);
            Ok(())
        })
    }

    pub fn set_unread(unread: u32) {
        TRAY.with_borrow(|slot| {
            if let Some(tray) = slot.as_ref() {
                tray.set_title((unread != 0).then(|| unread.to_string()));
            }
        });
    }

    pub fn poll() -> Option<Event> {
        if let Ok(menu) = MenuEvent::receiver().try_recv() {
            return match menu.id().0.as_str() {
                OPEN => Some(Event::Open),
                QUIT => Some(Event::Quit),
                _ => None,
            };
        }
        if let Ok(event) = TrayIconEvent::receiver().try_recv() {
            return match event {
                TrayIconEvent::Click {
                    position,
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } => Some(Event::Toggle {
                    x: position.x,
                    y: position.y,
                }),
                _ => None,
            };
        }
        let active = application_active();
        let became_active = !LAST_ACTIVE.replace(active) && active;
        became_active.then_some(Event::Open)
    }

    pub fn main_hidden() {
        if let Some(main_thread) = objc2::MainThreadMarker::new() {
            let app = objc2_app_kit::NSApplication::sharedApplication(main_thread);
            app.deactivate();
            LAST_ACTIVE.set(false);
        }
    }

    fn application_active() -> bool {
        objc2::MainThreadMarker::new().is_some_and(|main_thread| {
            objc2_app_kit::NSApplication::sharedApplication(main_thread).isActive()
        })
    }

    fn icon() -> Result<tray_icon::Icon, String> {
        let decoder = png::Decoder::new(Cursor::new(include_bytes!("../assets/icons/tray.png")));
        let mut reader = decoder.read_info().map_err(|error| error.to_string())?;
        let mut rgba = vec![
            0;
            reader
                .output_buffer_size()
                .ok_or_else(|| "tray icon is too large".to_string())?
        ];
        let info = reader
            .next_frame(&mut rgba)
            .map_err(|error| error.to_string())?;
        if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
            return Err("tray icon must be an 8-bit RGBA PNG".into());
        }
        rgba.truncate(info.buffer_size());
        tray_icon::Icon::from_rgba(rgba, info.width, info.height).map_err(|error| error.to_string())
    }
}

#[cfg(target_os = "macos")]
pub use imp::*;

#[cfg(not(target_os = "macos"))]
#[derive(Debug, Clone, Copy)]
pub enum Event {}

#[cfg(not(target_os = "macos"))]
pub fn init() -> Result<(), String> {
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn set_unread(_unread: u32) {}

#[cfg(not(target_os = "macos"))]
pub fn main_hidden() {}

#[cfg(not(target_os = "macos"))]
pub fn poll() -> Option<Event> {
    None
}
