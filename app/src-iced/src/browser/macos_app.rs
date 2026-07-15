use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};

use cef::application_mac::{CefAppProtocol, CrAppControlProtocol, CrAppProtocol};
use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyObject, Bool};
use objc2::{ClassType, DefinedClass, MainThreadOnly, msg_send};
use objc2_app_kit::{NSApplication, NSEvent};
use objc2_foundation::MainThreadMarker;

static TERMINATE_REQUESTED: AtomicBool = AtomicBool::new(false);

#[derive(Default)]
struct ApplicationIvars {
    handling_send_event: Cell<bool>,
}

objc2::define_class!(
    // SAFETY: NSApplication supports subclassing and this type is registered
    // before Winit asks AppKit for the shared application. Rust owns no Drop
    // implementation; AppKit retains the process-global instance.
    #[unsafe(super = NSApplication)]
    #[name = "DucktapeApplication"]
    #[thread_kind = MainThreadOnly]
    #[ivars = ApplicationIvars]
    struct DucktapeApplication;

    impl DucktapeApplication {
        #[unsafe(method_id(init))]
        fn init(this: Allocated<Self>) -> Retained<Self> {
            let this = this.set_ivars(ApplicationIvars::default());
            // SAFETY: This invokes NSApplication's designated initializer on
            // an allocated DucktapeApplication with initialized Rust ivars.
            unsafe { msg_send![super(this), init] }
        }

        #[unsafe(method(sendEvent:))]
        fn send_event(&self, event: &NSEvent) {
            let previous = self.ivars().handling_send_event.replace(true);
            // SAFETY: `event` is supplied by AppKit for this sendEvent: call.
            unsafe {
                let _: () = msg_send![super(self), sendEvent: event];
            }
            self.ivars().handling_send_event.set(previous);
        }

        #[unsafe(method(terminate:))]
        fn terminate(&self, _sender: Option<&AnyObject>) {
            // NSApplication's implementation calls exit(3), which bypasses
            // CEF OnBeforeClose/shutdown. The shell consumes this request and
            // leaves Winit's run loop only after closing its browser surface.
            TERMINATE_REQUESTED.store(true, Ordering::Release);
        }
    }

    // SAFETY: Access is restricted to AppKit's main thread by the class type.
    unsafe impl CrAppProtocol for DucktapeApplication {
        #[unsafe(method(isHandlingSendEvent))]
        unsafe fn is_handling_send_event(&self) -> Bool {
            Bool::from(self.ivars().handling_send_event.get())
        }
    }

    // SAFETY: Access is restricted to AppKit's main thread by the class type.
    unsafe impl CrAppControlProtocol for DucktapeApplication {
        #[unsafe(method(setHandlingSendEvent:))]
        unsafe fn set_handling_send_event(&self, handling_send_event: Bool) {
            self.ivars()
                .handling_send_event
                .set(handling_send_event.as_bool());
        }
    }

    // SAFETY: The two inherited control methods are implemented above.
    unsafe impl CefAppProtocol for DucktapeApplication {}
);

/// Register the principal class before Winit creates the shared application.
pub(super) fn register() {
    assert!(
        MainThreadMarker::new().is_some(),
        "the macOS application must be registered on the main thread"
    );
    let _ = DucktapeApplication::class();
}

pub(super) fn take_terminate_request() -> bool {
    TERMINATE_REQUESTED.swap(false, Ordering::AcqRel)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn principal_class_name_matches_the_bundle() {
        assert_eq!(DucktapeApplication::NAME, "DucktapeApplication");
    }
}
