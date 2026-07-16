//! Native Pages rendering.

use iced::keyboard;
use iced::widget::{
    Space, button, column, container, mouse_area, row, scrollable, text, text_editor,
    text_input,
};
use iced::{Alignment, Background, Border, Color, Element, Length};

use super::{
    BlockKind, BlockMove, CommentTarget, EditorState, InlineMark, Message, PageBlock,
    PageCommentThread, PageDocument, PageMeta, PagesData, State, block_descendant_count,
    block_hidden_by_collapse, block_kind_label, block_placeholder, byte_for_utf16,
    editor_selection_utf16, next_block_kind, page_block_input_id, slash_options,
};
use crate::icons::{self, Icon};
use crate::theme::{self, MONO, Palette, RADIUS_SM, SANS};
use crate::view_api::Resource;

const PAGES_RAIL_WIDTH: f32 = 224.0;
const DOC_COLUMN_MAX: f32 = 780.0;

pub fn view(state: &State, p: Palette) -> Element<'_, Message> {
    match &state.data {
        Resource::Loading => shell(
            state,
            None,
            Some(empty_state(
                "Loading pages…",
                "Reading the workspace page tree.",
                p,
            )),
            p,
        ),
        Resource::Empty => shell(state, None, None, p),
        Resource::Error(error) => shell(
            state,
            None,
            Some(empty_state("Couldn't load Pages", error, p)),
            p,
        ),
        Resource::Ready(data) => shell(state, Some(data), None, p),
    }
}

fn shell<'a>(
    state: &'a State,
    data: Option<&'a PagesData>,
    override_body: Option<Element<'a, Message>>,
    p: Palette,
) -> Element<'a, Message> {
    let pages = data.map(|data| data.pages.as_slice()).unwrap_or_default();
    let mut rail = column![
        container(
            row![
                icon_tile(Icon::Pages, 24.0, p),
                text("Pages").font(SANS).size(13).color(p.ink),
                Space::new().width(Length::Fill),
                outline("+", Message::NewPage, p),
                outline("↻", Message::Refresh, p),
            ]
            .spacing(9)
            .align_y(Alignment::Center)
        )
        .height(52)
        .padding(0)
        .align_y(Alignment::Center)
        .style(move |_| bottom_border(p.sidebar, p.border_soft)),
        container(
            row![
                icons::view(Icon::Search, 13.0, p.muted_2),
                sem_input(
                    "Search",
                    &state.query,
                    field("Search", &state.query, Message::QueryChanged, p),
                ),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
        )
        .height(32),
        text("WORKSPACE").font(MONO).size(9).color(p.muted_2),
    ]
    .spacing(8)
    .padding([0, 14]);
    let needle = state.query.trim().to_lowercase();
    let visible = pages
        .iter()
        .filter(|page| {
            (needle.is_empty() || page.title.to_lowercase().contains(&needle))
                && (!needle.is_empty()
                    || !page_hidden_by_collapse(pages, &state.collapsed_pages, &page.id))
        })
        .collect::<Vec<_>>();
    if visible.is_empty() {
        rail = rail.push(notice(
            if needle.is_empty() {
                "No pages yet. Use + to start writing."
            } else {
                "No pages match this search."
            },
            p,
        ));
    } else {
        for page in visible {
            rail = rail.push(page_button(
                page,
                page_depth(pages, &page.id),
                pages
                    .iter()
                    .any(|candidate| candidate.parent.as_deref() == Some(&page.id)),
                state.collapsed_pages.contains(&page.id),
                data.and_then(|data| data.document.as_ref())
                    .is_some_and(|doc| doc.id == page.id),
                p,
            ));
        }
    }
    let rail = container(scrollable(rail))
        .width(PAGES_RAIL_WIDTH)
        .height(Length::Fill)
        // Square full-height side panel butted against the module rail / window edge.
        // bottom_border == panel with radius 0, so reuse it rather than a rounded panel.
        .style(move |_| bottom_border(p.sidebar, p.border_soft));
    let main = override_body
        .unwrap_or_else(|| data.map_or_else(|| no_page(p), |data| pages_main(state, data, p)));
    row![
        rail,
        container(main).width(Length::Fill).height(Length::Fill)
    ]
    .into()
}

fn pages_main<'a>(state: &'a State, data: &'a PagesData, p: Palette) -> Element<'a, Message> {
    let tabs = if data.open_tabs.is_empty() {
        Space::new().height(0).into()
    } else {
        doc_tabs(data, p)
    };
    let document = data.document.as_ref().map_or_else(
        || no_page(p),
        |document| document_view(state, document, &data.pages, p),
    );
    column![tabs, document].into()
}

fn document_view<'a>(
    state: &'a State,
    document: &'a PageDocument,
    pages: &'a [PageMeta],
    p: Palette,
) -> Element<'a, Message> {
    let header = container(
        row![
            text(
                document
                    .ancestry
                    .iter()
                    .map(|page| if page.title.is_empty() {
                        "Untitled"
                    } else {
                        &page.title
                    })
                    .collect::<Vec<_>>()
                    .join(" / ")
            )
            .font(SANS)
            .size(13)
            .color(p.ink),
            if document.presence.is_empty() {
                text("").font(MONO).size(9)
            } else {
                text(format!("{} editing", document.presence.len()))
                    .font(MONO)
                    .size(9)
                    .color(p.green)
            },
            Space::new().width(Length::Fill),
            outline("+ Child page", Message::CreateChildPage, p),
            outline("Delete page", Message::RequestDeletePage, p),
        ]
        .align_y(Alignment::Center),
    )
    .height(52)
    .padding([0, 24])
    .align_y(Alignment::Center)
    .style(move |_| bottom_border(p.paper, p.border_soft));
    let target_label = match &state.comment_target {
        Some(CommentTarget::Reply { .. }) => "Reply to this thread",
        Some(CommentTarget::New {
            anchor: Some(_), ..
        }) => "Comment on selected text",
        Some(CommentTarget::New { .. }) => "Comment on selected block",
        None => "Comment on this page",
    };
    let mut blocks = column![
        sem_input(
            "Page title",
            &document.title,
            plain_input("Untitled", &document.title, Message::TitleChanged, p)
                .on_submit(Message::CommitTitle),
        ),
        row![
            icon_tile(Icon::Chat, 22.0, p),
            compact_editor(
                &state.comment_draft,
                target_label,
                Message::CommentAction,
                p
            ),
            outline_enabled(
                "Comment",
                Message::AddComment,
                !state.comment_draft.text().trim().is_empty(),
                p
            ),
        ]
        .spacing(9)
        .align_y(Alignment::Center),
        divider(p),
    ]
    .spacing(14)
    .padding([44, 80]);
    for thread in &document.comment_threads {
        blocks = blocks.push(comment_thread(state, document, thread, p));
    }
    for (index, block) in document.blocks.iter().enumerate() {
        if !block_hidden_by_collapse(document, &state.collapsed_blocks, block) {
            blocks = blocks.push(
                mouse_area(block_row(state, document, index, block, p))
                    .on_enter(Message::HoverBlock(index))
                    .on_release(Message::DropDraggedBlock),
            );
        }
    }
    if state.pending_page_delete {
        blocks = blocks.push(destructive_confirmation(
            format!(
                "Delete page “{}” and all of its blocks? This cannot be undone.",
                if document.title.trim().is_empty() {
                    "Untitled"
                } else {
                    &document.title
                }
            ),
            Message::ConfirmDeletePage,
            Message::CancelDeletePage,
            p,
        ));
    }
    if let Some(id) = &state.pending_block_delete
        && document.blocks.iter().any(|block| &block.id == id)
    {
        let descendants = block_descendant_count(document, id);
        blocks = blocks.push(destructive_confirmation(
            if descendants == 0 {
                "Delete this block? This cannot be undone.".into()
            } else {
                format!(
                    "Delete this block and its {descendants} nested {}? This cannot be undone.",
                    if descendants == 1 { "block" } else { "blocks" }
                )
            },
            Message::ConfirmRemoveBlock,
            Message::CancelRemoveBlock,
            p,
        ));
    }
    let mut add = row![].spacing(5);
    for (label, kind) in [
        ("Text", BlockKind::Paragraph),
        ("Heading", BlockKind::Heading1),
        ("To-do", BlockKind::Todo),
        ("Toggle", BlockKind::Toggle),
        ("Quote", BlockKind::Quote),
        ("Code", BlockKind::Code),
        ("Callout", BlockKind::Callout),
        ("Divider", BlockKind::Divider),
    ] {
        add = add.push(outline(label, Message::AddBlock(kind), p));
    }
    blocks = blocks.push(
        column![
            text("ADD BLOCK").font(MONO).size(9.5).color(p.muted_2),
            add.wrap(),
        ]
        .spacing(7)
        .padding([14, 0]),
    );
    let mut parent_picker = row![
        text("MOVE PAGE").font(MONO).size(9.5).color(p.muted_2),
        outline("Top level", Message::SetPageParent(None), p),
    ]
    .spacing(5);
    for parent in pages
        .iter()
        .filter(|parent| parent.id != document.id)
        .take(24)
    {
        parent_picker = parent_picker.push(outline(
            if parent.title.trim().is_empty() {
                "Untitled".into()
            } else {
                parent.title.clone()
            },
            Message::SetPageParent(Some(parent.id.clone())),
            p,
        ));
    }
    blocks = blocks.push(parent_picker.wrap());
    if state.paste_dropped > 0 {
        blocks = blocks.push(notice_owned(
            format!(
                "{} pasted lines were dropped at the 60-block safety limit.",
                state.paste_dropped
            ),
            p,
        ));
    }
    if let Some(error) = &state.error {
        blocks = blocks.push(error_banner(error, p));
    }
    column![
        header,
        container(scrollable(
            container(blocks)
                .max_width(DOC_COLUMN_MAX)
                .width(Length::Fill)
        ))
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center),
    ]
    .into()
}

fn block_row<'a>(
    state: &'a State,
    document: &'a PageDocument,
    index: usize,
    block: &'a PageBlock,
    p: Palette,
) -> Element<'a, Message> {
    let marker = match block.kind {
        BlockKind::Bulleted => "•",
        BlockKind::Numbered => "1.",
        BlockKind::Todo if block.checked => "☑",
        BlockKind::Todo => "☐",
        BlockKind::Quote => "│",
        _ => "",
    };
    let size = match block.kind {
        BlockKind::Heading1 => 26.0,
        BlockKind::Heading2 => 22.0,
        BlockKind::Heading3 => 18.0,
        BlockKind::Code => 13.5,
        _ => 15.0,
    };
    let peer_count = document
        .presence
        .iter()
        .filter(|presence| presence.block.as_deref() == Some(&block.id))
        .count();
    let input: Element<'a, Message> = if block.kind == BlockKind::Divider {
        container(Space::new().height(1))
            .width(Length::Fill)
            .style(move |_| panel(p.border_strong, Color::TRANSPARENT))
            .into()
    } else if let Some(editor) = state
        .editors
        .iter()
        .find(|(id, _)| id == &block.id)
        .map(|(_, editor)| editor)
    {
        text_editor(&editor.content)
            .id(iced::widget::Id::from(page_block_input_id(&block.id)))
            .placeholder(block_placeholder(block.kind))
            .on_action(move |action| Message::BlockAction(index, action))
            .key_binding(move |press| {
                if matches!(
                    press.key.as_ref(),
                    keyboard::Key::Named(keyboard::key::Named::Enter)
                ) && press.modifiers.command()
                {
                    Some(text_editor::Binding::Custom(Message::ActivateFocusedBlock))
                } else if matches!(
                    press.key.as_ref(),
                    keyboard::Key::Named(keyboard::key::Named::Enter)
                ) && !press.modifiers.shift()
                {
                    Some(text_editor::Binding::Custom(Message::BlockEnter(index)))
                } else if matches!(
                    press.key.as_ref(),
                    keyboard::Key::Named(keyboard::key::Named::Backspace)
                ) {
                    Some(text_editor::Binding::Custom(Message::BlockBackspace(index)))
                } else {
                    text_editor::Binding::from_key_press(press)
                }
            })
            .font(if block.kind == BlockKind::Code {
                MONO
            } else {
                SANS
            })
            .size(size)
            .min_height(if block.kind == BlockKind::Code {
                62
            } else {
                32
            })
            .max_height(if block.kind == BlockKind::Code {
                220
            } else {
                160
            })
            .padding([5, 7])
            .style(move |_, status| text_editor::Style {
                background: Background::Color(if block.kind == BlockKind::Code {
                    p.sunken
                } else {
                    Color::TRANSPARENT
                }),
                border: Border {
                    color: if matches!(status, text_editor::Status::Focused { .. }) {
                        theme::ACCENTS[0]
                    } else {
                        Color::TRANSPARENT
                    },
                    width: 1.0,
                    radius: RADIUS_SM.into(),
                },
                placeholder: p.muted_2,
                value: p.ink,
                selection: theme::ACCENTS[0],
            })
            .into()
    } else {
        text(block.text.clone())
            .font(if block.kind == BlockKind::Code {
                MONO
            } else {
                SANS
            })
            .size(size)
            .color(p.ink)
            .into()
    };
    let mut editor = column![
        row![
            Space::new().width((block.depth * 26) as f32),
            mouse_area(container(text("⠿").font(MONO).size(12).color(p.muted_2)).padding([5, 3]))
                .on_press(Message::BeginBlockDrag(index)),
            if block.kind == BlockKind::Todo {
                outline(marker, Message::ToggleChecked(index), p)
            } else {
                outline(
                    block_kind_label(block.kind),
                    Message::SetBlockKind(index, next_block_kind(block.kind)),
                    p,
                )
            },
            input,
            if peer_count > 0 {
                Element::from(
                    text(format!("{peer_count} here"))
                        .font(MONO)
                        .size(9)
                        .color(p.green),
                )
            } else {
                Element::from(Space::new().width(0))
            },
            if block.kind == BlockKind::Toggle {
                outline(
                    if state.collapsed_blocks.contains(&block.id) {
                        "▸"
                    } else {
                        "▾"
                    },
                    Message::ToggleBlockCollapsed(index),
                    p,
                )
            } else {
                outline_enabled("·", Message::Refresh, false, p)
            },
            outline("×", Message::RequestRemoveBlock(index), p),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    ]
    .spacing(4);
    let mut tools = row![
        Space::new().width((block.depth * 26 + 42) as f32),
        outline("↑", Message::MoveBlock(index, BlockMove::Up), p),
        outline("↓", Message::MoveBlock(index, BlockMove::Down), p),
        outline("←", Message::MoveBlock(index, BlockMove::Outdent), p),
        outline("→", Message::MoveBlock(index, BlockMove::Indent), p),
        outline("Paste", Message::PasteFromClipboard(index), p),
        outline("Comment", Message::CommentOnBlock(index), p),
    ]
    .spacing(4);
    for mark in [
        InlineMark::Bold,
        InlineMark::Italic,
        InlineMark::Underline,
        InlineMark::Strikethrough,
        InlineMark::Code,
    ] {
        let selected = editor_selection_utf16(state, &block.id, &block.text)
            .is_some_and(|(start, end)| start < end);
        tools = tools.push(outline_enabled(
            mark.label(),
            Message::ToggleMark(index, mark),
            selected,
            p,
        ));
    }
    editor = editor.push(tools.wrap());
    if state.slash_for == Some(index) {
        let mut slash = row![Space::new().width((block.depth * 26 + 42) as f32)].spacing(4);
        for kind in slash_options(&block.text) {
            slash = slash.push(outline(
                block_kind_label(kind),
                Message::ApplySlash(index, kind),
                p,
            ));
        }
        editor = editor.push(slash.wrap());
    }
    editor.into()
}

fn comment_thread<'a>(
    state: &'a State,
    document: &'a PageDocument,
    thread: &'a PageCommentThread,
    p: Palette,
) -> Element<'a, Message> {
    let excerpt = thread.anchor.and_then(|anchor| {
        let block = document
            .blocks
            .iter()
            .find(|block| block.id == thread.target)?;
        let start = byte_for_utf16(&block.text, anchor.start);
        let end = byte_for_utf16(&block.text, anchor.end);
        (start < end).then(|| block.text[start..end].to_owned())
    });
    let mut comments = column![
        row![
            text(if thread.resolved {
                "Resolved"
            } else {
                "Comment"
            })
            .font(MONO)
            .size(9.5)
            .color(if thread.resolved { p.green } else { p.amber }),
            text(excerpt.map_or_else(
                || format!("on {}", short(&thread.target)),
                |value| format!("“{}”", short(&value))
            ))
            .font(MONO)
            .size(9.5)
            .color(p.muted_2),
            Space::new().width(Length::Fill),
            outline(
                "Reply",
                Message::ReplyToThread(thread.id.clone(), thread.target.clone()),
                p
            ),
            outline(
                if thread.resolved { "Reopen" } else { "Resolve" },
                Message::ResolveComment(thread.id.clone(), !thread.resolved),
                p
            ),
        ]
        .spacing(7)
        .align_y(Alignment::Center),
    ]
    .spacing(5);
    for comment in &thread.comments {
        let own = comment.author_key.as_ref().is_some()
            && comment.author_key.as_ref() == document.self_key.as_ref();
        let editing = state
            .editing_comment
            .as_ref()
            .filter(|(id, _)| id == &comment.id);
        let body = if comment.deleted {
            "Comment deleted".into()
        } else if comment.edited {
            format!("{}  (edited)", comment.text)
        } else {
            comment.text.clone()
        };
        let mut item = column![
            row![
                avatar(&comment.author, 20.0, p),
                text(body).font(SANS).size(12).color(p.ink_soft),
                Space::new().width(Length::Fill),
                if own && !comment.deleted {
                    Element::from(outline(
                        "Edit",
                        Message::BeginCommentEdit(comment.id.clone(), comment.text.clone()),
                        p,
                    ))
                } else {
                    Element::from(Space::new().width(0))
                },
                if own && !comment.deleted {
                    Element::from(outline(
                        "Delete",
                        Message::DeleteComment(comment.id.clone()),
                        p,
                    ))
                } else {
                    Element::from(Space::new().width(0))
                },
            ]
            .spacing(7)
            .align_y(Alignment::Center),
        ]
        .spacing(5);
        if let Some((_, draft)) = editing {
            item = item.push(
                row![
                    compact_editor(draft, "Edit comment", Message::CommentEditAction, p),
                    outline_enabled(
                        "Save",
                        Message::CommitCommentEdit,
                        !draft.text().trim().is_empty(),
                        p
                    ),
                    outline("Cancel", Message::CancelCommentEdit, p),
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            );
        }
        comments = comments.push(item);
    }
    container(comments)
        .padding(10)
        .style(move |_| panel(p.sunken, p.border_soft))
        .into()
}

fn page_button(
    page: &PageMeta,
    depth: usize,
    has_children: bool,
    collapsed: bool,
    active: bool,
    p: Palette,
) -> Element<'static, Message> {
    let open = button(
        row![
            text("▱").font(SANS).size(12).color(p.muted_2),
            text(if page.title.is_empty() {
                "Untitled".into()
            } else {
                page.title.clone()
            })
            .font(SANS)
            .size(12.5)
            .color(if active { p.ink } else { p.ink_softer }),
        ]
        .spacing(8),
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
    .on_press(Message::OpenPage(page.id.clone()));
    #[cfg(all(feature = "agent", debug_assertions))]
    let open = iced_agent_plugin::sem(
        iced_agent_plugin::Role::ListItem,
        if page.title.is_empty() {
            "Untitled".to_string()
        } else {
            page.title.clone()
        },
        open,
    );
    row![
        Space::new().width((depth.min(8) * 12) as f32),
        if has_children {
            outline(
                if collapsed { "▸" } else { "▾" },
                Message::TogglePageCollapsed(page.id.clone()),
                p,
            )
        } else {
            outline_enabled("·", Message::Refresh, false, p)
        },
        open,
    ]
    .spacing(3)
    .align_y(Alignment::Center)
    .into()
}

pub(super) fn page_depth(pages: &[PageMeta], id: &str) -> usize {
    let mut depth = 0;
    let mut cursor = id;
    while let Some(parent) = pages
        .iter()
        .find(|page| page.id == cursor)
        .and_then(|page| page.parent.as_deref())
    {
        depth += 1;
        cursor = parent;
        if depth >= pages.len() {
            break;
        }
    }
    depth
}

fn page_hidden_by_collapse(pages: &[PageMeta], collapsed: &[String], id: &str) -> bool {
    let mut cursor = pages
        .iter()
        .find(|page| page.id == id)
        .and_then(|page| page.parent.as_deref());
    let mut depth = 0;
    while let Some(parent) = cursor {
        if collapsed.iter().any(|id| id == parent) {
            return true;
        }
        cursor = pages
            .iter()
            .find(|page| page.id == parent)
            .and_then(|page| page.parent.as_deref());
        depth += 1;
        if depth >= pages.len() {
            break;
        }
    }
    false
}

fn doc_tabs(data: &PagesData, p: Palette) -> Element<'static, Message> {
    let active = data.document.as_ref().map(|document| document.id.as_str());
    let mut tabs = row![].spacing(2).padding([0, 10]);
    for id in &data.open_tabs {
        let title = data
            .pages
            .iter()
            .find(|page| &page.id == id)
            .map(|page| page.title.as_str())
            .filter(|title| !title.is_empty())
            .unwrap_or("Untitled");
        let selected = Some(id.as_str()) == active;
        let open = button(text(title.to_owned()).font(SANS).size(11.5))
            .height(32)
            .padding([0, 9])
            .style(move |_, _| iced::widget::button::Style {
                background: None,
                text_color: if selected { p.ink } else { p.ink_softer },
                border: Border::default(),
                ..Default::default()
            })
            .on_press(Message::OpenPage(id.clone()));
        #[cfg(all(feature = "agent", debug_assertions))]
        let open = iced_agent_plugin::sem(iced_agent_plugin::Role::Tab, title.to_owned(), open);
        tabs = tabs.push(
            container(
                row![
                    open,
                    button(text("×").font(SANS).size(10))
                        .height(32)
                        .padding([0, 6])
                        .style(move |_, status| iced::widget::button::Style {
                            background: matches!(status, iced::widget::button::Status::Hovered)
                                .then_some(Background::Color(p.hover)),
                            text_color: p.muted,
                            border: Border::default(),
                            ..Default::default()
                        })
                        .on_press(Message::CloseTab(id.clone())),
                ]
                .align_y(Alignment::Center),
            )
            .height(34)
            .style(move |_| iced::widget::container::Style {
                background: selected.then_some(Background::Color(p.paper)),
                border: Border {
                    color: if selected {
                        p.filled
                    } else {
                        Color::TRANSPARENT
                    },
                    width: if selected { 2.0 } else { 0.0 },
                    radius: RADIUS_SM.into(),
                },
                ..Default::default()
            }),
        );
    }
    container(tabs)
        .height(34)
        .style(move |_| bottom_border(p.sidebar, p.border_soft))
        .into()
}

fn no_page(p: Palette) -> Element<'static, Message> {
    container(
        column![
            icon_tile(Icon::Pages, 42.0, p),
            text("No page open").font(SANS).size(14).color(p.ink),
            text("Pick a page from the rail, or create one to start writing.")
                .font(SANS)
                .size(12)
                .color(p.muted),
        ]
        .spacing(5)
        .align_x(Alignment::Center)
        .max_width(330),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .into()
}

fn empty_state<'a>(title: &'a str, detail: &'a str, p: Palette) -> Element<'a, Message> {
    container(
        column![
            icon_tile(Icon::Pages, 38.0, p),
            text(title).font(SANS).size(14).color(p.ink),
            text(detail).font(SANS).size(12).color(p.muted),
        ]
        .spacing(7)
        .align_x(Alignment::Center)
        .max_width(420),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .into()
}

fn compact_editor<'a>(
    state: &'a EditorState,
    placeholder: &'a str,
    on_action: impl Fn(text_editor::Action) -> Message + 'a,
    p: Palette,
) -> Element<'a, Message> {
    text_editor(&state.content)
        .placeholder(placeholder)
        .on_action(on_action)
        .padding([7, 9])
        .size(12)
        .font(SANS)
        .min_height(36)
        .max_height(120)
        .style(move |_, status| text_editor::Style {
            background: Background::Color(p.sunken),
            border: Border {
                color: if matches!(status, text_editor::Status::Focused { .. }) {
                    theme::ACCENTS[0]
                } else {
                    p.border_strong
                },
                width: 1.0,
                radius: RADIUS_SM.into(),
            },
            placeholder: p.muted_2,
            value: p.ink,
            selection: theme::ACCENTS[0],
        })
        .into()
}

fn field<'a>(
    placeholder: &'a str,
    value: &'a str,
    on_input: impl Fn(String) -> Message + 'a,
    p: Palette,
) -> iced::widget::TextInput<'a, Message> {
    text_input(placeholder, value)
        .on_input(on_input)
        .padding([6, 9])
        .size(12)
        .font(SANS)
        .style(move |_, status| iced::widget::text_input::Style {
            background: Background::Color(p.sunken),
            border: Border {
                color: if matches!(status, iced::widget::text_input::Status::Focused { .. }) {
                    theme::ACCENTS[0]
                } else {
                    p.border_strong
                },
                width: 1.0,
                radius: RADIUS_SM.into(),
            },
            icon: p.muted,
            placeholder: p.muted_2,
            value: p.ink,
            selection: theme::ACCENTS[0],
        })
}

fn plain_input<'a>(
    placeholder: &'a str,
    value: &'a str,
    on_input: impl Fn(String) -> Message + 'a,
    p: Palette,
) -> iced::widget::TextInput<'a, Message> {
    text_input(placeholder, value)
        .on_input(on_input)
        .padding([5, 0])
        .size(28)
        .font(SANS)
        .style(move |_, _| iced::widget::text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            icon: p.muted,
            placeholder: p.muted_2,
            value: p.ink,
            selection: theme::ACCENTS[0],
        })
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

fn outline<'a>(label: impl ToString, message: Message, p: Palette) -> Element<'a, Message> {
    outline_enabled(label, message, true, p)
}

fn outline_enabled<'a>(
    label: impl ToString,
    message: Message,
    enabled: bool,
    p: Palette,
) -> Element<'a, Message> {
    let label = label.to_string();
    let button = button(text(label.clone()).font(SANS).size(11.5))
        .padding([6, 9])
        .style(move |_, status| iced::widget::button::Style {
            background: Some(Background::Color(
                if enabled && matches!(status, iced::widget::button::Status::Hovered) {
                    p.hover
                } else {
                    p.paper
                },
            )),
            text_color: if enabled { p.ink_soft } else { p.muted_2 },
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
    let button = if enabled {
        button.on_press(message)
    } else {
        button
    };
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::Sem::new(iced_agent_plugin::Role::Button, label, button)
        .disabled(!enabled)
        .into();
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    button.into()
}

fn destructive_confirmation(
    copy: String,
    confirm: Message,
    cancel: Message,
    p: Palette,
) -> Element<'static, Message> {
    let delete = button(text("Delete").font(SANS).size(11.5))
        .padding([6, 10])
        .style(move |_, _| iced::widget::button::Style {
            background: Some(Background::Color(p.red)),
            text_color: Color::WHITE,
            border: Border {
                radius: RADIUS_SM.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .on_press(confirm);
    #[cfg(all(feature = "agent", debug_assertions))]
    let delete = iced_agent_plugin::sem(iced_agent_plugin::Role::Button, "Delete", delete);
    container(
        column![
            text(copy).font(SANS).size(12).color(p.ink_soft),
            row![delete, outline("Cancel", cancel, p)].spacing(6),
        ]
        .spacing(8),
    )
    .padding(10)
    .style(move |_| panel(p.sunken, p.red))
    .into()
}

fn icon_tile(icon: Icon, size: f32, p: Palette) -> Element<'static, Message> {
    container(icons::view(icon, size * 0.55, p.ink_soft))
        .width(size)
        .height(size)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(p.hover)),
            border: Border {
                color: p.border_soft,
                width: 1.0,
                radius: RADIUS_SM.into(),
            },
            ..Default::default()
        })
        .into()
}

fn avatar(name: &str, size: f32, p: Palette) -> Element<'static, Message> {
    let initial = name
        .chars()
        .next()
        .unwrap_or('?')
        .to_uppercase()
        .collect::<String>();
    container(text(initial).font(SANS).size(size * 0.43).color(p.ink_soft))
        .width(size)
        .height(size)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(p.hover)),
            border: Border {
                color: p.border_soft,
                width: 1.0,
                radius: (size / 2.0).into(),
            },
            ..Default::default()
        })
        .into()
}

fn divider(p: Palette) -> Element<'static, Message> {
    container(Space::new().height(1))
        .width(Length::Fill)
        .style(move |_| panel(p.border_soft, Color::TRANSPARENT))
        .into()
}

fn notice<'a>(copy: &'a str, p: Palette) -> Element<'a, Message> {
    container(text(copy).font(SANS).size(11.5).color(p.muted))
        .padding(9)
        .style(move |_| panel(p.sunken, p.border_soft))
        .into()
}

fn notice_owned(copy: String, p: Palette) -> Element<'static, Message> {
    container(text(copy).font(SANS).size(11.5).color(p.muted))
        .padding(9)
        .style(move |_| panel(p.sunken, p.border_soft))
        .into()
}

fn error_banner<'a>(copy: &'a str, p: Palette) -> Element<'a, Message> {
    container(text(copy).font(SANS).size(11.5).color(p.red))
        .padding(9)
        .style(move |_| panel(p.sunken, p.red))
        .into()
}

fn panel(color: Color, border: Color) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(color)),
        border: Border {
            color: border,
            width: 1.0,
            radius: RADIUS_SM.into(),
        },
        ..Default::default()
    }
}

fn bottom_border(bg: Color, border: Color) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(bg)),
        border: Border {
            color: border,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

fn short(value: &str) -> String {
    if value.chars().count() <= 18 {
        value.to_owned()
    } else {
        format!(
            "{}…{}",
            value.chars().take(10).collect::<String>(),
            value
                .chars()
                .rev()
                .take(6)
                .collect::<String>()
                .chars()
                .rev()
                .collect::<String>()
        )
    }
}
