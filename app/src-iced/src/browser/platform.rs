use iced::window::raw_window_handle::{RawDisplayHandle, RawWindowHandle};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Bounds {
    pub(crate) fn validate(self) -> Result<Self, String> {
        if self.width <= 0 || self.height <= 0 {
            Err("CEF child bounds require positive width and height".into())
        } else {
            Ok(self)
        }
    }

    pub(crate) fn cef(self) -> cef::Rect {
        cef::Rect {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
        }
    }
}

/// A copied native parent handle that can safely leave `iced::window::run`.
#[derive(Debug, Clone, Copy)]
pub struct ParentWindow {
    native: usize,
}

impl ParentWindow {
    pub fn from_iced(window: &dyn iced::window::Window) -> Result<Self, String> {
        let raw_window = window
            .window_handle()
            .map_err(|error| format!("iced window handle unavailable: {error}"))?
            .as_raw();
        let raw_display = window
            .display_handle()
            .map_err(|error| format!("iced display handle unavailable: {error}"))?
            .as_raw();

        #[cfg(target_os = "linux")]
        let native = match (raw_window, raw_display) {
            (RawWindowHandle::Xlib(window), RawDisplayHandle::Xlib(_)) => window.window as usize,
            (RawWindowHandle::Xcb(window), RawDisplayHandle::Xcb(_)) => {
                window.window.get() as usize
            }
            (window, display) => {
                return Err(format!(
                    "CEF requires iced's X11 backend; got {window:?} on {display:?}"
                ));
            }
        };

        #[cfg(target_os = "macos")]
        let native = match (raw_window, raw_display) {
            (RawWindowHandle::AppKit(window), RawDisplayHandle::AppKit(_)) => {
                window.ns_view.as_ptr() as usize
            }
            (window, display) => {
                return Err(format!(
                    "expected AppKit handles, got {window:?} on {display:?}"
                ));
            }
        };

        #[cfg(target_os = "windows")]
        let native = match (raw_window, raw_display) {
            (RawWindowHandle::Win32(window), RawDisplayHandle::Windows(_)) => {
                window.hwnd.get() as usize
            }
            (window, display) => {
                return Err(format!(
                    "expected Win32 handles, got {window:?} on {display:?}"
                ));
            }
        };

        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        return Err(format!(
            "CEF child windows are not implemented for {raw_window:?} on {raw_display:?}"
        ));

        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        if native == 0 {
            Err("iced returned a null native window handle".into())
        } else {
            Ok(Self { native })
        }
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn cef(self) -> cef::sys::cef_window_handle_t {
        self.native as cef::sys::cef_window_handle_t
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn cef(self) -> cef::sys::cef_window_handle_t {
        self.native as cef::sys::cef_window_handle_t
    }

    #[cfg(target_os = "windows")]
    pub(crate) fn cef(self) -> cef::sys::cef_window_handle_t {
        cef::sys::HWND(self.native as *mut _)
    }
}

#[cfg(target_os = "linux")]
pub(crate) struct NativeChild {
    xid: x11_dl::xlib::Window,
}

#[cfg(target_os = "linux")]
impl NativeChild {
    pub(crate) fn new(host: &cef::BrowserHost) -> Result<Self, String> {
        use cef::ImplBrowserHost as _;

        let xid = host.window_handle();
        if xid == 0 {
            Err("CEF did not expose its X11 child window".into())
        } else {
            Ok(Self { xid })
        }
    }

    pub(crate) fn set_bounds(&self, bounds: Bounds) -> Result<(), String> {
        with_cef_display(|xlib, display| unsafe {
            (xlib.XMoveResizeWindow)(
                display,
                self.xid,
                bounds.x,
                bounds.y,
                bounds.width as u32,
                bounds.height as u32,
            );
        })
    }

    pub(crate) fn set_visible(&self, visible: bool) -> Result<(), String> {
        with_cef_display(|xlib, display| unsafe {
            if visible {
                (xlib.XMapWindow)(display, self.xid);
            } else {
                (xlib.XUnmapWindow)(display, self.xid);
            }
        })
    }
}

#[cfg(target_os = "linux")]
fn with_cef_display(
    f: impl FnOnce(&x11_dl::xlib::Xlib, *mut x11_dl::xlib::Display),
) -> Result<(), String> {
    use std::sync::LazyLock;

    static XLIB: LazyLock<Result<x11_dl::xlib::Xlib, String>> =
        LazyLock::new(|| x11_dl::xlib::Xlib::open().map_err(|error| error.to_string()));

    let xlib = XLIB.as_ref().map_err(Clone::clone)?;
    let display = cef::get_xdisplay() as *mut x11_dl::xlib::Display;
    if display.is_null() {
        return Err("CEF X11 display is unavailable".into());
    }

    f(xlib, display);
    unsafe {
        (xlib.XFlush)(display);
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) struct NativeChild {
    view: std::ptr::NonNull<objc2_app_kit::NSView>,
    // AppKit view operations are main-thread-only. Keeping the marker in the
    // handle also makes the complete browser surface !Send and !Sync.
    _main_thread: objc2::MainThreadMarker,
}

#[cfg(target_os = "macos")]
impl NativeChild {
    pub(crate) fn new(host: &cef::BrowserHost) -> Result<Self, String> {
        use cef::ImplBrowserHost as _;

        let main_thread = objc2::MainThreadMarker::new()
            .ok_or_else(|| "CEF AppKit child was created off the main thread".to_string())?;
        let view = std::ptr::NonNull::new(host.window_handle().cast::<objc2_app_kit::NSView>())
            .ok_or_else(|| "CEF did not expose its AppKit child view".to_string())?;
        // CEF owns the NSView. Do not retain it independently: BrowserHost is
        // the lifetime owner, and close_browser must remain authoritative.
        let child = unsafe { view.as_ref() };
        if unsafe { child.superview() }.is_none() {
            return Err("CEF AppKit child view has no iced parent".into());
        }
        Ok(Self {
            view,
            _main_thread: main_thread,
        })
    }

    pub(crate) fn set_bounds(&self, bounds: Bounds) -> Result<(), String> {
        use objc2_foundation::{NSPoint, NSRect, NSSize};

        self.ensure_main_thread()?;
        let child = unsafe { self.view.as_ref() };
        let parent = unsafe { child.superview() }
            .ok_or_else(|| "CEF AppKit child view is detached".to_string())?;
        let parent_bounds = parent.bounds();
        let y = appkit_y(
            bounds,
            parent_bounds.origin.y,
            parent_bounds.size.height,
            parent.isFlipped(),
        );
        child.setFrame(NSRect::new(
            NSPoint::new(parent_bounds.origin.x + f64::from(bounds.x), y),
            NSSize::new(f64::from(bounds.width), f64::from(bounds.height)),
        ));
        child.setNeedsDisplay(true);
        Ok(())
    }

    pub(crate) fn set_visible(&self, visible: bool) -> Result<(), String> {
        self.ensure_main_thread()?;
        unsafe { self.view.as_ref() }.setHidden(!visible);
        Ok(())
    }

    fn ensure_main_thread(&self) -> Result<(), String> {
        if objc2::MainThreadMarker::new().is_none() {
            Err("CEF AppKit child view operation ran off the main thread".into())
        } else {
            Ok(())
        }
    }
}

/// Convert iced's top-left-relative y coordinate into the coordinate system
/// of the CEF child's AppKit superview. A flipped NSView already uses a
/// top-left origin; a normal AppKit view uses a bottom-left origin.
#[cfg(any(target_os = "macos", test))]
fn appkit_y(bounds: Bounds, parent_y: f64, parent_height: f64, flipped: bool) -> f64 {
    if flipped {
        parent_y + f64::from(bounds.y)
    } else {
        parent_y + parent_height - f64::from(bounds.y) - f64::from(bounds.height)
    }
}

#[cfg(target_os = "windows")]
pub(crate) struct NativeChild {
    hwnd: windows::Win32::Foundation::HWND,
    thread: std::thread::ThreadId,
}

#[cfg(target_os = "windows")]
impl NativeChild {
    pub(crate) fn new(host: &cef::BrowserHost) -> Result<Self, String> {
        use cef::ImplBrowserHost as _;
        use windows::Win32::{Foundation::HWND, UI::WindowsAndMessaging::IsWindow};

        let raw = host.window_handle();
        let hwnd = HWND(raw.0.cast());
        if hwnd.is_invalid() || !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
            return Err("CEF did not expose a live Win32 child window".into());
        }
        Ok(Self {
            hwnd,
            thread: std::thread::current().id(),
        })
    }

    pub(crate) fn set_bounds(&self, bounds: Bounds) -> Result<(), String> {
        use windows::Win32::UI::WindowsAndMessaging::{SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos};

        self.ensure_live()?;
        unsafe {
            SetWindowPos(
                self.hwnd,
                None,
                bounds.x,
                bounds.y,
                bounds.width,
                bounds.height,
                SWP_NOACTIVATE | SWP_NOZORDER,
            )
        }
        .map_err(|error| format!("resize CEF Win32 child: {error}"))
    }

    pub(crate) fn set_visible(&self, visible: bool) -> Result<(), String> {
        use windows::Win32::UI::WindowsAndMessaging::{SW_HIDE, SW_SHOWNA, ShowWindow};

        self.ensure_live()?;
        unsafe {
            let _ = ShowWindow(self.hwnd, if visible { SW_SHOWNA } else { SW_HIDE });
        }
        Ok(())
    }

    fn ensure_live(&self) -> Result<(), String> {
        use windows::Win32::UI::WindowsAndMessaging::IsWindow;

        self.ensure_thread()?;
        if !unsafe { IsWindow(Some(self.hwnd)) }.as_bool() {
            Err("CEF Win32 child window is already destroyed".into())
        } else {
            Ok(())
        }
    }

    fn ensure_thread(&self) -> Result<(), String> {
        if std::thread::current().id() == self.thread {
            Ok(())
        } else {
            Err("CEF Win32 child operation ran on a different thread".into())
        }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub(crate) struct NativeChild;

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
impl NativeChild {
    pub(crate) fn new(_host: &cef::BrowserHost) -> Result<Self, String> {
        Err("native CEF child windows are unsupported on this platform".into())
    }

    pub(crate) fn set_bounds(&self, _bounds: Bounds) -> Result<(), String> {
        Err("native CEF child windows are unsupported on this platform".into())
    }

    pub(crate) fn set_visible(&self, _visible: bool) -> Result<(), String> {
        Err("native CEF child windows are unsupported on this platform".into())
    }
}

#[cfg(test)]
mod tests {
    use super::{Bounds, appkit_y};

    #[test]
    fn bounds_require_a_real_surface() {
        assert!(
            Bounds {
                x: -10,
                y: -10,
                width: 1,
                height: 1,
            }
            .validate()
            .is_ok()
        );
        assert!(
            Bounds {
                x: 0,
                y: 0,
                width: 0,
                height: 10,
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn appkit_coordinates_preserve_iced_top_left_bounds() {
        let bounds = Bounds {
            x: 12,
            y: 30,
            width: 200,
            height: 80,
        };
        assert_eq!(appkit_y(bounds, 0.0, 500.0, true), 30.0);
        assert_eq!(appkit_y(bounds, 0.0, 500.0, false), 390.0);
        assert_eq!(appkit_y(bounds, 10.0, 500.0, false), 400.0);
    }
}
