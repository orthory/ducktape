use super::*;
use iced::widget::{column, row, stack, tooltip};

fn notifications_overlay(state: &Shell) -> Element<'_, Message> {
    let p = theme::palette(state.mode);
    let dismiss = mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
        .on_press(Message::CloseNotifications);
    let mut items = column![].spacing(2);
    if state.notifications.recent.is_empty() {
        items = items.push(
            container(
                column![
                    icons::view(Icon::Bell, 22.0, p.muted_2),
                    text("You're all caught up")
                        .size(12.5)
                        .font(theme::SANS_SEMIBOLD)
                        .color(p.ink),
                    text("Mentions, replies, huddles, runs, Forge, and governance activity appear here.")
                        .size(10.5)
                        .color(p.muted),
                ]
                .spacing(8)
                .align_x(Alignment::Center),
            )
            .width(Length::Fill)
            .padding([30, 20])
            .center_x(Length::Fill),
        );
    } else {
        let groups = notifications::groups(&state.notifications.recent, |id| {
            match &state.user_screens.chat.data {
                user_screens::Resource::Ready(data) => data
                    .channels
                    .iter()
                    .find(|channel| channel.id == id)
                    .map(|channel| channel.name.clone())
                    .unwrap_or_else(|| id.to_owned()),
                _ => id.to_owned(),
            }
        });
        for group in groups {
            if group.indices.len() == 1 {
                let index = group.indices[0];
                if let Some(item) = state.notifications.recent.get(index) {
                    items = items.push(notification_item_row(item, index, p));
                }
                continue;
            }
            let expanded = state.notifications.expanded.as_deref() == Some(&group.key);
            items = items.push(
                button(
                    column![
                        row![
                            text(if expanded { "▾" } else { "▸" })
                                .size(10.5)
                                .color(p.muted_2),
                            text(group.label.clone())
                                .size(11)
                                .font(theme::SANS_SEMIBOLD)
                                .width(Length::Fill),
                            text(notification_time(
                                state.notifications.recent[group.indices[0]].at
                            ))
                            .size(9.5)
                            .color(p.muted_2),
                        ]
                        .spacing(6)
                        .align_y(Alignment::Center),
                        text(group.summary(&state.notifications.recent))
                            .size(10.5)
                            .color(p.muted),
                    ]
                    .spacing(3),
                )
                .width(Length::Fill)
                .padding([7, 9])
                .on_press(Message::ToggleNotificationGroup(group.key.clone()))
                .style(move |_, status| notification_row_style(status, p)),
            );
            if expanded {
                let mut nested = column![].spacing(2).padding(iced::Padding {
                    top: 0.0,
                    right: 0.0,
                    bottom: 0.0,
                    left: 12.0,
                });
                for index in group.indices {
                    if let Some(item) = state.notifications.recent.get(index) {
                        nested = nested.push(notification_item_row(item, index, p));
                    }
                }
                items = items.push(nested);
            }
        }
    }
    let mut header = row![
        text("Notifications")
            .size(13)
            .font(theme::SANS_SEMIBOLD)
            .color(p.ink)
            .width(Length::Fill),
    ]
    .align_y(Alignment::Center);
    let count = state.notifications.recent.len();
    if count > 0 {
        header = header.push(text(count.to_string()).size(10).font(theme::MONO).color(p.muted_2));
    }
    let panel = container(column![
        container(header).padding(iced::Padding {
            top: 11.0,
            right: 12.0,
            bottom: 11.0,
            left: 12.0,
        }),
        container(Space::new().width(Length::Fill).height(1))
            .style(move |_| container::Style::default().background(p.border_soft)),
        scrollable(container(items).padding(6)).height(Length::Shrink),
    ])
    .width(320)
    .max_height(400)
    .style(move |_| container::Style {
        background: Some(Background::Color(p.paper)),
        border: Border {
            color: p.border,
            width: 1.0,
            radius: 8.0.into(),
        },
        shadow: iced::Shadow {
            color: Color {
                a: 0.18,
                ..Color::BLACK
            },
            offset: iced::Vector::new(0.0, 8.0),
            blur_radius: 24.0,
        },
        ..container::Style::default()
    });
    let anchored = container(panel)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(iced::alignment::Horizontal::Right)
        .align_y(iced::alignment::Vertical::Top)
        .padding(iced::Padding {
            top: 48.0,
            right: 13.0,
            bottom: 0.0,
            left: 0.0,
        });
    stack![dismiss, anchored]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn notification_item_row<'a>(
    item: &'a notifications::Item,
    index: usize,
    p: &'a theme::Palette,
) -> Element<'a, Message> {
    button(
        column![
            row![
                text(&item.title)
                    .size(11.5)
                    .font(theme::SANS_SEMIBOLD)
                    .width(Length::Fill),
                text(notification_time(item.at)).size(9.5).color(p.muted_2),
            ]
            .spacing(8),
            text(if item.body.is_empty() {
                item.category.fallback_screen()
            } else {
                &item.body
            })
            .size(10.5)
            .color(p.muted),
        ]
        .spacing(4),
    )
    .width(Length::Fill)
    .padding([7, 9])
    .on_press(Message::OpenNotification(index))
    .style(move |_, status| notification_row_style(status, p))
    .into()
}

fn notification_row_style(status: button::Status, p: &theme::Palette) -> button::Style {
    button::Style {
        background: matches!(status, button::Status::Hovered).then_some(Background::Color(p.hover)),
        text_color: p.ink,
        border: Border {
            radius: 5.0.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

fn notification_time(at: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64);
    let seconds = now.saturating_sub(at) / 1_000;
    match seconds {
        0..=59 => "now".into(),
        60..=3_599 => format!("{}m", seconds / 60),
        3_600..=86_399 => format!("{}h", seconds / 3_600),
        _ => format!("{}d", seconds / 86_400),
    }
}

pub(super) fn view(state: &Shell, id: window::Id) -> Element<'_, Message> {
    let kind = state.desktop.kind(id);
    let content = match kind {
        desktop::Kind::Main => main_view(state),
        desktop::Kind::Huddle => {
            huddle_ui::window_view(&state.huddle, huddle_context(state)).map(Message::Huddle)
        }
        desktop::Kind::Tray => tray_view(state),
    };
    // Dev-only: the window root is the anchor of the agent's semantic tree;
    // its name ("main"/"huddle"/"tray") is the tools' window key.
    #[cfg(all(feature = "agent", debug_assertions))]
    let content = super::agent_wire::root(kind, content);
    content
}

fn huddle_context(state: &Shell) -> huddle_ui::ViewContext<'_> {
    huddle_ui::ViewContext {
        chat: &state.user_screens.chat,
        members: &state.members,
        mode: state.mode,
    }
}

fn main_view(state: &Shell) -> Element<'_, Message> {
    let palette = theme::palette(state.mode);
    let body: Element<'_, Message> = if state.onboarding.is_ready() {
        let frame = app_frame(state);
        if state.workspace_overlay {
            stack![
                frame,
                workspace_screens::view(&state.workspace, state.mode).map(Message::Workspace)
            ]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
        } else {
            frame
        }
    } else {
        onboarding::view(&state.onboarding, state.mode).map(Message::Onboarding)
    };
    let mut content = column![titlebar(state)].spacing(0);
    if let Some(error) = &state.backend_error {
        content = content.push(
            container(text(format!("Desktop backend unavailable: {error}")).size(11))
                .padding([7, 14])
                .width(Length::Fill)
                .style(move |_| {
                    bordered(palette.danger_soft, palette.danger_border, 0.0).color(palette.danger)
                }),
        );
    }
    content = content.push(body);
    let base: Element<'_, Message> = container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| container::Style::default().background(palette.paper))
        .into();
    #[cfg(feature = "cef-browser")]
    let layered: Element<'_, Message> = if let Some(prompt) = &state.browser_permission
        && !state.search.open
    {
        stack![base, permission_prompt_view(prompt, state.mode)]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    } else {
        base
    };
    #[cfg(not(feature = "cef-browser"))]
    let layered: Element<'_, Message> = base;
    let layered = if state.notifications.open && !state.search.open {
        stack![layered, notifications_overlay(state)]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    } else {
        layered
    };
    let layered: Element<'_, Message> = if state.search.open {
        stack![
            layered,
            search::view(
                &state.search,
                state.mode,
                NETWORK_RAIL_WIDTH + MODULE_RAIL_WIDTH,
            )
            .map(Message::Search)
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    } else {
        layered
    };
    if state.huddle.is_expanded() && state.huddle.is_active() && state.desktop.huddle.is_none() {
        return huddle_ui::stage_view(&state.huddle, huddle_context(state)).map(Message::Huddle);
    }
    if state.huddle.is_active()
        && state.desktop.huddle.is_none()
        && !state.workspace_overlay
        && !state.search.open
    {
        let dock = container(
            huddle_ui::dock_view(&state.huddle, huddle_context(state)).map(Message::Huddle),
        )
        .width(320)
        .max_height(520)
        .align_x(iced::alignment::Horizontal::Left)
        .align_y(iced::alignment::Vertical::Bottom)
        .padding(iced::Padding {
            top: 0.0,
            right: 0.0,
            bottom: 12.0,
            left: NETWORK_RAIL_WIDTH + MODULE_RAIL_WIDTH + 12.0,
        });
        stack![layered, dock]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    } else {
        layered
    }
}

fn tray_view(state: &Shell) -> Element<'_, Message> {
    let canvas = Color::from_rgba8(20, 20, 23, 0.92);
    let panel = Color::from_rgba8(0, 0, 0, 0.18);
    let hairline = Color::from_rgba8(255, 255, 255, 0.12);
    let text_color = Color::from_rgba8(255, 255, 255, 0.94);
    let dim = Color::from_rgba8(255, 255, 255, 0.55);
    let connected = state.node_client.is_some() && state.node_stream_connected;
    let local_managed = state.active_workspace.is_some();
    let mut modules = column![].spacing(1);
    for screen in Screen::USER {
        modules = modules.push(tray_nav(
            screen,
            state.tray_selected == screen,
            text_color,
            dim,
            state.accent,
        ));
    }
    if local_managed {
        for screen in Screen::OPERATOR {
            if screen != Screen::Node {
                modules = modules.push(tray_nav(
                    screen,
                    state.tray_selected == screen,
                    text_color,
                    dim,
                    state.accent,
                ));
            }
        }
    }
    let rail = column![
        tray_nav(
            Screen::Node,
            state.tray_selected == Screen::Node,
            text_color,
            dim,
            state.accent,
        ),
        tray_divider(hairline),
        scrollable(modules).height(Length::Fill),
        tray_divider(hairline),
        tray_launch(
            "Settings",
            Icon::Settings,
            Screen::Settings,
            text_color,
            dim
        ),
        button(
            row![
                icons::view(Icon::Close, 15.0, dim),
                text("Quit").size(11.5).color(text_color)
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .padding([7, 10])
        .style(move |_, status| tray_button_style(status, text_color, false))
        .on_press(Message::Quit),
    ]
    .spacing(1);
    let detail = if state.tray_selected == Screen::Node {
        tray_node_detail(state, text_color, dim)
    } else {
        tray_module_detail(state.tray_selected, text_color, dim)
    };
    let header = row![
        container(text("D").size(11).color(Color::WHITE))
            .center_x(24)
            .center_y(24)
            .style(move |_| rounded(theme::ACCENTS[0], 6.0)),
        column![
            text("Ducktape").size(12.5).font(theme::SANS_SEMIBOLD),
            text(if connected {
                "●  Connected"
            } else if state.node_client.is_some() {
                "●  Reconnecting"
            } else {
                "●  Stopped"
            })
            .size(10)
            .font(theme::MONO)
            .color(if connected {
                Color::from_rgb8(92, 180, 95)
            } else if state.node_client.is_some() {
                Color::from_rgb8(209, 162, 77)
            } else {
                Color::from_rgb8(207, 106, 94)
            }),
        ]
        .spacing(2),
    ]
    .spacing(9)
    .align_y(Alignment::Center);
    container(column![
        container(header)
            .padding([10, 12])
            .width(Length::Fill)
            .style(move |_| bordered(panel, hairline, 0.0)),
        row![
            container(rail)
                .width(150)
                .height(Length::Fill)
                .padding([8, 7])
                .style(move |_| bordered(panel, hairline, 0.0)),
            container(detail)
                .width(Length::Fill)
                .height(Length::Fill)
                .padding([12, 13]),
        ]
        .height(Length::Fill),
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_| {
        container::Style::default()
            .background(canvas)
            .color(text_color)
    })
    .into()
}

fn tray_nav(
    screen: Screen,
    selected: bool,
    text_color: Color,
    dim: Color,
    accent: Color,
) -> Element<'static, Message> {
    button(
        row![
            container(Space::new().width(2).height(18)).style(move |_| {
                container::Style::default().background(if selected {
                    accent
                } else {
                    Color::TRANSPARENT
                })
            }),
            icons::view(
                screen.icon(),
                15.0,
                if selected { Color::WHITE } else { dim }
            ),
            text(screen.label()).size(11.5).color(text_color)
        ]
        .spacing(7)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([7, 10])
    .style(move |_, status| tray_button_style(status, text_color, selected))
    .on_press(Message::TraySelect(screen))
    .into()
}

fn tray_launch(
    label: &'static str,
    icon: Icon,
    screen: Screen,
    text_color: Color,
    dim: Color,
) -> Element<'static, Message> {
    button(
        row![
            icons::view(icon, 15.0, dim),
            text(label).size(11.5).color(text_color)
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([7, 10])
    .style(move |_, status| tray_button_style(status, text_color, false))
    .on_press(Message::OpenMain(Some(screen)))
    .into()
}

fn tray_button_style(status: button::Status, text_color: Color, selected: bool) -> button::Style {
    button::Style {
        background: if selected {
            Some(Background::Color(Color::from_rgba8(255, 255, 255, 0.13)))
        } else {
            matches!(status, button::Status::Hovered)
                .then(|| Background::Color(Color::from_rgba8(255, 255, 255, 0.08)))
        },
        text_color,
        border: Border {
            radius: 5.0.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

fn tray_divider(color: Color) -> Element<'static, Message> {
    container(Space::new())
        .height(1)
        .width(Length::Fill)
        .style(move |_| container::Style::default().background(color))
        .into()
}

fn tray_node_detail(state: &Shell, text_color: Color, dim: Color) -> Element<'_, Message> {
    let connected = state.node_client.is_some() && state.node_stream_connected;
    let snapshot = match &state.operator.node.data {
        operator_screens::Resource::Ready(snapshot) => Some(snapshot),
        _ => None,
    };
    let workspace = state
        .active_workspace
        .as_ref()
        .map(|workspace| workspace.name.clone())
        .or_else(|| snapshot.map(|snapshot| snapshot.workspace_name.clone()))
        .unwrap_or_else(|| "No network".into());
    let node_key = state
        .active_workspace
        .as_ref()
        .map(|workspace| workspace.pubkey.as_str())
        .or_else(|| snapshot.map(|snapshot| snapshot.peer.as_str()))
        .map(short_tray_key)
        .unwrap_or_else(|| "—".into());
    let role = state.active_workspace.as_ref().map_or_else(
        || snapshot.map_or("—", |snapshot| node_role_label(snapshot.role)),
        |workspace| {
            if workspace.founder {
                "genesis · validator"
            } else if workspace.member {
                "member · validator"
            } else {
                "guest"
            }
        },
    );
    let status = if connected && snapshot.is_some_and(|snapshot| snapshot.connected) {
        "Running"
    } else if connected {
        "Connected"
    } else if state.node_client.is_some() {
        "Reconnecting"
    } else if state.active_workspace.is_some() {
        "Stopped"
    } else {
        "Stopped"
    };
    let height = snapshot
        .filter(|snapshot| connected && snapshot.connected)
        .map(|snapshot| snapshot.height.to_string())
        .unwrap_or_else(|| "—".into());
    let members = match &state.members.data {
        members_screen::Resource::Ready(data) => data.members.len().to_string(),
        _ => "—".into(),
    };
    let modules = snapshot
        .map(|snapshot| format!("{} installed", snapshot.modules.len()))
        .unwrap_or_else(|| "—".into());
    let mut detail = column![
        text("Node").size(15).font(theme::SANS_SEMIBOLD),
        tray_field("Network", &workspace, text_color, dim),
        tray_field("Key", &node_key, text_color, dim),
        tray_field("Role", role, text_color, dim),
        tray_field("Status", status, text_color, dim),
        tray_field("Height", &height, text_color, dim),
        tray_field("Members", &members, text_color, dim),
        tray_field("Modules", &modules, text_color, dim),
        Space::new().height(8),
        text("SOFTWARE").size(10).color(dim),
        tray_field("Version", env!("CARGO_PKG_VERSION"), text_color, dim),
        Space::new().height(Length::Fill),
    ]
    .spacing(8);
    if state.active_workspace.is_some() {
        detail = detail.push(tray_open_button(Screen::Node, text_color));
    }
    detail.into()
}

fn tray_module_detail(screen: Screen, text_color: Color, dim: Color) -> Element<'static, Message> {
    column![
        row![
            container(icons::view(screen.icon(), 16.0, text_color))
                .width(26)
                .height(26)
                .center_x(26)
                .center_y(26)
                .style(move |_| rounded(Color::from_rgba8(255, 255, 255, 0.10), 7.0)),
            text(screen.label())
                .size(13)
                .font(theme::SANS_SEMIBOLD)
                .color(text_color),
        ]
        .spacing(9)
        .align_y(Alignment::Center),
        text("Open this module in the main Ducktape window.")
            .size(11)
            .color(dim),
        Space::new().height(Length::Fill),
        tray_open_button(screen, text_color),
    ]
    .spacing(12)
    .into()
}

fn tray_open_button(screen: Screen, text_color: Color) -> Element<'static, Message> {
    button(container(text("Open in console").size(11.5)).center_x(Length::Fill))
        .width(Length::Fill)
        .padding([7, 10])
        .style(move |_, status| tray_button_style(status, text_color, true))
        .on_press(Message::OpenMain(Some(screen)))
        .into()
}

const fn node_role_label(role: operator_screens::NodeRole) -> &'static str {
    match role {
        operator_screens::NodeRole::GenesisValidator => "genesis · validator",
        operator_screens::NodeRole::MemberValidator => "member · validator",
        operator_screens::NodeRole::RemoteUser => "user · node",
        operator_screens::NodeRole::Guest => "guest",
    }
}

fn short_tray_key(value: &str) -> String {
    if value.is_empty() {
        "—".into()
    } else if value.chars().count() > 12 {
        let start = value.chars().take(6).collect::<String>();
        let end = value
            .chars()
            .rev()
            .take(4)
            .collect::<String>()
            .chars()
            .rev()
            .collect::<String>();
        format!("0x{start}…{end}")
    } else {
        format!("0x{value}")
    }
}

fn tray_field(
    label: &'static str,
    value: &str,
    text_color: Color,
    dim: Color,
) -> Element<'static, Message> {
    row![
        text(label).size(10.5).color(dim).width(70),
        text(value.to_owned()).size(11).color(text_color),
    ]
    .align_y(Alignment::Center)
    .into()
}

#[cfg(feature = "cef-browser")]
fn permission_prompt_view(prompt: &PermissionPrompt, mode: Mode) -> Element<'_, Message> {
    let p = *theme::palette(mode);
    let site = prompt
        .origin
        .strip_prefix("duck://")
        .unwrap_or(prompt.origin.as_str());
    let mut permissions = column![].spacing(5);
    for permission in &prompt.permissions {
        let label = match permission.label() {
            "microphone" => "Your microphone",
            "camera" => "Your camera",
            "screen-capture" => "Your screen",
            other => other,
        };
        permissions = permissions.push(text(format!("• {label}")).size(12.5));
    }
    let card = container(
        column![
            container(text("Remote content").size(10.5).color(p.danger))
                .padding([2, 7])
                .style(move |_| bordered(p.danger_soft, p.danger_border, 4.0)),
            column![
                text(site).size(14).font(theme::SANS_SEMIBOLD),
                text(&prompt.origin)
                    .size(10.5)
                    .font(theme::MONO)
                    .color(p.muted),
            ]
            .spacing(3),
            column![text("wants to use:").size(12).color(p.muted), permissions].spacing(4),
            text(
                "This is a page published on your Ducktape network. It is not part of Ducktape and it is not asking for these on Ducktape's behalf."
            )
            .size(11.5)
            .color(p.muted),
            Space::new().height(Length::Fill),
            permission_button(
                "Allow while this page is open",
                Message::BrowserPermissionDecision {
                    id: prompt.id,
                    allow: true,
                    session: true,
                },
                true,
                p,
            ),
            row![
                permission_button(
                    "Allow once",
                    Message::BrowserPermissionDecision {
                        id: prompt.id,
                        allow: true,
                        session: false,
                    },
                    false,
                    p,
                ),
                permission_button(
                    "Don't allow",
                    Message::BrowserPermissionDecision {
                        id: prompt.id,
                        allow: false,
                        session: true,
                    },
                    false,
                    p,
                ),
            ]
            .spacing(7),
        ]
        .spacing(13),
    )
    .width(460)
    .height(360)
    .padding([18, 20])
    .style(move |_| bordered(p.paper, p.border_strong, 9.0));
    container(card)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(move |_| {
            rounded(
                Color {
                    a: 0.28,
                    ..p.filled
                },
                0.0,
            )
        })
        .into()
}

#[cfg(feature = "cef-browser")]
fn permission_button(
    label: &'static str,
    message: Message,
    primary: bool,
    p: theme::Palette,
) -> Element<'static, Message> {
    button(container(text(label).size(12)).center_x(Length::Fill))
        .width(Length::Fill)
        .padding([7, 10])
        .on_press(message)
        .style(move |_, status| button::Style {
            background: Some(Background::Color(if primary {
                if matches!(status, button::Status::Hovered) {
                    p.ink_soft
                } else {
                    p.filled
                }
            } else if matches!(status, button::Status::Hovered) {
                p.hover
            } else {
                p.paper
            })),
            text_color: if primary { p.on_filled } else { p.ink },
            border: Border {
                color: if primary { p.filled } else { p.border },
                width: 1.0,
                radius: 6.0.into(),
            },
            ..button::Style::default()
        })
        .into()
}

fn titlebar(state: &Shell) -> Element<'_, Message> {
    let p = theme::palette(state.mode);
    let (connection_label, connection_color) = titlebar_connection(state, p);
    let back = icon_button(
        Icon::ChevronLeft,
        "Back",
        Message::Back,
        state.history_index > 0 && !state.workspace_overlay,
        state,
    );
    let forward = icon_button(
        Icon::ChevronRight,
        "Forward",
        Message::Forward,
        state.history_index + 1 < state.history.len() && !state.workspace_overlay,
        state,
    );
    let identity = mouse_area(
        container(
            row![
                container(text("D").size(11).color(p.on_filled))
                    .center_x(22)
                    .center_y(22)
                    .style(move |_| rounded(p.filled, 5.0)),
                text(
                    state
                        .active_workspace
                        .as_ref()
                        .map(|workspace| workspace.name.as_str())
                        .unwrap_or_else(|| {
                            if state.node_client.is_some() {
                                "Remote node"
                            } else {
                                "Ducktape"
                            }
                        }),
                )
                .size(12),
                text(connection_label).size(9).color(connection_color),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_y(Alignment::Center),
    )
    .on_press(Message::Window(WindowAction::Drag));
    let search = button(
        row![
            icons::view(Icon::Search, 14.0, p.muted_2),
            text("Search").size(12).color(p.muted),
            Space::new().width(Length::Fill),
            text(if cfg!(target_os = "macos") {
                "⌘ K"
            } else {
                "Ctrl K"
            })
            .size(10)
            .color(p.muted_2),
        ]
        .height(Length::Fill)
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding([0, 12])
    .width(340)
    .height(28)
    .on_press_maybe(
        (state.onboarding.is_ready() && !state.workspace_overlay).then_some(Message::ToggleSearch),
    )
    .style(move |_, status| button::Style {
        background: Some(Background::Color(
            if matches!(status, button::Status::Hovered) {
                p.hover
            } else {
                p.paper
            },
        )),
        text_color: p.ink,
        border: Border {
            color: p.border,
            width: 1.0,
            radius: 8.0.into(),
        },
        ..button::Style::default()
    });
    let mut bell_content = row![icons::view(
        Icon::Bell,
        15.0,
        if state.notifications.unread > 0 {
            p.ink
        } else {
            p.muted_2
        },
    )]
    .spacing(4)
    .align_y(Alignment::Center);
    if state.notifications.unread > 0 {
        bell_content = bell_content.push(
            container(
                text(if state.notifications.unread > 99 {
                    "99+".into()
                } else {
                    state.notifications.unread.to_string()
                })
                .size(8.5)
                .font(theme::MONO)
                .color(Color::WHITE),
            )
            .padding([1, 5])
            .style(move |_| rounded(state.accent, 8.0)),
        );
    }
    let tip = match state.notifications.unread {
        0 => "Notifications".to_string(),
        1 => "1 new notification".to_string(),
        100.. => "99+ new notifications".to_string(),
        n => format!("{n} new notifications"),
    };
    let bell = tooltip(
        button(bell_content)
            .padding([3, 8])
            .on_press(Message::ToggleNotifications)
            .style(move |_, status| button::Style {
                background: matches!(status, button::Status::Hovered)
                    .then_some(Background::Color(p.hover)),
                text_color: p.ink,
                border: Border {
                    radius: 5.0.into(),
                    ..Border::default()
                },
                ..button::Style::default()
            }),
        container(text(tip).size(11).color(p.ink))
            .padding([4, 8])
            .style(move |_| bordered(p.paper, p.border, 6.0)),
        tooltip::Position::Bottom,
    )
    .gap(6)
    .padding(6);
    let mut controls = row![bell].spacing(1).align_y(Alignment::Center);
    if !cfg!(target_os = "macos") {
        controls = controls
            .push(icon_button(
                Icon::Minimize,
                "Minimize",
                Message::Window(WindowAction::Minimize),
                true,
                state,
            ))
            .push(icon_button(
                Icon::Maximize,
                "Maximize",
                Message::Window(WindowAction::Maximize),
                true,
                state,
            ))
            .push(icon_button(
                Icon::Close,
                "Close",
                Message::Window(WindowAction::Close),
                true,
                state,
            ));
    }
    let native_titlebar_inset = Space::new().width(if cfg!(target_os = "macos") { 68 } else { 0 });
    container(
        row![
            row![native_titlebar_inset, back, forward, identity]
                .spacing(2)
                .align_y(Alignment::Center)
                .width(Length::FillPortion(1)),
            search,
            container(controls)
                .width(Length::FillPortion(1))
                .align_x(iced::alignment::Horizontal::Right),
        ]
        .padding([0, 10])
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .height(TITLEBAR_HEIGHT)
    .style(move |_| container::Style {
        background: Some(Background::Color(p.titlebar)),
        border: Border {
            color: p.window_border,
            width: 0.0,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    })
    .into()
}

pub(super) fn titlebar_connection(state: &Shell, p: &theme::Palette) -> (String, Color) {
    if state.node_client.is_none() {
        return ("OFFLINE".into(), p.muted_2);
    }
    let mode = if state.active_workspace.is_some() {
        "LOCAL"
    } else {
        "REMOTE"
    };
    if !state.node_stream_connected {
        return (format!("{mode} · RECONNECTING"), p.amber);
    }
    match &state.operator.node.data {
        operator_screens::Resource::Ready(snapshot) if snapshot.connected => {
            (format!("{mode} · #{}", snapshot.height), p.green)
        }
        _ => (format!("{mode} · CONNECTED"), p.green),
    }
}

fn app_frame(state: &Shell) -> Element<'_, Message> {
    row![network_rail(state), module_rail(state), screen_view(state)]
        .spacing(0)
        .height(Length::Fill)
        .into()
}

pub(super) fn workspace_initials(name: &str) -> String {
    let mut words = name
        .split_whitespace()
        .filter_map(|word| word.chars().next());
    match (words.next(), words.next()) {
        (Some(first), Some(second)) => [first, second]
            .into_iter()
            .flat_map(char::to_uppercase)
            .collect(),
        (Some(first), None) => first.to_uppercase().take(2).collect(),
        (None, _) => "?".into(),
    }
}

fn network_rail(state: &Shell) -> Element<'_, Message> {
    let p = theme::palette(state.mode);
    let item = |letter: String, active: bool, message| {
        button(container(text(letter).size(13)).center_x(34).center_y(34))
            .padding(0)
            .on_press(message)
            .style(move |_, status| rail_circle(p, state.accent, active, status))
    };
    let mut networks = column![item(
        "⌂".into(),
        state.screen() == Screen::Home,
        Message::Navigate(Screen::Home),
    )]
    .spacing(10)
    .align_x(Alignment::Center);
    for workspace in &state.workspace.workspaces {
        let active = state
            .active_workspace
            .as_ref()
            .is_some_and(|current| current.id == workspace.id);
        networks = networks.push(item(
            workspace_initials(&workspace.name),
            active,
            Message::Workspace(workspace_screens::Message::SelectWorkspace(
                workspace.id.clone(),
            )),
        ));
    }
    if state.active_workspace.is_none() && state.node_client.is_some() {
        networks = networks.push(item("R".into(), true, Message::Navigate(Screen::Home)));
    }
    networks = networks
        .push(item(
            "+".into(),
            false,
            Message::Workspace(workspace_screens::Message::Open),
        ))
        .push(Space::new().height(Length::Fill));
    container(networks.padding([10, 0]))
        .width(NETWORK_RAIL_WIDTH)
        .height(Length::Fill)
        .align_x(iced::alignment::Horizontal::Center)
        .style(move |_| right_border(p.sidebar, p.border))
        .into()
}

fn module_rail(state: &Shell) -> Element<'_, Message> {
    let p = theme::palette(state.mode);
    // Follow the section, not just local_managed: shell.rs's navigate() lands on
    // operator screens (and sets section = Operator) even with no local workspace,
    // so the rail must reach and highlight operator content whenever we're on it.
    let on_operator =
        state.section == Section::Operator || Screen::OPERATOR.contains(&state.screen());
    let show_operator = state.active_workspace.is_some() || on_operator;
    let tabs = if show_operator {
        row![
            section_button("USER", Section::User, state),
            section_button("NODE", Section::Operator, state),
        ]
        .spacing(2)
    } else {
        row![section_button("USER", Section::User, state)].spacing(2)
    };
    let screens = if on_operator {
        &Screen::OPERATOR[..]
    } else {
        &Screen::USER[..]
    };
    let mut modules = column![tabs].spacing(4).align_x(Alignment::Center);
    modules = modules.push(Space::new().height(2));
    for &screen in screens {
        modules = modules.push(module_button(screen, state));
    }
    modules = modules
        .push(Space::new().height(Length::Fill))
        .push(module_button(Screen::Settings, state))
        .push(
            button(
                column![
                    icons::view(
                        match state.mode {
                            Mode::Light => Icon::Sun,
                            Mode::Dark => Icon::Moon,
                        },
                        18.0,
                        p.icon_idle,
                    ),
                    text("Theme").size(9).color(p.muted),
                ]
                .spacing(5)
                .align_x(Alignment::Center)
                .width(Length::Fill),
            )
            .width(66)
            .height(54)
            .padding([7, 2])
            .on_press(Message::ToggleTheme)
            .style(move |_, s| tab_style(p, false, s)),
        );
    container(modules.padding([8, 4]))
        .width(MODULE_RAIL_WIDTH)
        .height(Length::Fill)
        .style(move |_| right_border(p.sidebar, p.border))
        .into()
}

fn screen_view(state: &Shell) -> Element<'_, Message> {
    if let Some(view) = user_view(state.screen()) {
        return module_host::view(&state.user_screens, view, state.mode).map(Message::UserView);
    }
    if state.screen() == Screen::Browser {
        #[cfg(feature = "cef-browser")]
        let cef_ready = state.browser.is_some();
        #[cfg(not(feature = "cef-browser"))]
        let cef_ready = false;
        return browser_chrome::view(&state.browser_chrome, state.mode, cef_ready)
            .map(Message::Browser);
    }
    if state.screen() == Screen::Forge {
        return forge_screen::view(&state.forge, state.mode).map(Message::Forge);
    }
    if state.screen() == Screen::Agents {
        return agents_screen::view(&state.agents, state.mode, state.accent).map(Message::Agents);
    }
    if state.screen() == Screen::Members {
        return members_screen::view(&state.members, state.mode).map(Message::Members);
    }
    if state.screen() == Screen::Governance {
        return governance_screen::view(&state.governance, state.mode).map(Message::Governance);
    }
    if state.screen() == Screen::Explorer {
        return explorer_screen::view(&state.explorer, state.mode).map(Message::Explorer);
    }
    if state.screen() == Screen::Terminal {
        return terminal_screen::view(&state.terminal_screen, state.mode).map(Message::Terminal);
    }
    if let Some(screen) = operator_screen(state.screen()) {
        return operator_screens::view(&state.operator, screen, state.mode).map(Message::Operator);
    }
    if state.screen() == Screen::Settings {
        return settings_screen::view(&state.settings).map(Message::Settings);
    }
    let p = theme::palette(state.mode);
    let screen = state.screen();
    container(
        column![
            container(text(screen.label()).size(16))
                .height(50)
                .width(Length::Fill)
                .align_y(Alignment::Center)
                .padding([0, 20])
                .style(move |_| container::Style {
                    border: Border {
                        color: p.border,
                        width: 0.0,
                        radius: 0.0.into(),
                    },
                    ..container::Style::default()
                }),
            container(
                column![
                    icons::view(screen.icon(), 30.0, p.icon_idle),
                    text(screen.label()).size(16),
                    text("Native iced surface").size(12).color(p.muted),
                ]
                .spacing(10)
                .align_x(Alignment::Center),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill),
        ]
        .spacing(0),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_| container::Style::default().background(p.paper))
    .into()
}

fn icon_button<'a>(
    icon: Icon,
    label: &'static str,
    message: Message,
    enabled: bool,
    state: &Shell,
) -> Element<'a, Message> {
    let p = theme::palette(state.mode);
    let control = button(icons::view(
        icon,
        15.0,
        if enabled { p.muted_3 } else { p.icon_idle },
    ))
    .width(32)
    .height(32)
    .padding(8)
    .on_press_maybe(enabled.then_some(message))
    .style(move |_, status| transparent_button(p, status));
    tooltip(control, text(label).size(11), tooltip::Position::Bottom)
        .gap(6)
        .padding(6)
        .into()
}

fn section_button<'a>(
    label: &'static str,
    section: Section,
    state: &Shell,
) -> Element<'a, Message> {
    let p = theme::palette(state.mode);
    let active = state.section == section;
    let tab = button(container(text(label).size(8)).center_y(Length::Fill))
        .height(28)
        .padding([0, 5])
        .on_press(Message::Section(section))
        .style(move |_, status| tab_style(p, active, status));
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(iced_agent_plugin::Role::Tab, label, tab);
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    tab.into()
}

fn module_button<'a>(screen: Screen, state: &Shell) -> Element<'a, Message> {
    let p = theme::palette(state.mode);
    let active = state.screen() == screen;
    let nav = button(
        column![
            icons::view(
                screen.icon(),
                18.0,
                if active { p.ink } else { p.icon_idle }
            ),
            text(screen.label())
                .size(9)
                .color(if active { p.ink } else { p.muted }),
        ]
        .width(Length::Fill)
        .spacing(5)
        .align_x(Alignment::Center),
    )
    .width(66)
    .height(54)
    .padding([7, 2])
    .on_press(Message::Navigate(screen))
    .style(move |_, status| tab_style(p, active, status));
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(iced_agent_plugin::Role::Button, screen.label(), nav);
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    nav.into()
}

fn rounded(background: Color, radius: f32) -> container::Style {
    container::Style::default()
        .background(background)
        .border(Border {
            radius: radius.into(),
            ..Border::default()
        })
}

fn bordered(background: Color, border: Color, radius: f32) -> container::Style {
    container::Style::default()
        .background(background)
        .border(Border {
            color: border,
            width: 1.0,
            radius: radius.into(),
        })
}

fn right_border(background: Color, border: Color) -> container::Style {
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

fn transparent_button(p: &theme::Palette, status: button::Status) -> button::Style {
    button::Style {
        background: matches!(status, button::Status::Hovered | button::Status::Pressed)
            .then_some(Background::Color(p.hover)),
        text_color: p.ink,
        border: Border {
            radius: theme::RADIUS_SM.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

fn tab_style(p: &theme::Palette, active: bool, status: button::Status) -> button::Style {
    let background = if active {
        Some(Background::Color(p.paper))
    } else if matches!(status, button::Status::Hovered | button::Status::Pressed) {
        Some(Background::Color(p.hover))
    } else {
        None
    };
    button::Style {
        background,
        text_color: if active { p.ink } else { p.muted },
        border: Border {
            radius: theme::RADIUS_SM.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}

fn rail_circle(
    p: &theme::Palette,
    accent: Color,
    active: bool,
    status: button::Status,
) -> button::Style {
    let background = if active {
        accent
    } else if matches!(status, button::Status::Hovered | button::Status::Pressed) {
        p.hover
    } else {
        p.panel
    };
    button::Style {
        background: Some(Background::Color(background)),
        text_color: if active { Color::WHITE } else { p.ink },
        border: Border {
            color: if active { p.paper } else { p.border },
            width: if active { 2.0 } else { 1.0 },
            radius: 17.0.into(),
        },
        ..button::Style::default()
    }
}
