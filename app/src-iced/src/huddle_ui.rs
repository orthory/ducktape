//! Native Huddle state, reducer, media/session lifecycle, and presentation.
//!
//! The desktop shell keeps native window IDs, browser visibility, navigation,
//! and backend command execution. This module owns only the Huddle feature.

use std::collections::BTreeMap;

use iced::widget::{
    Space, button, column, container, image, pick_list, progress_bar, row, scrollable, stack, text,
};
use iced::{Alignment, Background, Border, Color, Element, Length};

use crate::huddle_media;
use crate::huddle_session;
use crate::screens::members as members_screen;
use crate::screens::user as user_screens;
use crate::theme::{self, Mode};
use crate::transport::NodeClient;

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    Mute,
    Camera,
    Share,
    Leave,
    Retry,
    Expand,
    Collapse,
    ToggleLayout,
    ToggleDevices,
    Microphone(String),
    CameraDevice(String),
    Speaker(String),
    ScreenSource(String),
    PopOut,
    PopIn,
    OpenChat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    PopOut,
    PopIn,
    OpenChat,
    Leave(String),
    HideBrowser,
    SyncBrowser,
    ClosePopout,
}

#[derive(Default)]
pub struct State {
    runtime: Option<Runtime>,
    expanded: bool,
    spotlight: bool,
    device_prefs: DevicePrefs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Connecting,
    Reconnecting,
    Live,
    Unavailable,
}

struct Runtime {
    channel: String,
    session: Option<huddle_session::Handle>,
    media: Option<huddle_media::Handle>,
    status: Status,
    muted: bool,
    camera_on: bool,
    sharing: bool,
    error: Option<String>,
    local_frame: Option<image::Handle>,
    peer_frames: BTreeMap<String, image::Handle>,
    peers: BTreeMap<String, Peer>,
    devices: huddle_media::DeviceOptions,
    devices_open: bool,
    level: u8,
    speaking_until: Option<std::time::Instant>,
    recipients: Vec<String>,
    retry_pending: bool,
    last_reconnect: Option<std::time::Instant>,
}

struct Peer {
    muted: bool,
    camera_on: bool,
    sharing: bool,
    seen: std::time::Instant,
}

#[derive(Default, Clone, Copy)]
struct DevicePrefs {
    microphone: Option<usize>,
    camera: Option<usize>,
    speaker: Option<usize>,
}

#[derive(Clone, Copy)]
pub struct ViewContext<'a> {
    pub chat: &'a user_screens::ChatState,
    pub members: &'a members_screen::State,
    pub mode: Mode,
}

impl State {
    pub fn is_active(&self) -> bool {
        self.runtime.is_some()
    }

    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    pub fn reset(&mut self) -> bool {
        let was_active = self.runtime.take().is_some();
        self.expanded = false;
        was_active
    }

    pub fn take_channel(&mut self) -> Option<String> {
        self.expanded = false;
        self.runtime.take().map(|runtime| runtime.channel)
    }
}

pub fn update(
    state: &mut State,
    message: Message,
    chat: &user_screens::ChatState,
    local_node: Option<&str>,
    client: Option<&NodeClient>,
) -> Option<Action> {
    match message {
        Message::Tick => return tick(state, chat, local_node, client),
        Message::Mute => {
            if let Some(runtime) = &mut state.runtime {
                runtime.muted = !runtime.muted;
                if let Some(media) = &runtime.media {
                    media.send(huddle_media::Command::SetMuted(runtime.muted));
                }
                send_beacon(runtime);
            }
        }
        Message::Camera => {
            if let Some(runtime) = &mut state.runtime {
                runtime.error = None;
                if let Some(media) = &runtime.media {
                    media.send(huddle_media::Command::SetCamera(!runtime.camera_on));
                }
            }
        }
        Message::Share => {
            if let Some(runtime) = &mut state.runtime {
                runtime.error = None;
                if let Some(media) = &runtime.media {
                    media.send(huddle_media::Command::SetScreenShare(!runtime.sharing));
                }
            }
        }
        Message::Leave => {
            let channel = state.take_channel()?;
            return Some(Action::Leave(channel));
        }
        Message::Retry => {
            state.expanded = false;
            let runtime = state.runtime.as_mut()?;
            runtime.status = Status::Connecting;
            runtime.error = None;
            runtime.retry_pending = true;
            runtime.last_reconnect = None;
            runtime.session.take();
            if let Some(media) = &runtime.media {
                media.send(huddle_media::Command::Stop);
            }
            return restart_if_stopped(state, chat, local_node, client);
        }
        Message::Expand => {
            state.expanded = true;
            return Some(Action::HideBrowser);
        }
        Message::Collapse => {
            state.expanded = false;
            return Some(Action::SyncBrowser);
        }
        Message::ToggleLayout => state.spotlight = !state.spotlight,
        Message::ToggleDevices => {
            if let Some(runtime) = &mut state.runtime {
                runtime.devices_open = !runtime.devices_open;
                if runtime.devices_open
                    && let Some(media) = &runtime.media
                {
                    media.send(huddle_media::Command::RefreshDevices);
                }
            }
        }
        Message::Microphone(value) => send_device_command(
            state,
            huddle_media::Command::SetMicrophone(device_index(&value)),
        ),
        Message::CameraDevice(value) => send_device_command(
            state,
            huddle_media::Command::SetCameraDevice(device_index(&value)),
        ),
        Message::Speaker(value) => send_device_command(
            state,
            huddle_media::Command::SetSpeaker(device_index(&value)),
        ),
        Message::ScreenSource(value) => send_device_command(
            state,
            huddle_media::Command::SetScreenSource(device_index(&value)),
        ),
        Message::PopOut => {
            state.expanded = false;
            return state.is_active().then_some(Action::PopOut);
        }
        Message::PopIn => return Some(Action::PopIn),
        Message::OpenChat => return Some(Action::OpenChat),
    }
    None
}

fn send_device_command(state: &State, command: huddle_media::Command) {
    if let Some(runtime) = &state.runtime
        && let Some(media) = &runtime.media
    {
        media.send(command);
    }
}

pub fn sync(
    state: &mut State,
    chat: &user_screens::ChatState,
    local_node: Option<&str>,
    client: Option<&NodeClient>,
) -> Option<Action> {
    let Some((channel, recipients)) = joined(chat, local_node) else {
        return state.reset().then_some(Action::ClosePopout);
    };
    if let Some(runtime) = &mut state.runtime
        && runtime.channel == channel
    {
        if runtime.recipients != recipients {
            runtime.recipients.clone_from(&recipients);
            if let Some(session) = &runtime.session {
                let _ = session
                    .control
                    .try_send(huddle_session::Control::Recipients(recipients));
            }
        }
        return None;
    }

    state.runtime = Some(start_runtime(
        client,
        channel,
        recipients,
        true,
        Status::Connecting,
        None,
        state.device_prefs,
    ));
    None
}

fn joined(
    chat: &user_screens::ChatState,
    local_node: Option<&str>,
) -> Option<(String, Vec<String>)> {
    let user_screens::Resource::Ready(data) = &chat.data else {
        return None;
    };
    let self_key = data.self_key.as_ref()?;
    let channel = data
        .channels
        .iter()
        .find(|channel| {
            channel.id.as_str() == chat.active_channel.as_deref().unwrap_or("")
                && channel.huddle.iter().any(|member| &member.user == self_key)
        })
        .or_else(|| {
            data.channels
                .iter()
                .find(|channel| channel.huddle.iter().any(|member| &member.user == self_key))
        })?;
    let mut recipients = channel
        .huddle
        .iter()
        .filter(|member| Some(member.node.as_str()) != local_node)
        .map(|member| member.node.clone())
        .collect::<Vec<_>>();
    recipients.sort();
    recipients.dedup();
    recipients.truncate(64);
    Some((channel.id.clone(), recipients))
}

fn start_runtime(
    client: Option<&NodeClient>,
    channel: String,
    recipients: Vec<String>,
    muted: bool,
    status: Status,
    last_reconnect: Option<std::time::Instant>,
    prefs: DevicePrefs,
) -> Runtime {
    let started = client
        .ok_or_else(|| "connect to the workspace before starting a huddle".to_string())
        .and_then(|client| huddle_session::Handle::start(client, &channel));
    let (session, media, error) = match started {
        Ok((session, port)) => {
            let _ = session
                .control
                .try_send(huddle_session::Control::Recipients(recipients.clone()));
            let _ = session.control.try_send(huddle_session::Control::Beacon {
                muted,
                camera_on: false,
                sharing: false,
            });
            let media = huddle_media::Handle::start(port);
            if !muted {
                media.send(huddle_media::Command::SetMuted(false));
            }
            if prefs.microphone.is_some() {
                media.send(huddle_media::Command::SetMicrophone(prefs.microphone));
            }
            if prefs.camera.is_some() {
                media.send(huddle_media::Command::SetCameraDevice(prefs.camera));
            }
            if prefs.speaker.is_some() {
                media.send(huddle_media::Command::SetSpeaker(prefs.speaker));
            }
            (Some(session), Some(media), None)
        }
        Err(error) => (None, None, Some(error)),
    };
    Runtime {
        channel,
        session,
        media,
        status: if error.is_some() {
            Status::Unavailable
        } else {
            status
        },
        muted,
        camera_on: false,
        sharing: false,
        error,
        local_frame: None,
        peer_frames: BTreeMap::new(),
        peers: BTreeMap::new(),
        devices: huddle_media::DeviceOptions::default(),
        devices_open: false,
        level: 0,
        speaking_until: None,
        recipients,
        retry_pending: false,
        last_reconnect,
    }
}

fn restart_if_stopped(
    state: &mut State,
    chat: &user_screens::ChatState,
    local_node: Option<&str>,
    client: Option<&NodeClient>,
) -> Option<Action> {
    let ready = state.runtime.as_ref().is_some_and(|runtime| {
        runtime.retry_pending
            && runtime
                .media
                .as_ref()
                .is_none_or(huddle_media::Handle::is_stopped)
    });
    if !ready {
        return None;
    }
    let previous = state.runtime.take().expect("retry checked a huddle");
    let Some((channel, recipients)) = joined(chat, local_node) else {
        state.expanded = false;
        return Some(Action::ClosePopout);
    };
    state.runtime = Some(start_runtime(
        client,
        channel,
        recipients,
        previous.muted,
        previous.status,
        previous.last_reconnect,
        state.device_prefs,
    ));
    None
}

fn send_beacon(runtime: &Runtime) {
    if let Some(session) = &runtime.session {
        let _ = session.control.try_send(huddle_session::Control::Beacon {
            muted: runtime.muted,
            camera_on: runtime.camera_on,
            sharing: runtime.sharing,
        });
    }
}

fn tick(
    state: &mut State,
    chat: &user_screens::ChatState,
    local_node: Option<&str>,
    client: Option<&NodeClient>,
) -> Option<Action> {
    let Some(runtime) = &mut state.runtime else {
        return None;
    };
    let mut terminal = None;
    if let Some(session) = &mut runtime.session {
        for _ in 0..16 {
            match session.events.try_recv() {
                Ok(huddle_session::Event::Connecting) => {
                    if runtime.status != Status::Reconnecting {
                        runtime.status = Status::Connecting;
                    }
                }
                Ok(huddle_session::Event::Live) => {
                    runtime.status = Status::Live;
                    runtime.error = None;
                }
                Ok(huddle_session::Event::PeerBeacon {
                    peer,
                    muted,
                    camera_on,
                    sharing,
                }) => {
                    if !camera_on && !sharing {
                        runtime.peer_frames.remove(&peer);
                    }
                    runtime.peers.insert(
                        peer,
                        Peer {
                            muted,
                            camera_on,
                            sharing,
                            seen: std::time::Instant::now(),
                        },
                    );
                }
                Ok(huddle_session::Event::Closed) => {
                    terminal = Some(None);
                    break;
                }
                Ok(huddle_session::Event::Failed(error)) => {
                    terminal = Some(Some(error));
                    break;
                }
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => break,
            }
        }
    }
    if let Some(error) = terminal {
        let now = std::time::Instant::now();
        let reconnect = error.is_none()
            && runtime.status == Status::Live
            && runtime
                .last_reconnect
                .is_none_or(|last| now.duration_since(last) > std::time::Duration::from_secs(30));
        runtime.session.take();
        if let Some(media) = &runtime.media {
            media.send(huddle_media::Command::Stop);
        }
        if reconnect {
            runtime.status = Status::Reconnecting;
            runtime.error = None;
            runtime.retry_pending = true;
            runtime.last_reconnect = Some(now);
        } else {
            runtime.status = Status::Unavailable;
            runtime.error = Some(error.unwrap_or_else(|| "The huddle connection closed".into()));
        }
    }
    if let Some(media) = &runtime.media {
        for _ in 0..16 {
            match media.events.try_recv() {
                Ok(huddle_media::Event::Ready) => {}
                Ok(huddle_media::Event::Devices(devices)) => {
                    state.device_prefs = DevicePrefs {
                        microphone: devices.microphone,
                        camera: devices.camera,
                        speaker: devices.speaker,
                    };
                    if !devices.screen_sources.is_empty() && devices.screen_source.is_none() {
                        runtime.devices_open = true;
                    }
                    runtime.devices = devices;
                }
                Ok(huddle_media::Event::VideoState { camera_on, sharing }) => {
                    runtime.camera_on = camera_on;
                    runtime.sharing = sharing;
                    if !camera_on && !sharing {
                        runtime.local_frame = None;
                    }
                    if sharing && !runtime.devices.screen_sources.is_empty() {
                        runtime.devices_open = false;
                    }
                    send_beacon(runtime);
                }
                Ok(huddle_media::Event::LocalFrame(frame)) => {
                    runtime.local_frame = Some(image::Handle::from_rgba(
                        frame.width,
                        frame.height,
                        frame.rgba.as_ref().to_vec(),
                    ));
                }
                Ok(huddle_media::Event::PeerFrame { peer, frame }) => {
                    if runtime.peer_frames.len() < 8 || runtime.peer_frames.contains_key(&peer) {
                        runtime.peer_frames.insert(
                            peer,
                            image::Handle::from_rgba(
                                frame.width,
                                frame.height,
                                frame.rgba.as_ref().to_vec(),
                            ),
                        );
                    }
                }
                Ok(huddle_media::Event::RequestKeyframe(peer)) => {
                    if let Some(session) = &runtime.session {
                        let _ = session
                            .control
                            .try_send(huddle_session::Control::RequestKeyframe(peer));
                    }
                }
                Ok(huddle_media::Event::Failed { kind, detail }) => {
                    runtime.error = Some(format!("{}: {detail}", kind.label()));
                    if matches!(
                        kind,
                        huddle_media::FailureKind::MicrophoneDenied
                            | huddle_media::FailureKind::MicrophoneUnavailable
                            | huddle_media::FailureKind::Unsupported
                    ) {
                        runtime.status = Status::Unavailable;
                    }
                }
                Ok(huddle_media::Event::Stopped) => {
                    if !runtime.retry_pending {
                        runtime.status = Status::Unavailable;
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
            }
        }
        runtime.level = media.level();
        if runtime.level >= 8 {
            runtime.speaking_until =
                Some(std::time::Instant::now() + std::time::Duration::from_millis(600));
        }
    }
    restart_if_stopped(state, chat, local_node, client)
}

fn speaking(runtime: &Runtime) -> bool {
    runtime
        .speaking_until
        .is_some_and(|until| std::time::Instant::now() < until)
}

fn device_index(value: &str) -> Option<usize> {
    if value == "System default" {
        None
    } else {
        value
            .split_once(" · ")
            .and_then(|(index, _)| index.parse::<usize>().ok())
            .and_then(|index| index.checked_sub(1))
    }
}

fn device_labels(options: &[String]) -> Vec<String> {
    std::iter::once("System default".to_string())
        .chain(
            options
                .iter()
                .enumerate()
                .map(|(index, label)| format!("{} · {label}", index + 1)),
        )
        .collect()
}

fn selected_device(options: &[String], selection: Option<usize>) -> Option<String> {
    Some(selection.map_or_else(
        || "System default".to_string(),
        |index| {
            options.get(index).map_or_else(
                || "System default".to_string(),
                |label| format!("{} · {label}", index + 1),
            )
        },
    ))
}

pub fn dock_view<'a>(state: &'a State, context: ViewContext<'a>) -> Element<'a, Message> {
    let p = theme::palette(context.mode);
    let runtime = state.runtime.as_ref().expect("dock requires a huddle");
    let (status, status_color) = status_label(runtime, p);
    let count = member_count(context.chat, &runtime.channel);
    let header = row![
        container(Space::new())
            .width(8)
            .height(8)
            .style(move |_| rounded(status_color, 4.0)),
        column![
            text(format!("Huddle · #{}", runtime.channel))
                .size(12.5)
                .font(theme::SANS_SEMIBOLD),
            text(format!("{status} · {count} in call"))
                .size(10)
                .color(p.muted),
        ]
        .spacing(1)
        .width(Length::Fill),
        button(text("Expand").size(10))
            .on_press(Message::Expand)
            .padding([4, 7]),
        button(text("Pop out").size(10))
            .on_press(Message::PopOut)
            .padding([4, 7]),
    ]
    .spacing(7)
    .align_y(Alignment::Center);

    let mut body = column![header].spacing(8);
    if let Some(error) = &runtime.error {
        body = body.push(
            container(text(error).size(10).color(p.danger))
                .width(Length::Fill)
                .padding([6, 8])
                .style(move |_| bordered(p.danger_soft, p.danger_border, 5.0)),
        );
    }
    body = body.push(compact_body(state, context, p));
    if runtime.devices_open {
        body = body.push(devices_view(runtime, p));
    }
    body = body.push(controls_view(state, context, false, p));
    container(body)
        .padding(10)
        .width(320)
        .style(move |_| {
            bordered(p.paper, p.border_strong, 9.0).shadow(iced::Shadow {
                color: Color::from_rgba8(0, 0, 0, 0.14),
                offset: iced::Vector::new(0.0, 5.0),
                blur_radius: 16.0,
            })
        })
        .into()
}

pub fn stage_view<'a>(state: &'a State, context: ViewContext<'a>) -> Element<'a, Message> {
    let p = theme::palette(context.mode);
    let runtime = state.runtime.as_ref().expect("stage requires a huddle");
    let (status, status_color) = status_label(runtime, p);
    let count = member_count(context.chat, &runtime.channel);
    let header = row![
        container(Space::new())
            .width(9)
            .height(9)
            .style(move |_| rounded(status_color, 4.5)),
        text(format!("#{}", runtime.channel))
            .size(14)
            .font(theme::SANS_SEMIBOLD),
        text(format!("{status} · {count} in call"))
            .size(11)
            .color(p.muted),
        Space::new().width(Length::Fill),
        button(
            text(if state.spotlight {
                "Gallery"
            } else {
                "Spotlight"
            })
            .size(11)
        )
        .on_press(Message::ToggleLayout)
        .padding([7, 11]),
        button(text("Pop out").size(11))
            .on_press(Message::PopOut)
            .padding([7, 11]),
        button(text("Collapse").size(11))
            .on_press(Message::Collapse)
            .padding([7, 11]),
    ]
    .spacing(9)
    .align_y(Alignment::Center);
    let mut notices = column![].spacing(6);
    if let Some(error) = &runtime.error {
        notices = notices.push(text(error).size(10.5).color(p.danger));
    }
    if runtime.muted && speaking(runtime) {
        notices = notices.push(
            text("Your mic is muted, but it is picking you up.")
                .size(10.5)
                .color(p.danger),
        );
    }
    let center = if member_count(context.chat, &runtime.channel) <= 1 {
        self_check(state, context, p)
    } else {
        gallery(state, context, p, state.spotlight)
    };
    let controls = column![
        if runtime.devices_open {
            devices_view(runtime, p)
        } else {
            Space::new().height(0).into()
        },
        controls_view(state, context, true, p),
    ]
    .spacing(8)
    .align_x(Alignment::Center);
    container(
        column![header, notices, center, controls]
            .spacing(10)
            .height(Length::Fill),
    )
    .padding([10, 14])
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_| container::Style::default().background(p.paper).color(p.ink))
    .into()
}

pub fn window_view<'a>(state: &'a State, context: ViewContext<'a>) -> Element<'a, Message> {
    let p = theme::palette(context.mode);
    let Some(runtime) = &state.runtime else {
        return container(text("Not in a huddle").size(12).color(p.muted))
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(move |_| {
                container::Style::default()
                    .background(p.canvas)
                    .color(p.ink)
            })
            .into();
    };
    let (status, status_color) = status_label(runtime, p);
    let count = member_count(context.chat, &runtime.channel);
    let header = row![
        container(Space::new())
            .width(8)
            .height(8)
            .style(move |_| rounded(status_color, 4.0)),
        column![
            text(format!("Huddle · #{}", runtime.channel))
                .size(12.5)
                .font(theme::SANS_SEMIBOLD),
            text(format!("{status} · {count} in call"))
                .size(9.5)
                .color(p.muted),
        ]
        .spacing(1)
        .width(Length::Fill),
        button(text("Chat").size(10))
            .on_press(Message::OpenChat)
            .padding([4, 7]),
        button(text("Pop in").size(10))
            .on_press(Message::PopIn)
            .padding([4, 7]),
    ]
    .spacing(7)
    .align_y(Alignment::Center);
    let mut content = column![header].spacing(7);
    if let Some(error) = &runtime.error {
        content = content.push(text(error).size(9.5).color(p.danger));
    }
    content = content.push(compact_body(state, context, p));
    content = content.push(
        row![
            text(if runtime.muted {
                "Mic muted"
            } else {
                "Microphone"
            })
            .size(9.5)
            .width(75),
            progress_bar(0.0..=100.0, f32::from(runtime.level)),
        ]
        .spacing(7)
        .align_y(Alignment::Center),
    );
    if runtime.devices_open {
        content = content.push(devices_view(runtime, p));
    }
    content = content.push(controls_view(state, context, false, p));
    container(scrollable(content).height(Length::Fill))
        .padding(10)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_| {
            container::Style::default()
                .background(p.canvas)
                .color(p.ink)
        })
        .into()
}

fn status_label(runtime: &Runtime, p: &theme::Palette) -> (&'static str, Color) {
    match runtime.status {
        Status::Live => ("Live", p.green),
        Status::Connecting => ("Connecting", p.amber),
        Status::Reconnecting => ("Reconnecting", p.amber),
        Status::Unavailable => ("Unavailable", p.danger),
    }
}

fn channel<'a>(chat: &'a user_screens::ChatState, id: &str) -> Option<&'a user_screens::Channel> {
    let user_screens::Resource::Ready(data) = &chat.data else {
        return None;
    };
    data.channels
        .iter()
        .find(|candidate| candidate.id == id || candidate.name == id)
}

fn member_count(chat: &user_screens::ChatState, id: &str) -> usize {
    channel(chat, id).map_or(1, |channel| channel.huddle.len().max(1))
}

fn compact_body<'a>(
    state: &'a State,
    context: ViewContext<'a>,
    p: &'a theme::Palette,
) -> Element<'a, Message> {
    let runtime = state
        .runtime
        .as_ref()
        .expect("compact huddle requires runtime");
    let participants = participants(context, runtime);
    if participants.is_empty() {
        return container(
            column![
                text("Huddle is ready").size(11).font(theme::SANS_SEMIBOLD),
                text("Waiting for participants…").size(10).color(p.muted),
            ]
            .spacing(3),
        )
        .width(Length::Fill)
        .padding([8, 9])
        .style(move |_| bordered(p.canvas, p.border, 6.0))
        .into();
    }
    let overflow = participants.len().saturating_sub(4);
    let mut tiles = participants
        .into_iter()
        .take(4)
        .map(|participant| participant_tile(participant, p, 84.0));
    let mut grid = column![].spacing(6);
    loop {
        let Some(first) = tiles.next() else {
            break;
        };
        let mut line = row![first].spacing(6).width(Length::Fill);
        line = if let Some(second) = tiles.next() {
            line.push(second)
        } else {
            line.push(Space::new().width(Length::Fill))
        };
        grid = grid.push(line);
    }
    if overflow > 0 {
        grid = grid.push(
            text(format!("+{overflow} more not shown"))
                .size(9)
                .color(p.muted),
        );
    }
    grid.into()
}

fn devices_view<'a>(runtime: &'a Runtime, p: &'a theme::Palette) -> Element<'a, Message> {
    let microphones = device_labels(&runtime.devices.microphones);
    let cameras = device_labels(&runtime.devices.cameras);
    let speakers = device_labels(&runtime.devices.speakers);
    let mut devices = column![
        row![
            text("Devices")
                .size(11)
                .font(theme::SANS_SEMIBOLD)
                .width(Length::Fill),
            button(text("Close").size(9.5))
                .on_press(Message::ToggleDevices)
                .padding([3, 6]),
        ]
        .align_y(Alignment::Center),
        text("Microphone").size(9.5).color(p.muted),
        pick_list(
            microphones,
            selected_device(&runtime.devices.microphones, runtime.devices.microphone),
            Message::Microphone,
        )
        .text_size(10.5)
        .width(Length::Fill),
        text("Camera").size(9.5).color(p.muted),
        pick_list(
            cameras,
            selected_device(&runtime.devices.cameras, runtime.devices.camera),
            Message::CameraDevice,
        )
        .text_size(10.5)
        .width(Length::Fill),
        text("Speaker").size(9.5).color(p.muted),
        pick_list(
            speakers,
            selected_device(&runtime.devices.speakers, runtime.devices.speaker),
            Message::Speaker,
        )
        .text_size(10.5)
        .width(Length::Fill),
    ]
    .spacing(5);
    if !runtime.devices.screen_sources.is_empty() {
        let screen_sources = runtime
            .devices
            .screen_sources
            .iter()
            .enumerate()
            .map(|(index, label)| format!("{} · {label}", index + 1))
            .collect::<Vec<_>>();
        let selected = runtime
            .devices
            .screen_source
            .and_then(|index| screen_sources.get(index).cloned());
        devices = devices.push(
            container(
                column![
                    text("Choose what to share")
                        .size(10)
                        .font(theme::SANS_SEMIBOLD),
                    text("Select a display or window to start sharing.")
                        .size(9.5)
                        .color(p.muted),
                    pick_list(screen_sources, selected, Message::ScreenSource)
                        .placeholder("Select a screen source")
                        .text_size(10.5)
                        .width(Length::Fill),
                ]
                .spacing(4),
            )
            .padding([5, 0]),
        );
    }
    container(devices)
        .padding(8)
        .width(Length::Fill)
        .style(move |_| bordered(p.canvas, p.border, 6.0))
        .into()
}

fn controls_view<'a>(
    state: &'a State,
    context: ViewContext<'a>,
    comfortable: bool,
    p: &'a theme::Palette,
) -> Element<'a, Message> {
    let runtime = state.runtime.as_ref().expect("controls require a huddle");
    let live = runtime.status == Status::Live;
    let video_allowed = live && member_count(context.chat, &runtime.channel) <= 8;
    let padding = if comfortable { [7, 12] } else { [5, 7] };
    let mut controls = row![]
        .spacing(if comfortable { 8 } else { 4 })
        .align_y(Alignment::Center);
    controls = controls.push(control_button(
        if runtime.muted {
            "Unmute"
        } else if comfortable {
            "Mute"
        } else {
            "Mic"
        },
        live.then_some(Message::Mute),
        runtime.muted,
        false,
        padding,
        p,
    ));
    controls = controls.push(control_button(
        if runtime.camera_on {
            "Camera off"
        } else if comfortable {
            "Camera"
        } else {
            "Cam"
        },
        video_allowed.then_some(Message::Camera),
        runtime.camera_on,
        false,
        padding,
        p,
    ));
    controls = controls.push(control_button(
        if runtime.sharing {
            "Stop share"
        } else {
            "Share"
        },
        video_allowed.then_some(Message::Share),
        runtime.sharing,
        false,
        padding,
        p,
    ));
    controls = controls.push(control_button(
        if comfortable { "Devices" } else { "Dev" },
        Some(Message::ToggleDevices),
        runtime.devices_open,
        false,
        padding,
        p,
    ));
    if runtime.status == Status::Unavailable {
        controls = controls.push(control_button(
            "Retry",
            Some(Message::Retry),
            false,
            false,
            padding,
            p,
        ));
    }
    controls = controls.push(control_button(
        "Leave",
        Some(Message::Leave),
        false,
        true,
        padding,
        p,
    ));
    controls.into()
}

fn control_button<'a>(
    label: &'a str,
    message: Option<Message>,
    active: bool,
    danger: bool,
    padding: [u16; 2],
    p: &'a theme::Palette,
) -> Element<'a, Message> {
    button(text(label).size(10.5))
        .on_press_maybe(message)
        .padding(padding)
        .style(move |_, status| button::Style {
            background: Some(Background::Color(if danger {
                p.danger_soft
            } else if active {
                p.filled
            } else if matches!(status, button::Status::Hovered) {
                p.hover
            } else {
                p.paper
            })),
            text_color: if danger {
                p.danger
            } else if active {
                p.on_filled
            } else {
                p.ink
            },
            border: Border {
                color: if danger { p.danger_border } else { p.border },
                width: 1.0,
                radius: 6.0.into(),
            },
            ..button::Style::default()
        })
        .into()
}

fn self_check<'a>(
    state: &'a State,
    _context: ViewContext<'a>,
    p: &'a theme::Palette,
) -> Element<'a, Message> {
    let runtime = state
        .runtime
        .as_ref()
        .expect("self-check requires a huddle");
    let is_speaking = speaking(runtime);
    let preview: Element<'a, Message> = if let Some(handle) = &runtime.local_frame {
        video_tile_sized(
            handle,
            "You",
            p,
            300.0,
            is_speaking && !runtime.muted,
            runtime.sharing,
        )
    } else {
        container(
            column![
                text("Camera is off").size(14).font(theme::SANS_SEMIBOLD),
                button(text("Turn on camera").size(11))
                    .on_press_maybe((runtime.status == Status::Live).then_some(Message::Camera))
                    .padding([7, 11]),
            ]
            .spacing(9)
            .align_x(Alignment::Center),
        )
        .width(Length::Fill)
        .height(300)
        .center_x(Length::Fill)
        .center_y(300)
        .style(move |_| bordered(p.canvas, p.border, 8.0))
        .into()
    };
    container(
        column![
            text("You're the only one here")
                .size(16)
                .font(theme::SANS_SEMIBOLD),
            text("Check your camera and microphone while others join.")
                .size(11)
                .color(p.muted),
            preview,
            row![
                text(if runtime.muted {
                    "Mic muted"
                } else {
                    "Microphone"
                })
                .size(10)
                .width(82),
                progress_bar(0.0..=100.0, f32::from(runtime.level)),
                text(format!("{}%", runtime.level)).size(9.5).color(p.muted),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        ]
        .spacing(8)
        .align_x(Alignment::Center),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .max_width(720)
    .center_x(Length::Fill)
    .into()
}

struct Participant<'a> {
    label: &'a str,
    frame: Option<&'a image::Handle>,
    muted: bool,
    sharing: bool,
    speaking: bool,
    stale: bool,
}

fn participants<'a>(context: ViewContext<'a>, runtime: &'a Runtime) -> Vec<Participant<'a>> {
    let self_key = match &context.chat.data {
        user_screens::Resource::Ready(chat) => chat.self_key.as_deref(),
        _ => None,
    };
    let Some(channel) = channel(context.chat, &runtime.channel) else {
        return Vec::new();
    };
    channel
        .huddle
        .iter()
        .map(|member| {
            let is_self = self_key.is_some_and(|key| key == member.user || key == member.node);
            let peer = (!is_self)
                .then(|| runtime.peers.get(&member.node))
                .flatten();
            let sharing = if is_self {
                runtime.sharing
            } else {
                peer.is_some_and(|peer| peer.sharing)
            };
            let video_on = if is_self {
                runtime.camera_on || runtime.sharing
            } else {
                peer.is_some_and(|peer| peer.camera_on || peer.sharing)
            };
            let frame = if !video_on {
                None
            } else if is_self {
                runtime.local_frame.as_ref()
            } else {
                runtime.peer_frames.get(&member.node)
            };
            Participant {
                label: if is_self {
                    "You"
                } else {
                    member_label(context.members, &member.user)
                },
                frame,
                muted: if is_self {
                    runtime.muted
                } else {
                    peer.is_some_and(|peer| peer.muted)
                },
                sharing,
                speaking: is_self && speaking(runtime) && !runtime.muted,
                stale: peer.is_some_and(|peer| peer.seen.elapsed().as_secs() > 10),
            }
        })
        .collect()
}

fn member_label<'a>(members: &'a members_screen::State, key: &'a str) -> &'a str {
    if let members_screen::Resource::Ready(data) = &members.data
        && let Some(member) = data
            .members
            .iter()
            .find(|member| member.key.eq_ignore_ascii_case(key))
    {
        return &member.display_name;
    }
    key.get(..8).unwrap_or(key)
}

fn participant_tile<'a>(
    participant: Participant<'a>,
    p: &'a theme::Palette,
    height: f32,
) -> Element<'a, Message> {
    let media: Element<'a, Message> = participant.frame.map_or_else(
        || {
            let initials = participant
                .label
                .chars()
                .take(2)
                .flat_map(char::to_uppercase)
                .collect::<String>();
            container(
                text(initials)
                    .size(if height > 300.0 { 28 } else { 15 })
                    .color(p.muted_3),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .style(move |_| rounded(p.panel, 7.0))
            .into()
        },
        |handle| {
            image(handle.clone())
                .width(Length::Fill)
                .height(Length::Fill)
                .content_fit(if participant.sharing {
                    iced::ContentFit::Contain
                } else {
                    iced::ContentFit::Cover
                })
                .into()
        },
    );
    let mut name = row![text(participant.label).size(10).color(p.on_filled)]
        .spacing(5)
        .align_y(Alignment::Center);
    if participant.sharing {
        name = name.push(text("screen").size(8.5).color(p.on_filled));
    }
    if participant.muted {
        name = name.push(text("muted").size(8.5).color(p.on_filled));
    }
    let name = container(name)
        .padding([4, 7])
        .style(move |_| rounded(Color::from_rgba8(0, 0, 0, 0.68), 5.0));
    let top: Element<'a, Message> = if participant.stale {
        container(text("no signal").size(8.5).color(Color::WHITE))
            .padding([3, 6])
            .style(move |_| rounded(p.danger, 5.0))
            .into()
    } else {
        Space::new().height(0).into()
    };
    let overlay = column![
        row![Space::new().width(Length::Fill), top],
        Space::new().height(Length::Fill),
        row![name, Space::new().width(Length::Fill)],
    ]
    .padding(6);
    container(stack![media, overlay])
        .height(height)
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(Background::Color(p.panel)),
            border: Border {
                color: if participant.speaking {
                    p.green
                } else {
                    p.border
                },
                width: if participant.speaking { 2.0 } else { 1.0 },
                radius: 7.0.into(),
            },
            ..container::Style::default()
        })
        .into()
}

fn gallery<'a>(
    state: &'a State,
    context: ViewContext<'a>,
    p: &'a theme::Palette,
    spotlight: bool,
) -> Element<'a, Message> {
    let runtime = state.runtime.as_ref().expect("gallery requires a huddle");
    let mut participants = participants(context, runtime);
    if participants.is_empty() {
        return empty_stage(p);
    }
    if spotlight {
        let selected = participants
            .iter()
            .position(|participant| participant.speaking)
            .unwrap_or(0);
        let selected = participants.remove(selected);
        let has_filmstrip = !participants.is_empty();
        let mut filmstrip = row![].spacing(8);
        for participant in participants {
            filmstrip = filmstrip.push(
                container(participant_tile(participant, p, 96.0))
                    .width(150)
                    .height(96),
            );
        }
        let mut stage = column![participant_tile(selected, p, 390.0)]
            .spacing(9)
            .height(Length::Fill);
        if has_filmstrip {
            stage = stage.push(
                scrollable(filmstrip)
                    .direction(iced::widget::scrollable::Direction::Horizontal(
                        iced::widget::scrollable::Scrollbar::new(),
                    ))
                    .height(100),
            );
        }
        return container(stage)
            .max_width(920)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .into();
    }

    let columns = match participants.len() {
        0 | 1 => 1,
        2..=4 => 2,
        _ => 3,
    };
    let mut tiles = participants
        .into_iter()
        .map(|participant| participant_tile(participant, p, 220.0));
    let mut grid = column![].spacing(8);
    loop {
        let Some(first) = tiles.next() else {
            break;
        };
        let mut line = row![first].spacing(8).width(Length::Fill);
        for _ in 1..columns {
            line = if let Some(tile) = tiles.next() {
                line.push(tile)
            } else {
                line.push(Space::new().width(Length::Fill))
            };
        }
        grid = grid.push(line);
    }
    container(scrollable(grid).height(Length::Fill))
        .max_width(980)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .into()
}

fn empty_stage(p: &theme::Palette) -> Element<'_, Message> {
    container(
        column![
            text("Waiting for video")
                .size(15)
                .font(theme::SANS_SEMIBOLD),
            text("Participant video and screen shares appear here.")
                .size(10.5)
                .color(p.muted),
        ]
        .spacing(6)
        .align_x(Alignment::Center),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .style(move |_| bordered(p.canvas, p.border, 8.0))
    .into()
}

fn video_tile_sized<'a>(
    handle: &'a image::Handle,
    label: &'a str,
    p: &'a theme::Palette,
    height: f32,
    speaking: bool,
    contain: bool,
) -> Element<'a, Message> {
    container(stack![
        image(handle.clone())
            .width(Length::Fill)
            .height(Length::Fill)
            .content_fit(if contain {
                iced::ContentFit::Contain
            } else {
                iced::ContentFit::Cover
            }),
        container(text(label).size(10).color(p.on_filled))
            .padding([4, 7])
            .style(move |_| rounded(Color::from_rgba8(0, 0, 0, 0.58), 5.0)),
    ])
    .height(height)
    .width(Length::Fill)
    .style(move |_| container::Style {
        background: Some(Background::Color(p.panel)),
        border: Border {
            color: if speaking { p.green } else { p.border },
            width: if speaking { 2.0 } else { 1.0 },
            radius: 7.0.into(),
        },
        ..container::Style::default()
    })
    .into()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime(channel: &str) -> Runtime {
        Runtime {
            channel: channel.into(),
            session: None,
            media: None,
            status: Status::Live,
            muted: true,
            camera_on: false,
            sharing: false,
            error: None,
            local_frame: None,
            peer_frames: BTreeMap::new(),
            peers: BTreeMap::new(),
            devices: huddle_media::DeviceOptions::default(),
            devices_open: false,
            level: 0,
            speaking_until: None,
            recipients: Vec::new(),
            retry_pending: false,
            last_reconnect: None,
        }
    }

    fn channel(id: &str, nodes: impl IntoIterator<Item = String>) -> user_screens::Channel {
        user_screens::Channel {
            id: id.into(),
            name: id.into(),
            archived: false,
            policy: user_screens::PostPolicy::Open,
            owner: None,
            huddle: std::iter::once(user_screens::HuddleMember {
                user: "self".into(),
                node: "local-node".into(),
            })
            .chain(nodes.into_iter().map(|node| user_screens::HuddleMember {
                user: format!("user-{node}"),
                node,
            }))
            .collect(),
        }
    }

    fn chat(channels: Vec<user_screens::Channel>, active: &str) -> user_screens::ChatState {
        user_screens::ChatState {
            data: user_screens::Resource::Ready(user_screens::ChatData {
                channels,
                messages: Vec::new(),
                thread: None,
                members: Vec::new(),
                tags: Vec::new(),
                hits: Vec::new(),
                history_window: None,
                self_key: Some("self".into()),
            }),
            active_channel: Some(active.into()),
            ..user_screens::ChatState::default()
        }
    }

    #[test]
    fn joined_prefers_active_and_bounds_sorted_unique_remote_nodes() {
        let nodes = (0..70)
            .rev()
            .map(|index| format!("node-{index:02}"))
            .chain(["node-03".into(), "local-node".into()]);
        let chat = chat(
            vec![
                channel("fallback", ["fallback-node".into()]),
                channel("active", nodes),
            ],
            "active",
        );

        let (channel, recipients) = joined(&chat, Some("local-node")).unwrap();

        assert_eq!(channel, "active");
        assert_eq!(recipients.len(), 64);
        assert_eq!(recipients.first().map(String::as_str), Some("node-00"));
        assert_eq!(recipients.last().map(String::as_str), Some("node-63"));
        assert!(!recipients.iter().any(|node| node == "local-node"));
    }

    #[test]
    fn device_labels_and_indices_round_trip_with_default() {
        let options = vec!["Studio Mic".into(), "Laptop Mic".into()];
        assert_eq!(
            device_labels(&options),
            ["System default", "1 · Studio Mic", "2 · Laptop Mic"]
        );
        assert_eq!(device_index("System default"), None);
        assert_eq!(device_index("2 · Laptop Mic"), Some(1));
        assert_eq!(
            selected_device(&options, Some(1)).as_deref(),
            Some("2 · Laptop Mic")
        );
        assert_eq!(
            selected_device(&options, Some(9)).as_deref(),
            Some("System default")
        );
    }

    #[test]
    fn leave_and_reset_clear_runtime_and_expansion_once() {
        let mut state = State {
            runtime: Some(runtime("general")),
            expanded: true,
            ..State::default()
        };
        let chat = user_screens::ChatState::default();

        assert_eq!(
            update(&mut state, Message::Leave, &chat, None, None),
            Some(Action::Leave("general".into()))
        );
        assert!(!state.is_active());
        assert!(!state.is_expanded());
        assert_eq!(update(&mut state, Message::Leave, &chat, None, None), None);

        state.runtime = Some(runtime("again"));
        state.expanded = true;
        assert!(state.reset());
        assert!(!state.is_active());
        assert!(!state.is_expanded());
        assert!(!state.reset());
    }

    #[test]
    fn preserved_huddle_views_construct_in_both_palettes() {
        let chat = chat(vec![channel("general", std::iter::empty())], "general");
        let members = members_screen::State::default();
        let state = State {
            runtime: Some(runtime("general")),
            ..State::default()
        };
        for mode in [Mode::Light, Mode::Dark] {
            let context = ViewContext {
                chat: &chat,
                members: &members,
                mode,
            };
            drop(dock_view(&state, context));
            drop(stage_view(&state, context));
            drop(window_view(&state, context));
        }
    }
}
