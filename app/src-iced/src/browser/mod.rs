#[cfg(target_os = "macos")]
mod macos_app;
mod platform;
mod policy;
mod proxy;

use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    time::{Duration, Instant},
};

use cef::*;
use policy::{NavigationPolicy, PermissionBroker};
use proxy::GatewayProxy;

#[cfg(target_os = "windows")]
use std::sync::atomic::AtomicPtr;

#[cfg(target_os = "windows")]
static WINDOWS_INSTANCE: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());
#[cfg(target_os = "windows")]
static WINDOWS_SANDBOX_INFO: AtomicPtr<u8> = AtomicPtr::new(std::ptr::null_mut());

pub use platform::{Bounds, ParentWindow};
#[allow(unused_imports)]
pub use policy::{BrowserPermission, PermissionPrompt};

#[cfg(target_os = "macos")]
pub(crate) fn prepare_macos_application() {
    macos_app::register();
}

#[cfg(target_os = "macos")]
pub(crate) fn take_macos_terminate_request() -> bool {
    macos_app::take_terminate_request()
}

/// Install the process-local values supplied by CEF's Windows bootstrap.
#[cfg(target_os = "windows")]
pub fn install_windows_bootstrap(
    instance: cef::sys::HINSTANCE,
    sandbox_info: *mut std::ffi::c_void,
) -> Result<(), &'static str> {
    let instance = instance.0.cast();
    let sandbox_info = sandbox_info.cast::<u8>();
    if instance.is_null() || sandbox_info.is_null() {
        return Err("CEF bootstrap did not provide a sandbox context");
    }
    if std::env::var("DUCKTAPE_CEF_NO_SANDBOX").as_deref() == Ok("1") {
        return Err("CEF sandbox cannot be disabled on Windows");
    }

    WINDOWS_INSTANCE
        .compare_exchange(
            std::ptr::null_mut(),
            instance,
            Ordering::SeqCst,
            Ordering::SeqCst,
        )
        .map_err(|_| "CEF bootstrap was installed more than once")?;
    if WINDOWS_SANDBOX_INFO
        .compare_exchange(
            std::ptr::null_mut(),
            sandbox_info,
            Ordering::SeqCst,
            Ordering::SeqCst,
        )
        .is_err()
    {
        WINDOWS_INSTANCE.store(std::ptr::null_mut(), Ordering::SeqCst);
        return Err("CEF sandbox context was installed more than once");
    }
    Ok(())
}

fn process_args() -> cef::args::Args {
    #[cfg(target_os = "windows")]
    {
        let instance = WINDOWS_INSTANCE.load(Ordering::SeqCst);
        if instance.is_null() {
            tracing::error!(
                target: "ducktape::browser",
                reason = "missing_windows_bootstrap",
                "refusing to run CEF outside the sandbox bootstrap"
            );
            std::process::exit(70);
        }
        return cef::args::Args::from(cef::MainArgs {
            instance: cef::sys::HINSTANCE(instance.cast()),
        });
    }
    #[cfg(not(target_os = "windows"))]
    cef::args::Args::new()
}

fn windows_sandbox_info() -> *mut u8 {
    #[cfg(target_os = "windows")]
    {
        let sandbox_info = WINDOWS_SANDBOX_INFO.load(Ordering::SeqCst);
        if sandbox_info.is_null() {
            tracing::error!(
                target: "ducktape::browser",
                reason = "missing_windows_sandbox",
                "refusing to initialize CEF without its Windows sandbox"
            );
            std::process::exit(70);
        }
        return sandbox_info;
    }
    #[cfg(not(target_os = "windows"))]
    std::ptr::null_mut()
}

cef::wrap_client! {
    struct ProbeClient {
        closed: Arc<AtomicBool>,
        navigation: Arc<Mutex<NavigationPolicy>>,
        gateway: GatewayProxy,
        permissions: PermissionBroker,
        events: Sender<BrowserEvent>,
    }

    impl Client {
        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(ProbeLifeSpanHandler::new(self.closed.clone()))
        }

        fn request_handler(&self) -> Option<RequestHandler> {
            Some(ProbeRequestHandler::new(
                self.navigation.clone(),
                self.gateway.clone(),
            ))
        }

        fn permission_handler(&self) -> Option<PermissionHandler> {
            Some(GatewayPermissionHandler::new(self.permissions.clone()))
        }

        fn download_handler(&self) -> Option<DownloadHandler> {
            Some(DenyDownloadHandler::new())
        }

        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(ProbeDisplayHandler::new(self.events.clone()))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BrowserEvent {
    NavigationCommitted { browser_id: i32, url: String },
}

cef::wrap_display_handler! {
    struct ProbeDisplayHandler {
        events: Sender<BrowserEvent>,
    }

    impl DisplayHandler {
        fn on_address_change(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            url: Option<&CefString>,
        ) {
            if frame.is_none_or(|frame| frame.is_main() != 1) {
                return;
            }
            let Some(browser_id) = browser.map(|browser| browser.identifier()) else { return };
            let Some(url) = url.map(CefString::to_string) else { return };
            let _ = self
                .events
                .send(BrowserEvent::NavigationCommitted { browser_id, url });
        }
    }
}

cef::wrap_life_span_handler! {
    struct ProbeLifeSpanHandler {
        closed: Arc<AtomicBool>,
    }

    impl LifeSpanHandler {
        fn on_before_popup(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _popup_id: ::std::os::raw::c_int,
            _target_url: Option<&CefString>,
            _target_frame_name: Option<&CefString>,
            _target_disposition: WindowOpenDisposition,
            _user_gesture: ::std::os::raw::c_int,
            _popup_features: Option<&PopupFeatures>,
            _window_info: Option<&mut WindowInfo>,
            _client: Option<&mut Option<Client>>,
            _settings: Option<&mut BrowserSettings>,
            _extra_info: Option<&mut Option<DictionaryValue>>,
            _no_javascript_access: Option<&mut ::std::os::raw::c_int>,
        ) -> ::std::os::raw::c_int {
            tracing::warn!(
                target: "ducktape::browser",
                reason = "popup_policy",
                "popup refused"
            );
            1
        }

        fn on_before_close(&self, _browser: Option<&mut Browser>) {
            self.closed.store(true, Ordering::SeqCst);
        }
    }
}

cef::wrap_request_handler! {
    struct ProbeRequestHandler {
        navigation: Arc<Mutex<NavigationPolicy>>,
        gateway: GatewayProxy,
    }

    impl RequestHandler {
        fn on_before_browse(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            _user_gesture: ::std::os::raw::c_int,
            _is_redirect: ::std::os::raw::c_int,
        ) -> ::std::os::raw::c_int {
            let Some(request) = request else { return 1 };
            let url = CefString::from(&request.url()).to_string();
            if self
                .navigation
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .allows(&url)
            {
                0
            } else {
                tracing::warn!(
                    target: "ducktape::browser",
                    reason = "navigation_policy",
                    "main-frame navigation refused"
                );
                1
            }
        }

        fn on_open_urlfrom_tab(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _target_url: Option<&CefString>,
            _target_disposition: WindowOpenDisposition,
            _user_gesture: ::std::os::raw::c_int,
        ) -> ::std::os::raw::c_int {
            1
        }

        fn resource_request_handler(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            _request: Option<&mut Request>,
            is_navigation: ::std::os::raw::c_int,
            _is_download: ::std::os::raw::c_int,
            request_initiator: Option<&CefString>,
            _disable_default_handling: Option<&mut ::std::os::raw::c_int>,
        ) -> Option<ResourceRequestHandler> {
            let (initiator, main_frame) = proxy::request_provenance(
                request_initiator
                    .map(CefString::to_string)
                    .filter(|origin| !origin.is_empty()),
                is_navigation,
                frame.is_some_and(|frame| frame.is_main() == 1),
            );
            Some(GuardedResourceRequestHandler::new(
                self.navigation.clone(),
                self.gateway.clone(),
                initiator,
                main_frame,
            ))
        }
    }
}

cef::wrap_resource_request_handler! {
    struct GuardedResourceRequestHandler {
        navigation: Arc<Mutex<NavigationPolicy>>,
        gateway: GatewayProxy,
        initiator: Option<String>,
        main_frame: bool,
    }

    impl ResourceRequestHandler {
        fn on_before_resource_load(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            _callback: Option<&mut Callback>,
        ) -> ReturnValue {
            let Some(request) = request else {
                return ReturnValue::CANCEL;
            };
            let url = CefString::from(&request.url()).to_string();
            if self
                .navigation
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .allows(&url)
            {
                ReturnValue::CONTINUE
            } else {
                tracing::warn!(
                    target: "ducktape::browser",
                    reason = "resource_policy",
                    "cross-origin browser resource refused"
                );
                ReturnValue::CANCEL
            }
        }

        fn resource_handler(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            request: Option<&mut Request>,
        ) -> Option<ResourceHandler> {
            let request = request?;
            let url = CefString::from(&request.url()).to_string();
            if !url
                .get(..5)
                .is_some_and(|scheme| scheme.eq_ignore_ascii_case("duck:"))
            {
                return None;
            }
            Some(proxy::resource_handler(
                self.gateway.clone(),
                self.initiator.clone(),
                self.main_frame,
            ))
        }
    }
}

cef::wrap_permission_handler! {
    struct GatewayPermissionHandler {
        permissions: PermissionBroker,
    }

    impl PermissionHandler {
        fn on_request_media_access_permission(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            requesting_origin: Option<&CefString>,
            requested_permissions: u32,
            callback: Option<&mut MediaAccessCallback>,
        ) -> ::std::os::raw::c_int {
            let Some(callback) = callback else { return 0 };
            self.permissions.request_media(
                &requesting_origin.map(CefString::to_string).unwrap_or_default(),
                frame.is_some_and(|frame| frame.is_main() == 1),
                requested_permissions,
                callback.clone(),
            );
            1
        }

        fn on_show_permission_prompt(
            &self,
            _browser: Option<&mut Browser>,
            _prompt_id: u64,
            requesting_origin: Option<&CefString>,
            requested_permissions: u32,
            callback: Option<&mut PermissionPromptCallback>,
        ) -> ::std::os::raw::c_int {
            let Some(callback) = callback else { return 0 };
            self.permissions.request_prompt(
                &requesting_origin.map(CefString::to_string).unwrap_or_default(),
                requested_permissions,
                callback.clone(),
            );
            1
        }
    }
}

cef::wrap_download_handler! {
    struct DenyDownloadHandler;

    impl DownloadHandler {
        fn can_download(
            &self,
            _browser: Option<&mut Browser>,
            _url: Option<&CefString>,
            _request_method: Option<&CefString>,
        ) -> ::std::os::raw::c_int {
            0
        }
    }
}

cef::wrap_browser_process_handler! {
    struct ProbeBrowserProcessHandler {
        initialized: Arc<AtomicBool>,
        pump: Arc<PumpSchedule>,
    }

    impl BrowserProcessHandler {
        fn on_context_initialized(&self) {
            self.initialized.store(true, Ordering::SeqCst);
            self.pump.schedule(0);
        }

        fn on_schedule_message_pump_work(&self, delay_ms: i64) {
            self.pump.schedule(delay_ms);
        }
    }
}

cef::wrap_app! {
    struct ProbeApp {
        initialized: Arc<AtomicBool>,
        pump: Arc<PumpSchedule>,
    }

    impl App {
        fn on_register_custom_schemes(&self, registrar: Option<&mut SchemeRegistrar>) {
            let Some(registrar) = registrar else { return };
            let options = SchemeOptions::STANDARD.get_raw() as i32
                | SchemeOptions::SECURE.get_raw() as i32
                | SchemeOptions::CORS_ENABLED.get_raw() as i32
                | SchemeOptions::FETCH_ENABLED.get_raw() as i32;
            registrar.add_custom_scheme(Some(&CefString::from("duck")), options);
        }

        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
            Some(ProbeBrowserProcessHandler::new(
                self.initialized.clone(),
                self.pump.clone(),
            ))
        }

        fn on_before_command_line_processing(
            &self,
            _process_type: Option<&CefString>,
            command_line: Option<&mut CommandLine>,
        ) {
            if let Some(command_line) = command_line {
                // Headless/minimal Linux desktops may not have a Secret
                // Service provider. Keep that fallback platform-local: macOS
                // must use Keychain and Windows must retain DPAPI rather than
                // inheriting Chromium's development-only mock keychain.
                #[cfg(target_os = "linux")]
                {
                    command_line.append_switch(Some(&CefString::from("use-mock-keychain")));
                    command_line.append_switch_with_value(
                        Some(&CefString::from("password-store")),
                        Some(&CefString::from("basic")),
                    );
                }
                if cef_no_sandbox() {
                    command_line.append_switch(Some(&CefString::from("no-sandbox")));
                }
                #[cfg(target_os = "linux")]
                command_line.append_switch_with_value(
                    Some(&CefString::from("ozone-platform")),
                    Some(&CefString::from("x11")),
                );
            }
        }
    }
}

/// Dispatch CEF's renderer/GPU/utility re-execs before iced creates an event loop.
pub fn dispatch_helper_processes() {
    let args = process_args();
    let is_browser_process = args
        .as_cmd_line()
        .map(|command| command.has_switch(Some(&CefString::from("type"))) != 1)
        .unwrap_or(true);

    // CEF M138+ ships macOS sandbox support as libcef_sandbox.dylib. A helper
    // must enter it before the framework is loaded; setting no_sandbox=0 by
    // itself does not initialize the seatbelt policy. Keep the context alive
    // until execute_process returns and destroys it on the way out.
    #[cfg(target_os = "macos")]
    let _sandbox = if !is_browser_process && !cef_no_sandbox() {
        let mut sandbox = cef::sandbox::Sandbox::new();
        sandbox.initialize(args.as_main_args());
        Some(sandbox)
    } else {
        None
    };

    load_cef_library();
    initialize_api_table();
    if is_browser_process {
        return;
    }

    let mut app = ProbeApp::new(
        Arc::new(AtomicBool::new(false)),
        Arc::new(PumpSchedule::default()),
    );
    let exit_code = cef::execute_process(
        Some(args.as_main_args()),
        Some(&mut app),
        windows_sandbox_info(),
    );
    std::process::exit(exit_code.max(0));
}

pub struct BrowserRuntime {
    browsers: Vec<BrowserSurface>,
    active_browser: usize,
    gateway: GatewayProxy,
    permissions: PermissionBroker,
    pump: Arc<PumpSchedule>,
    parent: ParentWindow,
    bounds: Bounds,
    visible: bool,
    request_context: Option<RequestContext>,
    event_tx: Sender<BrowserEvent>,
    event_rx: Receiver<BrowserEvent>,
    cef: CefShutdown,
}

impl BrowserRuntime {
    pub fn create(parent: ParentWindow, bounds: Bounds, url: &str) -> Result<Self, String> {
        Self::create_with_gateway(parent, bounds, url, None)
    }

    pub fn validate_gateway_base(base: &str) -> Result<(), String> {
        GatewayProxy::default().set_gateway_base(Some(base.to_owned()))
    }

    /// Create a browser with its active workspace gateway installed before the
    /// first `duck://` navigation begins.
    pub fn create_with_gateway(
        parent: ParentWindow,
        bounds: Bounds,
        url: &str,
        gateway_base: Option<String>,
    ) -> Result<Self, String> {
        let bounds = bounds.validate()?;
        let navigation = Arc::new(Mutex::new(NavigationPolicy::new(url)?));
        let gateway = GatewayProxy::default();
        gateway.set_gateway_base(gateway_base)?;
        let permissions = PermissionBroker::default();
        if CEF_STARTED
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err("CEF can only be initialized once per process".into());
        }
        initialize_api_table();

        let args = process_args();
        let initialized = Arc::new(AtomicBool::new(false));
        let pump = Arc::new(PumpSchedule::default());
        pump.schedule(0);
        let mut app = ProbeApp::new(initialized.clone(), pump.clone());

        let execute_result = cef::execute_process(
            Some(args.as_main_args()),
            Some(&mut app),
            windows_sandbox_info(),
        );
        if execute_result != -1 {
            CEF_STARTED.store(false, Ordering::SeqCst);
            return Err(format!(
                "CEF browser process unexpectedly returned {execute_result}"
            ));
        }

        let cache_path = match private_cache_dir() {
            Ok(path) => path,
            Err(error) => {
                CEF_STARTED.store(false, Ordering::SeqCst);
                return Err(error);
            }
        };
        let cache = cache_path.to_string_lossy();
        #[allow(unused_mut)]
        let mut settings = cef::Settings {
            no_sandbox: i32::from(cef_no_sandbox()),
            root_cache_path: CefString::from(cache.as_ref()),
            cache_path: CefString::from(cache.as_ref()),
            cookieable_schemes_list: CefString::from("duck"),
            external_message_pump: 1,
            ..Default::default()
        };
        #[cfg(target_os = "macos")]
        if let Err(error) = configure_macos_bundle(&mut settings) {
            let _ = std::fs::remove_dir_all(cache_path);
            CEF_STARTED.store(false, Ordering::SeqCst);
            return Err(error);
        }
        if cef::initialize(
            Some(args.as_main_args()),
            Some(&settings),
            Some(&mut app),
            windows_sandbox_info(),
        ) != 1
        {
            let _ = std::fs::remove_dir_all(cache_path);
            return Err("CEF initialization failed".into());
        }
        let mut cef = CefShutdown {
            armed: true,
            cache_path: Some(cache_path),
        };

        for _ in 0..10_000 {
            cef::do_message_loop_work();
            if initialized.load(Ordering::SeqCst) {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        if !initialized.load(Ordering::SeqCst) {
            return Err("CEF context initialization timed out".into());
        }
        let (event_tx, event_rx) = mpsc::channel();
        let mut request_context = private_request_context()?;
        let browser = match create_browser_surface(
            parent,
            bounds,
            url,
            navigation,
            gateway.clone(),
            permissions.clone(),
            event_tx.clone(),
            &mut request_context,
        ) {
            Ok(browser) => browser,
            Err(error) => {
                cef.armed = false;
                return Err(format!(
                    "{error}; shutdown skipped in case CEF created a live browser"
                ));
            }
        };

        Ok(Self {
            browsers: vec![browser],
            active_browser: 0,
            gateway,
            permissions,
            pump,
            parent,
            bounds,
            visible: true,
            request_context: Some(request_context),
            event_tx,
            event_rx,
            cef,
        })
    }

    pub fn pump(&self) {
        self.permissions.expire();
        if self.pump.take_due(Instant::now()) {
            cef::do_message_loop_work();
        }
    }

    pub fn set_bounds(&mut self, bounds: Bounds) -> Result<(), String> {
        let bounds = bounds.validate()?;
        self.bounds = bounds;
        for surface in &mut self.browsers {
            surface.set_bounds(bounds)?;
        }
        Ok(())
    }

    pub fn set_visible(&mut self, visible: bool) -> Result<(), String> {
        self.visible = visible;
        for (index, surface) in self.browsers.iter_mut().enumerate() {
            surface.set_visible(visible && index == self.active_browser)?;
        }
        Ok(())
    }

    pub fn navigate(&self, url: &str) -> Result<(), String> {
        self.surface()?.navigate(url)
    }

    /// Atomically select or clear the dedicated browser gateway. Replacing it
    /// invalidates in-flight work from the previous workspace generation.
    pub fn set_gateway_base(&self, base: Option<String>) -> Result<(), String> {
        self.gateway.set_gateway_base(base)
    }

    pub fn has_surface(&self) -> bool {
        !self.browsers.is_empty()
    }

    pub fn take_events(&self) -> Vec<BrowserEvent> {
        self.event_rx.try_iter().collect()
    }

    pub fn tab_index(&self, browser_id: i32) -> Option<usize> {
        self.browsers
            .iter()
            .position(|surface| surface.browser.identifier() == browser_id)
    }

    pub fn new_tab(&mut self, url: &str) -> Result<(), String> {
        let navigation = Arc::new(Mutex::new(NavigationPolicy::new(url)?));
        let request_context = self
            .request_context
            .as_mut()
            .ok_or_else(|| "browser request context is unavailable".to_string())?;
        if let Some(active) = self.browsers.get_mut(self.active_browser) {
            active.set_visible(false)?;
        }
        let surface = create_browser_surface(
            self.parent,
            self.bounds,
            url,
            navigation,
            self.gateway.clone(),
            self.permissions.clone(),
            self.event_tx.clone(),
            request_context,
        );
        let mut surface = match surface {
            Ok(surface) => surface,
            Err(error) => {
                self.cef.armed = false;
                if let Some(active) = self.browsers.get_mut(self.active_browser) {
                    let _ = active.set_visible(self.visible);
                }
                return Err(format!(
                    "{error}; CEF runtime will not be reused after a failed tab creation"
                ));
            }
        };
        if let Err(error) = surface.set_visible(self.visible) {
            let (safe, _) = surface.close();
            if !safe {
                self.cef.armed = false;
            }
            if let Some(active) = self.browsers.get_mut(self.active_browser) {
                let _ = active.set_visible(self.visible);
            }
            return Err(format!("could not show the new browser tab: {error}"));
        }
        self.browsers.push(surface);
        self.active_browser = self.browsers.len() - 1;
        Ok(())
    }

    pub fn select_tab(&mut self, index: usize) -> Result<(), String> {
        if index >= self.browsers.len() {
            return Err("browser tab index is out of range".into());
        }
        if index == self.active_browser {
            return Ok(());
        }
        let previous = self.active_browser;
        self.browsers[previous].set_visible(false)?;
        self.active_browser = index;
        if let Err(error) = self.browsers[index].set_visible(self.visible) {
            self.active_browser = previous;
            let _ = self.browsers[previous].set_visible(self.visible);
            return Err(error);
        }
        Ok(())
    }

    pub fn close_tab(
        &mut self,
        index: usize,
        next_active: usize,
        replacement_url: &str,
    ) -> Result<(), String> {
        if index >= self.browsers.len() {
            return Err("browser tab index is out of range".into());
        }
        let surface = self.browsers.remove(index);
        let (safe_to_reuse, result) = surface.close();
        if !safe_to_reuse {
            self.cef.armed = false;
            return Err("CEF tab close timed out; browser runtime cannot be reused".into());
        }
        let selection = if self.browsers.is_empty() {
            self.active_browser = 0;
            self.new_tab(replacement_url)
        } else {
            self.active_browser = next_active.min(self.browsers.len() - 1);
            self.set_visible(self.visible)
        };
        selection?;
        result
    }

    /// Drop all browser-owned origin state while keeping the process-wide CEF
    /// runtime alive. The next workspace gets a fresh in-memory request
    /// context before any of its routes are loaded.
    pub fn reset_workspace(&mut self) -> Result<(), String> {
        self.gateway.set_gateway_base(None)?;
        self.permissions.close();
        self.permissions = PermissionBroker::default();
        self.event_rx.try_iter().for_each(drop);
        let result = self.close_surfaces();
        self.request_context = None;
        self.event_rx.try_iter().for_each(drop);
        result
    }

    pub fn reopen(&mut self, url: &str) -> Result<(), String> {
        if !self.browsers.is_empty() {
            return self.navigate(url);
        }
        if !self.cef.armed {
            return Err("CEF runtime is not safe to reuse after a failed close".into());
        }
        let navigation = Arc::new(Mutex::new(NavigationPolicy::new(url)?));
        let mut request_context = private_request_context()?;
        let surface = create_browser_surface(
            self.parent,
            self.bounds,
            url,
            navigation,
            self.gateway.clone(),
            self.permissions.clone(),
            self.event_tx.clone(),
            &mut request_context,
        );
        let mut surface = match surface {
            Ok(surface) => surface,
            Err(error) => {
                self.cef.armed = false;
                return Err(format!(
                    "{error}; CEF runtime will not be reused after a failed browser creation"
                ));
            }
        };
        if let Err(error) = surface.set_visible(self.visible) {
            let (safe, _) = surface.close();
            if !safe {
                self.cef.armed = false;
            }
            return Err(format!("could not show the reopened browser: {error}"));
        }
        self.request_context = Some(request_context);
        self.browsers.push(surface);
        self.active_browser = 0;
        Ok(())
    }

    /// Return the one live native-consent question, if any.
    pub fn permission_prompt(&self) -> Option<PermissionPrompt> {
        self.permissions.prompt()
    }

    /// Answer the current all-or-nothing consent question. `session` remembers
    /// the answer only for this workspace's browser context and exact Duck origin.
    pub fn decide_permission(&self, id: u64, allow: bool, session: bool) -> Result<(), String> {
        self.permissions.decide(id, allow, session)
    }

    pub fn close(&mut self) -> Result<(), String> {
        self.gateway.set_gateway_base(None)?;
        self.permissions.close();
        self.event_rx.try_iter().for_each(drop);
        let result = self.close_surfaces();
        self.request_context = None;
        result
    }

    fn surface(&self) -> Result<&BrowserSurface, String> {
        self.browsers
            .get(self.active_browser)
            .ok_or_else(|| "CEF browser is already closed".into())
    }

    fn close_surfaces(&mut self) -> Result<(), String> {
        let mut first_error = None;
        for surface in std::mem::take(&mut self.browsers) {
            let (safe, result) = surface.close();
            if !safe {
                self.cef.armed = false;
            }
            if let Err(error) = result
                && first_error.is_none()
            {
                first_error = Some(error);
            }
        }
        self.active_browser = 0;
        first_error.map_or(Ok(()), Err)
    }
}

fn private_request_context() -> Result<RequestContext, String> {
    let settings = RequestContextSettings {
        cookieable_schemes_list: CefString::from("duck"),
        ..Default::default()
    };
    cef::request_context_create_context(Some(&settings), None)
        .ok_or_else(|| "CEF failed to create an isolated browser request context".into())
}

#[allow(clippy::too_many_arguments)]
fn create_browser_surface(
    parent: ParentWindow,
    bounds: Bounds,
    url: &str,
    navigation: Arc<Mutex<NavigationPolicy>>,
    gateway: GatewayProxy,
    permissions: PermissionBroker,
    events: Sender<BrowserEvent>,
    request_context: &mut RequestContext,
) -> Result<BrowserSurface, String> {
    let window_info = cef::WindowInfo::default().set_as_child(parent.cef(), &bounds.cef());
    let browser_settings = cef::BrowserSettings::default();
    let closed = Arc::new(AtomicBool::new(false));
    let mut client = ProbeClient::new(
        closed.clone(),
        navigation.clone(),
        gateway,
        permissions,
        events,
    );
    let url = CefString::from(url);
    let browser = cef::browser_host_create_browser_sync(
        Some(&window_info),
        Some(&mut client),
        Some(&url),
        Some(&browser_settings),
        None,
        Some(request_context),
    )
    .ok_or_else(|| "CEF failed to create its child browser".to_string())?;
    let host = browser
        .host()
        .ok_or_else(|| "CEF browser has no host".to_string())?;
    let native = platform::NativeChild::new(&host)?;
    Ok(BrowserSurface {
        browser,
        host,
        native,
        navigation,
        closed,
        bounds,
        visible: true,
    })
}

fn cef_no_sandbox() -> bool {
    #[cfg(target_os = "windows")]
    {
        false
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var("DUCKTAPE_CEF_NO_SANDBOX").as_deref() == Ok("1")
    }
}

#[cfg(target_os = "macos")]
fn configure_macos_bundle(settings: &mut cef::Settings) -> Result<(), String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("could not locate the app executable: {error}"))?;
    let paths = MacBundlePaths::from_executable(&executable)?;
    paths.validate()?;

    // These values are intentionally absolute. They match CEF's macOS
    // contract and the layout produced by ops/stage-macos-iced-app.sh.
    settings.main_bundle_path = CefString::from(paths.app.to_string_lossy().as_ref());
    settings.framework_dir_path = CefString::from(paths.framework.to_string_lossy().as_ref());
    settings.browser_subprocess_path =
        CefString::from(paths.browser_subprocess.to_string_lossy().as_ref());
    Ok(())
}

#[cfg(any(target_os = "macos", test))]
#[derive(Debug, PartialEq, Eq)]
struct MacBundlePaths {
    app: PathBuf,
    framework: PathBuf,
    browser_subprocess: PathBuf,
    helper_executables: Vec<PathBuf>,
}

#[cfg(any(target_os = "macos", test))]
impl MacBundlePaths {
    fn from_executable(executable: &std::path::Path) -> Result<Self, String> {
        let macos = executable
            .parent()
            .filter(|path| path.file_name().and_then(|value| value.to_str()) == Some("MacOS"))
            .ok_or_else(|| "app executable is outside Contents/MacOS".to_string())?;
        let contents = macos
            .parent()
            .filter(|path| path.file_name().and_then(|value| value.to_str()) == Some("Contents"))
            .ok_or_else(|| "app executable is outside a macOS bundle".to_string())?;
        let app = contents
            .parent()
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("app"))
            .ok_or_else(|| "CEF on macOS must run from a staged .app bundle".to_string())?;
        if !executable.is_absolute() {
            return Err("CEF macOS bundle paths must be absolute".into());
        }
        let name = executable
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| "app executable name is not valid UTF-8".to_string())?;
        let frameworks = contents.join("Frameworks");
        let framework = frameworks.join("Chromium Embedded Framework.framework");
        let helper_executables = ["", " (Alerts)", " (GPU)", " (Plugin)", " (Renderer)"]
            .into_iter()
            .map(|suffix| {
                let helper = format!("{name} Helper{suffix}");
                frameworks
                    .join(format!("{helper}.app"))
                    .join("Contents/MacOS")
                    .join(helper)
            })
            .collect::<Vec<_>>();

        Ok(Self {
            app: app.to_path_buf(),
            framework,
            browser_subprocess: helper_executables[0].clone(),
            helper_executables,
        })
    }

    #[cfg(target_os = "macos")]
    fn validate(&self) -> Result<(), String> {
        let framework_binary = self.framework.join("Chromium Embedded Framework");
        let icu_data = self.framework.join("Resources/icudtl.dat");
        let missing = std::iter::once(&framework_binary)
            .chain(std::iter::once(&icu_data))
            .chain(self.helper_executables.iter())
            .find(|path| !path.is_file());
        if let Some(path) = missing {
            Err(format!(
                "staged macOS app is missing required CEF file: {}",
                path.display()
            ))
        } else {
            Ok(())
        }
    }
}

impl Drop for BrowserRuntime {
    fn drop(&mut self) {
        if let Err(error) = self.close() {
            tracing::warn!(
                target: "ducktape::browser",
                reason = "close_failed",
                error = %error,
                "CEF shutdown skipped because its browser did not close cleanly"
            );
        }
    }
}

struct BrowserSurface {
    browser: cef::Browser,
    host: cef::BrowserHost,
    native: platform::NativeChild,
    navigation: Arc<Mutex<NavigationPolicy>>,
    closed: Arc<AtomicBool>,
    bounds: Bounds,
    visible: bool,
}

impl BrowserSurface {
    fn set_bounds(&mut self, bounds: Bounds) -> Result<(), String> {
        let bounds = bounds.validate()?;
        if bounds == self.bounds {
            return Ok(());
        }
        self.host.notify_move_or_resize_started();
        self.native.set_bounds(bounds)?;
        self.host.was_resized();
        self.bounds = bounds;
        Ok(())
    }

    fn set_visible(&mut self, visible: bool) -> Result<(), String> {
        if visible == self.visible {
            return Ok(());
        }
        self.native.set_visible(visible)?;
        self.host.was_hidden((!visible) as i32);
        self.visible = visible;
        Ok(())
    }

    fn navigate(&self, url: &str) -> Result<(), String> {
        let next_policy = NavigationPolicy::new(url)?;
        let frame = self
            .browser
            .main_frame()
            .ok_or_else(|| "CEF browser has no main frame".to_string())?;
        // Only this shell-owned API may replace the pinned top-level origin.
        // Page navigations, redirects and popups still consult the current
        // policy through `on_before_browse` and cannot change it themselves.
        *self
            .navigation
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = next_policy;
        frame.load_url(Some(&CefString::from(url)));
        Ok(())
    }

    fn close(self) -> (bool, Result<(), String>) {
        self.host.close_browser(1);
        let native = self.native.destroy();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !self.closed.load(Ordering::SeqCst) && Instant::now() < deadline {
            cef::do_message_loop_work();
            std::thread::sleep(Duration::from_millis(1));
        }
        if !self.closed.load(Ordering::SeqCst) {
            return (false, Err("CEF browser close timed out".into()));
        }
        (true, native)
    }
}

#[derive(Default)]
struct PumpSchedule(Mutex<Option<Instant>>);

impl PumpSchedule {
    fn schedule(&self, delay_ms: i64) {
        let now = Instant::now();
        let delay = Duration::from_millis(u64::try_from(delay_ms).unwrap_or(0));
        let deadline = now.checked_add(delay).unwrap_or(now);
        self.schedule_at(deadline);
    }

    fn schedule_at(&self, deadline: Instant) {
        let mut scheduled = self.0.lock().unwrap_or_else(|poison| poison.into_inner());
        if scheduled.is_none_or(|current| deadline < current) {
            *scheduled = Some(deadline);
        }
    }

    fn take_due(&self, now: Instant) -> bool {
        let mut scheduled = self.0.lock().unwrap_or_else(|poison| poison.into_inner());
        if scheduled.is_some_and(|deadline| deadline <= now) {
            *scheduled = None;
            true
        } else {
            false
        }
    }
}

struct CefShutdown {
    armed: bool,
    cache_path: Option<PathBuf>,
}

impl Drop for CefShutdown {
    fn drop(&mut self) {
        if self.armed {
            tracing::info!(target: "ducktape::browser", "shutting down CEF");
            cef::shutdown();
            if let Some(cache_path) = self.cache_path.take() {
                let _ = std::fs::remove_dir_all(cache_path);
            }
        }
    }
}

static CEF_STARTED: AtomicBool = AtomicBool::new(false);

fn private_cache_dir() -> Result<PathBuf, String> {
    use std::time::{SystemTime, UNIX_EPOCH};

    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .unwrap_or_else(std::env::temp_dir)
        .join("ducktape/cef");
    std::fs::create_dir_all(&base)
        .map_err(|error| format!("failed to create CEF cache parent: {error}"))?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = base.join(format!("iced-{}-{nonce}", std::process::id()));
    std::fs::create_dir(&path)
        .map_err(|error| format!("failed to create private CEF cache: {error}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        if let Err(error) = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))
        {
            let _ = std::fs::remove_dir(&path);
            return Err(format!("failed to protect private CEF cache: {error}"));
        }
    }
    Ok(path)
}

fn initialize_api_table() {
    let _ = cef::api_hash(cef::sys::CEF_API_VERSION_LAST, 0);
    assert_pinned_libcef();
}

#[cfg(target_os = "macos")]
fn load_cef_library() {
    let executable = std::env::current_exe().expect("current executable is unavailable");
    let is_helper = std::env::args().any(|arg| arg == "--type" || arg.starts_with("--type="));
    let loader = cef::library_loader::LibraryLoader::new(&executable, is_helper);
    assert!(loader.load(), "failed to load the bundled CEF framework");
    // CEF requires the framework loader for the whole process lifetime.
    Box::leak(Box::new(loader));
}

#[cfg(not(target_os = "macos"))]
fn load_cef_library() {}

#[cfg(target_os = "linux")]
fn assert_pinned_libcef() {
    unsafe extern "C" {
        fn cef_version_info(entry: std::os::raw::c_int) -> std::os::raw::c_int;
    }

    let loaded = unsafe {
        (
            cef_version_info(0),
            cef_version_info(1),
            cef_version_info(2),
        )
    };
    let pinned = (
        cef::sys::CEF_VERSION_MAJOR,
        cef::sys::CEF_VERSION_MINOR,
        cef::sys::CEF_VERSION_PATCH,
    );
    if loaded != pinned {
        tracing::error!(
            loaded = ?loaded,
            pinned = ?pinned,
            "loaded libcef does not match the pinned build"
        );
        std::process::exit(70);
    }
}

#[cfg(target_os = "macos")]
fn assert_pinned_libcef() {
    let loaded = loaded_macos_cef_version().unwrap_or_else(|error| {
        tracing::error!(
            target: "ducktape::browser",
            event = "cef_version_unavailable",
            reason = "version_symbol_unavailable",
            error = %error,
            "refusing a CEF runtime whose version cannot be verified"
        );
        std::process::exit(70);
    });
    // The commit number is part of Cargo's exact
    // `147.0.10+gd58e84d+chromium-147.0.7727.118` runtime pin. Keep it beside
    // the semantic CEF and Chromium constants so a framework with the same
    // public API version but different executable code is refused.
    const PINNED_CEF_COMMIT_NUMBER: i32 = 3512;
    let pinned = [
        cef::sys::CEF_VERSION_MAJOR,
        cef::sys::CEF_VERSION_MINOR,
        cef::sys::CEF_VERSION_PATCH,
        PINNED_CEF_COMMIT_NUMBER,
        cef::sys::CHROME_VERSION_MAJOR,
        cef::sys::CHROME_VERSION_MINOR,
        cef::sys::CHROME_VERSION_BUILD,
        cef::sys::CHROME_VERSION_PATCH,
    ];
    if loaded != pinned {
        tracing::error!(
            target: "ducktape::browser",
            event = "cef_version_mismatch",
            loaded = ?loaded,
            pinned = ?pinned,
            "refusing a mismatched bundled CEF runtime"
        );
        std::process::exit(70);
    }
}

#[cfg(target_os = "macos")]
fn loaded_macos_cef_version() -> Result<[i32; 8], String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let paths = MacBundlePaths::from_executable(&executable)?;
    let framework = paths.framework.join("Chromium Embedded Framework");
    // SAFETY: The framework is already loaded by LibraryLoader and remains
    // resident for the process lifetime. This temporary handle only resolves
    // CEF's immutable version function from that exact bundled path.
    let library = unsafe { libloading::Library::new(&framework) }
        .map_err(|error| format!("could not open {}: {error}", framework.display()))?;
    // SAFETY: cef_version_info has the stable C signature declared by the
    // pinned cef_version_info.h and returns an integer without retaining data.
    let version = unsafe {
        let version_info = library
            .get::<unsafe extern "C" fn(std::os::raw::c_int) -> std::os::raw::c_int>(
                b"cef_version_info\0",
            )
            .map_err(|error| format!("cef_version_info is unavailable: {error}"))?;
        [
            version_info(0),
            version_info(1),
            version_info(2),
            version_info(3),
            version_info(4),
            version_info(5),
            version_info(6),
            version_info(7),
        ]
    };
    Ok(version)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn assert_pinned_libcef() {}

#[cfg(test)]
mod tests {
    use super::{MacBundlePaths, PumpSchedule};
    use std::path::Path;
    use std::time::{Duration, Instant};

    #[test]
    fn pump_schedule_keeps_the_earliest_deadline() {
        let schedule = PumpSchedule::default();
        let now = Instant::now();
        schedule.schedule_at(now + Duration::from_millis(20));
        schedule.schedule_at(now + Duration::from_millis(5));
        schedule.schedule_at(now + Duration::from_millis(10));

        assert!(!schedule.take_due(now + Duration::from_millis(4)));
        assert!(schedule.take_due(now + Duration::from_millis(5)));
        assert!(!schedule.take_due(now + Duration::from_millis(20)));
    }

    #[test]
    fn mac_bundle_paths_match_the_staged_app_layout() {
        let paths = MacBundlePaths::from_executable(Path::new(
            "/build/Ducktape.app/Contents/MacOS/ducktape",
        ))
        .unwrap();

        assert_eq!(paths.app, Path::new("/build/Ducktape.app"));
        assert_eq!(
            paths.framework,
            Path::new(
                "/build/Ducktape.app/Contents/Frameworks/Chromium Embedded Framework.framework"
            )
        );
        assert_eq!(
            paths.browser_subprocess,
            Path::new(
                "/build/Ducktape.app/Contents/Frameworks/ducktape Helper.app/Contents/MacOS/ducktape Helper"
            )
        );
        assert_eq!(paths.helper_executables.len(), 5);
        assert!(
            paths.helper_executables[4].ends_with(
                "ducktape Helper (Renderer).app/Contents/MacOS/ducktape Helper (Renderer)"
            )
        );
    }

    #[test]
    fn mac_bundle_paths_reject_a_loose_executable() {
        assert!(
            MacBundlePaths::from_executable(Path::new("/build/ducktape"))
                .unwrap_err()
                .contains("Contents/MacOS")
        );
    }
}
