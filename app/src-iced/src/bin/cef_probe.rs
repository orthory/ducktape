// The probe intentionally exercises only the minimal CEF lifecycle surface of
// the production browser module.
#[allow(dead_code)]
#[path = "../browser/mod.rs"]
mod browser;
#[path = "../duck_url.rs"]
mod duck_url;

use std::time::Duration;

use browser::{Bounds, BrowserRuntime, ParentWindow};
use iced::{
    Element, Length, Subscription, Task,
    widget::{button, column, container, row, text},
    window,
};

const PROBE_URL: &str = concat!(
    "data:text/html,%3Cbody%20style%3D%27margin%3A0%3Bdisplay%3Agrid%3B",
    "place-items%3Acenter%3Bheight%3A100vh%3Bbackground%3Argb%2824%2C28%2C36%29%3B",
    "color%3Argb%28235%2C239%2C246%29%3Bfont-family%3Asans-serif%27%3E",
    "%3Cdiv%3E%3Ch1%3EDucktape%20CEF%20inside%20iced%3C%2Fh1%3E",
    "%3Cp%3EThis%20page%20is%20a%20native%20CEF%20child%20window.%3C%2Fp%3E",
    "%3C%2Fdiv%3E%3C%2Fbody%3E"
);

const SMALL: Bounds = Bounds {
    x: 40,
    y: 170,
    width: 680,
    height: 320,
};
const LARGE: Bounds = Bounds {
    x: 40,
    y: 170,
    width: 820,
    height: 440,
};

fn main() -> iced::Result {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ducktape_iced=info".into()),
        )
        .try_init()
        .ok();

    #[cfg(target_os = "linux")]
    force_x11();
    browser::dispatch_helper_processes();
    #[cfg(target_os = "macos")]
    browser::prepare_macos_application();

    let result = iced::application(Probe::boot, Probe::update, Probe::view)
        .title("Ducktape iced + CEF probe")
        .window_size((920.0, 680.0))
        .exit_on_close_request(false)
        .subscription(Probe::subscription)
        .run();
    browser::shutdown_after_event_loop();
    result
}

#[cfg(target_os = "linux")]
fn force_x11() {
    // SAFETY: this runs before iced, CEF, or their worker threads exist.
    unsafe {
        std::env::remove_var("WAYLAND_DISPLAY");
        std::env::remove_var("WAYLAND_SOCKET");
    }
}

struct Probe {
    browser: Option<BrowserRuntime>,
    status: String,
    visible: bool,
    large: bool,
    closing: bool,
}

#[derive(Debug, Clone)]
enum Message {
    WindowReady(Option<window::Id>),
    ParentReady(Result<ParentWindow, String>),
    Pump,
    ToggleVisible,
    ToggleBounds,
    Reload,
    CloseBrowser,
}

impl Probe {
    fn boot() -> (Self, Task<Message>) {
        (
            Self {
                browser: None,
                status: "Waiting for iced's native window…".into(),
                visible: true,
                large: false,
                closing: false,
            },
            window::latest().map(Message::WindowReady),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::WindowReady(Some(id)) => {
                self.status = "Reading iced's window and display handles…".into();
                return window::run(id, ParentWindow::from_iced).map(Message::ParentReady);
            }
            Message::WindowReady(None) => {
                self.status = "iced did not create a window".into();
            }
            Message::ParentReady(Ok(parent)) => {
                match BrowserRuntime::create(parent, SMALL, PROBE_URL) {
                    Ok(mut browser) => {
                        if smoke_requested() {
                            if let Err(error) = exercise_browser(&mut browser) {
                                tracing::error!(
                                    target: "ducktape::browser",
                                    event = "cef_probe_smoke_failed",
                                    reason = "browser_exercise_failed",
                                    error = %error,
                                    "CEF probe smoke sequence failed"
                                );
                                self.status = error;
                                self.browser = Some(browser);
                                self.begin_shutdown();
                                return Task::none();
                            }
                            self.large = true;
                            tracing::info!(
                                target: "ducktape::browser",
                                event = "cef_probe_smoke_ready",
                                "CEF probe completed bounds, visibility, and reload request checks"
                            );
                        }
                        self.browser = Some(browser);
                        self.status = "CEF child browser is live".into();
                    }
                    Err(error) => self.status = error,
                }
            }
            Message::ParentReady(Err(error)) => self.status = error,
            Message::Pump => {
                #[cfg(target_os = "macos")]
                if browser::take_macos_terminate_request() {
                    self.begin_shutdown();
                }
                if let Some(browser) = &mut self.browser {
                    browser.pump();
                }
                if self.closing {
                    return self.finish_shutdown();
                }
            }
            Message::ToggleVisible => {
                self.visible = !self.visible;
                if let Some(browser) = &mut self.browser
                    && let Err(error) = browser.set_visible(self.visible)
                {
                    self.status = error;
                }
            }
            Message::ToggleBounds => {
                self.large = !self.large;
                if let Some(browser) = &mut self.browser {
                    let bounds = if self.large { LARGE } else { SMALL };
                    if let Err(error) = browser.set_bounds(bounds) {
                        self.status = error;
                    }
                }
            }
            Message::Reload => {
                if let Some(browser) = &self.browser
                    && let Err(error) = browser.navigate(PROBE_URL)
                {
                    self.status = error;
                }
            }
            Message::CloseBrowser => {
                self.begin_shutdown();
            }
        }
        Task::none()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            iced::time::every(Duration::from_millis(8)).map(|_| Message::Pump),
            window::close_requests().map(|_| Message::CloseBrowser),
        ])
    }

    fn view(&self) -> Element<'_, Message> {
        let controls = row![
            button(if self.visible { "Hide CEF" } else { "Show CEF" })
                .on_press(Message::ToggleVisible),
            button(if self.large {
                "Small bounds"
            } else {
                "Large bounds"
            })
            .on_press(Message::ToggleBounds),
            button("Reload CEF").on_press(Message::Reload),
            button("Close CEF").on_press(Message::CloseBrowser),
        ]
        .spacing(12);

        container(
            column![
                text("Direct iced 0.14 + CEF 148 child-window probe").size(24),
                text(&self.status),
                controls,
            ]
            .spacing(14),
        )
        .padding(24)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }

    fn begin_shutdown(&mut self) {
        if self.closing {
            return;
        }
        self.closing = true;
        if let Some(browser) = &mut self.browser
            && let Err(error) = browser.begin_shutdown()
        {
            self.status = error;
        }
    }

    fn finish_shutdown(&mut self) -> Task<Message> {
        let Some(browser) = &mut self.browser else {
            return iced::exit();
        };
        match browser.finish_shutdown() {
            Ok(false) => return Task::none(),
            Ok(true) => {}
            Err(error) => self.status = error,
        }
        let mut browser = self.browser.take().expect("browser presence checked");
        browser.defer_shutdown_after_event_loop();
        iced::exit()
    }
}

fn smoke_requested() -> bool {
    std::env::var("DUCKTAPE_CEF_PROBE_SMOKE").as_deref() == Ok("1")
}

fn exercise_browser(browser: &mut BrowserRuntime) -> Result<(), String> {
    browser.set_bounds(LARGE)?;
    browser.set_visible(false)?;
    browser.set_visible(true)?;
    browser.navigate(PROBE_URL)
}
