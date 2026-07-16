//! Channel, message, thread, huddle, and composer surface.

use iced::widget::{
    Space, button, column, container, rich_text, row, scrollable, span, text, text_input,
};
use iced::{Alignment, Background, Border, Color, Element, Font, Length, font};

use crate::icons::Icon;
use crate::theme::{self, MONO, Palette, RADIUS_MD, RADIUS_SM, SANS};
use crate::view_api::Resource;

use super::chat_composer;
use super::pages::PageMeta;
use super::user::{
    Command, Message, Screen, avatar, bottom_border, center_state, danger_outline, divider,
    error_state, field, filled, icon_tile, nonempty, notice, notice_owned, outline,
    outline_enabled, panel, surface, top_border,
};

const CHAT_RAIL_WIDTH: f32 = 200.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostPolicy {
    Open,
    MembersOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Channel {
    pub id: String,
    pub name: String,
    pub archived: bool,
    pub policy: PostPolicy,
    pub owner: Option<String>,
    pub huddle: Vec<HuddleMember>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HuddleMember {
    pub user: String,
    pub node: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reaction {
    pub emoji: String,
    pub count: usize,
    pub self_reacted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatLink {
    Tag(String),
    Page(String),
    File {
        path: String,
        name: String,
    },
    Channel {
        id: String,
        sequence: Option<u64>,
    },
    Forge {
        repository: String,
        number: Option<u64>,
    },
    External(String),
    User(String),
    Agent {
        module: String,
        id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatSpan {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub link: Option<ChatLink>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    pub sequence: u64,
    pub message_id: String,
    pub revision: u64,
    pub author: String,
    pub body: String,
    pub time: String,
    pub day: Option<String>,
    pub replies: usize,
    pub reactions: Vec<Reaction>,
    pub author_key: Option<String>,
    pub edited: bool,
    pub rich: Vec<ChatSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatThread {
    pub root: ChatMessage,
    pub replies: Vec<ChatMessage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatTag {
    pub label: String,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatHit {
    pub channel: String,
    pub sequence: u64,
    pub author: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatData {
    pub channels: Vec<Channel>,
    pub messages: Vec<ChatMessage>,
    pub thread: Option<ChatThread>,
    pub members: Vec<String>,
    pub tags: Vec<ChatTag>,
    pub hits: Vec<ChatHit>,
    pub history_window: Option<u64>,
    pub self_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelContent {
    pub messages: Vec<ChatMessage>,
    pub members: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatState {
    pub data: Resource<ChatData>,
    pub active_channel: Option<String>,
    pub draft: chat_composer::State,
    pub creating_channel: bool,
    pub channel_draft: String,
    pub channel_policy: PostPolicy,
    pub reply_draft: chat_composer::State,
    pub editing: Option<(u64, u64)>,
    pub edit_draft: String,
    pub pending_delete: Option<u64>,
    pub attachment_busy: bool,
    pub attachment_for_thread: bool,
    pub page_picker_for_thread: Option<bool>,
    pub rename_draft: String,
    pub member_key_draft: String,
    pub tag_filter: Option<String>,
    pub show_channel_details: bool,
    pub error: Option<String>,
}

impl Default for ChatState {
    fn default() -> Self {
        Self {
            data: Resource::Loading,
            active_channel: None,
            draft: chat_composer::State::new(),
            creating_channel: false,
            channel_draft: String::new(),
            channel_policy: PostPolicy::Open,
            reply_draft: chat_composer::State::new(),
            editing: None,
            edit_draft: String::new(),
            pending_delete: None,
            attachment_busy: false,
            attachment_for_thread: false,
            page_picker_for_thread: None,
            rename_draft: String::new(),
            member_key_draft: String::new(),
            tag_filter: None,
            show_channel_details: false,
            error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChatMessageEvent {
    SelectChannel(String),
    ToggleNewChannel,
    ChannelNameChanged(String),
    SetPolicy(PostPolicy),
    CreateChannel,
    Composer {
        thread: bool,
        message: chat_composer::Message,
    },
    ToggleDetails,
    RenameChanged(String),
    Rename,
    SetArchived(bool),
    OpenThread(u64),
    CloseThread,
    BeginEdit(u64, u64, String),
    EditChanged(String),
    CommitEdit,
    CancelEdit,
    RequestDeleteMessage(u64),
    ConfirmDeleteMessage,
    CancelDeleteMessage,
    OpenLink(ChatLink),
    React {
        sequence: u64,
        emoji: String,
        remove: bool,
    },
    MemberKeyChanged(String),
    SetMembership(bool),
    LoadTags,
    FilterTag(String),
    ClearTag,
    FocusMessage(u64),
    JoinHuddle,
    LeaveHuddle,
    PopOutHuddle,
    DismissError,
}

pub(super) fn update(state: &mut ChatState, message: ChatMessageEvent) -> Option<Command> {
    state.error = None;
    match message {
        ChatMessageEvent::SelectChannel(id) => {
            state.active_channel = Some(id.clone());
            state.tag_filter = None;
            state.reply_draft.clear();
            state.editing = None;
            state.edit_draft.clear();
            // A pending delete confirmation is scoped to the channel it was
            // opened in; switching channels must drop it or the confirm would
            // delete that sequence in the newly-selected channel.
            state.pending_delete = None;
            state.rename_draft = match &state.data {
                Resource::Ready(data) => data
                    .channels
                    .iter()
                    .find(|channel| channel.id == id)
                    .map(|channel| channel.name.clone())
                    .unwrap_or_default(),
                _ => String::new(),
            };
            if let Resource::Ready(data) = &mut state.data {
                data.thread = None;
                data.hits.clear();
                data.history_window = None;
            }
            Some(Command::LoadChannel(id))
        }
        ChatMessageEvent::ToggleNewChannel => {
            state.creating_channel = !state.creating_channel;
            None
        }
        ChatMessageEvent::ChannelNameChanged(value) => {
            state.channel_draft = value;
            None
        }
        ChatMessageEvent::SetPolicy(policy) => {
            state.channel_policy = policy;
            None
        }
        ChatMessageEvent::CreateChannel => {
            let name = nonempty(&state.channel_draft)?;
            state.channel_draft.clear();
            state.creating_channel = false;
            Some(Command::CreateChannel {
                name,
                policy: state.channel_policy,
            })
        }
        ChatMessageEvent::Composer { thread, message } => {
            let destination = if matches!(&message, chat_composer::Message::Submit) {
                let channel = state.active_channel.clone()?;
                let root = if thread {
                    match &state.data {
                        Resource::Ready(data) => Some(data.thread.as_ref()?.root.sequence),
                        _ => return None,
                    }
                } else {
                    None
                };
                Some((channel, root))
            } else {
                None
            };
            let output = chat_composer::update(
                if thread {
                    &mut state.reply_draft
                } else {
                    &mut state.draft
                },
                message,
            )?;
            match output {
                chat_composer::Output::Submit(body) => {
                    let (channel, thread) = destination?;
                    Some(Command::SendMessage {
                        channel,
                        body,
                        thread,
                    })
                }
                chat_composer::Output::ChooseAttachment => {
                    if state.attachment_busy {
                        return None;
                    }
                    state.attachment_busy = true;
                    state.attachment_for_thread = thread;
                    Some(Command::ChooseChatAttachment)
                }
                chat_composer::Output::TogglePagePicker => {
                    state.page_picker_for_thread =
                        (state.page_picker_for_thread != Some(thread)).then_some(thread);
                    None
                }
                chat_composer::Output::ReferenceInserted => {
                    state.page_picker_for_thread = None;
                    None
                }
            }
        }
        ChatMessageEvent::ToggleDetails => {
            state.show_channel_details = !state.show_channel_details;
            None
        }
        ChatMessageEvent::RenameChanged(value) => {
            state.rename_draft = value;
            None
        }
        ChatMessageEvent::Rename => Some(Command::RenameChannel {
            channel: state.active_channel.clone()?,
            name: nonempty(&state.rename_draft)?,
        }),
        ChatMessageEvent::SetArchived(archived) => Some(Command::SetChannelArchived {
            channel: state.active_channel.clone()?,
            archived,
        }),
        ChatMessageEvent::OpenThread(root) => Some(Command::LoadThread {
            channel: state.active_channel.clone()?,
            root,
        }),
        ChatMessageEvent::CloseThread => {
            if let Resource::Ready(data) = &mut state.data {
                data.thread = None;
            }
            state.reply_draft.clear();
            None
        }
        ChatMessageEvent::BeginEdit(sequence, revision, body) => {
            state.editing = Some((sequence, revision));
            state.edit_draft = body;
            None
        }
        ChatMessageEvent::EditChanged(value) => {
            state.edit_draft = value;
            None
        }
        ChatMessageEvent::CommitEdit => {
            let channel = state.active_channel.clone()?;
            let (sequence, base_revision) = state.editing.take()?;
            let body = nonempty(&state.edit_draft)?;
            state.edit_draft.clear();
            Some(Command::EditMessage {
                channel,
                sequence,
                base_revision,
                body,
            })
        }
        ChatMessageEvent::CancelEdit => {
            state.editing = None;
            state.edit_draft.clear();
            None
        }
        ChatMessageEvent::RequestDeleteMessage(sequence) => {
            state.pending_delete = Some(sequence);
            None
        }
        ChatMessageEvent::ConfirmDeleteMessage => Some(Command::DeleteMessage {
            channel: state.active_channel.clone()?,
            sequence: state.pending_delete.take()?,
        }),
        ChatMessageEvent::CancelDeleteMessage => {
            state.pending_delete = None;
            None
        }
        ChatMessageEvent::OpenLink(ChatLink::File { path, .. }) => {
            Some(Command::DownloadChatAttachment(path))
        }
        ChatMessageEvent::OpenLink(ChatLink::Tag(tag)) => {
            state.tag_filter = Some(tag.clone());
            Some(Command::FilterTag {
                channel: state.active_channel.clone()?,
                tag,
            })
        }
        ChatMessageEvent::OpenLink(ChatLink::Channel { id, sequence }) => {
            state.active_channel = Some(id.clone());
            state.tag_filter = None;
            state.pending_delete = None;
            if let Resource::Ready(data) = &mut state.data {
                data.thread = None;
                data.hits.clear();
            }
            match sequence {
                Some(sequence) => Some(Command::LoadMessageWindow {
                    channel: id,
                    sequence,
                }),
                None => Some(Command::LoadChannel(id)),
            }
        }
        ChatMessageEvent::OpenLink(_) => None,
        ChatMessageEvent::React {
            sequence,
            emoji,
            remove,
        } => Some(Command::SetReaction {
            channel: state.active_channel.clone()?,
            sequence,
            emoji,
            remove,
        }),
        ChatMessageEvent::MemberKeyChanged(value) => {
            state.member_key_draft = value;
            None
        }
        ChatMessageEvent::SetMembership(member) => Some(Command::SetChannelMembership {
            channel: state.active_channel.clone()?,
            key: nonempty(&state.member_key_draft)?,
            member,
        }),
        ChatMessageEvent::LoadTags => Some(Command::LoadTags(state.active_channel.clone()?)),
        ChatMessageEvent::FilterTag(tag) => {
            state.tag_filter = Some(tag.clone());
            Some(Command::FilterTag {
                channel: state.active_channel.clone()?,
                tag,
            })
        }
        ChatMessageEvent::ClearTag => {
            state.tag_filter = None;
            if let Resource::Ready(data) = &mut state.data {
                data.hits.clear();
            }
            Some(Command::LoadChannel(state.active_channel.clone()?))
        }
        ChatMessageEvent::FocusMessage(sequence) => Some(Command::LoadMessageWindow {
            channel: state.active_channel.clone()?,
            sequence,
        }),
        ChatMessageEvent::JoinHuddle => Some(Command::SetHuddle {
            channel: state.active_channel.clone()?,
            joined: true,
        }),
        ChatMessageEvent::LeaveHuddle => Some(Command::SetHuddle {
            channel: state.active_channel.clone()?,
            joined: false,
        }),
        ChatMessageEvent::PopOutHuddle => None,
        // The leading `state.error = None` already cleared the banner; this
        // arm just gives the dismiss control a message to emit.
        ChatMessageEvent::DismissError => None,
    }
}

fn destructive_confirmation(
    description: String,
    confirm: Message,
    cancel: Message,
    p: Palette,
) -> Element<'static, Message> {
    container(
        column![
            text("Confirm deletion")
                .font(SANS)
                .size(12.5)
                .color(p.danger),
            text(description).font(SANS).size(11).color(p.muted),
            row![
                outline("Cancel", cancel, p),
                danger_outline("Delete", confirm, p),
            ]
            .spacing(8),
        ]
        .spacing(7),
    )
    .width(Length::Fill)
    .padding([11, 13])
    .style(move |_| iced::widget::container::Style {
        background: Some(Background::Color(p.danger_soft)),
        border: Border {
            color: p.danger_border,
            width: 1.0,
            radius: RADIUS_MD.into(),
        },
        ..Default::default()
    })
    .into()
}

pub(crate) fn view<'a>(
    state: &'a ChatState,
    pages: &'a [PageMeta],
    p: Palette,
) -> Element<'a, Message> {
    let data = match &state.data {
        Resource::Loading => {
            return center_state(
                "Loading chat…",
                "Reading channels and messages.",
                Icon::Chat,
                p,
            );
        }
        Resource::Empty => return chat_empty_shell(state, None, pages, p),
        Resource::Error(error) => return error_state("Couldn't load Chat", error, Screen::Chat, p),
        Resource::Ready(data) => data,
    };
    chat_empty_shell(state, Some(data), pages, p)
}

fn chat_empty_shell<'a>(
    state: &'a ChatState,
    data: Option<&'a ChatData>,
    pages: &'a [PageMeta],
    p: Palette,
) -> Element<'a, Message> {
    let channels = data
        .map(|data| data.channels.as_slice())
        .unwrap_or_default();
    let active = channels
        .iter()
        .find(|channel| Some(&channel.id) == state.active_channel.as_ref());
    let mut rail = column![
        row![
            text("CHANNELS").font(SANS).size(11).color(p.muted),
            Space::new().width(Length::Fill),
            outline("+", Message::Chat(ChatMessageEvent::ToggleNewChannel), p)
        ]
        .align_y(Alignment::Center),
    ]
    .spacing(7)
    .padding([13, 8]);
    if state.creating_channel {
        rail = rail.push(
            column![
                field(
                    "channel name",
                    &state.channel_draft,
                    |value| Message::Chat(ChatMessageEvent::ChannelNameChanged(value)),
                    p
                ),
                policy_toggle(state.channel_policy, p),
                filled(
                    "Create channel",
                    Message::Chat(ChatMessageEvent::CreateChannel),
                    !state.channel_draft.trim().is_empty(),
                    p
                ),
            ]
            .spacing(6),
        );
    }
    let visible: Vec<_> = channels
        .iter()
        .filter(|channel| !channel.archived)
        .collect();
    if visible.is_empty() {
        rail = rail.push(
            text("No channels yet — create one.")
                .font(SANS)
                .size(11.5)
                .color(p.muted_2),
        );
    } else {
        for channel in visible {
            rail = rail.push(channel_button(
                channel,
                Some(&channel.id) == state.active_channel.as_ref(),
                p,
            ));
        }
    }
    let archived: Vec<_> = channels.iter().filter(|channel| channel.archived).collect();
    if !archived.is_empty() {
        rail = rail.push(
            text(format!("ARCHIVED · {}", archived.len()))
                .font(SANS)
                .size(11)
                .color(p.muted),
        );
        for channel in archived {
            rail = rail.push(channel_button(
                channel,
                Some(&channel.id) == state.active_channel.as_ref(),
                p,
            ));
        }
    }
    let rail = container(scrollable(rail))
        .width(CHAT_RAIL_WIDTH)
        .height(Length::Fill)
        .style(move |_| panel(p.sidebar, p.border_soft));

    let lane: Element<'a, Message> = if let Some(channel) = active {
        let messages = data
            .map(|data| data.messages.as_slice())
            .unwrap_or_default();
        let roots = messages.len();
        let self_in_huddle = data
            .and_then(|data| data.self_key.as_deref())
            .is_some_and(|key| {
                channel
                    .huddle
                    .iter()
                    .any(|member| member.user.eq_ignore_ascii_case(key))
            });
        let mut header_row = row![
            text("#").font(SANS).size(15).color(p.muted),
            text(&channel.name).font(SANS).size(15).color(p.ink),
            text(format!(
                "· {roots} {}",
                if roots == 1 { "message" } else { "messages" }
            ))
            .font(SANS)
            .size(12)
            .color(p.muted_2),
            Space::new().width(Length::Fill),
            outline("# Tags", Message::Chat(ChatMessageEvent::LoadTags), p),
            outline(
                if self_in_huddle {
                    format!("Leave huddle · {}", channel.huddle.len())
                } else if channel.huddle.is_empty() {
                    "Start huddle".into()
                } else {
                    format!("Join huddle · {}", channel.huddle.len())
                },
                Message::Chat(if self_in_huddle {
                    ChatMessageEvent::LeaveHuddle
                } else {
                    ChatMessageEvent::JoinHuddle
                }),
                p,
            ),
            outline("…", Message::Chat(ChatMessageEvent::ToggleDetails), p),
        ]
        .spacing(9)
        .align_y(Alignment::Center);
        if self_in_huddle {
            header_row = header_row.push(outline(
                "Pop out",
                Message::Chat(ChatMessageEvent::PopOutHuddle),
                p,
            ));
        }
        let header = container(header_row)
            .height(50)
            .padding([0, 18])
            .align_y(Alignment::Center)
            .style(move |_| bottom_border(p.paper, p.border_soft));
        let details: Element<'a, Message> = if state.show_channel_details {
            let member_summary = data
                .map(|data| {
                    format!(
                        "{} admitted member{}",
                        data.members.len(),
                        if data.members.len() == 1 { "" } else { "s" }
                    )
                })
                .unwrap_or_default();
            container(
                column![
                    row![
                        field(
                            "Channel name",
                            &state.rename_draft,
                            |value| Message::Chat(ChatMessageEvent::RenameChanged(value)),
                            p
                        ),
                        outline("Rename", Message::Chat(ChatMessageEvent::Rename), p),
                        outline(
                            if channel.archived {
                                "Unarchive"
                            } else {
                                "Archive"
                            },
                            Message::Chat(ChatMessageEvent::SetArchived(!channel.archived)),
                            p,
                        )
                    ]
                    .spacing(8),
                    row![
                        field(
                            "32-byte member key (hex)",
                            &state.member_key_draft,
                            |value| Message::Chat(ChatMessageEvent::MemberKeyChanged(value)),
                            p
                        ),
                        outline(
                            "Add",
                            Message::Chat(ChatMessageEvent::SetMembership(true)),
                            p
                        ),
                        outline(
                            "Remove",
                            Message::Chat(ChatMessageEvent::SetMembership(false)),
                            p
                        ),
                    ]
                    .spacing(8),
                    text(member_summary).font(MONO).size(10.5).color(p.muted_2),
                ]
                .spacing(8),
            )
            .padding([10, 18])
            .style(move |_| bottom_border(p.sunken, p.border_soft))
            .into()
        } else {
            Space::new().height(0).into()
        };
        let mut stream = column![channel_intro(&channel.name, messages.is_empty(), p)]
            .spacing(1)
            .padding([14, 18]);
        if let Some(tag) = &state.tag_filter {
            stream = stream.push(
                row![
                    text(format!(
                        "#{tag} · {} messages",
                        data.map(|data| data.hits.len()).unwrap_or_default()
                    ))
                    .font(SANS)
                    .size(12.5)
                    .color(theme::ACCENTS[0]),
                    Space::new().width(Length::Fill),
                    outline("Clear", Message::Chat(ChatMessageEvent::ClearTag), p),
                ]
                .align_y(Alignment::Center),
            );
            if let Some(data) = data {
                for hit in &data.hits {
                    stream = stream.push(
                        button(
                            column![
                                text(format!("{} · #{}", hit.author, hit.sequence))
                                    .font(MONO)
                                    .size(10.5)
                                    .color(p.muted_2),
                                text(hit.text.clone())
                                    .font(SANS)
                                    .size(13.5)
                                    .color(p.ink_soft),
                            ]
                            .spacing(3),
                        )
                        .width(Length::Fill)
                        .padding([8, 10])
                        .style(move |_, status| iced::widget::button::Style {
                            background: matches!(status, iced::widget::button::Status::Hovered)
                                .then_some(Background::Color(p.hover)),
                            text_color: p.ink,
                            border: Border {
                                radius: RADIUS_SM.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        })
                        .on_press(Message::Chat(ChatMessageEvent::FocusMessage(hit.sequence))),
                    );
                }
            }
        } else {
            if let Some(sequence) = data.and_then(|data| data.history_window) {
                stream = stream.push(notice_owned(format!("Older history around message #{sequence}. Select this channel again to jump to latest."), p));
            }
            if let Some(data) = data
                && !data.tags.is_empty()
            {
                let mut tags = row![].spacing(5);
                for tag in &data.tags {
                    tags = tags.push(outline(
                        format!("#{} · {}", tag.label, tag.count),
                        Message::Chat(ChatMessageEvent::FilterTag(tag.label.clone())),
                        p,
                    ));
                }
                stream = stream.push(tags);
            }
            let mut last_day: Option<&str> = None;
            for message in messages {
                let day = message.day.as_deref();
                if let Some(label) = day
                    && day != last_day
                {
                    stream = stream.push(day_divider(label, p));
                }
                last_day = day;
                stream = stream.push(message_row(
                    message,
                    channel.archived,
                    true,
                    data.and_then(|data| data.self_key.as_deref()),
                    state.editing,
                    &state.edit_draft,
                    data.and_then(|data| data.history_window) == Some(message.sequence),
                    p,
                ));
            }
        }
        let bottom: Element<'a, Message> = if channel.archived {
            container(
                row![
                    text("This channel is archived — posting and reactions are disabled.")
                        .font(SANS)
                        .size(12)
                        .color(p.muted),
                    Space::new().width(Length::Fill),
                    outline(
                        "Unarchive",
                        Message::Chat(ChatMessageEvent::SetArchived(false)),
                        p
                    ),
                ]
                .align_y(Alignment::Center),
            )
            .padding([10, 18])
            .style(move |_| top_border(p.sunken, p.border_soft))
            .into()
        } else {
            composer(state, channel, pages, false, p)
        };
        let mut main =
            column![header, details, scrollable(stream).height(Length::Fill)].width(Length::Fill);
        if let Some(sequence) = state.pending_delete {
            main = main.push(destructive_confirmation(
                format!("Delete message #{sequence}? This cannot be undone."),
                Message::Chat(ChatMessageEvent::ConfirmDeleteMessage),
                Message::Chat(ChatMessageEvent::CancelDeleteMessage),
                p,
            ));
        }
        let main = main.push(bottom);
        if let Some(thread) = data.and_then(|data| data.thread.as_ref()) {
            row![
                main,
                thread_panel(
                    state,
                    thread,
                    channel,
                    data.and_then(|data| data.self_key.as_deref()),
                    pages,
                    p,
                )
            ]
            .into()
        } else {
            main.into()
        }
    } else {
        let has_channels = !channels.is_empty();
        let center = column![
            icon_tile(Icon::Chat, 48.0, p),
            text(if has_channels {
                "Pick a channel"
            } else {
                "No channels yet"
            })
            .font(SANS)
            .size(14)
            .color(p.ink),
            text(if has_channels {
                "Choose a channel from the list to start reading and posting."
            } else {
                "Create the first channel to start the conversation."
            })
            .font(SANS)
            .size(12.5)
            .color(p.muted_2),
        ]
        .spacing(10)
        .align_x(Alignment::Center)
        .max_width(260);
        container(center)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .into()
    };
    let board = row![
        rail,
        container(lane)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_| surface(p.paper))
    ];
    match &state.error {
        Some(error) => column![chat_error_banner(error, p), board].into(),
        None => board.into(),
    }
}

/// Dismissible inline banner for a chat write failure. The error text sits in a
/// read-only `text_input` so it stays selectable/copyable (the `selectable_error`
/// idiom from `screens/workspace.rs`).
fn chat_error_banner<'a>(message: &'a str, p: Palette) -> Element<'a, Message> {
    container(
        row![
            text_input("", message)
                .font(MONO)
                .size(11.5)
                .padding(0)
                .style(move |_, _| iced::widget::text_input::Style {
                    background: Background::Color(Color::TRANSPARENT),
                    border: Border::default(),
                    icon: p.danger,
                    placeholder: p.danger,
                    value: p.danger,
                    selection: theme::ACCENTS[0],
                }),
            Space::new().width(Length::Fill),
            outline("Dismiss", Message::Chat(ChatMessageEvent::DismissError), p),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([8, 14])
    .style(move |_| iced::widget::container::Style {
        background: Some(Background::Color(p.danger_soft)),
        border: Border {
            color: p.danger_border,
            width: 1.0,
            radius: RADIUS_SM.into(),
        },
        ..Default::default()
    })
    .into()
}

fn policy_toggle(active: PostPolicy, p: Palette) -> Element<'static, Message> {
    row![
        policy_button("Open", PostPolicy::Open, active, p),
        policy_button("Members", PostPolicy::MembersOnly, active, p)
    ]
    .spacing(3)
    .into()
}
fn policy_button(
    label: &'static str,
    policy: PostPolicy,
    active: PostPolicy,
    p: Palette,
) -> Element<'static, Message> {
    let btn = button(text(label).font(SANS).size(11))
        .width(Length::FillPortion(1))
        .padding([4, 8])
        .style(move |_, _| iced::widget::button::Style {
            background: (policy == active).then_some(Background::Color(p.paper)),
            text_color: if policy == active { p.ink } else { p.muted_2 },
            border: Border {
                radius: 5.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .on_press(Message::Chat(ChatMessageEvent::SetPolicy(policy)));
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(iced_agent_plugin::Role::Button, label, btn);
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
}
fn channel_button(channel: &Channel, active: bool, p: Palette) -> Element<'static, Message> {
    let btn = button(
        row![
            text("#").font(SANS).size(13).color(p.muted_2),
            text(channel.name.clone())
                .font(SANS)
                .size(13.5)
                .color(if active {
                    p.ink
                } else if channel.archived {
                    p.muted_2
                } else {
                    p.muted_3
                })
        ]
        .spacing(7),
    )
    .width(Length::Fill)
    .padding([6, 9])
    .style(move |_, status| iced::widget::button::Style {
        background: (active || matches!(status, iced::widget::button::Status::Hovered))
            .then_some(Background::Color(p.hover)),
        text_color: p.ink,
        border: Border {
            radius: RADIUS_SM.into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .on_press(Message::Chat(ChatMessageEvent::SelectChannel(
        channel.id.clone(),
    )));
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(
        iced_agent_plugin::Role::ListItem,
        channel.name.clone(),
        btn,
    );
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
}
fn channel_intro<'a>(name: &'a str, empty: bool, p: Palette) -> Element<'a, Message> {
    column![
        icon_tile(Icon::Chat, 46.0, p),
        text(format!("#{name}")).font(SANS).size(18).color(p.ink),
        text(format!(
            "This is the very beginning of the #{name} channel.{}",
            if empty {
                " Send the first message to start the conversation."
            } else {
                ""
            }
        ))
        .font(SANS)
        .size(13.5)
        .color(p.muted_2)
    ]
    .spacing(9)
    .padding([6, 2])
    .into()
}
fn day_divider<'a>(day: &'a str, p: Palette) -> Element<'a, Message> {
    row![
        divider(p),
        text(day).font(MONO).size(11).color(p.muted_2),
        divider(p)
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .into()
}
#[allow(clippy::too_many_arguments)]
fn message_row<'a>(
    message: &'a ChatMessage,
    archived: bool,
    thread_action: bool,
    self_key: Option<&'a str>,
    editing: Option<(u64, u64)>,
    edit_draft: &'a str,
    focused: bool,
    p: Palette,
) -> Element<'a, Message> {
    let mut actions = row![].spacing(5).align_y(Alignment::Center);
    if thread_action && message.replies > 0 {
        actions = actions.push(outline(
            format!(
                "{} {}",
                message.replies,
                if message.replies == 1 {
                    "reply"
                } else {
                    "replies"
                }
            ),
            Message::Chat(ChatMessageEvent::OpenThread(message.sequence)),
            p,
        ));
    } else if thread_action && !archived {
        actions = actions.push(outline(
            "Reply",
            Message::Chat(ChatMessageEvent::OpenThread(message.sequence)),
            p,
        ));
    }
    for reaction in &message.reactions {
        if !archived {
            actions = actions.push(outline(
                format!("{} {}", reaction.emoji, reaction.count),
                Message::Chat(ChatMessageEvent::React {
                    sequence: message.sequence,
                    emoji: reaction.emoji.clone(),
                    remove: reaction.self_reacted,
                }),
                p,
            ));
        }
    }
    if !archived {
        actions = actions.push(outline(
            "+ 👍",
            Message::Chat(ChatMessageEvent::React {
                sequence: message.sequence,
                emoji: "👍".into(),
                remove: false,
            }),
            p,
        ));
    }
    let is_self = self_key.is_some_and(|self_key| {
        message
            .author_key
            .as_deref()
            .is_some_and(|author| author.eq_ignore_ascii_case(self_key))
    });
    if is_self && !archived {
        actions = actions.push(outline(
            "Edit",
            Message::Chat(ChatMessageEvent::BeginEdit(
                message.sequence,
                message.revision,
                message.body.clone(),
            )),
            p,
        ));
        actions = actions.push(danger_outline(
            "Delete",
            Message::Chat(ChatMessageEvent::RequestDeleteMessage(message.sequence)),
            p,
        ));
    }
    let body: Element<'a, Message> =
        if editing.is_some_and(|(sequence, _)| sequence == message.sequence) {
            column![
                field(
                    "Edit message",
                    edit_draft,
                    |value| Message::Chat(ChatMessageEvent::EditChanged(value)),
                    p,
                )
                .on_submit(Message::Chat(ChatMessageEvent::CommitEdit)),
                row![
                    outline("Cancel", Message::Chat(ChatMessageEvent::CancelEdit), p),
                    outline_enabled(
                        "Save",
                        Message::Chat(ChatMessageEvent::CommitEdit),
                        !edit_draft.trim().is_empty(),
                        p,
                    )
                ]
                .spacing(5)
            ]
            .spacing(5)
            .into()
        } else {
            chat_message_body(message, p)
        };
    container(
        row![
            avatar(&message.author, 32.0, p),
            column![
                row![
                    text(message.author.clone())
                        .font(SANS)
                        .size(13)
                        .color(p.ink),
                    text(message.time.clone())
                        .font(MONO)
                        .size(10)
                        .color(p.muted_2),
                    text(if message.edited { "edited" } else { "" })
                        .font(MONO)
                        .size(9)
                        .color(p.muted_3)
                ]
                .spacing(7),
                body,
                actions
            ]
            .spacing(3)
            .width(Length::Fill)
        ]
        .spacing(10),
    )
    .padding([6, 8])
    .style(move |_| iced::widget::container::Style {
        background: focused.then_some(Background::Color(p.chip)),
        border: Border {
            color: if focused {
                p.border_strong
            } else {
                Color::TRANSPARENT
            },
            width: if focused { 1.0 } else { 0.0 },
            radius: RADIUS_SM.into(),
        },
        ..Default::default()
    })
    .into()
}

fn chat_message_body(message: &ChatMessage, p: Palette) -> Element<'static, Message> {
    if message.rich.is_empty() {
        return text(message.body.clone())
            .font(SANS)
            .size(13.5)
            .color(p.ink_soft)
            .into();
    }
    let mut spans = Vec::with_capacity(message.rich.len());
    for run in &message.rich {
        let mut item = span(run.text.clone()).color(p.ink_soft);
        if run.bold || run.italic {
            item = item.font(Font {
                weight: if run.bold {
                    font::Weight::Semibold
                } else {
                    font::Weight::Normal
                },
                style: if run.italic {
                    font::Style::Italic
                } else {
                    font::Style::Normal
                },
                ..SANS
            });
        }
        if let Some(link) = run.link.clone() {
            item = item.link(link).color(theme::ACCENTS[0]).underline(true);
        }
        spans.push(item);
    }
    rich_text(spans)
        .font(SANS)
        .size(13.5)
        .on_link_click(|link| Message::Chat(ChatMessageEvent::OpenLink(link)))
        .into()
}

fn thread_panel<'a>(
    state: &'a ChatState,
    thread: &'a ChatThread,
    channel: &'a Channel,
    self_key: Option<&'a str>,
    pages: &'a [PageMeta],
    p: Palette,
) -> Element<'a, Message> {
    let mut replies = column![
        message_row(
            &thread.root,
            channel.archived,
            false,
            self_key,
            state.editing,
            &state.edit_draft,
            false,
            p,
        ),
        divider(p)
    ]
    .spacing(5)
    .padding([10, 14]);
    for reply in &thread.replies {
        replies = replies.push(message_row(
            reply,
            channel.archived,
            false,
            self_key,
            state.editing,
            &state.edit_draft,
            false,
            p,
        ));
    }
    let composer: Element<'a, Message> = if channel.archived {
        notice("This channel is archived — thread replies are disabled.", p)
    } else {
        composer(state, channel, pages, true, p)
    };
    container(column![
        container(row![
            text("Thread").font(SANS).size(14).color(p.ink),
            Space::new().width(Length::Fill),
            outline("Close", Message::Chat(ChatMessageEvent::CloseThread), p),
        ])
        .padding([10, 14])
        .style(move |_| bottom_border(p.sidebar, p.border_soft)),
        scrollable(replies).height(Length::Fill),
        composer,
    ])
    .width(328)
    .height(Length::Fill)
    .style(move |_| panel(p.sidebar, p.border_soft))
    .into()
}
fn composer<'a>(
    state: &'a ChatState,
    channel: &'a Channel,
    pages: &'a [PageMeta],
    thread: bool,
    p: Palette,
) -> Element<'a, Message> {
    let draft = if thread {
        &state.reply_draft
    } else {
        &state.draft
    };
    let placeholder = if thread {
        "Reply in thread".to_string()
    } else {
        format!("Message #{}", channel.name)
    };
    chat_composer::view(
        draft,
        placeholder,
        if thread { "Reply" } else { "Send" },
        state.attachment_busy,
        state.page_picker_for_thread == Some(thread),
        pages
            .iter()
            .map(|page| (page.id.as_str(), page.title.as_str())),
        p,
    )
    .map(move |message| Message::Chat(ChatMessageEvent::Composer { thread, message }))
}
