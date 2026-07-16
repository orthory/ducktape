//! Native Browser chrome. The page pane itself is a direct CEF child window.

use iced::widget::{button, column, container, row, text, text_input, Space};
use iced::{Alignment, Background, Border, Color, Element, Length};

use crate::duck_url::validate_duck_host;
use crate::theme::{self, BODY, CAPTION, LABEL, MONO, Mode, SANS, SANS_SEMIBOLD, TITLE};

pub const IDLE_URL: &str = "about:blank";
pub const TAB_BAR_HEIGHT: f32 = 36.0;
pub const TOOLBAR_HEIGHT: f32 = 48.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    AddressChanged(String),
    Open,
    Reload,
    Back,
    Forward,
    NewTab,
    SelectTab(usize),
    CloseTab(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tab {
    pub address: String,
    pub current: String,
    history: Vec<String>,
    history_index: usize,
}

impl Default for Tab {
    fn default() -> Self {
        Self {
            address: "net.duck".into(),
            current: "net.duck".into(),
            history: Vec::new(),
            history_index: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub address: String,
    pub current: String,
    pub error: Option<String>,
    pub loading: bool,
    pub tabs: Vec<Tab>,
    pub active_tab: usize,
}

impl Default for State {
    fn default() -> Self {
        Self {
            address: "net.duck".into(),
            current: "net.duck".into(),
            error: None,
            loading: false,
            tabs: vec![Tab::default()],
            active_tab: 0,
        }
    }
}

impl State {
    pub fn is_idle(&self) -> bool {
        self.tabs
            .get(self.active_tab)
            .is_none_or(|tab| tab.history.is_empty())
    }

    pub fn runtime_url(&self) -> &str {
        self.runtime_url_at(self.active_tab).unwrap_or(IDLE_URL)
    }

    pub fn runtime_url_at(&self, index: usize) -> Option<&str> {
        let tab = self.tabs.get(index)?;
        Some(
            tab.history
                .get(tab.history_index)
                .map_or(IDLE_URL, String::as_str),
        )
    }

    pub fn runtime_urls(&self) -> Vec<&str> {
        self.tabs
            .iter()
            .map(|tab| {
                tab.history
                    .get(tab.history_index)
                    .map_or(IDLE_URL, String::as_str)
            })
            .collect()
    }

    pub fn url(&self) -> Result<String, String> {
        let address = self.address.trim();
        if address.is_empty()
            || address.len() > 2 * 1024
            || address
                .bytes()
                .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
        {
            return Err("Enter a valid .duck address.".into());
        }
        if address.contains("://")
            && !address
                .get(..7)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("duck://"))
        {
            return Err("Browser accepts only signed .duck routes.".into());
        }
        let address = address
            .get(..7)
            .filter(|prefix| prefix.eq_ignore_ascii_case("duck://"))
            .and_then(|_| address.get(7..))
            .unwrap_or(address);
        let host = address.split(['/', '?', '#']).next().unwrap_or_default();
        validate_duck_host(host)?;
        let host = host.to_ascii_lowercase();
        let suffix = &address[host.len()..];
        Ok(if suffix.is_empty() {
            format!("duck://{host}/")
        } else {
            format!("duck://{host}{suffix}")
        })
    }

    fn save_active(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.address.clone_from(&self.address);
            tab.current.clone_from(&self.current);
        }
    }

    fn load_active(&mut self) {
        if let Some(tab) = self.tabs.get(self.active_tab) {
            self.address.clone_from(&tab.address);
            self.current.clone_from(&tab.current);
        }
        self.error = None;
    }

    fn record_navigation(&mut self, url: String) {
        self.save_active();
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.history.truncate(tab.history_index.saturating_add(1));
            if tab.history.last() != Some(&url) {
                tab.history.push(url);
                tab.history_index = tab.history.len().saturating_sub(1);
            }
        }
    }

    fn move_history(&mut self, offset: isize) -> Option<String> {
        self.save_active();
        let tab = self.tabs.get_mut(self.active_tab)?;
        let index = tab.history_index.checked_add_signed(offset)?;
        let url = tab.history.get(index)?.clone();
        tab.history_index = index;
        let address = url.strip_prefix("duck://").unwrap_or(&url).to_owned();
        tab.address.clone_from(&address);
        tab.current.clone_from(&address);
        self.address = address.clone();
        self.current = address;
        self.loading = true;
        self.error = None;
        Some(url)
    }

    fn can_go_back(&self) -> bool {
        self.tabs
            .get(self.active_tab)
            .is_some_and(|tab| tab.history_index > 0)
    }

    fn can_go_forward(&self) -> bool {
        self.tabs
            .get(self.active_tab)
            .is_some_and(|tab| tab.history_index.saturating_add(1) < tab.history.len())
    }
}

pub fn is_network_url(raw: &str) -> bool {
    reqwest::Url::parse(raw).is_ok_and(|url| {
        url.scheme().eq_ignore_ascii_case("duck")
            && url
                .host_str()
                .is_some_and(|host| host.eq_ignore_ascii_case("net.duck"))
            && url.port().is_none()
            && url.username().is_empty()
            && url.password().is_none()
    })
}

/// Reconcile a main-frame URL reported by one persistent CEF tab.
pub fn commit_navigation(state: &mut State, tab_index: usize, url: &str) -> Result<(), String> {
    if url == IDLE_URL {
        return state
            .tabs
            .get(tab_index)
            .map(|_| ())
            .ok_or_else(|| "Browser tab no longer exists.".to_string());
    }
    let address = url
        .get(7..)
        .filter(|_| {
            url.get(..7)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("duck://"))
        })
        .ok_or_else(|| "Browser committed a non-.duck route.".to_string())?
        .to_string();
    let mut validated = state.clone();
    validated.address.clone_from(&address);
    validated.url()?;

    let tab = state
        .tabs
        .get_mut(tab_index)
        .ok_or_else(|| "Browser tab no longer exists.".to_string())?;
    if tab
        .history
        .get(tab.history_index)
        .is_none_or(|entry| entry != url)
    {
        tab.history.truncate(tab.history_index.saturating_add(1));
        tab.history.push(url.to_string());
        tab.history_index = tab.history.len() - 1;
    }
    tab.address.clone_from(&address);
    tab.current.clone_from(&address);
    if tab_index == state.active_tab {
        state.address = address.clone();
        state.current = address;
        state.loading = false;
        state.error = None;
    }
    Ok(())
}

pub fn update(state: &mut State, message: Message) -> Option<String> {
    match message {
        Message::AddressChanged(address) => {
            state.address = address;
            state.error = None;
            state.save_active();
            None
        }
        action @ (Message::Open | Message::Reload) => match state.url() {
            _ if matches!(action, Message::Reload) && state.is_idle() => None,
            Ok(url) => {
                state.current.clone_from(&state.address);
                state.loading = true;
                state.error = None;
                if matches!(action, Message::Open) {
                    state.record_navigation(url.clone());
                } else {
                    state.save_active();
                }
                Some(url)
            }
            Err(error) => {
                state.error = Some(error);
                None
            }
        },
        Message::Back => state.move_history(-1),
        Message::Forward => state.move_history(1),
        Message::NewTab => {
            state.save_active();
            state.tabs.push(Tab::default());
            state.active_tab = state.tabs.len() - 1;
            state.load_active();
            state.loading = false;
            Some(IDLE_URL.into())
        }
        Message::SelectTab(index) => {
            if index == state.active_tab || index >= state.tabs.len() {
                return None;
            }
            state.save_active();
            state.active_tab = index;
            state.load_active();
            state.loading = !state.is_idle();
            Some(state.runtime_url().into())
        }
        Message::CloseTab(index) => {
            if index >= state.tabs.len() {
                return None;
            }
            state.save_active();
            let old_active = state.active_tab;
            state.tabs.remove(index);
            if state.tabs.is_empty() {
                state.tabs.push(Tab::default());
                state.active_tab = 0;
            } else if index < old_active {
                state.active_tab = old_active - 1;
            } else if index == old_active {
                state.active_tab = index.min(state.tabs.len() - 1);
            } else {
                state.active_tab = old_active;
            }
            state.load_active();
            state.loading = !state.is_idle();
            Some(state.runtime_url().into())
        }
    }
}

pub fn view(state: &State, mode: Mode, cef_ready: bool) -> Element<'_, Message> {
    let p = theme::palette(mode);
    let mut tab_items = row![].spacing(3).align_y(Alignment::End);
    for (index, tab) in state.tabs.iter().enumerate() {
        let active = index == state.active_tab;
        // Left-aligned label, vertically centered via inner container (iced
        // `button` pins its content to the top-left otherwise).
        let select = button(
            container(
                text(&tab.current)
                    .font(SANS)
                    .size(CAPTION)
                    .color(if active { p.ink } else { p.muted_3 })
                    .wrapping(iced::widget::text::Wrapping::None),
            )
            .width(Length::Fill)
            .center_y(Length::Fill)
            .padding([0, 9]),
        )
        .width(Length::Fill)
        .height(28)
        .padding(0)
        .on_press(Message::SelectTab(index))
        .style(move |_, status| tab_style(p, active, status));
        #[cfg(all(feature = "agent", debug_assertions))]
        let select =
            iced_agent_plugin::sem(iced_agent_plugin::Role::Tab, tab.current.clone(), select);
        let close = icon_button("×", BODY, 24.0, 28.0, Some(Message::CloseTab(index)), "Close tab", move |status| {
            tab_style(p, active, status)
        });
        tab_items = tab_items.push(
            container(row![select, close].spacing(0).align_y(Alignment::Center))
                .width(180)
                .height(30)
                .style(move |_| container::Style {
                    background: Some(Background::Color(if active {
                        p.paper
                    } else {
                        p.sidebar
                    })),
                    border: Border {
                        color: if active { p.border_strong } else { p.border_soft },
                        width: 1.0,
                        radius: iced::border::top(theme::RADIUS_MD),
                    },
                    ..container::Style::default()
                }),
        );
    }
    tab_items = tab_items.push(icon_button(
        "+",
        BODY,
        30.0,
        28.0,
        Some(Message::NewTab),
        "New tab",
        move |status| chrome_style(p, status),
    ));
    // Section height is TAB_BAR_HEIGHT - 1 so the trailing 1px `divider` lands
    // the seam exactly on the CEF-child boundary (browser_session::bounds sums
    // the untouched TAB_BAR_HEIGHT + TOOLBAR_HEIGHT constants).
    let tabs = container(tab_items)
        .width(Length::Fill)
        .height(TAB_BAR_HEIGHT - 1.0)
        .padding(iced::Padding {
            top: 5.0,
            right: 10.0,
            bottom: 0.0,
            left: 10.0,
        })
        .align_y(iced::alignment::Vertical::Bottom)
        .style(move |_| section_bg(p.canvas));

    let has_error = state.error.is_some();
    let address = text_input("net.duck", &state.address)
        .font(MONO)
        .size(LABEL)
        .padding([0, 2])
        .on_input(Message::AddressChanged)
        .on_submit(Message::Open)
        .style(move |_, _| text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            icon: p.muted_2,
            placeholder: p.muted,
            value: p.ink,
            selection: p.chip,
        });
    let address = sem_input("Address", &state.address, address);

    let mut pill = row![].spacing(6).align_y(Alignment::Center);
    if !state
        .address
        .get(..7)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("duck://"))
    {
        pill = pill.push(text("duck://").font(MONO).size(LABEL).color(p.muted_2));
    }
    pill = pill.push(address);
    let pill = container(pill)
        .width(Length::Fill)
        .height(31)
        .padding([0, 10])
        .align_y(Alignment::Center)
        .style(move |_| container::Style {
            background: Some(Background::Color(p.paper)),
            border: Border {
                color: if has_error {
                    p.danger_border
                } else {
                    p.border_strong
                },
                width: 1.0,
                radius: theme::RADIUS_MD.into(),
            },
            ..container::Style::default()
        });

    let toolbar = container(
        row![
            text("Browser").font(SANS_SEMIBOLD).size(TITLE).color(p.filled),
            icon_button(
                "‹",
                BODY,
                30.0,
                30.0,
                state.can_go_back().then_some(Message::Back),
                "Back",
                move |status| chrome_style(p, status),
            ),
            icon_button(
                "›",
                BODY,
                30.0,
                30.0,
                state.can_go_forward().then_some(Message::Forward),
                "Forward",
                move |status| chrome_style(p, status),
            ),
            icon_button(
                "↻",
                TITLE,
                30.0,
                30.0,
                (!state.is_idle()).then_some(Message::Reload),
                "Reload",
                move |status| chrome_style(p, status),
            ),
            pill,
        ]
        .spacing(7)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .height(TOOLBAR_HEIGHT - 1.0)
    .padding([0, 12])
    .align_y(Alignment::Center)
    .style(move |_| section_bg(p.sidebar));

    let pane_copy = if let Some(error) = &state.error {
        error.as_str()
    } else if state.is_idle() {
        "Press Enter for net.duck, or type <account>.duck or <label>.<account>.duck."
    } else if !cef_ready {
        "Starting the isolated browser…"
    } else if state.loading {
        "Resolving route…"
    } else {
        ""
    };
    let pane = container(text(pane_copy).font(SANS).size(BODY).color(p.muted))
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(24)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(move |_| container::Style::default().background(p.canvas));
    #[cfg(all(feature = "agent", debug_assertions))]
    let pane = iced_agent_plugin::sem(iced_agent_plugin::Role::Region, "browser", pane);
    column![
        tabs,
        divider(p.border_soft),
        toolbar,
        divider(p.border_soft),
        pane,
    ]
    .spacing(0)
    .into()
}

/// The one icon-button shape: a glyph centered in a fixed `width`×`height` box
/// (iced `button` pins content top-left otherwise) with `on_press_maybe` and a
/// caller-supplied style catalog. Back/Forward/Reload/New-tab/Close all route
/// through it, so they share one grid and one centering rule.
fn icon_button<'a>(
    glyph: &'a str,
    size: f32,
    width: f32,
    height: f32,
    message: Option<Message>,
    name: &'a str,
    style: impl Fn(button::Status) -> button::Style + 'a,
) -> Element<'a, Message> {
    let enabled = message.is_some();
    let btn = button(
        container(text(glyph).font(SANS).size(size))
            .center_x(width)
            .center_y(height),
    )
    .padding(0)
    .on_press_maybe(message)
    .style(move |_, status| style(status));
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::Sem::new(iced_agent_plugin::Role::Button, name, btn)
        .disabled(!enabled)
        .into();
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    {
        let _ = (enabled, name);
        btn.into()
    }
}

/// A 1px filled hairline between chrome sections (mandate #6: the card owns the
/// frame, rows are borderless, dividers are 1px filled containers).
fn divider(color: Color) -> Element<'static, Message> {
    container(Space::new().width(Length::Fill).height(1))
        .width(Length::Fill)
        .height(1)
        .style(move |_| container::Style::default().background(color))
        .into()
}

/// Dev-only text-input tagging: wraps `input` in a `TextInput` semantic node
/// carrying `value`. Compiled out entirely unless the agent bridge is built.
#[cfg(all(feature = "agent", debug_assertions))]
fn sem_input<'a>(
    name: &'static str,
    value: &str,
    input: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    iced_agent_plugin::Sem::new(iced_agent_plugin::Role::TextInput, name, input)
        .value(value.to_string())
        .into()
}
#[cfg(not(all(feature = "agent", debug_assertions)))]
fn sem_input<'a>(
    _name: &'static str,
    _value: &str,
    input: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    input.into()
}

fn chrome_style(p: &theme::Palette, status: button::Status) -> button::Style {
    // Disabled controls must read disabled (mandate #4): dim the glyph and drop
    // the border/hover so a greyed nav arrow doesn't look pressable.
    if matches!(status, button::Status::Disabled) {
        return button::Style {
            background: Some(Background::Color(p.paper)),
            text_color: p.muted_2,
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: theme::RADIUS_SM.into(),
            },
            ..button::Style::default()
        };
    }
    button::Style {
        background: Some(Background::Color(
            if matches!(status, button::Status::Hovered | button::Status::Pressed) {
                p.hover
            } else {
                p.paper
            },
        )),
        text_color: p.ink_soft,
        border: Border {
            color: p.border_strong,
            width: 1.0,
            radius: theme::RADIUS_SM.into(),
        },
        ..button::Style::default()
    }
}

fn tab_style(p: &theme::Palette, active: bool, status: button::Status) -> button::Style {
    button::Style {
        background: matches!(status, button::Status::Hovered | button::Status::Pressed)
            .then_some(Background::Color(p.hover)),
        text_color: if active { p.ink } else { p.muted },
        border: Border::default(),
        ..button::Style::default()
    }
}

/// Background-only section fill; the seam between sections is a real 1px
/// `divider`, never a 4-side `Border` masquerading as an underline.
fn section_bg(background: Color) -> container::Style {
    container::Style {
        background: Some(Background::Color(background)),
        ..container::Style::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_input_accepts_only_duck_routes() {
        let mut state = State {
            address: "app.demo.duck/start".into(),
            ..State::default()
        };
        assert_eq!(state.url().unwrap(), "duck://app.demo.duck/start");
        state.address = "https://example.com".into();
        assert!(state.url().is_err());
        state.address = "javascript:alert(1)".into();
        assert!(state.url().is_err());
        state.address = "bad address".into();
        assert!(state.url().is_err());
    }

    #[test]
    fn address_input_normalizes_mixed_case_duck_scheme() {
        let state = State {
            address: "DuCk://App.Demo.Duck/start".into(),
            ..State::default()
        };

        assert_eq!(state.url().unwrap(), "duck://app.demo.duck/start");
    }

    #[test]
    fn tabs_keep_independent_history_and_close_without_leaving_zero_tabs() {
        let mut state = State {
            address: "docs.demo.duck".into(),
            ..State::default()
        };
        assert_eq!(
            update(&mut state, Message::Open),
            Some("duck://docs.demo.duck/".into())
        );

        assert_eq!(update(&mut state, Message::NewTab), Some(IDLE_URL.into()));
        assert_eq!(state.tabs.len(), 2);
        state.address = "chat.demo.duck".into();
        update(&mut state, Message::Open);
        assert_eq!(
            update(&mut state, Message::SelectTab(0)),
            Some("duck://docs.demo.duck/".into())
        );
        assert_eq!(state.address, "docs.demo.duck");

        update(&mut state, Message::CloseTab(0));
        update(&mut state, Message::CloseTab(0));
        assert_eq!(state.tabs.len(), 1);
        assert_eq!(state.current, "net.duck");
    }

    #[test]
    fn committed_page_navigation_updates_history_without_destroying_forward_history() {
        let mut state = State::default();
        commit_navigation(&mut state, 0, "duck://net.duck/docs").unwrap();
        commit_navigation(&mut state, 0, "duck://net.duck/reference").unwrap();
        assert_eq!(
            update(&mut state, Message::Back),
            Some("duck://net.duck/docs".into())
        );
        assert_eq!(state.address, "net.duck/docs");
        commit_navigation(&mut state, 0, "duck://net.duck/docs").unwrap();
        assert_eq!(
            update(&mut state, Message::Forward),
            Some("duck://net.duck/reference".into())
        );
    }

    #[test]
    fn fresh_tabs_stay_idle_until_the_user_opens_a_duck_route() {
        let mut state = State::default();
        assert!(state.is_idle());
        assert_eq!(state.runtime_url(), IDLE_URL);
        assert_eq!(update(&mut state, Message::Reload), None);

        assert_eq!(update(&mut state, Message::NewTab), Some(IDLE_URL.into()));
        assert!(state.is_idle());
        let active_tab = state.active_tab;
        commit_navigation(&mut state, active_tab, IDLE_URL).unwrap();
        assert!(state.error.is_none());
        assert!(state.tabs[state.active_tab].history.is_empty());

        assert_eq!(
            update(&mut state, Message::Open),
            Some("duck://net.duck/".into())
        );
        assert!(!state.is_idle());
        commit_navigation(&mut state, active_tab, IDLE_URL).unwrap();
        assert_eq!(state.runtime_url(), "duck://net.duck/");
    }

    #[test]
    fn runtime_urls_preserve_idle_tab_indices_before_first_cef_start() {
        let mut state = State::default();
        update(&mut state, Message::NewTab);
        update(&mut state, Message::NewTab);
        update(&mut state, Message::Open);

        assert_eq!(
            state.runtime_urls(),
            vec![IDLE_URL, IDLE_URL, "duck://net.duck/"]
        );
        assert_eq!(state.active_tab, 2);
    }

    #[test]
    fn network_urls_are_exactly_the_reserved_root() {
        assert!(is_network_url("duck://net.duck"));
        assert!(is_network_url("DUCK://NET.DUCK/docs.html"));
        assert!(!is_network_url("duck://api.net.duck/index.html"));
        assert!(!is_network_url("https://net.duck/index.html"));
        assert!(!is_network_url("about:blank"));

        assert!(State::default().url().is_ok());
        let mut reserved = State {
            address: "agents.duck".into(),
            ..State::default()
        };
        assert!(reserved.url().is_err());
        reserved.address = "api.net.duck".into();
        assert!(reserved.url().is_err());
    }

    #[test]
    fn network_root_commit_does_not_duplicate_history() {
        let mut state = State::default();
        update(&mut state, Message::Open);
        commit_navigation(&mut state, 0, "duck://net.duck/").unwrap();

        assert_eq!(state.tabs[0].history, vec!["duck://net.duck/"]);
        assert!(!state.can_go_back());
    }

    #[test]
    fn uppercase_host_commit_does_not_duplicate_history() {
        let mut state = State {
            address: "DUCK://NET.DUCK/Docs.HTML".into(),
            ..State::default()
        };
        assert_eq!(
            update(&mut state, Message::Open),
            Some("duck://net.duck/Docs.HTML".into())
        );
        commit_navigation(&mut state, 0, "duck://net.duck/Docs.HTML").unwrap();

        assert_eq!(state.tabs[0].history.len(), 1);
        assert!(!state.can_go_back());
    }

    #[test]
    fn closing_a_tab_before_the_active_last_tab_keeps_that_tab_active() {
        let mut state = State::default();
        update(&mut state, Message::NewTab);
        state.address = "middle.demo.duck".into();
        update(&mut state, Message::Open);
        update(&mut state, Message::NewTab);
        state.address = "last.demo.duck".into();
        update(&mut state, Message::Open);

        assert_eq!(state.active_tab, 2);
        assert_eq!(
            update(&mut state, Message::CloseTab(0)),
            Some("duck://last.demo.duck/".into())
        );
        assert_eq!(state.active_tab, 1);
        assert_eq!(state.address, "last.demo.duck");
    }

    #[test]
    fn background_tab_commit_does_not_replace_active_chrome() {
        let mut state = State::default();
        update(&mut state, Message::NewTab);
        state.address = "active.demo.duck".into();
        update(&mut state, Message::Open);

        commit_navigation(&mut state, 0, "duck://net.duck/background").unwrap();

        assert_eq!(state.address, "active.demo.duck");
        assert_eq!(state.tabs[0].current, "net.duck/background");
    }
}
