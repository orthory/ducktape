//! Capability-free native terminal presentation and input encoding.
//!
//! The trusted session/websocket lifecycle lives in `terminal_service`; this
//! module only parses terminal bytes, renders cells, and emits typed effects.

use std::time::{Duration, Instant};

use iced::advanced::renderer::Renderer as _;
use iced::advanced::text::Renderer as _;
use iced::advanced::widget::Operation;
use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::{Layout, Shell, Text, Widget, input_method, layout, mouse, renderer};
use iced::keyboard::{self, Key, Modifiers, key};
use iced::widget::{Space, button, column, container, row, scrollable, stack, text, text_input};
use iced::{
    Alignment, Background, Border, Color, Element, Event, Font, Length, Pixels, Point, Rectangle,
    Size, Theme, window,
};

use crate::icons::{self, Icon};
pub use crate::terminal_contract::SessionMode;
use crate::theme::{self, MONO, SANS, SANS_SEMIBOLD};

const INITIAL_ROWS: u16 = 24;
const INITIAL_COLS: u16 = 80;
const SCROLLBACK_ROWS: usize = 5_000;
const FONT_SIZE: f32 = 13.0;
const CELL_WIDTH: f32 = 8.0;
const CELL_HEIGHT: f32 = 18.0;
const CURSOR_BLINK: Duration = Duration::from_millis(500);
const MAX_COLS: u16 = 500;
const MAX_ROWS: u16 = 300;
const MAX_PASTE_BYTES: usize = 60 * 1024;
const MAX_COMMAND_BYTES: usize = 64 * 1024;
const MAX_COMMAND_ROWS: usize = 1_024;

#[derive(Debug, Clone, PartialEq, Eq)]
struct CommandRow {
    seq: u64,
    origin: String,
    text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Idle,
    Starting,
    Live,
    Reconnecting,
    Failed,
}

pub struct State {
    parser: vt100::Parser,
    modes: ModeTracker,
    status: Status,
    error: Option<String>,
    generation: u64,
    geometry: Option<(u16, u16)>,
    session_mode: SessionMode,
    commands: Vec<CommandRow>,
    command_draft: String,
}

impl std::fmt::Debug for State {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("State")
            .field("status", &self.status)
            .field("error", &self.error)
            .field("generation", &self.generation)
            .field("geometry", &self.geometry)
            .field("session_mode", &self.session_mode)
            .field("commands", &self.commands)
            .field("command_draft", &self.command_draft)
            .finish_non_exhaustive()
    }
}

impl Default for State {
    fn default() -> Self {
        Self {
            parser: vt100::Parser::new(INITIAL_ROWS, INITIAL_COLS, SCROLLBACK_ROWS),
            modes: ModeTracker::default(),
            status: Status::Idle,
            error: None,
            generation: 0,
            geometry: None,
            session_mode: SessionMode::Single,
            commands: Vec::new(),
            command_draft: String::new(),
        }
    }
}

impl State {
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    #[allow(dead_code)]
    pub const fn status(&self) -> Status {
        self.status
    }

    pub const fn session_mode(&self) -> SessionMode {
        self.session_mode
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Start,
    Retry,
    Stop,
    SetMode(SessionMode),
    Connected {
        generation: u64,
    },
    Reconnecting {
        generation: u64,
        detail: String,
    },
    Output {
        generation: u64,
        bytes: Vec<u8>,
    },
    CommandLogged {
        generation: u64,
        seq: u64,
        origin: String,
        text: String,
    },
    Failed {
        generation: u64,
        detail: String,
    },
    CommandDraftChanged(String),
    SubmitCommand,
    Input(Vec<u8>),
    Copy(String),
    RequestPaste,
    Paste {
        generation: u64,
        value: String,
    },
    Resize {
        cols: u16,
        rows: u16,
    },
    Scroll(i32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    Start {
        generation: u64,
        mode: SessionMode,
    },
    Stop {
        generation: u64,
    },
    Input {
        generation: u64,
        bytes: Vec<u8>,
    },
    Command {
        generation: u64,
        text: String,
    },
    Copy(String),
    ReadClipboard {
        generation: u64,
    },
    Resize {
        generation: u64,
        cols: u16,
        rows: u16,
    },
}

pub fn update(state: &mut State, message: Message) -> Option<Effect> {
    match message {
        Message::Start | Message::Retry => Some(reset_session(state)),
        Message::SetMode(mode) if mode != state.session_mode => {
            state.session_mode = mode;
            Some(reset_session(state))
        }
        Message::Stop => {
            let generation = state.generation;
            state.generation = state.generation.wrapping_add(1).max(1);
            state.modes = ModeTracker::default();
            state.status = Status::Idle;
            state.error = None;
            state.commands.clear();
            state.command_draft.clear();
            (generation != 0).then_some(Effect::Stop { generation })
        }
        Message::Connected { generation } if generation == state.generation => {
            state.status = Status::Live;
            state.error = None;
            state.geometry.map(|(cols, rows)| Effect::Resize {
                generation,
                cols,
                rows,
            })
        }
        Message::Reconnecting { generation, detail } if generation == state.generation => {
            state.status = Status::Reconnecting;
            state.error = Some(detail);
            None
        }
        Message::Output { generation, bytes } if generation == state.generation => {
            state.modes.process(&bytes);
            state.parser.process(&bytes);
            None
        }
        Message::CommandLogged {
            generation,
            seq,
            origin,
            text,
        } if generation == state.generation
            && state.session_mode == SessionMode::Shared
            && seq > state.commands.last().map_or(0, |command| command.seq) =>
        {
            if state.commands.len() == MAX_COMMAND_ROWS {
                state.commands.remove(0);
            }
            state.commands.push(CommandRow { seq, origin, text });
            None
        }
        Message::Failed { generation, detail } if generation == state.generation => {
            state.status = Status::Failed;
            state.error = Some(detail);
            None
        }
        Message::CommandDraftChanged(value) if state.session_mode == SessionMode::Shared => {
            state.command_draft = bounded_command(value);
            None
        }
        Message::SubmitCommand
            if state.session_mode == SessionMode::Shared && state.status == Status::Live =>
        {
            let text = state.command_draft.trim().to_owned();
            if text.is_empty() {
                return None;
            }
            state.command_draft.clear();
            Some(Effect::Command {
                generation: state.generation,
                text,
            })
        }
        Message::Input(bytes)
            if state.session_mode == SessionMode::Single
                && state.status == Status::Live
                && !bytes.is_empty() =>
        {
            state.parser.screen_mut().set_scrollback(0);
            Some(Effect::Input {
                generation: state.generation,
                bytes,
            })
        }
        Message::Copy(value) if !value.is_empty() => Some(Effect::Copy(value)),
        Message::RequestPaste
            if state.session_mode == SessionMode::Single && state.status == Status::Live =>
        {
            Some(Effect::ReadClipboard {
                generation: state.generation,
            })
        }
        Message::Paste { generation, value }
            if generation == state.generation
                && state.session_mode == SessionMode::Single
                && state.status == Status::Live
                && !value.is_empty() =>
        {
            let bytes = paste_bytes(state.parser.screen().bracketed_paste(), &value)?;
            state.parser.screen_mut().set_scrollback(0);
            Some(Effect::Input {
                generation: state.generation,
                bytes,
            })
        }
        Message::Resize { cols, rows } if cols > 0 && rows > 0 => {
            let geometry = (cols.min(MAX_COLS), rows.min(MAX_ROWS));
            if state.geometry == Some(geometry) {
                return None;
            }
            state.geometry = Some(geometry);
            state.parser.screen_mut().set_size(geometry.1, geometry.0);
            (state.status == Status::Live).then_some(Effect::Resize {
                generation: state.generation,
                cols: geometry.0,
                rows: geometry.1,
            })
        }
        Message::Scroll(lines) => {
            let current = state.parser.screen().scrollback();
            let next = if lines >= 0 {
                current.saturating_add(lines as usize)
            } else {
                current.saturating_sub(lines.unsigned_abs() as usize)
            };
            state.parser.screen_mut().set_scrollback(next);
            None
        }
        _ => None,
    }
}

fn reset_session(state: &mut State) -> Effect {
    state.generation = state.generation.wrapping_add(1).max(1);
    state.parser = vt100::Parser::new(INITIAL_ROWS, INITIAL_COLS, SCROLLBACK_ROWS);
    state.modes = ModeTracker::default();
    if let Some((cols, rows)) = state.geometry {
        state.parser.screen_mut().set_size(rows, cols);
    }
    state.status = Status::Starting;
    state.error = None;
    state.commands.clear();
    state.command_draft.clear();
    Effect::Start {
        generation: state.generation,
        mode: state.session_mode,
    }
}

fn bounded_command(mut value: String) -> String {
    if value.len() <= MAX_COMMAND_BYTES {
        return value;
    }
    let mut end = MAX_COMMAND_BYTES;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value.truncate(end);
    value
}

pub fn view(state: &State, mode: theme::Mode) -> Element<'_, Message> {
    let p = theme::palette(mode);
    let terminal_p = &theme::DARK;
    let mode_switch = container(
        row![
            mode_button("Single", SessionMode::Single, state.session_mode, *p),
            mode_button("Shared", SessionMode::Shared, state.session_mode, *p),
        ]
        .spacing(2),
    )
    .padding(2)
    .style(move |_| {
        container::Style::default()
            .background(p.canvas)
            .border(Border {
                color: p.border_soft,
                width: 1.0,
                radius: 6.0.into(),
            })
    });
    let header = row![
        container(icons::view(Icon::Terminal, 16.0, p.on_filled))
            .width(30)
            .height(30)
            .center_x(30)
            .center_y(30)
            .style(move |_| container::Style::default()
                .background(p.filled)
                .border(Border::default().rounded(6))),
        text("Terminal").size(18).font(SANS_SEMIBOLD).color(p.ink),
        text("codex").size(12).font(MONO).color(p.muted),
        Space::new().width(Length::Fill),
        mode_switch,
    ]
    .spacing(12)
    .align_y(Alignment::Center);

    let terminal: Element<'_, Message> = Terminal::new(state).into();
    #[cfg(all(feature = "agent", debug_assertions))]
    let terminal: Element<'_, Message> =
        iced_agent_plugin::sem(iced_agent_plugin::Role::Region, "terminal", terminal);
    let body: Element<'_, Message> = match state.status {
        Status::Idle => stack![
            terminal,
            terminal_notice(
                "codex session is not running",
                terminal_p.muted_2,
                None,
                terminal_p
            )
        ]
        .into(),
        Status::Starting => stack![
            terminal,
            terminal_notice(
                "starting codex session…",
                terminal_p.muted_2,
                None,
                terminal_p
            )
        ]
        .into(),
        Status::Reconnecting => stack![
            terminal,
            terminal_notice(
                "reconnecting terminal session…",
                terminal_p.amber,
                None,
                terminal_p
            )
        ]
        .into(),
        Status::Failed => stack![
            terminal,
            terminal_notice(
                state.error.as_deref().unwrap_or("terminal session failed"),
                terminal_p.danger,
                Some(Message::Retry),
                terminal_p,
            )
        ]
        .into(),
        Status::Live => terminal,
    };

    let terminal_area: Element<'_, Message> = container(body)
        .padding(10)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| container::Style::default().background(Color::BLACK))
        .into();
    let content: Element<'_, Message> = if state.session_mode == SessionMode::Shared {
        column![terminal_area, shared_command_panel(state, *p)]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    } else {
        terminal_area
    };

    column![
        column![
            container(header)
                .height(55)
                .width(Length::Fill)
                .padding([0, 22])
                .style(move |_| container::Style::default().background(p.paper)),
            container(Space::new())
                .height(1)
                .width(Length::Fill)
                .style(move |_| container::Style::default().background(p.border)),
        ]
        .height(56),
        content,
    ]
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn mode_button(
    label: &'static str,
    mode: SessionMode,
    active: SessionMode,
    p: theme::Palette,
) -> Element<'static, Message> {
    let selected = mode == active;
    let btn = button(text(label).size(12).font(SANS_SEMIBOLD))
        .padding([5, 12])
        .style(move |_, status| iced::widget::button::Style {
            background: Some(Background::Color(if selected {
                p.filled
            } else if matches!(status, iced::widget::button::Status::Hovered) {
                p.hover
            } else {
                p.canvas
            })),
            text_color: if selected { p.on_filled } else { p.muted_2 },
            border: Border {
                radius: 5.0.into(),
                ..Border::default()
            },
            ..iced::widget::button::Style::default()
        })
        .on_press(Message::SetMode(mode));
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(iced_agent_plugin::Role::Tab, label, btn);
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
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

fn shared_command_panel(state: &State, p: theme::Palette) -> Element<'_, Message> {
    let mut rows = column![].spacing(4).width(Length::Fill);
    if state.commands.is_empty() {
        rows = rows.push(
            text("No commands yet — the ordered log appears here.")
                .size(12)
                .font(MONO)
                .color(p.muted_2),
        );
    } else {
        for command in &state.commands {
            rows = rows.push(
                row![
                    text(command.seq.to_string())
                        .size(12)
                        .font(MONO)
                        .color(p.muted_2)
                        .width(28),
                    text(&command.origin).size(12).font(MONO).color(p.muted_2),
                    text(&command.text)
                        .size(12)
                        .font(MONO)
                        .color(p.ink)
                        .width(Length::Fill),
                ]
                .spacing(10),
            );
        }
    }

    let ready = state.status == Status::Live;
    let can_send = ready && !state.command_draft.trim().is_empty();
    let input = text_input("Send a command…", &state.command_draft)
        .on_input_maybe(ready.then_some(Message::CommandDraftChanged))
        .on_submit_maybe(can_send.then_some(Message::SubmitCommand))
        .padding([8, 10])
        .size(13)
        .font(MONO)
        .width(Length::Fill)
        .style(move |_, status| iced::widget::text_input::Style {
            background: Background::Color(p.canvas),
            border: Border {
                color: if matches!(status, iced::widget::text_input::Status::Focused { .. }) {
                    p.border_strong
                } else {
                    p.border_soft
                },
                width: 1.0,
                radius: 6.0.into(),
            },
            icon: p.muted,
            placeholder: p.muted_2,
            value: p.ink,
            selection: theme::ACCENTS[0],
        });
    let send = button(text("Send").size(13).font(SANS_SEMIBOLD))
        .padding([8, 16])
        .style(move |_, _| iced::widget::button::Style {
            background: Some(Background::Color(if can_send {
                p.filled
            } else {
                p.sunken
            })),
            text_color: if can_send { p.on_filled } else { p.muted_2 },
            border: Border {
                radius: 6.0.into(),
                ..Border::default()
            },
            ..iced::widget::button::Style::default()
        });
    let send = if can_send {
        send.on_press(Message::SubmitCommand)
    } else {
        send
    };
    #[cfg(all(feature = "agent", debug_assertions))]
    let send = iced_agent_plugin::sem(iced_agent_plugin::Role::Button, "Send", send);
    let input = sem_input("Command", &state.command_draft, input);

    container(column![
        container(Space::new())
            .height(1)
            .width(Length::Fill)
            .style(move |_| container::Style::default().background(p.border_soft)),
        container(scrollable(rows).height(Length::Shrink))
            .max_height(160)
            .padding([8, 14])
            .width(Length::Fill),
        container(column![
            container(Space::new())
                .height(1)
                .width(Length::Fill)
                .style(move |_| container::Style::default().background(p.border_soft)),
            row![input, send].spacing(8).padding([10, 14]),
        ])
        .width(Length::Fill),
    ])
    .width(Length::Fill)
    .style(move |_| container::Style::default().background(p.paper))
    .into()
}

fn terminal_notice<'a>(
    label: &'a str,
    color: Color,
    retry: Option<Message>,
    p: &'a theme::Palette,
) -> Element<'a, Message> {
    let mut content = column![text(label).size(13).font(SANS).color(color)]
        .spacing(12)
        .align_x(Alignment::Center);
    if let Some(message) = retry {
        let retry_btn = button(text("Retry").size(12))
            .on_press(message)
            .padding([7, 14]);
        #[cfg(all(feature = "agent", debug_assertions))]
        let retry_btn = iced_agent_plugin::sem(iced_agent_plugin::Role::Button, "Retry", retry_btn);
        content = content.push(retry_btn);
    }
    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(move |_| {
            container::Style::default().background(Color {
                a: 0.94,
                ..p.canvas
            })
        })
        .into()
}

#[derive(Debug)]
struct Terminal<'a> {
    state: &'a State,
}

impl<'a> Terminal<'a> {
    const fn new(state: &'a State) -> Self {
        Self { state }
    }
}

#[derive(Debug)]
struct Selection {
    anchor: (u16, u16),
    head: (u16, u16),
}

#[derive(Debug, Default)]
struct ModeTracker {
    ansi: AnsiState,
    alternate_scroll: bool,
}

#[derive(Debug, Default)]
enum AnsiState {
    #[default]
    Ground,
    Escape,
    Csi(Vec<u8>),
    ControlString,
    ControlStringEscape,
}

impl ModeTracker {
    fn process(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.ansi = match std::mem::take(&mut self.ansi) {
                AnsiState::Ground if byte == 0x1b => AnsiState::Escape,
                AnsiState::Ground => AnsiState::Ground,
                AnsiState::Escape => match byte {
                    b'[' => AnsiState::Csi(Vec::new()),
                    b']' | b'P' | b'^' | b'_' | b'X' => AnsiState::ControlString,
                    0x1b => AnsiState::Escape,
                    _ => AnsiState::Ground,
                },
                AnsiState::Csi(parameters) if (0x40..=0x7e).contains(&byte) => {
                    if matches!(byte, b'h' | b'l') && private_mode(&parameters, 1007) {
                        self.alternate_scroll = byte == b'h';
                    }
                    AnsiState::Ground
                }
                AnsiState::Csi(_) if byte == 0x1b => AnsiState::Escape,
                AnsiState::Csi(mut parameters) if parameters.len() < 64 => {
                    parameters.push(byte);
                    AnsiState::Csi(parameters)
                }
                AnsiState::Csi(_) => AnsiState::Ground,
                AnsiState::ControlString if matches!(byte, 0x07 | 0x9c) => AnsiState::Ground,
                AnsiState::ControlString if byte == 0x1b => AnsiState::ControlStringEscape,
                AnsiState::ControlString => AnsiState::ControlString,
                AnsiState::ControlStringEscape if byte == b'\\' => AnsiState::Ground,
                AnsiState::ControlStringEscape if byte == 0x1b => AnsiState::ControlStringEscape,
                AnsiState::ControlStringEscape => AnsiState::ControlString,
            };
        }
    }
}

fn private_mode(parameters: &[u8], expected: u16) -> bool {
    parameters.strip_prefix(b"?").is_some_and(|parameters| {
        parameters
            .split(|byte| *byte == b';')
            .any(|parameter| parse_decimal(parameter) == Some(expected))
    })
}

fn parse_decimal(bytes: &[u8]) -> Option<u16> {
    bytes.iter().try_fold(0u16, |value, byte| {
        if !byte.is_ascii_digit() {
            return None;
        }
        value
            .checked_mul(10)?
            .checked_add(u16::from(byte.checked_sub(b'0')?))
    })
}

#[derive(Debug)]
struct WidgetState {
    focused: bool,
    window_focused: bool,
    last_geometry: Option<(u16, u16)>,
    preedit: Option<input_method::Preedit>,
    cursor_on: bool,
    last_blink: Instant,
    selection: Option<Selection>,
    dragging: bool,
    modifiers: Modifiers,
    reported_button: Option<u8>,
    last_mouse_cell: Option<(u16, u16)>,
    mouse_mode: vt100::MouseProtocolMode,
    scroll_remainder: f32,
}

impl Default for WidgetState {
    fn default() -> Self {
        Self {
            focused: false,
            window_focused: true,
            last_geometry: None,
            preedit: None,
            cursor_on: true,
            last_blink: Instant::now(),
            selection: None,
            dragging: false,
            modifiers: Modifiers::empty(),
            reported_button: None,
            last_mouse_cell: None,
            mouse_mode: vt100::MouseProtocolMode::None,
            scroll_remainder: 0.0,
        }
    }
}

impl Widget<Message, Theme, iced::Renderer> for Terminal<'_> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<WidgetState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(WidgetState::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(limits.max())
    }

    fn operate(
        &mut self,
        _tree: &mut Tree,
        layout: Layout<'_>,
        _renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.container(None, layout.bounds());
        let contents = self.state.parser.screen().contents();
        operation.text(None, layout.bounds(), &contents);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &iced::Renderer,
        _clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<WidgetState>();
        let accepts_stdin = self.state.session_mode == SessionMode::Single;
        let mouse_mode = self.state.parser.screen().mouse_protocol_mode();
        if state.mouse_mode != mouse_mode {
            state.mouse_mode = mouse_mode;
            state.reported_button = None;
            state.last_mouse_cell = None;
            state.dragging = false;
            state.scroll_remainder = 0.0;
        }
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(button)) => {
                state.focused = cursor.is_over(layout.bounds());
                state.cursor_on = true;
                state.last_blink = Instant::now();
                shell.request_redraw();
                if state.focused {
                    if let Some(position) = cursor.position() {
                        let cell = cell_at(position, layout.bounds(), self.state.parser.screen());
                        let screen = self.state.parser.screen();
                        if accepts_stdin
                            && mouse_reporting(screen.mouse_protocol_mode(), state.modifiers)
                        {
                            if let Some(button) = mouse_button_code(*button) {
                                if let Some(bytes) = mouse_bytes(
                                    screen.mouse_protocol_encoding(),
                                    button,
                                    cell,
                                    mouse_protocol_modifiers(
                                        screen.mouse_protocol_mode(),
                                        state.modifiers,
                                    ),
                                    false,
                                    false,
                                ) {
                                    state.reported_button = Some(button);
                                    state.last_mouse_cell = Some(cell);
                                    shell.publish(Message::Input(bytes));
                                } else {
                                    state.reported_button = None;
                                    state.last_mouse_cell = None;
                                }
                            }
                        } else if matches!(button, mouse::Button::Left) {
                            state.selection = Some(Selection {
                                anchor: cell,
                                head: cell,
                            });
                            state.dragging = true;
                        }
                    }
                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::CursorMoved { position }) => {
                let screen = self.state.parser.screen();
                let cell = cell_at(*position, layout.bounds(), screen);
                if cursor.is_over(layout.bounds())
                    && accepts_stdin
                    && mouse_reporting(screen.mouse_protocol_mode(), state.modifiers)
                    && mouse_motion_enabled(
                        screen.mouse_protocol_mode(),
                        state.reported_button.is_some(),
                    )
                    && state.last_mouse_cell != Some(cell)
                {
                    let button = state.reported_button.unwrap_or(3);
                    state.last_mouse_cell = Some(cell);
                    if let Some(bytes) = mouse_bytes(
                        screen.mouse_protocol_encoding(),
                        button,
                        cell,
                        state.modifiers,
                        false,
                        true,
                    ) {
                        shell.publish(Message::Input(bytes));
                    }
                    shell.capture_event();
                } else if state.dragging
                    && let Some(selection) = &mut state.selection
                {
                    selection.head = cell;
                    shell.request_redraw();
                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(button)) => {
                let screen = self.state.parser.screen();
                if let Some(reported) = state.reported_button
                    && mouse_button_code(*button) == Some(reported)
                {
                    if accepts_stdin
                        && mouse_reporting(screen.mouse_protocol_mode(), state.modifiers)
                        && !matches!(
                            screen.mouse_protocol_mode(),
                            vt100::MouseProtocolMode::Press
                        )
                        && let Some(position) = cursor.position()
                        && let Some(bytes) = mouse_bytes(
                            screen.mouse_protocol_encoding(),
                            reported,
                            cell_at(position, layout.bounds(), screen),
                            state.modifiers,
                            true,
                            false,
                        )
                    {
                        shell.publish(Message::Input(bytes));
                    }
                    state.reported_button = None;
                    shell.capture_event();
                } else if matches!(button, mouse::Button::Left) {
                    state.dragging = false;
                }
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta })
                if cursor.is_over(layout.bounds()) =>
            {
                let lines = scroll_lines(*delta, &mut state.scroll_remainder);
                if lines != 0 {
                    let screen = self.state.parser.screen();
                    if accepts_stdin
                        && mouse_wheel_reporting(screen.mouse_protocol_mode(), state.modifiers)
                    {
                        let button = if lines > 0 { 64 } else { 65 };
                        if let Some(position) = cursor.position() {
                            let cell = cell_at(position, layout.bounds(), screen);
                            for _ in 0..lines.unsigned_abs().min(32) {
                                if let Some(bytes) = mouse_bytes(
                                    screen.mouse_protocol_encoding(),
                                    button,
                                    cell,
                                    state.modifiers,
                                    false,
                                    false,
                                ) {
                                    shell.publish(Message::Input(bytes));
                                }
                            }
                        }
                    } else if accepts_stdin
                        && self.state.modes.alternate_scroll
                        && screen.alternate_screen()
                        && matches!(screen.mouse_protocol_mode(), vt100::MouseProtocolMode::None)
                        && !state.modifiers.shift()
                    {
                        let key = if lines > 0 {
                            key::Named::ArrowUp
                        } else {
                            key::Named::ArrowDown
                        };
                        if let Some(bytes) = named_key_bytes(
                            &Key::Named(key),
                            Modifiers::empty(),
                            screen.application_cursor(),
                        ) {
                            for _ in 0..lines.unsigned_abs().min(32) {
                                shell.publish(Message::Input(bytes.clone()));
                            }
                        }
                    } else {
                        shell.publish(Message::Scroll(lines));
                    }
                    shell.capture_event();
                }
            }
            Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
                state.modifiers = *modifiers;
                if modifiers.shift() {
                    state.reported_button = None;
                    state.last_mouse_cell = None;
                    state.dragging = false;
                }
            }
            Event::Keyboard(keyboard::Event::KeyPressed {
                key,
                text,
                modifiers,
                ..
            }) if state.focused && state.window_focused => {
                if is_copy_shortcut(key, *modifiers) {
                    if let Some(value) =
                        selected_text(self.state.parser.screen(), state.selection.as_ref())
                    {
                        shell.publish(Message::Copy(value));
                    }
                    shell.capture_event();
                } else if accepts_stdin && is_paste_shortcut(key, *modifiers) {
                    shell.publish(Message::RequestPaste);
                    shell.capture_event();
                } else if accepts_stdin
                    && let Some(bytes) = key_bytes(
                        key,
                        text.as_deref(),
                        *modifiers,
                        self.state.parser.screen().application_cursor(),
                    )
                {
                    shell.publish(Message::Input(bytes));
                    shell.capture_event();
                }
            }
            Event::InputMethod(input_method::Event::Opened) if accepts_stdin => {
                state.preedit = Some(input_method::Preedit::new());
                shell.request_redraw();
            }
            Event::InputMethod(input_method::Event::Closed) => {
                state.preedit = None;
                shell.request_redraw();
            }
            Event::InputMethod(input_method::Event::Preedit(content, selection))
                if accepts_stdin && state.focused =>
            {
                state.preedit = Some(input_method::Preedit {
                    content: content.clone(),
                    selection: selection.clone(),
                    text_size: Some(Pixels(FONT_SIZE)),
                });
                shell.request_redraw();
            }
            Event::InputMethod(input_method::Event::Commit(value))
                if accepts_stdin && state.focused =>
            {
                if !value.is_empty() {
                    shell.publish(Message::Input(value.as_bytes().to_vec()));
                    shell.capture_event();
                }
                state.preedit = Some(input_method::Preedit::new());
            }
            Event::Window(window::Event::Focused) => {
                state.window_focused = true;
                shell.request_redraw();
            }
            Event::Window(window::Event::Unfocused) => {
                state.window_focused = false;
                state.reported_button = None;
                state.last_mouse_cell = None;
                state.dragging = false;
                state.scroll_remainder = 0.0;
                shell.request_redraw();
            }
            Event::Window(window::Event::RedrawRequested(now)) => {
                let geometry = terminal_geometry(layout.bounds().size());
                if geometry != state.last_geometry {
                    state.last_geometry = geometry;
                    if let Some((cols, rows)) = geometry {
                        shell.publish(Message::Resize { cols, rows });
                    }
                }
                if accepts_stdin && state.focused && state.window_focused {
                    if now.duration_since(state.last_blink) >= CURSOR_BLINK {
                        state.cursor_on = !state.cursor_on;
                        state.last_blink = *now;
                    }
                    shell.request_redraw_at(state.last_blink + CURSOR_BLINK);
                    let (row, col) = self.state.parser.screen().cursor_position();
                    shell.request_input_method(&input_method::InputMethod::Enabled {
                        cursor: Rectangle::new(
                            Point::new(
                                layout.bounds().x + f32::from(col) * CELL_WIDTH,
                                layout.bounds().y + f32::from(row) * CELL_HEIGHT,
                            ),
                            Size::new(CELL_WIDTH, CELL_HEIGHT),
                        ),
                        purpose: input_method::Purpose::Terminal,
                        preedit: state.preedit.as_ref().map(input_method::Preedit::as_ref),
                    });
                }
            }
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        if cursor.is_over(layout.bounds()) {
            mouse::Interaction::Text
        } else {
            mouse::Interaction::default()
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        _theme: &Theme,
        _defaults: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                ..renderer::Quad::default()
            },
            Color::BLACK,
        );
        let Some(clip) = bounds.intersection(viewport) else {
            return;
        };
        renderer.with_layer(clip, |renderer| {
            draw_screen(
                renderer,
                self.state.parser.screen(),
                tree.state.downcast_ref(),
                bounds,
                self.state.session_mode == SessionMode::Single,
            );
        });
    }
}

impl<'a> From<Terminal<'a>> for Element<'a, Message> {
    fn from(terminal: Terminal<'a>) -> Self {
        Element::new(terminal)
    }
}

fn draw_screen(
    renderer: &mut iced::Renderer,
    screen: &vt100::Screen,
    widget: &WidgetState,
    bounds: Rectangle,
    show_cursor: bool,
) {
    let (rows, cols) = screen.size();
    let clip = bounds;
    for row in 0..rows {
        for col in 0..cols {
            let Some(cell) = screen.cell(row, col) else {
                continue;
            };
            if cell.is_wide_continuation() {
                continue;
            }
            let x = bounds.x + f32::from(col) * CELL_WIDTH;
            let y = bounds.y + f32::from(row) * CELL_HEIGHT;
            if x >= bounds.x + bounds.width || y >= bounds.y + bounds.height {
                continue;
            }
            let width = if cell.is_wide() {
                CELL_WIDTH * 2.0
            } else {
                CELL_WIDTH
            };
            let (foreground, mut background) = cell_colors(cell);
            if widget
                .selection
                .as_ref()
                .is_some_and(|selection| cell_selected(selection, row, col))
            {
                background = Color::from_rgb8(38, 79, 120);
            }
            if background != Color::BLACK {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: Rectangle::new(Point::new(x, y), Size::new(width, CELL_HEIGHT)),
                        ..renderer::Quad::default()
                    },
                    background,
                );
            }
            // vt100 0.16.2 exposes bold/dim/italic/underline/inverse, but not
            // hidden or strikethrough cell attributes. Do not invent state the
            // parser cannot report.
            if !cell.contents().is_empty() {
                renderer.fill_text(
                    Text {
                        content: cell.contents().to_owned(),
                        bounds: Size::new(width, CELL_HEIGHT),
                        size: Pixels(FONT_SIZE),
                        line_height: iced::advanced::text::LineHeight::Absolute(Pixels(
                            CELL_HEIGHT,
                        )),
                        font: terminal_font(cell),
                        align_x: iced::advanced::text::Alignment::Default,
                        align_y: iced::alignment::Vertical::Top,
                        shaping: iced::advanced::text::Shaping::Advanced,
                        wrapping: iced::advanced::text::Wrapping::None,
                    },
                    Point::new(x, y),
                    foreground,
                    clip,
                );
                if cell.underline() {
                    renderer.fill_quad(
                        renderer::Quad {
                            bounds: Rectangle::new(
                                Point::new(x, y + CELL_HEIGHT - 2.0),
                                Size::new(width, 1.0),
                            ),
                            ..renderer::Quad::default()
                        },
                        foreground,
                    );
                }
            }
        }
    }
    if show_cursor
        && widget.focused
        && widget.window_focused
        && widget.cursor_on
        && !screen.hide_cursor()
        && screen.scrollback() == 0
    {
        let (row, col) = screen.cursor_position();
        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle::new(
                    Point::new(
                        bounds.x + f32::from(col) * CELL_WIDTH,
                        bounds.y + f32::from(row) * CELL_HEIGHT + CELL_HEIGHT - 2.0,
                    ),
                    Size::new(CELL_WIDTH, 2.0),
                ),
                ..renderer::Quad::default()
            },
            Color::WHITE,
        );
    }
}

fn terminal_font(cell: &vt100::Cell) -> Font {
    Font {
        weight: if cell.bold() {
            iced::font::Weight::Bold
        } else {
            iced::font::Weight::Normal
        },
        style: if cell.italic() {
            iced::font::Style::Italic
        } else {
            iced::font::Style::Normal
        },
        ..MONO
    }
}

fn cell_colors(cell: &vt100::Cell) -> (Color, Color) {
    let foreground = terminal_color(cell.fgcolor(), true, cell.bold(), cell.dim());
    let background = terminal_color(cell.bgcolor(), false, false, false);
    if cell.inverse() {
        (background, foreground)
    } else {
        (foreground, background)
    }
}

fn terminal_color(color: vt100::Color, foreground: bool, bold: bool, dim: bool) -> Color {
    let mut color = match color {
        vt100::Color::Default if foreground => Color::from_rgb8(229, 231, 235),
        vt100::Color::Default => Color::BLACK,
        vt100::Color::Rgb(red, green, blue) => Color::from_rgb8(red, green, blue),
        vt100::Color::Idx(index) => {
            indexed_color(if bold && index < 8 { index + 8 } else { index })
        }
    };
    if dim {
        color = Color {
            a: color.a,
            r: color.r * 0.6,
            g: color.g * 0.6,
            b: color.b * 0.6,
        };
    }
    color
}

fn indexed_color(index: u8) -> Color {
    const ANSI: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (205, 49, 49),
        (13, 188, 121),
        (229, 229, 16),
        (36, 114, 200),
        (188, 63, 188),
        (17, 168, 205),
        (229, 229, 229),
        (102, 102, 102),
        (241, 76, 76),
        (35, 209, 139),
        (245, 245, 67),
        (59, 142, 234),
        (214, 112, 214),
        (41, 184, 219),
        (255, 255, 255),
    ];
    if let Some(&(red, green, blue)) = ANSI.get(usize::from(index)) {
        return Color::from_rgb8(red, green, blue);
    }
    if index < 232 {
        let index = index - 16;
        let component = |value: u8| if value == 0 { 0 } else { 55 + value * 40 };
        return Color::from_rgb8(
            component(index / 36),
            component((index / 6) % 6),
            component(index % 6),
        );
    }
    let gray = 8 + (index - 232) * 10;
    Color::from_rgb8(gray, gray, gray)
}

fn terminal_geometry(size: Size) -> Option<(u16, u16)> {
    let cols = (size.width / CELL_WIDTH).floor() as u16;
    let rows = (size.height / CELL_HEIGHT).floor() as u16;
    (cols > 0 && rows > 0).then_some((cols.min(MAX_COLS), rows.min(MAX_ROWS)))
}

fn paste_bytes(bracketed: bool, value: &str) -> Option<Vec<u8>> {
    if value.len() > MAX_PASTE_BYTES {
        return None;
    }
    if !bracketed {
        return Some(value.as_bytes().to_vec());
    }
    let value = value
        .chars()
        .filter(|character| !character.is_control() || matches!(character, '\n' | '\r' | '\t'))
        .collect::<String>();
    let mut bytes = Vec::with_capacity(value.len() + 12);
    bytes.extend_from_slice(b"\x1b[200~");
    bytes.extend_from_slice(value.as_bytes());
    bytes.extend_from_slice(b"\x1b[201~");
    Some(bytes)
}

fn is_paste_shortcut(key: &Key, modifiers: Modifiers) -> bool {
    let Key::Character(value) = key.as_ref() else {
        return matches!(key, Key::Named(key::Named::Paste));
    };
    value.eq_ignore_ascii_case("v")
        && if cfg!(target_os = "macos") {
            modifiers.logo() && !modifiers.control() && !modifiers.alt()
        } else {
            modifiers.control() && modifiers.shift() && !modifiers.alt()
        }
}

fn is_copy_shortcut(key: &Key, modifiers: Modifiers) -> bool {
    let Key::Character(value) = key.as_ref() else {
        return matches!(key, Key::Named(key::Named::Copy));
    };
    value.eq_ignore_ascii_case("c")
        && if cfg!(target_os = "macos") {
            modifiers.logo() && !modifiers.control() && !modifiers.alt()
        } else {
            modifiers.control() && modifiers.shift() && !modifiers.alt()
        }
}

fn mouse_reporting(mode: vt100::MouseProtocolMode, modifiers: Modifiers) -> bool {
    !matches!(mode, vt100::MouseProtocolMode::None) && !modifiers.shift()
}

fn mouse_wheel_reporting(mode: vt100::MouseProtocolMode, modifiers: Modifiers) -> bool {
    mouse_reporting(mode, modifiers) && !matches!(mode, vt100::MouseProtocolMode::Press)
}

fn mouse_protocol_modifiers(mode: vt100::MouseProtocolMode, modifiers: Modifiers) -> Modifiers {
    if matches!(mode, vt100::MouseProtocolMode::Press) {
        Modifiers::empty()
    } else {
        modifiers
    }
}

fn mouse_motion_enabled(mode: vt100::MouseProtocolMode, button_down: bool) -> bool {
    matches!(mode, vt100::MouseProtocolMode::AnyMotion)
        || button_down && matches!(mode, vt100::MouseProtocolMode::ButtonMotion)
}

fn mouse_button_code(button: mouse::Button) -> Option<u8> {
    match button {
        mouse::Button::Left => Some(0),
        mouse::Button::Middle => Some(1),
        mouse::Button::Right => Some(2),
        _ => None,
    }
}

fn scroll_lines(delta: mouse::ScrollDelta, remainder: &mut f32) -> i32 {
    match delta {
        mouse::ScrollDelta::Lines { y, .. } => (y * 3.0).round() as i32,
        mouse::ScrollDelta::Pixels { y, .. } => {
            let pixels = (*remainder + y).clamp(-CELL_HEIGHT * 64.0, CELL_HEIGHT * 64.0);
            let lines = (pixels / CELL_HEIGHT).trunc() as i32;
            *remainder = pixels - lines as f32 * CELL_HEIGHT;
            lines
        }
    }
}

fn mouse_bytes(
    encoding: vt100::MouseProtocolEncoding,
    button: u8,
    (row, col): (u16, u16),
    modifiers: Modifiers,
    released: bool,
    motion: bool,
) -> Option<Vec<u8>> {
    let modifier = 4 * u8::from(modifiers.shift())
        + 8 * u8::from(modifiers.alt())
        + 16 * u8::from(modifiers.control());
    let mut code = button | modifier;
    if motion {
        code |= 32;
    }
    let col = u32::from(col) + 1;
    let row = u32::from(row) + 1;
    match encoding {
        vt100::MouseProtocolEncoding::Sgr => Some(
            format!(
                "\x1b[<{code};{col};{row}{}",
                if released { 'm' } else { 'M' }
            )
            .into_bytes(),
        ),
        vt100::MouseProtocolEncoding::Default => {
            if released {
                code = 3 | modifier;
            }
            let code = code.checked_add(32)?;
            let col = u8::try_from(col.checked_add(32)?).ok()?;
            let row = u8::try_from(row.checked_add(32)?).ok()?;
            Some(vec![0x1b, b'[', b'M', code, col, row])
        }
        vt100::MouseProtocolEncoding::Utf8 => {
            if released {
                code = 3 | modifier;
            }
            let mut bytes = b"\x1b[M".to_vec();
            for value in [u32::from(code) + 32, col + 32, row + 32] {
                let value = char::from_u32(value)?;
                let mut encoded = [0; 4];
                bytes.extend_from_slice(value.encode_utf8(&mut encoded).as_bytes());
            }
            Some(bytes)
        }
    }
}

fn key_bytes(
    key: &Key,
    text: Option<&str>,
    modifiers: Modifiers,
    application_cursor: bool,
) -> Option<Vec<u8>> {
    if modifiers.logo() {
        return None;
    }
    if let Some(named) = named_key_bytes(key, modifiers, application_cursor) {
        return Some(named);
    }
    if modifiers.control()
        && let Key::Character(value) = key.as_ref()
        && let Some(control) = control_byte(value)
    {
        return Some(alt_prefix(modifiers.alt(), vec![control]));
    }
    let text = text.filter(|text| !text.is_empty())?;
    Some(alt_prefix(modifiers.alt(), text.as_bytes().to_vec()))
}

fn named_key_bytes(key: &Key, modifiers: Modifiers, application_cursor: bool) -> Option<Vec<u8>> {
    use key::Named;
    let Key::Named(named) = key else { return None };
    let modifier = key_modifier(modifiers);
    let cursor = |final_byte: char, application: bool| {
        if modifier > 1 {
            format!("\x1b[1;{modifier}{final_byte}").into_bytes()
        } else if application {
            format!("\x1bO{final_byte}").into_bytes()
        } else {
            format!("\x1b[{final_byte}").into_bytes()
        }
    };
    let tilde = |code: u8| {
        if modifier > 1 {
            format!("\x1b[{code};{modifier}~").into_bytes()
        } else {
            format!("\x1b[{code}~").into_bytes()
        }
    };
    let function = |final_byte: char| {
        if modifier > 1 {
            format!("\x1b[1;{modifier}{final_byte}").into_bytes()
        } else {
            format!("\x1bO{final_byte}").into_bytes()
        }
    };
    Some(match named {
        Named::Enter => alt_prefix(modifiers.alt(), b"\r".to_vec()),
        Named::Backspace => alt_prefix(modifiers.alt(), b"\x7f".to_vec()),
        Named::Tab if modifiers.shift() => b"\x1b[Z".to_vec(),
        Named::Tab => alt_prefix(modifiers.alt(), b"\t".to_vec()),
        Named::Escape => b"\x1b".to_vec(),
        Named::ArrowUp => cursor('A', application_cursor),
        Named::ArrowDown => cursor('B', application_cursor),
        Named::ArrowRight => cursor('C', application_cursor),
        Named::ArrowLeft => cursor('D', application_cursor),
        Named::Home => cursor('H', false),
        Named::End => cursor('F', false),
        Named::Insert => tilde(2),
        Named::Delete => tilde(3),
        Named::PageUp => tilde(5),
        Named::PageDown => tilde(6),
        Named::F1 => function('P'),
        Named::F2 => function('Q'),
        Named::F3 => function('R'),
        Named::F4 => function('S'),
        Named::F5 => tilde(15),
        Named::F6 => tilde(17),
        Named::F7 => tilde(18),
        Named::F8 => tilde(19),
        Named::F9 => tilde(20),
        Named::F10 => tilde(21),
        Named::F11 => tilde(23),
        Named::F12 => tilde(24),
        _ => return None,
    })
}

fn key_modifier(modifiers: Modifiers) -> u8 {
    1 + u8::from(modifiers.shift())
        + 2 * u8::from(modifiers.alt())
        + 4 * u8::from(modifiers.control())
}

fn cell_at(position: Point, bounds: Rectangle, screen: &vt100::Screen) -> (u16, u16) {
    let (rows, cols) = screen.size();
    let mut col = ((position.x - bounds.x).max(0.0) / CELL_WIDTH).floor() as u16;
    let row = ((position.y - bounds.y).max(0.0) / CELL_HEIGHT).floor() as u16;
    let row = row.min(rows.saturating_sub(1));
    col = col.min(cols.saturating_sub(1));
    if screen
        .cell(row, col)
        .is_some_and(vt100::Cell::is_wide_continuation)
    {
        col = col.saturating_sub(1);
    }
    (row, col)
}

fn selected_text(screen: &vt100::Screen, selection: Option<&Selection>) -> Option<String> {
    let selection = selection?;
    let (start, end) = if selection.anchor <= selection.head {
        (selection.anchor, selection.head)
    } else {
        (selection.head, selection.anchor)
    };
    if start == end {
        return None;
    }
    let (_, cols) = screen.size();
    let end_col = end.1.saturating_add(1).min(cols);
    let text = screen.contents_between(start.0, start.1, end.0, end_col);
    (!text.is_empty()).then_some(text)
}

fn cell_selected(selection: &Selection, row: u16, col: u16) -> bool {
    let cell = (row, col);
    let (start, end) = if selection.anchor <= selection.head {
        (selection.anchor, selection.head)
    } else {
        (selection.head, selection.anchor)
    };
    cell >= start && cell <= end && start != end
}

fn control_byte(value: &str) -> Option<u8> {
    let byte = value.as_bytes().first()?.to_ascii_lowercase();
    match byte {
        b'@' | b' ' => Some(0),
        b'a'..=b'z' => Some(byte - b'a' + 1),
        b'[' => Some(27),
        b'\\' => Some(28),
        b']' => Some(29),
        b'^' => Some(30),
        b'_' => Some(31),
        b'?' => Some(127),
        _ => None,
    }
}

fn alt_prefix(alt: bool, mut bytes: Vec<u8>) -> Vec<u8> {
    if alt {
        bytes.insert(0, 0x1b);
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ansi_and_korean_are_kept_as_terminal_cells() {
        let mut state = State {
            generation: 7,
            ..State::default()
        };
        update(
            &mut state,
            Message::Output {
                generation: 7,
                bytes: "\x1b[31m오리\x1b[0m ok".as_bytes().to_vec(),
            },
        );
        let screen = state.parser.screen();
        assert_eq!(screen.cell(0, 0).unwrap().contents(), "오");
        assert!(screen.cell(0, 0).unwrap().is_wide());
        assert_eq!(screen.cell(0, 0).unwrap().fgcolor(), vt100::Color::Idx(1));
        assert_eq!(screen.rows(0, INITIAL_COLS).next().unwrap(), "오리 ok");
    }

    #[test]
    fn key_encoding_covers_control_alt_and_application_cursor() {
        assert_eq!(
            key_bytes(
                &Key::Character("c".into()),
                Some("c"),
                Modifiers::CTRL,
                false
            ),
            Some(vec![3])
        );
        assert_eq!(
            key_bytes(
                &Key::Character("x".into()),
                Some("x"),
                Modifiers::ALT,
                false
            ),
            Some(b"\x1bx".to_vec())
        );
        assert_eq!(
            key_bytes(
                &Key::Named(key::Named::ArrowUp),
                None,
                Modifiers::empty(),
                true
            ),
            Some(b"\x1bOA".to_vec())
        );
        assert_eq!(
            key_bytes(
                &Key::Named(key::Named::ArrowLeft),
                None,
                Modifiers::CTRL | Modifiers::SHIFT,
                true,
            ),
            Some(b"\x1b[1;6D".to_vec())
        );
        assert_eq!(
            key_bytes(&Key::Named(key::Named::F5), None, Modifiers::ALT, false,),
            Some(b"\x1b[15;3~".to_vec())
        );
    }

    #[test]
    fn bracketed_paste_and_geometry_are_bounded() {
        assert_eq!(paste_bytes(false, "오리").unwrap(), "오리".as_bytes());
        assert_eq!(
            paste_bytes(true, "duck").unwrap(),
            b"\x1b[200~duck\x1b[201~"
        );
        assert_eq!(
            paste_bytes(true, "safe\x1b[201~\nrm -rf ~").unwrap(),
            b"\x1b[200~safe[201~\nrm -rf ~\x1b[201~"
        );
        assert!(paste_bytes(false, &"x".repeat(MAX_PASTE_BYTES + 1)).is_none());
        assert_eq!(terminal_geometry(Size::new(800.0, 450.0)), Some((100, 25)));
        assert_eq!(terminal_geometry(Size::new(0.0, 450.0)), None);
        assert_eq!(
            terminal_geometry(Size::new(99_999.0, 99_999.0)),
            Some((500, 300))
        );
    }

    #[test]
    fn input_snaps_scrollback_to_live_output() {
        let mut state = State {
            status: Status::Live,
            ..State::default()
        };
        for line in 0..40 {
            state.parser.process(format!("line {line}\r\n").as_bytes());
        }
        state.parser.screen_mut().set_scrollback(8);
        assert!(state.parser.screen().scrollback() > 0);

        let effect = update(&mut state, Message::Input(b"x".to_vec()));

        assert!(matches!(effect, Some(Effect::Input { .. })));
        assert_eq!(state.parser.screen().scrollback(), 0);
    }

    #[test]
    fn clipboard_access_leaves_the_view_as_typed_effects() {
        let mut state = State {
            status: Status::Live,
            generation: 7,
            ..State::default()
        };

        assert_eq!(
            update(&mut state, Message::Copy("duck".into())),
            Some(Effect::Copy("duck".into()))
        );
        assert_eq!(
            update(&mut state, Message::RequestPaste),
            Some(Effect::ReadClipboard { generation: 7 })
        );

        assert_eq!(
            update(
                &mut state,
                Message::Paste {
                    generation: 6,
                    value: "stale".into(),
                },
            ),
            None
        );

        state.status = Status::Failed;
        assert_eq!(update(&mut state, Message::RequestPaste), None);
    }

    #[test]
    fn mouse_protocols_encode_and_shift_forces_local_selection() {
        let cell = (1, 2);
        assert_eq!(
            mouse_bytes(
                vt100::MouseProtocolEncoding::Sgr,
                0,
                cell,
                Modifiers::empty(),
                false,
                false,
            )
            .unwrap(),
            b"\x1b[<0;3;2M"
        );
        assert_eq!(
            mouse_bytes(
                vt100::MouseProtocolEncoding::Sgr,
                0,
                cell,
                Modifiers::CTRL,
                true,
                false,
            )
            .unwrap(),
            b"\x1b[<16;3;2m"
        );
        assert_eq!(
            mouse_bytes(
                vt100::MouseProtocolEncoding::Default,
                0,
                cell,
                Modifiers::empty(),
                false,
                false,
            )
            .unwrap(),
            vec![0x1b, b'[', b'M', b' ', b'#', b'"']
        );
        let utf8 = mouse_bytes(
            vt100::MouseProtocolEncoding::Utf8,
            0,
            (299, 499),
            Modifiers::empty(),
            false,
            false,
        )
        .unwrap();
        assert!(std::str::from_utf8(&utf8).is_ok());
        assert!(mouse_reporting(
            vt100::MouseProtocolMode::PressRelease,
            Modifiers::empty()
        ));
        assert!(!mouse_reporting(
            vt100::MouseProtocolMode::PressRelease,
            Modifiers::SHIFT
        ));
        assert!(!mouse_wheel_reporting(
            vt100::MouseProtocolMode::Press,
            Modifiers::empty()
        ));
        assert_eq!(
            mouse_protocol_modifiers(vt100::MouseProtocolMode::Press, Modifiers::CTRL),
            Modifiers::empty()
        );
    }

    #[test]
    fn alternate_scroll_tracks_split_private_modes_only() {
        let mut tracker = ModeTracker::default();
        tracker.process(b"\x1b]title \x1b[?1007h\x07");
        assert!(!tracker.alternate_scroll);
        tracker.process(b"\x1b[?10");
        tracker.process(b"07h");
        assert!(tracker.alternate_scroll);
        tracker.process(b"\x1b[?1007:l");
        assert!(tracker.alternate_scroll);
        tracker.process(b"\x1b[?1000;1007l");
        assert!(!tracker.alternate_scroll);

        let mut state = State::default();
        state.modes.process(b"\x1b[?1007h");
        assert!(state.modes.alternate_scroll);
        update(&mut state, Message::Start);
        assert!(!state.modes.alternate_scroll);
        state.modes.process(b"\x1b[?1007h");
        update(&mut state, Message::Stop);
        assert!(!state.modes.alternate_scroll);
    }

    #[test]
    fn pixel_scroll_accumulates_mac_trackpad_remainder() {
        let mut remainder = 0.0;
        assert_eq!(
            scroll_lines(
                mouse::ScrollDelta::Pixels { x: 0.0, y: 6.0 },
                &mut remainder
            ),
            0
        );
        assert_eq!(
            scroll_lines(
                mouse::ScrollDelta::Pixels { x: 0.0, y: 6.0 },
                &mut remainder
            ),
            0
        );
        assert_eq!(
            scroll_lines(
                mouse::ScrollDelta::Pixels { x: 0.0, y: 6.0 },
                &mut remainder
            ),
            1
        );
        assert_eq!(remainder, 0.0);
    }

    #[test]
    fn stale_generations_cannot_mutate_the_screen() {
        let mut state = State::default();
        let Some(Effect::Start { generation, .. }) = update(&mut state, Message::Start) else {
            panic!("start effect missing");
        };
        update(
            &mut state,
            Message::Output {
                generation: generation + 1,
                bytes: b"stale".to_vec(),
            },
        );
        assert!(state.parser.screen().contents().is_empty());
        update(
            &mut state,
            Message::Output {
                generation,
                bytes: b"live".to_vec(),
            },
        );
        assert_eq!(state.parser.screen().contents(), "live");
    }

    #[test]
    fn shared_mode_restarts_cleanly_and_refuses_raw_input() {
        let mut state = State {
            status: Status::Live,
            generation: 4,
            ..State::default()
        };
        state.parser.process(b"old session");

        assert_eq!(
            update(&mut state, Message::SetMode(SessionMode::Shared)),
            Some(Effect::Start {
                generation: 5,
                mode: SessionMode::Shared,
            })
        );
        assert!(state.parser.screen().contents().is_empty());
        assert_eq!(update(&mut state, Message::Input(b"unsafe".to_vec())), None);
        assert_eq!(update(&mut state, Message::RequestPaste), None);
    }

    #[test]
    fn shared_commands_are_submitted_and_logged_in_monotonic_order() {
        let mut state = State {
            status: Status::Live,
            generation: 7,
            session_mode: SessionMode::Shared,
            ..State::default()
        };
        update(
            &mut state,
            Message::CommandDraftChanged("  cargo test  ".into()),
        );
        assert_eq!(
            update(&mut state, Message::SubmitCommand),
            Some(Effect::Command {
                generation: 7,
                text: "cargo test".into(),
            })
        );
        assert!(state.command_draft.is_empty());

        for (seq, text) in [(1, "first"), (1, "duplicate"), (2, "second")] {
            update(
                &mut state,
                Message::CommandLogged {
                    generation: 7,
                    seq,
                    origin: "Rae".into(),
                    text: text.into(),
                },
            );
        }
        update(
            &mut state,
            Message::CommandLogged {
                generation: 6,
                seq: 3,
                origin: "stale".into(),
                text: "ignored".into(),
            },
        );
        assert_eq!(
            state
                .commands
                .iter()
                .map(|command| (command.seq, command.text.as_str()))
                .collect::<Vec<_>>(),
            vec![(1, "first"), (2, "second")]
        );
    }

    #[test]
    fn selection_normalizes_drag_direction_and_copies_visible_text() {
        let mut parser = vt100::Parser::new(2, 8, 0);
        parser.process(b"duck\r\nterminal");
        let forward = Selection {
            anchor: (0, 1),
            head: (1, 2),
        };
        let reverse = Selection {
            anchor: forward.head,
            head: forward.anchor,
        };
        assert_eq!(
            selected_text(parser.screen(), Some(&forward)),
            Some("uck\nter".into())
        );
        assert_eq!(
            selected_text(parser.screen(), Some(&reverse)),
            selected_text(parser.screen(), Some(&forward))
        );
        assert!(cell_selected(&forward, 0, 2));
        assert!(!cell_selected(&forward, 1, 3));

        let mut wide = vt100::Parser::new(1, 4, 0);
        wide.process("오".as_bytes());
        assert_eq!(
            cell_at(
                Point::new(CELL_WIDTH * 1.5, CELL_HEIGHT / 2.0),
                Rectangle::new(Point::ORIGIN, Size::new(CELL_WIDTH * 4.0, CELL_HEIGHT)),
                wide.screen(),
            ),
            (0, 0)
        );
    }
}
