//! Native Settings surface.
//!
//! Settings keeps console preferences and workspace lifecycle. Account identity,
//! membership, and daemon details remain links to their canonical screens.

use iced::widget::{Button, Column, Space, button, column, container, row, scrollable, text};
use iced::{Alignment, Background, Border, Color, Element, Length};

use crate::theme::{self, MONO, Palette, RADIUS_LG, RADIUS_MD, RADIUS_SM, SANS};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resource<T> {
    Loading,
    Empty,
    Error(String),
    Ready(T),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationCategory {
    Mentions,
    Replies,
    Huddles,
    Runs,
    Forge,
    Governance,
}

impl NotificationCategory {
    const ALL: [Self; 6] = [
        Self::Mentions,
        Self::Replies,
        Self::Huddles,
        Self::Runs,
        Self::Forge,
        Self::Governance,
    ];

    const fn title(self) -> &'static str {
        match self {
            Self::Mentions => "Mentions",
            Self::Replies => "Replies",
            Self::Huddles => "Huddles",
            Self::Runs => "Agent runs",
            Self::Forge => "Forge",
            Self::Governance => "Governance",
        }
    }

    const fn detail(self) -> &'static str {
        match self {
            Self::Mentions => "When someone mentions you in a channel.",
            Self::Replies => "When someone replies to one of your messages.",
            Self::Huddles => "When a channel huddle starts.",
            Self::Runs => "When an agent run completes or needs attention.",
            Self::Forge => "For Forge activity that needs your attention.",
            Self::Governance => "For governance proposals and voting activity.",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationPrefs {
    pub enabled: bool,
    pub mentions: bool,
    pub replies: bool,
    pub huddles: bool,
    pub runs: bool,
    pub forge: bool,
    pub governance: bool,
    pub muted_channels: Vec<String>,
}

impl Default for NotificationPrefs {
    fn default() -> Self {
        Self {
            enabled: true,
            mentions: true,
            replies: true,
            huddles: true,
            runs: true,
            forge: true,
            governance: true,
            muted_channels: Vec::new(),
        }
    }
}

impl NotificationPrefs {
    fn get(&self, category: NotificationCategory) -> bool {
        match category {
            NotificationCategory::Mentions => self.mentions,
            NotificationCategory::Replies => self.replies,
            NotificationCategory::Huddles => self.huddles,
            NotificationCategory::Runs => self.runs,
            NotificationCategory::Forge => self.forge,
            NotificationCategory::Governance => self.governance,
        }
    }

    fn toggle(&mut self, category: NotificationCategory) {
        match category {
            NotificationCategory::Mentions => self.mentions = !self.mentions,
            NotificationCategory::Replies => self.replies = !self.replies,
            NotificationCategory::Huddles => self.huddles = !self.huddles,
            NotificationCategory::Runs => self.runs = !self.runs,
            NotificationCategory::Forge => self.forge = !self.forge,
            NotificationCategory::Governance => self.governance = !self.governance,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsData {
    pub client_mode: bool,
    pub can_control_node: bool,
    pub workspace_name: Option<String>,
    pub network_id: Option<String>,
    pub active_channel: Option<String>,
    pub in_validator_set: bool,
    pub validator_count: usize,
    pub roster_loaded: bool,
    pub forget_needs_force: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DangerAction {
    Leave,
    Forget,
    ForceForget,
}

impl DangerAction {
    const fn confirm_label(self) -> &'static str {
        match self {
            Self::Leave => "Request leave",
            Self::Forget => "Forget workspace",
            Self::ForceForget => "Force forget",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub data: Resource<SettingsData>,
    pub mode: theme::Mode,
    pub accent: usize,
    pub notifications: NotificationPrefs,
    pub pending: Option<DangerAction>,
    pub saving: bool,
    pub error: Option<String>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            data: Resource::Loading,
            mode: theme::Mode::Light,
            accent: 0,
            notifications: NotificationPrefs::default(),
            pending: None,
            saving: false,
            error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Load,
    ToggleTheme,
    SetAccent(usize),
    ToggleNotifications,
    ToggleCategory(NotificationCategory),
    ToggleActiveChannel(String),
    OpenAccount,
    OpenNetworks,
    OpenMembers,
    OpenNode,
    AskDanger(DangerAction),
    CancelDanger,
    ConfirmDanger,
    Service(ServiceEvent),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Load,
    SetTheme(theme::Mode),
    SetAccent(usize),
    SetNotifications(NotificationPrefs),
    OpenAccount,
    OpenNetworks,
    OpenMembers,
    OpenNode,
    RequestLeave,
    ForgetWorkspace { force: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceEvent {
    Loaded(Result<Option<SettingsData>, String>),
    PreferencesSaved(Result<(), String>),
    DangerFinished(Result<(), String>),
}

pub fn update(state: &mut State, message: Message) -> Option<Command> {
    match message {
        Message::Load => {
            state.data = Resource::Loading;
            Some(Command::Load)
        }
        Message::ToggleTheme => {
            state.mode = state.mode.toggled();
            state.saving = true;
            Some(Command::SetTheme(state.mode))
        }
        Message::SetAccent(index) => {
            if index >= 5 {
                return None;
            }
            state.accent = index;
            state.saving = true;
            Some(Command::SetAccent(index))
        }
        Message::ToggleNotifications => {
            state.notifications.enabled = !state.notifications.enabled;
            state.saving = true;
            Some(Command::SetNotifications(state.notifications.clone()))
        }
        Message::ToggleCategory(category) => {
            if !state.notifications.enabled {
                return None;
            }
            state.notifications.toggle(category);
            state.saving = true;
            Some(Command::SetNotifications(state.notifications.clone()))
        }
        Message::ToggleActiveChannel(channel) => {
            if let Some(index) = state
                .notifications
                .muted_channels
                .iter()
                .position(|item| item == &channel)
            {
                state.notifications.muted_channels.remove(index);
            } else {
                state.notifications.muted_channels.push(channel);
                state.notifications.muted_channels.sort();
                state.notifications.muted_channels.dedup();
            }
            state.saving = true;
            Some(Command::SetNotifications(state.notifications.clone()))
        }
        Message::OpenAccount => Some(Command::OpenAccount),
        Message::OpenNetworks => Some(Command::OpenNetworks),
        Message::OpenMembers => Some(Command::OpenMembers),
        Message::OpenNode => Some(Command::OpenNode),
        Message::AskDanger(action) => {
            if danger_allowed(state, action) {
                state.pending = Some(action);
            }
            None
        }
        Message::CancelDanger => {
            state.pending = None;
            None
        }
        Message::ConfirmDanger => {
            let action = state.pending.take()?;
            if !danger_allowed(state, action) {
                return None;
            }
            state.saving = true;
            match action {
                DangerAction::Leave => Some(Command::RequestLeave),
                DangerAction::Forget => Some(Command::ForgetWorkspace { force: false }),
                DangerAction::ForceForget => Some(Command::ForgetWorkspace { force: true }),
            }
        }
        Message::Service(event) => {
            match event {
                ServiceEvent::Loaded(result) => {
                    state.data = match result {
                        Ok(Some(data)) => Resource::Ready(data),
                        Ok(None) => Resource::Empty,
                        Err(error) => Resource::Error(error),
                    };
                }
                ServiceEvent::PreferencesSaved(result) => {
                    state.saving = false;
                    state.error = result.err();
                }
                ServiceEvent::DangerFinished(result) => {
                    state.saving = false;
                    match result {
                        Ok(()) => return Some(Command::Load),
                        Err(error) => state.error = Some(error),
                    }
                }
            }
            None
        }
    }
}

fn danger_allowed(state: &State, action: DangerAction) -> bool {
    let Resource::Ready(data) = &state.data else {
        return false;
    };
    if data.client_mode || !data.can_control_node || state.saving {
        return false;
    }
    match action {
        DangerAction::Leave => {
            data.in_validator_set && !(data.roster_loaded && data.validator_count < 2)
        }
        DangerAction::Forget => true,
        DangerAction::ForceForget => data.forget_needs_force,
    }
}

pub fn view(state: &State) -> Element<'_, Message> {
    let p = *theme::palette(state.mode);
    let Resource::Ready(data) = &state.data else {
        return resource_view(&state.data, p);
    };

    let mut content = column![
        text("Settings").font(SANS).size(16).color(p.ink),
        section_label("ACCOUNT", false, p),
        group_card(
            column![control_row(
                "Your account",
                "Display name, recovery phrase, linked devices, and your nodes.",
                outline_button("Open Account", Message::OpenAccount, true, p),
                true,
                p,
            )],
            p,
        ),
        section_label("PREFERENCES", false, p),
        preferences_card(state, p),
        section_label("NOTIFICATIONS", false, p),
        notifications_card(state, data, p),
        section_label("NETWORK", false, p),
        network_card(data, p),
    ]
    .spacing(9);

    if !data.client_mode {
        content = content
            .push(section_label("DANGER ZONE", true, p))
            .push(danger_zone(state, data, p));
    }
    if let Some(error) = &state.error {
        content = content.push(error_banner(error, p));
    }
    if let Some(action) = state.pending {
        content = content.push(confirm_card(action, data, p));
    }
    content = content.push(Space::new().height(22));
    container(scrollable(container(content).padding(22)))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| surface(p.canvas))
        .into()
}

fn preferences_card(state: &State, p: Palette) -> Element<'_, Message> {
    let mut accents = row![].spacing(7).align_y(Alignment::Center);
    for index in 0..5 {
        accents = accents.push(accent_button(
            index,
            state.accent == index,
            Message::SetAccent(index),
            p,
        ));
    }
    group_card(
        column![
            control_row(
                "Theme",
                "Dark mode",
                switch_button(
                    state.mode == theme::Mode::Dark,
                    Message::ToggleTheme,
                    true,
                    accent_color(state.accent, p),
                    p,
                ),
                false,
                p,
            ),
            control_row(
                "Accent",
                "Used for active navigation, focus, and primary controls.",
                accents,
                true,
                p,
            ),
        ],
        p,
    )
}

fn notifications_card<'a>(
    state: &'a State,
    data: &'a SettingsData,
    p: Palette,
) -> Element<'a, Message> {
    let accent = accent_color(state.accent, p);
    let mut rows = column![control_row(
        "Enable notifications",
        "Allow Ducktape to send native desktop notifications.",
        switch_button(
            state.notifications.enabled,
            Message::ToggleNotifications,
            true,
            accent,
            p,
        ),
        false,
        p,
    )];
    let visible: Vec<_> = NotificationCategory::ALL
        .into_iter()
        .filter(|category| !data.client_mode || *category != NotificationCategory::Governance)
        .collect();
    for (index, category) in visible.iter().enumerate() {
        let last = index == visible.len() - 1 && data.active_channel.is_none();
        rows = rows.push(control_row(
            category.title(),
            category.detail(),
            switch_button(
                state.notifications.get(*category),
                Message::ToggleCategory(*category),
                state.notifications.enabled,
                accent,
                p,
            ),
            last,
            p,
        ));
    }
    if let Some(channel) = &data.active_channel {
        let muted = state.notifications.muted_channels.contains(channel);
        rows = rows.push(control_row(
            if muted {
                "Unmute current channel"
            } else {
                "Mute current channel"
            },
            if muted {
                "Resume notifications from the current channel."
            } else {
                "Suppress notifications from the current channel."
            },
            switch_button(
                !muted,
                Message::ToggleActiveChannel(channel.clone()),
                true,
                accent,
                p,
            ),
            true,
            p,
        ));
    }
    group_card(rows, p)
}

fn network_card(data: &SettingsData, p: Palette) -> Element<'_, Message> {
    let mut rows = column![
        info_row(
            "Network name",
            data.workspace_name.as_deref().unwrap_or("Remote node"),
            false,
            p,
        ),
        info_row(
            "Network ID",
            data.network_id.as_deref().unwrap_or("not available"),
            false,
            p,
        ),
        control_row(
            "Switch network",
            if data.client_mode {
                "Connect to another network or remote node."
            } else {
                "Create, join, or select another local network."
            },
            outline_button("Networks", Message::OpenNetworks, true, p),
            data.client_mode && !data.can_control_node,
            p,
        ),
    ];
    if !data.client_mode {
        rows = rows.push(control_row(
            "Members & invites",
            "Invite, admit, and manage members from the Members view.",
            outline_button("Open Members", Message::OpenMembers, true, p),
            !data.can_control_node,
            p,
        ));
    }
    if data.can_control_node {
        rows = rows.push(control_row(
            "Node & daemon",
            "Start or stop the daemon and inspect ports, data dir, and quorum from the Node view.",
            outline_button("Open Node", Message::OpenNode, true, p),
            true,
            p,
        ));
    }
    group_card(rows, p)
}

fn danger_zone<'a>(state: &'a State, data: &'a SettingsData, p: Palette) -> Element<'a, Message> {
    let solo_known = data.roster_loaded && data.validator_count < 2;
    let leave_detail = if data.in_validator_set && solo_known {
        "Submits an on-chain self-removal. A solo node can't remove the last validator — forget it below."
    } else {
        "Submits an on-chain self-removal pending a strict majority of the remaining members. Your node keeps running until they approve."
    };
    let mut rows = column![
        danger_row(
            "Leave this network",
            leave_detail,
            "Request leave",
            Message::AskDanger(DangerAction::Leave),
            danger_allowed(state, DangerAction::Leave),
            p,
        ),
        danger_row(
            "Forget this workspace",
            "Stops this node and deletes the workspace locally. Refused while this node is still a current validator of a network with other members.",
            "Forget workspace",
            Message::AskDanger(DangerAction::Forget),
            danger_allowed(state, DangerAction::Forget),
            p,
        ),
    ]
    .spacing(9);
    if data.forget_needs_force {
        rows = rows.push(danger_row(
            "Force-forget (node won't start)",
            "Skips the liveness confirmation and deletes the directory, node key, and registry entry. Only for a solo or defunct network.",
            "Force forget",
            Message::AskDanger(DangerAction::ForceForget),
            danger_allowed(state, DangerAction::ForceForget),
            p,
        ));
    }
    rows.into()
}

fn confirm_card(
    action: DangerAction,
    data: &SettingsData,
    p: Palette,
) -> Element<'static, Message> {
    let name = data.workspace_name.as_deref().unwrap_or("this workspace");
    let (title, detail) = match action {
        DangerAction::Leave => (
            format!("Request to leave {name}?"),
            "This submits an on-chain self-removal and casts this node's yes ballot. Your node keeps running until a strict majority approves.",
        ),
        DangerAction::Forget => (
            format!("Forget {name}?"),
            "This stops this node and deletes the workspace locally. It is refused while this node remains a validator of a network with other members.",
        ),
        DangerAction::ForceForget => (
            format!("Force-forget {name}?"),
            "This skips the liveness confirmation and deletes the workspace, including directory, node key, and registry entry. Only use this for a solo or defunct network.",
        ),
    };
    container(
        column![
            text(title).font(SANS).size(14).color(p.ink),
            text(detail).font(SANS).size(11.5).color(p.muted_3),
            row![
                outline_button("Cancel", Message::CancelDanger, true, p),
                danger_button(action.confirm_label(), Message::ConfirmDanger, true, p,),
            ]
            .spacing(7),
        ]
        .spacing(10),
    )
    .width(Length::Fill)
    .padding(15)
    .style(move |_| rounded_surface(p.paper, p.border_strong, RADIUS_LG))
    .into()
}

fn resource_view<'a>(resource: &'a Resource<SettingsData>, p: Palette) -> Element<'a, Message> {
    let (title, detail, retry) = match resource {
        Resource::Loading => (
            "Loading Settings",
            "Reading preferences and workspace state…",
            false,
        ),
        Resource::Empty => (
            "No active network",
            "Choose a network before opening Settings.",
            true,
        ),
        Resource::Error(error) => ("Settings unavailable", error.as_str(), true),
        Resource::Ready(_) => unreachable!(),
    };
    let mut body = column![
        text(title).font(SANS).size(15).color(p.ink),
        text(detail).font(SANS).size(11.5).color(p.muted_2),
    ]
    .spacing(8)
    .align_x(Alignment::Center);
    if retry {
        body = body.push(outline_button("Retry", Message::Load, true, p));
    }
    container(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(move |_| surface(p.canvas))
        .into()
}

// --- Settings widget vocabulary ----------------------------------------

fn section_label(label: &'static str, danger: bool, p: Palette) -> Element<'static, Message> {
    container(
        text(label)
            .font(MONO)
            .size(9)
            .color(if danger { p.danger } else { p.muted_2 }),
    )
    .padding([11, 0])
    .into()
}

fn group_card<'a>(content: Column<'a, Message>, p: Palette) -> Element<'a, Message> {
    container(content)
        .width(Length::Fill)
        .style(move |_| rounded_surface(p.paper, p.border, RADIUS_LG))
        .into()
}

fn control_row<'a>(
    title: &'a str,
    detail: &'a str,
    control: impl Into<Element<'a, Message>>,
    last: bool,
    p: Palette,
) -> Element<'a, Message> {
    container(
        row![
            column![
                text(title).font(SANS).size(12.5).color(p.ink_soft),
                text(detail).font(SANS).size(10.5).color(p.muted_2),
            ]
            .spacing(2),
            Space::new().width(Length::Fill),
            control.into(),
        ]
        .spacing(16)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([13, 15])
    .style(move |_| row_surface(last, p))
    .into()
}

fn info_row<'a>(label: &'a str, value: &'a str, last: bool, p: Palette) -> Element<'a, Message> {
    container(
        row![
            text(label).font(SANS).size(12.5).color(p.ink_soft),
            Space::new().width(Length::Fill),
            text(value).font(MONO).size(12).color(p.muted),
        ]
        .spacing(16)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([13, 15])
    .style(move |_| row_surface(last, p))
    .into()
}

fn danger_row<'a>(
    title: &'a str,
    detail: &'a str,
    label: &'static str,
    message: Message,
    enabled: bool,
    p: Palette,
) -> Element<'a, Message> {
    container(
        row![
            column![
                text(title).font(SANS).size(12.5).color(p.ink_soft),
                text(detail).font(SANS).size(10.5).color(p.muted_2),
            ]
            .spacing(2),
            Space::new().width(Length::Fill),
            danger_button(label, message, enabled, p),
        ]
        .spacing(13)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding(15)
    .style(move |_| rounded_surface(p.danger_soft, p.danger_border, RADIUS_LG))
    .into()
}

fn switch_button<'a>(
    checked: bool,
    message: Message,
    enabled: bool,
    accent: Color,
    p: Palette,
) -> Button<'a, Message> {
    let button = button(
        row![
            container(Space::new().width(12).height(12)).style(move |_| {
                rounded_surface(
                    if checked { p.paper } else { p.muted_2 },
                    Color::TRANSPARENT,
                    99.0,
                )
            }),
            text(if checked { "ON" } else { "OFF" })
                .font(MONO)
                .size(8)
                .color(if checked { p.paper } else { p.muted_2 }),
        ]
        .spacing(4)
        .align_y(Alignment::Center),
    )
    .padding([3, 6])
    .style(move |_, _| iced::widget::button::Style {
        background: Some(Background::Color(if checked { accent } else { p.paper })),
        text_color: p.ink,
        border: Border {
            color: if checked { accent } else { p.border_strong },
            width: 1.0,
            radius: 99.0.into(),
        },
        ..Default::default()
    });
    if enabled {
        button.on_press(message)
    } else {
        button
    }
}

fn accent_button<'a>(
    index: usize,
    selected: bool,
    message: Message,
    p: Palette,
) -> Button<'a, Message> {
    let color = accent_color(index, p);
    button(Space::new().width(14).height(14))
        .padding(3)
        .style(move |_, _| iced::widget::button::Style {
            background: Some(Background::Color(color)),
            border: Border {
                color: if selected { p.ink } else { p.border_strong },
                width: if selected { 3.0 } else { 1.0 },
                radius: 99.0.into(),
            },
            ..Default::default()
        })
        .on_press(message)
}

fn outline_button<'a>(
    label: impl ToString,
    message: Message,
    enabled: bool,
    p: Palette,
) -> Button<'a, Message> {
    let button = button(text(label.to_string()).font(SANS).size(11.5))
        .padding([7, 13])
        .style(move |_, status| iced::widget::button::Style {
            background: Some(Background::Color(
                if enabled && matches!(status, iced::widget::button::Status::Hovered) {
                    p.titlebar
                } else {
                    p.paper
                },
            )),
            text_color: if enabled { p.muted_3 } else { p.muted_2 },
            border: Border {
                color: if enabled {
                    p.border_strong
                } else {
                    p.border_soft
                },
                width: 1.0,
                radius: RADIUS_SM.into(),
            },
            ..Default::default()
        });
    if enabled {
        button.on_press(message)
    } else {
        button
    }
}

fn danger_button<'a>(
    label: impl ToString,
    message: Message,
    enabled: bool,
    p: Palette,
) -> Button<'a, Message> {
    let button = button(text(label.to_string()).font(SANS).size(11.5))
        .padding([8, 15])
        .style(move |_, _| iced::widget::button::Style {
            background: Some(Background::Color(if enabled {
                p.red
            } else {
                p.border_soft
            })),
            text_color: if enabled { p.paper } else { p.muted_2 },
            border: Border {
                radius: RADIUS_SM.into(),
                ..Default::default()
            },
            ..Default::default()
        });
    if enabled {
        button.on_press(message)
    } else {
        button
    }
}

fn error_banner<'a>(copy: &'a str, p: Palette) -> Element<'a, Message> {
    container(text(copy).font(SANS).size(11.5).color(p.danger))
        .width(Length::Fill)
        .padding([10, 13])
        .style(move |_| rounded_surface(p.danger_soft, p.danger_border, RADIUS_MD))
        .into()
}

fn surface(color: Color) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(color)),
        ..Default::default()
    }
}

fn rounded_surface(color: Color, border: Color, radius: f32) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(color)),
        border: Border {
            color: border,
            width: 1.0,
            radius: radius.into(),
        },
        ..Default::default()
    }
}

fn row_surface(last: bool, p: Palette) -> iced::widget::container::Style {
    iced::widget::container::Style {
        border: Border {
            color: p.border_soft,
            width: if last { 0.0 } else { 1.0 },
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

fn accent_color(index: usize, p: Palette) -> Color {
    match index {
        0 => theme::ACCENTS[0],
        1 => theme::ACCENTS[1],
        2 => theme::ACCENTS[2],
        3 => p.purple,
        _ => p.red,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready() -> State {
        State {
            data: Resource::Ready(SettingsData {
                client_mode: false,
                can_control_node: true,
                workspace_name: Some("studio".into()),
                network_id: Some("duck-test".into()),
                active_channel: Some("general".into()),
                in_validator_set: true,
                validator_count: 3,
                roster_loaded: true,
                forget_needs_force: false,
            }),
            ..State::default()
        }
    }

    #[test]
    fn preference_changes_are_immediate_and_transport_free() {
        let mut state = ready();
        assert_eq!(
            update(&mut state, Message::ToggleTheme),
            Some(Command::SetTheme(theme::Mode::Dark))
        );
        assert_eq!(
            update(
                &mut state,
                Message::ToggleCategory(NotificationCategory::Mentions)
            ),
            Some(Command::SetNotifications(state.notifications.clone()))
        );
        assert!(!state.notifications.mentions);
    }

    #[test]
    fn disabled_notification_categories_do_not_emit_commands() {
        let mut state = ready();
        state.notifications.enabled = false;
        assert_eq!(
            update(
                &mut state,
                Message::ToggleCategory(NotificationCategory::Replies)
            ),
            None
        );
        assert!(state.notifications.replies);
    }

    #[test]
    fn destructive_actions_require_a_visible_confirmation() {
        let mut state = ready();
        assert_eq!(
            update(&mut state, Message::AskDanger(DangerAction::Forget)),
            None
        );
        assert_eq!(state.pending, Some(DangerAction::Forget));
        assert_eq!(
            update(&mut state, Message::ConfirmDanger),
            Some(Command::ForgetWorkspace { force: false })
        );
        assert_eq!(state.pending, None);
    }

    #[test]
    fn leave_is_hidden_for_a_known_solo_validator() {
        let mut state = ready();
        let Resource::Ready(data) = &mut state.data else {
            unreachable!()
        };
        data.validator_count = 1;
        assert_eq!(
            update(&mut state, Message::AskDanger(DangerAction::Leave)),
            None
        );
        assert_eq!(state.pending, None);
    }

    #[test]
    fn force_forget_only_exists_after_the_guarded_backend_requests_it() {
        let mut state = ready();
        assert_eq!(
            update(&mut state, Message::AskDanger(DangerAction::ForceForget)),
            None
        );
        let Resource::Ready(data) = &mut state.data else {
            unreachable!()
        };
        data.forget_needs_force = true;
        update(&mut state, Message::AskDanger(DangerAction::ForceForget));
        assert_eq!(state.pending, Some(DangerAction::ForceForget));
    }
}
