//! Browser-only orchestration between native chrome and the CEF runtime.

use iced::Task;
#[cfg(feature = "cef-browser")]
use iced::{Size, window};

#[cfg(feature = "cef-browser")]
use super::{MODULE_RAIL_WIDTH, NETWORK_RAIL_WIDTH, TITLEBAR_HEIGHT, finish_quit};
use super::{Message, Screen, Shell};
#[cfg(feature = "cef-browser")]
use crate::browser::{Bounds as BrowserBounds, BrowserEvent, BrowserRuntime, ParentWindow};
use crate::browser_chrome;
#[cfg(feature = "cef-browser")]
use crate::network_content::{self, LocalDocument};

#[cfg(feature = "cef-browser")]
pub(super) struct PendingLocalDocument {
    pub(super) generation: u64,
    pub(super) request_generation: u64,
    pub(super) workspace_id: Option<String>,
    pub(super) tab_index: usize,
    pub(super) expected_url: String,
    pub(super) document: LocalDocument,
}

pub(super) fn update_chrome(state: &mut Shell, message: browser_chrome::Message) -> Task<Message> {
    #[cfg(feature = "cef-browser")]
    let chrome_before = state.browser_chrome.clone();
    #[cfg(feature = "cef-browser")]
    let action = message.clone();
    let Some(url) = browser_chrome::update(&mut state.browser_chrome, message) else {
        return Task::none();
    };

    #[cfg(feature = "cef-browser")]
    {
        if url == browser_chrome::IDLE_URL {
            if let Some(browser) = &mut state.browser {
                let result = match &action {
                    browser_chrome::Message::NewTab => browser.new_tab(&url),
                    browser_chrome::Message::SelectTab(index) => browser.select_tab(*index),
                    browser_chrome::Message::CloseTab(index) => {
                        browser.close_tab(*index, state.browser_chrome.active_tab, &url)
                    }
                    _ => Ok(()),
                };
                if let Err(error) = result {
                    if matches!(
                        action,
                        browser_chrome::Message::NewTab | browser_chrome::Message::SelectTab(_)
                    ) {
                        state.browser_chrome = chrome_before;
                    }
                    state.browser_chrome.error = Some(error);
                }
            }
            return sync_visibility(state);
        }
        if browser_chrome::is_network_url(&url) {
            if let Some(browser) = &mut state.browser {
                let result = match &action {
                    browser_chrome::Message::SelectTab(index) => browser.select_tab(*index),
                    browser_chrome::Message::CloseTab(index) => {
                        browser.close_tab(*index, state.browser_chrome.active_tab, &url)
                    }
                    _ => Ok(()),
                };
                if let Err(error) = result {
                    if matches!(action, browser_chrome::Message::SelectTab(_)) {
                        state.browser_chrome = chrome_before;
                    }
                    state.browser_chrome.loading = false;
                    state.browser_chrome.error = Some(error);
                    return Task::none();
                }
            }
            let tab_index = state.browser_chrome.active_tab;
            return load_local_document(state, tab_index, url);
        }
        if let Some(browser) = &mut state.browser {
            if state.browser_gateway_base.is_none() {
                let topology = match &action {
                    browser_chrome::Message::SelectTab(index) => Some(browser.select_tab(*index)),
                    browser_chrome::Message::CloseTab(index) => {
                        Some(browser.close_tab(*index, state.browser_chrome.active_tab, &url))
                    }
                    _ => None,
                };
                if let Some(Err(error)) = topology {
                    if matches!(action, browser_chrome::Message::SelectTab(_)) {
                        state.browser_chrome = chrome_before;
                    }
                    state.browser_chrome.loading = false;
                    state.browser_chrome.error = Some(error.clone());
                    state.browser_error = Some(error);
                    return Task::none();
                }
                return sync_visibility(state);
            }
            let (result, rollback_chrome) = match &action {
                browser_chrome::Message::NewTab => (browser.new_tab(&url), true),
                browser_chrome::Message::SelectTab(index) => match browser.select_tab(*index) {
                    Ok(()) => (navigate_placeholder(browser, &url), false),
                    Err(error) => (Err(error), true),
                },
                browser_chrome::Message::CloseTab(index) => (
                    browser
                        .close_tab(*index, state.browser_chrome.active_tab, &url)
                        .and_then(|()| navigate_placeholder(browser, &url)),
                    false,
                ),
                _ => (browser.navigate(&url), false),
            };
            match result {
                Ok(()) => {
                    state.browser_chrome.loading = false;
                    state.browser_error = None;
                    return sync_visibility(state);
                }
                Err(error) => {
                    if rollback_chrome {
                        state.browser_chrome = chrome_before;
                    }
                    state.browser_chrome.loading = false;
                    state.browser_chrome.error = Some(error.clone());
                    state.browser_error = Some(error);
                }
            }
        } else {
            return sync_visibility(state);
        }
    }

    #[cfg(not(feature = "cef-browser"))]
    {
        let _ = url;
        state.browser_chrome.loading = false;
        if !state.browser_chrome.is_idle() {
            state.browser_chrome.error =
                Some("This build does not include the embedded CEF browser.".into());
        }
    }

    Task::none()
}

#[cfg(feature = "cef-browser")]
fn navigate_placeholder(browser: &BrowserRuntime, url: &str) -> Result<(), String> {
    if browser.active_tab_is_idle()? {
        browser.navigate(url)
    } else {
        Ok(())
    }
}

#[cfg(feature = "cef-browser")]
pub(super) fn local_document_loaded(
    state: &mut Shell,
    generation: u64,
    request_generation: u64,
    workspace_id: Option<String>,
    tab_index: usize,
    expected_url: String,
    result: Result<LocalDocument, String>,
) -> Task<Message> {
    if !local_result_is_current(
        state,
        generation,
        request_generation,
        workspace_id.as_deref(),
        tab_index,
        &expected_url,
    ) {
        return Task::none();
    }
    state.browser_chrome.loading = false;
    match result {
        Err(error) => state.browser_chrome.error = Some(error),
        Ok(document) => {
            if document.url != expected_url {
                state.browser_chrome.error =
                    Some("net.duck loader returned a different URL.".into());
                return Task::none();
            }
            if let Some(browser) = &mut state.browser {
                let result =
                    materialize_tabs(browser, &state.browser_chrome, true).and_then(|()| {
                        browser.navigate_local_document(&document.url, document.bytes.clone())
                    });
                if let Err(error) = result {
                    state.browser_chrome.error = Some(error);
                    state.browser_error = state.browser_chrome.error.clone();
                    return Task::none();
                }
                state.browser_error = None;
                return sync_visibility(state);
            }
            state.browser_local_pending = Some(PendingLocalDocument {
                generation,
                request_generation,
                workspace_id,
                tab_index,
                expected_url,
                document,
            });
            return Task::done(Message::BrowserWindowReady(state.desktop.main));
        }
    }
    Task::none()
}

#[cfg(feature = "cef-browser")]
pub(super) fn gateway_loaded(
    state: &mut Shell,
    generation: u64,
    workspace_id: Option<String>,
    result: Result<String, String>,
) -> Task<Message> {
    if generation != state.browser_gateway_generation || workspace_id != gateway_workspace(state) {
        return Task::none();
    }
    state.browser_gateway_loading = false;
    if state.browser_chrome.is_idle()
        || browser_chrome::is_network_url(state.browser_chrome.runtime_url())
    {
        return Task::none();
    }
    let base = match result {
        Ok(base) => base,
        Err(error) => {
            state.browser_chrome.loading = false;
            state.browser_chrome.error = Some(error.clone());
            state.browser_error = Some(error);
            return Task::none();
        }
    };
    if let Err(error) = BrowserRuntime::validate_gateway_base(&base) {
        state.browser_chrome.loading = false;
        state.browser_chrome.error = Some(error.clone());
        state.browser_error = Some(error);
        return Task::none();
    }
    if let Some(browser) = &mut state.browser {
        if let Err(error) = browser.set_gateway_base(Some(base.clone())) {
            state.browser_chrome.loading = false;
            state.browser_chrome.error = Some(error.clone());
            state.browser_error = Some(error);
            return Task::none();
        }
        let result = materialize_tabs(browser, &state.browser_chrome, false)
            .and_then(|()| state.browser_chrome.url())
            .and_then(|url| browser.navigate(&url));
        if let Err(error) = result {
            state.browser_chrome.loading = false;
            state.browser_chrome.error = Some(error.clone());
            state.browser_error = Some(error);
            return Task::none();
        }
        state.browser_gateway_base = Some(base);
        state.browser_chrome.loading = false;
        return sync_visibility(state);
    }
    state.browser_gateway_base = Some(base);
    Task::done(Message::BrowserWindowReady(state.desktop.main))
}

#[cfg(feature = "cef-browser")]
pub(super) fn window_ready(state: &mut Shell, id: Option<window::Id>) -> Task<Message> {
    let Some(id) = id else {
        let error = "iced did not expose its native window".to_string();
        state.browser_local_pending = None;
        state.browser_chrome.loading = false;
        state.browser_chrome.error = Some(error.clone());
        state.browser_error = Some(error);
        return Task::none();
    };
    window::run(id, ParentWindow::from_iced).map(Message::BrowserParentReady)
}

#[cfg(feature = "cef-browser")]
pub(super) fn parent_ready(
    state: &mut Shell,
    result: Result<ParentWindow, String>,
) -> Task<Message> {
    if !parent_request_is_current(state.quitting, state.browser.is_some()) {
        return Task::none();
    }
    let parent = match result {
        Ok(parent) => parent,
        Err(error) => {
            state.browser_local_pending = None;
            state.browser_chrome.loading = false;
            state.browser_chrome.error = Some(error.clone());
            state.browser_error = Some(error);
            return Task::none();
        }
    };
    if state.browser_local_pending.as_ref().is_some_and(|pending| {
        !local_result_is_current(
            state,
            pending.generation,
            pending.request_generation,
            pending.workspace_id.as_deref(),
            pending.tab_index,
            &pending.expected_url,
        )
    }) {
        state.browser_local_pending = None;
        return sync_visibility(state);
    }
    let local_start = state.browser_local_pending.is_some();
    let urls = state.browser_chrome.runtime_urls();
    let Some(runtime_url) = urls.first().copied() else {
        state.browser_error = Some("Browser has no tabs to open.".into());
        return Task::none();
    };
    let url = if local_start {
        browser_chrome::IDLE_URL
    } else {
        runtime_url
    };
    match BrowserRuntime::create_with_gateway(
        parent,
        bounds(state.window_size),
        url,
        state.browser_gateway_base.clone(),
    ) {
        Ok(mut browser) => {
            if let Err(error) = materialize_tabs(&mut browser, &state.browser_chrome, local_start) {
                state.browser = Some(browser);
                state.browser_chrome.loading = false;
                state.browser_chrome.error = Some(error.clone());
                state.browser_error = Some(error);
                return Task::none();
            }
            if let Some(pending) = state.browser_local_pending.take()
                && let Err(error) =
                    browser.navigate_local_document(&pending.document.url, pending.document.bytes)
            {
                state.browser = Some(browser);
                state.browser_chrome.loading = false;
                state.browser_chrome.error = Some(error.clone());
                state.browser_error = Some(error);
                return Task::none();
            }
            state.browser = Some(browser);
            state.browser_chrome.loading = false;
            state.browser_error = None;
            sync_visibility(state)
        }
        Err(error) => {
            state.browser_local_pending = None;
            state.browser_chrome.loading = false;
            state.browser_chrome.error = Some(error.clone());
            state.browser_error = Some(error);
            Task::none()
        }
    }
}

#[cfg(feature = "cef-browser")]
pub(super) const fn parent_request_is_current(quitting: bool, browser_present: bool) -> bool {
    !quitting && !browser_present
}

#[cfg(feature = "cef-browser")]
pub(super) fn pump(state: &mut Shell) -> Task<Message> {
    let page_visible =
        state.screen() == Screen::Browser && !state.workspace_overlay && !state.search.open;
    if let Some(browser) = &mut state.browser {
        browser.pump();
        if state.quitting
            && let Err(error) = browser.begin_shutdown()
        {
            tracing::warn!(
                target: "ducktape::browser",
                reason = "close_request_failed",
                error = %error,
                "CEF browser close request was only partially applied"
            );
        }
        for event in browser.take_events() {
            match event {
                BrowserEvent::NavigationCommitted { browser_id, url } => {
                    let Some(tab_index) = browser.tab_index(browser_id) else {
                        continue;
                    };
                    if let Err(error) = browser_chrome::commit_navigation(
                        &mut state.browser_chrome,
                        tab_index,
                        &url,
                    ) {
                        state.browser_chrome.error = Some(error);
                    }
                }
            }
        }
        let prompt = browser.permission_prompt();
        if prompt != state.browser_permission {
            state.browser_permission = prompt;
            let visible = state.browser_permission.is_none() && page_visible;
            if let Err(error) = browser.set_visible(visible) {
                state.browser_error = Some(error);
            }
        }
    }
    if state.quitting {
        finish_quit(state)
    } else {
        Task::none()
    }
}

#[cfg(feature = "cef-browser")]
pub(super) fn decide_permission(
    state: &mut Shell,
    id: u64,
    allow: bool,
    session: bool,
) -> Task<Message> {
    let page_visible =
        state.screen() == Screen::Browser && !state.workspace_overlay && !state.search.open;
    if state.browser_permission.as_ref().map(|prompt| prompt.id) == Some(id)
        && let Some(browser) = &mut state.browser
    {
        match browser.decide_permission(id, allow, session) {
            Ok(()) => {
                state.browser_permission = None;
                if let Err(error) = browser.set_visible(page_visible) {
                    state.browser_error = Some(error);
                }
            }
            Err(error) => state.browser_error = Some(error),
        }
    }
    Task::none()
}

#[cfg(feature = "cef-browser")]
fn load_local_document(state: &mut Shell, tab_index: usize, expected_url: String) -> Task<Message> {
    state.browser_local_generation = state.browser_local_generation.wrapping_add(1);
    state.browser_local_pending = None;
    state.browser_chrome.loading = true;
    state.browser_chrome.error = None;
    state.browser_error = None;
    if let Some(browser) = &mut state.browser
        && let Err(error) = browser.set_visible(false)
    {
        state.browser_error = Some(error);
    }
    let Some(client) = state.node_client.clone() else {
        state.browser_chrome.loading = false;
        state.browser_chrome.error = Some("Connect a workspace to open net.duck.".into());
        return Task::none();
    };
    let expected_url = canonical_local_document_url(&expected_url);
    let generation = state.browser_gateway_generation;
    let request_generation = state.browser_local_generation;
    let workspace_id = gateway_workspace(state);
    let requested_url = expected_url.clone();
    Task::perform(
        network_content::load(client, requested_url),
        move |result| Message::BrowserLocalDocumentLoaded {
            generation,
            request_generation,
            workspace_id,
            tab_index,
            expected_url,
            result,
        },
    )
}

#[cfg(feature = "cef-browser")]
fn local_result_is_current(
    state: &Shell,
    generation: u64,
    request_generation: u64,
    workspace_id: Option<&str>,
    tab_index: usize,
    expected_url: &str,
) -> bool {
    generation == state.browser_gateway_generation
        && request_generation == state.browser_local_generation
        && workspace_id == gateway_workspace(state).as_deref()
        && state.browser_chrome.active_tab == tab_index
        && state
            .browser_chrome
            .runtime_url_at(tab_index)
            .is_some_and(|url| canonical_local_document_url(url) == expected_url)
}

#[cfg(feature = "cef-browser")]
fn canonical_local_document_url(raw: &str) -> String {
    reqwest::Url::parse(raw).map_or_else(
        |_| raw.to_string(),
        |url| {
            let path = if url.path().is_empty() {
                "/"
            } else {
                url.path()
            };
            let mut canonical = format!("duck://net.duck{path}");
            if let Some(query) = url.query() {
                canonical.push('?');
                canonical.push_str(query);
            }
            if let Some(fragment) = url.fragment() {
                canonical.push('#');
                canonical.push_str(fragment);
            }
            canonical
        },
    )
}

#[cfg(feature = "cef-browser")]
fn materialize_tabs(
    browser: &mut BrowserRuntime,
    chrome: &browser_chrome::State,
    local_placeholders: bool,
) -> Result<(), String> {
    let urls = chrome.runtime_urls();
    if urls.is_empty() || chrome.active_tab >= urls.len() {
        return Err("Browser chrome has an invalid tab selection.".into());
    }
    let mut existing = browser.tab_count();
    if existing > urls.len() {
        return Err("CEF has more tabs than Browser chrome.".into());
    }
    if existing == 0 {
        browser.reopen(if local_placeholders {
            browser_chrome::IDLE_URL
        } else {
            urls[0]
        })?;
        existing = 1;
    }
    for url in &urls[existing..] {
        browser.new_tab(if local_placeholders {
            browser_chrome::IDLE_URL
        } else {
            url
        })?;
    }
    browser.select_tab(chrome.active_tab)
}

pub(super) fn sync_visibility(state: &mut Shell) -> Task<Message> {
    #[cfg(feature = "cef-browser")]
    {
        let browser_screen_visible = state.screen() == Screen::Browser
            && !state.workspace_overlay
            && !state.search.open
            && state.browser_permission.is_none();
        if state.browser_chrome.is_idle() {
            if let Some(browser) = &mut state.browser
                && let Err(error) = browser.set_visible(false)
            {
                state.browser_error = Some(error);
            }
            state.browser_chrome.loading = false;
            state.browser_chrome.error = None;
            return Task::none();
        }
        let visible = browser_screen_visible;
        let network_url = browser_chrome::is_network_url(state.browser_chrome.runtime_url());
        if browser_screen_visible && !network_url && state.browser_gateway_base.is_none() {
            state.browser_chrome.loading = true;
            if let Some(browser) = &mut state.browser {
                let _ = browser.set_visible(false);
            }
            if state.browser_gateway_loading {
                return Task::none();
            }
            let Some(client) = state.node_client.clone() else {
                state.browser_chrome.loading = false;
                state.browser_chrome.error =
                    Some("Connect a workspace to browse .duck routes.".into());
                return Task::none();
            };
            state.browser_gateway_loading = true;
            let generation = state.browser_gateway_generation;
            let workspace_id = gateway_workspace(state);
            return Task::perform(
                async move {
                    client
                        .gateway_browser_base()
                        .await
                        .map_err(|error| error.to_string())
                },
                move |result| Message::BrowserGatewayLoaded {
                    generation,
                    workspace_id,
                    result,
                },
            );
        }
        if state
            .browser
            .as_ref()
            .is_some_and(BrowserRuntime::has_surface)
        {
            let browser = state.browser.as_mut().expect("browser presence checked");
            if let Err(error) = browser.set_visible(visible) {
                state.browser_error = Some(error);
            }
            Task::none()
        } else if browser_screen_visible {
            state.browser_chrome.loading = !state.browser_chrome.is_idle();
            if network_url {
                return Task::none();
            }
            if state.browser_gateway_base.is_some() {
                if let Some(browser) = &mut state.browser {
                    let result = browser.reopen(state.browser_chrome.runtime_url());
                    match result {
                        Ok(()) => {
                            state.browser_chrome.loading = false;
                            if let Err(error) = browser.set_visible(visible) {
                                state.browser_error = Some(error);
                            }
                        }
                        Err(error) => {
                            state.browser_chrome.loading = false;
                            state.browser_chrome.error = Some(error.clone());
                            state.browser_error = Some(error);
                        }
                    }
                    Task::none()
                } else {
                    Task::done(Message::BrowserWindowReady(state.desktop.main))
                }
            } else if state.browser_gateway_loading {
                Task::none()
            } else if let Some(client) = state.node_client.clone() {
                state.browser_gateway_loading = true;
                let generation = state.browser_gateway_generation;
                let workspace_id = gateway_workspace(state);
                Task::perform(
                    async move {
                        client
                            .gateway_browser_base()
                            .await
                            .map_err(|error| error.to_string())
                    },
                    move |result| Message::BrowserGatewayLoaded {
                        generation,
                        workspace_id,
                        result,
                    },
                )
            } else {
                state.browser_chrome.loading = false;
                state.browser_chrome.error =
                    Some("Connect a workspace to browse .duck routes.".into());
                Task::none()
            }
        } else {
            Task::none()
        }
    }

    #[cfg(not(feature = "cef-browser"))]
    {
        if state.screen() == Screen::Browser
            && !state.workspace_overlay
            && !state.search.open
            && !state.browser_chrome.is_idle()
        {
            state.browser_chrome.error =
                Some("This build does not include the embedded CEF browser.".into());
        }
        Task::none()
    }
}

#[cfg(feature = "cef-browser")]
pub(super) fn bounds(size: Size) -> BrowserBounds {
    BrowserBounds {
        x: (NETWORK_RAIL_WIDTH + MODULE_RAIL_WIDTH) as i32,
        y: (TITLEBAR_HEIGHT + browser_chrome::TAB_BAR_HEIGHT + browser_chrome::TOOLBAR_HEIGHT)
            as i32,
        width: (size.width - NETWORK_RAIL_WIDTH - MODULE_RAIL_WIDTH)
            .max(1.0)
            .round() as i32,
        height: (size.height
            - TITLEBAR_HEIGHT
            - browser_chrome::TAB_BAR_HEIGHT
            - browser_chrome::TOOLBAR_HEIGHT)
            .max(1.0)
            .round() as i32,
    }
}

pub(super) fn hide(state: &mut Shell) {
    #[cfg(feature = "cef-browser")]
    if let Some(browser) = &mut state.browser {
        let _ = browser.set_visible(false);
    }
    #[cfg(not(feature = "cef-browser"))]
    let _ = state;
}

pub(super) fn reset_gateway(state: &mut Shell) {
    #[cfg(feature = "cef-browser")]
    {
        state.browser_gateway_generation = state.browser_gateway_generation.wrapping_add(1);
        state.browser_local_generation = state.browser_local_generation.wrapping_add(1);
        state.browser_local_pending = None;
        state.browser_gateway_base = None;
        state.browser_gateway_loading = false;
        state.browser_chrome = browser_chrome::State::default();
        state.browser_permission = None;
        if let Some(browser) = &mut state.browser
            && let Err(error) = browser.reset_workspace()
        {
            state.browser_error = Some(error);
        }
    }
    #[cfg(not(feature = "cef-browser"))]
    let _ = state;
}

#[cfg(feature = "cef-browser")]
fn gateway_workspace(state: &Shell) -> Option<String> {
    state
        .active_workspace
        .as_ref()
        .map(|workspace| workspace.id.clone())
}
