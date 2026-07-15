//! Native Browser chrome. The page pane itself is a direct CEF child window.

use iced::widget::{button, column, container, row, text, text_input};
use iced::{Alignment, Background, Border, Element, Length};

use crate::theme::{self, MONO, Mode, SANS};

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
            history: vec!["duck://net.duck".into()],
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
            .strip_prefix("duck://")
            .or_else(|| address.strip_prefix("DUCK://"))
            .unwrap_or(address);
        let host = address.split(['/', '?', '#']).next().unwrap_or_default();
        let labels = host.split('.').collect::<Vec<_>>();
        if !(labels.len() == 2 || labels.len() == 3)
            || labels
                .last()
                .is_none_or(|label| !label.eq_ignore_ascii_case("duck"))
            || labels.iter().any(|label| {
                label.is_empty()
                    || label.len() > 63
                    || label.starts_with('-')
                    || label.ends_with('-')
                    || !label
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            })
        {
            return Err("Enter <account>.duck or <label>.<account>.duck.".into());
        }
        Ok(format!("duck://{address}"))
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

/// Reconcile a main-frame URL reported by one persistent CEF tab.
pub fn commit_navigation(state: &mut State, tab_index: usize, url: &str) -> Result<(), String> {
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
            state.loading = true;
            Some("duck://net.duck".into())
        }
        Message::SelectTab(index) => {
            if index == state.active_tab || index >= state.tabs.len() {
                return None;
            }
            state.save_active();
            state.active_tab = index;
            state.load_active();
            state.loading = true;
            state.tabs[index]
                .history
                .get(state.tabs[index].history_index)
                .cloned()
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
            state.loading = true;
            state.tabs[state.active_tab]
                .history
                .get(state.tabs[state.active_tab].history_index)
                .cloned()
        }
    }
}

pub fn view(state: &State, mode: Mode, cef_ready: bool) -> Element<'_, Message> {
    let p = theme::palette(mode);
    let mut tab_items = row![].spacing(3).align_y(Alignment::End);
    for (index, tab) in state.tabs.iter().enumerate() {
        let active = index == state.active_tab;
        let select = button(text(&tab.current).font(SANS).size(10.5))
            .width(Length::Fill)
            .height(28)
            .padding([0, 8])
            .on_press(Message::SelectTab(index))
            .style(move |_, status| tab_style(p, active, status));
        let close = button(text("×").font(SANS).size(12))
            .width(24)
            .height(28)
            .padding(0)
            .on_press(Message::CloseTab(index))
            .style(move |_, status| tab_style(p, active, status));
        tab_items = tab_items.push(
            container(row![select, close].spacing(0).align_y(Alignment::Center))
                .width(180)
                .height(30)
                .style(move |_| container::Style {
                    background: Some(Background::Color(if active { p.paper } else { p.panel })),
                    border: Border {
                        color: p.border,
                        width: 1.0,
                        radius: iced::border::top(theme::RADIUS_MD),
                    },
                    ..container::Style::default()
                }),
        );
    }
    tab_items = tab_items.push(chrome_button("+", Some(Message::NewTab), p));
    let tabs = container(tab_items)
        .height(36)
        .padding(iced::Padding {
            top: 5.0,
            right: 10.0,
            bottom: 0.0,
            left: 10.0,
        })
        .align_y(iced::alignment::Vertical::Bottom)
        .style(move |_| bottom_border(p.canvas, p.border_soft));

    let address = text_input("net.duck", &state.address)
        .font(MONO)
        .size(11.5)
        .padding([0, 3])
        .on_input(Message::AddressChanged)
        .on_submit(Message::Open)
        .style(move |_, status| {
            let active = matches!(
                status,
                text_input::Status::Focused { .. } | text_input::Status::Hovered
            );
            text_input::Style {
                background: Background::Color(p.paper),
                border: Border {
                    color: if state.error.is_some() {
                        p.danger_border
                    } else if active {
                        p.border_strong
                    } else {
                        p.border
                    },
                    width: 1.0,
                    radius: theme::RADIUS_MD.into(),
                },
                icon: p.muted_2,
                placeholder: p.muted,
                value: p.ink,
                selection: p.chip,
            }
        });
    let toolbar = container(
        row![
            text("Browser").font(SANS).size(14).color(p.filled),
            chrome_button("‹", state.can_go_back().then_some(Message::Back), p,),
            chrome_button("›", state.can_go_forward().then_some(Message::Forward), p,),
            button(text("↻").font(SANS).size(14).color(p.muted_3))
                .width(30)
                .height(30)
                .padding(8)
                .on_press(Message::Reload)
                .style(move |_, status| chrome_style(p, status)),
            container(
                row![
                    text(
                        if state
                            .address
                            .get(..7)
                            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("duck://"))
                        {
                            ""
                        } else {
                            "duck://"
                        }
                    )
                    .font(MONO)
                    .size(11)
                    .color(p.muted_2),
                    address,
                ]
                .spacing(2)
                .align_y(Alignment::Center),
            )
            .width(Length::Fill),
        ]
        .spacing(7)
        .align_y(Alignment::Center),
    )
    .height(48)
    .padding([0, 12])
    .align_y(Alignment::Center)
    .style(move |_| bottom_border(p.sidebar, p.border_soft));

    let pane_copy = if let Some(error) = &state.error {
        error.as_str()
    } else if !cef_ready {
        "Starting the isolated browser…"
    } else if state.loading {
        "Resolving route…"
    } else {
        ""
    };
    let pane = container(text(pane_copy).font(SANS).size(11).color(p.muted))
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(move |_| container::Style::default().background(p.canvas));
    column![tabs, toolbar, pane].spacing(0).into()
}

fn chrome_button<'a>(
    label: &'a str,
    message: Option<Message>,
    p: &'a theme::Palette,
) -> iced::widget::Button<'a, Message> {
    button(text(label).font(SANS).size(13))
        .width(30)
        .height(30)
        .padding(0)
        .on_press_maybe(message)
        .style(move |_, status| chrome_style(p, status))
}

fn chrome_style(p: &theme::Palette, status: button::Status) -> button::Style {
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

fn bottom_border(background: iced::Color, border: iced::Color) -> container::Style {
    container::Style {
        background: Some(Background::Color(background)),
        border: Border {
            color: border,
            width: 1.0,
            radius: 0.0.into(),
        },
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
    fn tabs_keep_independent_history_and_close_without_leaving_zero_tabs() {
        let mut state = State::default();
        state.address = "docs.demo.duck".into();
        assert_eq!(
            update(&mut state, Message::Open),
            Some("duck://docs.demo.duck".into())
        );
        assert_eq!(
            update(&mut state, Message::Back),
            Some("duck://net.duck".into())
        );
        assert_eq!(
            update(&mut state, Message::Forward),
            Some("duck://docs.demo.duck".into())
        );

        assert_eq!(
            update(&mut state, Message::NewTab),
            Some("duck://net.duck".into())
        );
        assert_eq!(state.tabs.len(), 2);
        state.address = "chat.demo.duck".into();
        update(&mut state, Message::Open);
        assert_eq!(
            update(&mut state, Message::SelectTab(0)),
            Some("duck://docs.demo.duck".into())
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
        assert_eq!(state.address, "net.duck/docs");
        assert_eq!(
            update(&mut state, Message::Back),
            Some("duck://net.duck".into())
        );
        commit_navigation(&mut state, 0, "duck://net.duck").unwrap();
        assert_eq!(
            update(&mut state, Message::Forward),
            Some("duck://net.duck/docs".into())
        );
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
            Some("duck://last.demo.duck".into())
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
