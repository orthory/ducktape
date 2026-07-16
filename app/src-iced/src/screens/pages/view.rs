//! Native Pages rendering.

use iced::keyboard;
use iced::widget::{
    Space, button, column, container, mouse_area, row, scrollable, text, text_editor,
    text_input,
};
use iced::{Alignment, Background, Border, Color, Element, Length};

use super::{
    BlockKind, BlockMove, CommentTarget, EditorState, InlineMark, Message, PageBlock,
    PageCommentThread, PageDocument, PageMeta, PagePresence, PagesData, State, all_block_kinds,
    block_descendant_count, block_hidden_by_collapse, block_kind_label, block_placeholder,
    byte_for_utf16, editor_selection_utf16, page_block_input_id, slash_options,
};
use crate::icons::{self, Icon};
use crate::theme::{self, MONO, Palette, RADIUS_MD, RADIUS_SM, SANS, SANS_SEMIBOLD};
use crate::ui;
use crate::view_api::Resource;

const PAGES_RAIL_WIDTH: f32 = 224.0;
const DOC_COLUMN_MAX: f32 = 780.0;
/// Per-depth indent of a block, matching the reducer's tree math.
const INDENT: f32 = 26.0;
/// Reserved left margin that reveals the drag/menu grips on hover, so the
/// editor text keeps one left edge whether the gutter is shown or not.
const GUTTER_WIDTH: f32 = 34.0;
/// Fixed marker column so prose and marked blocks share a text baseline.
const MARKER_WIDTH: f32 = 22.0;

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
    // Full-bleed header strip: background only, with a divider drawn under it —
    // never a four-side `Border` box around shrink-width content.
    let header = container(
        row![
            filled_chip(Icon::Pages, 24.0, p),
            text("Pages")
                .font(SANS_SEMIBOLD)
                .size(theme::BODY)
                .color(p.ink),
            Space::new().width(Length::Fill),
            ghost_icon(Icon::Plus, 15.0, Message::NewPage, "New page", p),
            ghost_icon(Icon::Refresh, 14.0, Message::Refresh, "Refresh pages", p),
        ]
        .spacing(9)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .height(52)
    .padding([0, 14])
    .align_y(Alignment::Center)
    .style(move |_| surface(p.sidebar));

    let mut body = column![
        container(
            row![
                icons::view(Icon::Search, 13.0, p.muted_2),
                sem_input(
                    "Search",
                    &state.query,
                    field("Search pages", &state.query, Message::QueryChanged, p),
                ),
            ]
            .spacing(8)
            .align_y(Alignment::Center)
        )
        .height(34),
        text("WORKSPACE")
            .font(MONO)
            .size(theme::CAPTION)
            .color(p.muted_2),
    ]
    .spacing(10)
    .padding([12, 14]);
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
        body = body.push(notice(
            if needle.is_empty() {
                "No pages yet. Use + to start writing."
            } else {
                "No pages match this search."
            },
            p,
        ));
    } else {
        for page in visible {
            body = body.push(page_button(
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
    let rail = container(column![
        header,
        divider(p),
        scrollable(body).height(Length::Fill),
    ])
    .width(PAGES_RAIL_WIDTH)
    .height(Length::Fill)
    // Square full-height side panel butted against the module rail / window edge.
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
    let document = data.document.as_ref().map_or_else(
        || no_page(p),
        |document| document_view(state, document, &data.pages, p),
    );
    if data.open_tabs.is_empty() {
        document
    } else {
        column![doc_tabs(data, p), divider(p), document].into()
    }
}

fn document_view<'a>(
    state: &'a State,
    document: &'a PageDocument,
    _pages: &'a [PageMeta],
    p: Palette,
) -> Element<'a, Message> {
    let header = document_header(document, p);

    // A comment thread anchored to a real block hangs under that block; anything
    // else (a whole-page note) stays with the page.
    let is_block = |id: &str| document.blocks.iter().any(|block| block.id == id);
    let composer_block: Option<&str> = match &state.comment_target {
        Some(CommentTarget::New { target, .. }) if is_block(target) => Some(target.as_str()),
        Some(CommentTarget::Reply { target, .. }) if is_block(target) => Some(target.as_str()),
        _ => None,
    };
    let composer_label = match &state.comment_target {
        Some(CommentTarget::Reply { .. }) => "Reply to this thread",
        Some(CommentTarget::New {
            anchor: Some(_), ..
        }) => "Comment on selected text",
        Some(CommentTarget::New { .. }) => "Comment on selected block",
        None => "Comment on this page",
    };

    let mut body = column![sem_input(
        "Page title",
        &document.title,
        plain_input("Untitled", &document.title, Message::TitleChanged, p)
            .on_submit(Message::CommitTitle),
    )]
    .spacing(12)
    .padding([40, 80]);

    // Page-level threads + (when nothing block-targeted is active) the composer.
    for thread in &document.comment_threads {
        if !is_block(&thread.target) {
            body = body.push(comment_thread(state, document, thread, p));
        }
    }
    if composer_block.is_none() {
        body = body.push(comment_composer(&state.comment_draft, composer_label, p));
    }
    body = body.push(divider(p));

    for (index, block) in document.blocks.iter().enumerate() {
        if block_hidden_by_collapse(document, &state.collapsed_blocks, block) {
            continue;
        }
        // Drop indicator on the hovered edge while a drag is live (D1).
        if state.dragging_block.is_some() && state.drag_hover == Some(index) {
            body = body.push(drop_indicator(p));
        }
        body = body.push(
            mouse_area(block_row(state, document, index, block, p))
                .on_enter(Message::HoverBlock(index))
                .on_exit(Message::BlockRowExited(index))
                .on_release(Message::DropDraggedBlock),
        );
        // Threads and the composer for this specific block hang beneath it.
        for thread in &document.comment_threads {
            if thread.target == block.id {
                body = body.push(under_block(block.depth, comment_thread(state, document, thread, p)));
            }
        }
        if composer_block == Some(block.id.as_str()) {
            body = body.push(under_block(
                block.depth,
                comment_composer(&state.comment_draft, composer_label, p),
            ));
        }
        if state.pending_block_delete.as_deref() == Some(block.id.as_str()) {
            let descendants = block_descendant_count(document, &block.id);
            body = body.push(under_block(
                block.depth,
                destructive_confirmation(
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
                ),
            ));
        }
    }

    body = body.push(add_block_affordance(p));

    if state.pending_page_delete {
        body = body.push(destructive_confirmation(
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
    if state.paste_dropped > 0 {
        body = body.push(notice_owned(
            format!(
                "{} pasted lines were dropped at the 60-block safety limit.",
                state.paste_dropped
            ),
            p,
        ));
    }
    if let Some(error) = &state.error {
        body = body.push(selectable_error(error, p));
    }
    column![
        header,
        divider(p),
        container(scrollable(
            container(body)
                .max_width(DOC_COLUMN_MAX)
                .width(Length::Fill)
        ))
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center),
    ]
    .into()
}

fn document_header<'a>(document: &'a PageDocument, p: Palette) -> Element<'a, Message> {
    let mut crumbs = row![].spacing(4).align_y(Alignment::Center);
    let last = document.ancestry.len().saturating_sub(1);
    for (index, page) in document.ancestry.iter().enumerate() {
        let title = if page.title.trim().is_empty() {
            "Untitled"
        } else {
            &page.title
        };
        if index == last {
            crumbs = crumbs.push(
                text(title.to_owned())
                    .font(SANS_SEMIBOLD)
                    .size(theme::BODY)
                    .color(p.ink),
            );
        } else {
            crumbs = crumbs.push(crumb_button(title, page.id.clone(), p));
            crumbs = crumbs.push(text("/").font(SANS).size(theme::BODY).color(p.muted_2));
        }
    }
    let presence: Element<'a, Message> = if document.presence.is_empty() {
        Space::new().width(0).into()
    } else {
        presence_bar(&document.presence, p)
    };
    container(
        row![
            crumbs,
            Space::new().width(Length::Fill),
            presence,
            outline("+ Child page", Message::CreateChildPage, p),
            danger_outline("Delete page", Message::RequestDeletePage, p),
        ]
        .spacing(9)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .height(52)
    .padding([0, 24])
    .align_y(Alignment::Center)
    .style(move |_| surface(p.paper))
    .into()
}

fn block_row<'a>(
    state: &'a State,
    document: &'a PageDocument,
    index: usize,
    block: &'a PageBlock,
    p: Palette,
) -> Element<'a, Message> {
    let hovered = state.hovered_block.as_deref() == Some(block.id.as_str());
    let menu_open = state.menu_open_block.as_deref() == Some(block.id.as_str());
    let size = match block.kind {
        BlockKind::Heading1 => 26.0,
        BlockKind::Heading2 => 22.0,
        BlockKind::Heading3 => 18.0,
        BlockKind::Code => 13.5,
        _ => 15.0,
    };

    // Left gutter: drag grip + actions menu, revealed on hover (or while its
    // menu is open); otherwise reserved blank space so text never shifts.
    let gutter: Element<'a, Message> = if hovered || menu_open {
        row![drag_grip(index, p), menu_trigger(index, p)]
            .spacing(1)
            .align_y(Alignment::Center)
            .into()
    } else {
        Space::new().width(GUTTER_WIDTH).into()
    };

    // Marker: only the kinds that carry one in the contract.
    let marker: Element<'a, Message> = match block.kind {
        BlockKind::Bulleted => marker_glyph("•", p),
        BlockKind::Numbered => marker_glyph("1.", p),
        BlockKind::Todo => checkbox_marker(index, block.checked, p),
        BlockKind::Toggle if !block.children.is_empty() => {
            chevron_marker(index, state.collapsed_blocks.contains(&block.id), p)
        }
        _ => Space::new().width(MARKER_WIDTH).into(),
    };

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

    // Center column: a formatting bar rides above the editor only when this
    // block is focused and has a live selection (the floating-toolbar analogue).
    let selected = state.focused_block.as_deref() == Some(block.id.as_str())
        && editor_selection_utf16(state, &block.id, &block.text)
            .is_some_and(|(start, end)| start < end);
    let center: Element<'a, Message> = if selected {
        column![marks_bar(index, p), input]
            .spacing(6)
            .width(Length::Fill)
            .into()
    } else {
        container(input).width(Length::Fill).into()
    };

    let peer_count = document
        .presence
        .iter()
        .filter(|presence| presence.block.as_deref() == Some(&block.id))
        .count();
    let open_threads = document
        .comment_threads
        .iter()
        .filter(|thread| thread.target == block.id && !thread.resolved)
        .count();
    let right: Element<'a, Message> = if open_threads > 0 || hovered {
        comment_button(index, open_threads, peer_count, p)
    } else if peer_count > 0 {
        text(format!("{peer_count} here"))
            .font(MONO)
            .size(theme::CAPTION)
            .color(p.green)
            .into()
    } else {
        Space::new().width(0).into()
    };

    let mut stack = column![
        row![
            Space::new().width((block.depth as f32) * INDENT),
            gutter,
            marker,
            center,
            right,
        ]
        .spacing(6)
        .align_y(Alignment::Start),
    ]
    .spacing(4);
    if menu_open {
        stack = stack.push(
            row![
                Space::new().width((block.depth as f32) * INDENT + GUTTER_WIDTH),
                block_menu(index, block, p),
            ]
            .spacing(0),
        );
    }
    // Slash menu: kind picker shown while the block text is a `/query`.
    if state.slash_for == Some(index) {
        let mut slash = row![
            Space::new().width((block.depth as f32) * INDENT + GUTTER_WIDTH + MARKER_WIDTH)
        ]
        .spacing(4);
        for kind in slash_options(&block.text) {
            slash = slash.push(outline(
                block_kind_label(kind),
                Message::ApplySlash(index, kind),
                p,
            ));
        }
        stack = stack.push(slash.wrap());
    }
    stack.into()
}

fn block_menu<'a>(index: usize, block: &'a PageBlock, p: Palette) -> Element<'a, Message> {
    let mut turn_into = row![].spacing(4);
    for kind in all_block_kinds() {
        if kind == block.kind {
            continue;
        }
        turn_into = turn_into.push(outline(
            block_kind_label(kind),
            Message::SetBlockKind(index, kind),
            p,
        ));
    }
    container(
        column![
            // Reorder is drag-first; these keep single-step moves and block
            // paste reachable without the old always-on toolbar.
            row![
                outline("Move up", Message::MoveBlock(index, BlockMove::Up), p),
                outline("Move down", Message::MoveBlock(index, BlockMove::Down), p),
                outline("Paste below", Message::PasteFromClipboard(index), p),
            ]
            .spacing(4)
            .wrap(),
            danger_outline("Delete block", Message::RequestRemoveBlock(index), p),
            divider(p),
            text("Turn into")
                .font(MONO)
                .size(theme::CAPTION)
                .color(p.muted_2),
            turn_into.wrap(),
        ]
        .spacing(7),
    )
    .padding(10)
    .max_width(320)
    .style(move |_| soft_panel(p.paper, p.border, RADIUS_MD))
    .into()
}

fn marks_bar<'a>(index: usize, p: Palette) -> Element<'a, Message> {
    let mut bar = row![].spacing(2).align_y(Alignment::Center);
    for mark in [
        InlineMark::Bold,
        InlineMark::Italic,
        InlineMark::Underline,
        InlineMark::Strikethrough,
        InlineMark::Code,
    ] {
        bar = bar.push(mark_button(index, mark, p));
    }
    container(bar)
        .padding([3, 4])
        .style(move |_| soft_panel(p.paper, p.border_soft, RADIUS_MD))
        .into()
}

fn mark_button<'a>(index: usize, mark: InlineMark, p: Palette) -> Element<'a, Message> {
    let label = mark.label();
    let btn = button(
        text(label)
            .font(SANS_SEMIBOLD)
            .size(theme::CAPTION)
            .color(p.ink_soft),
    )
    .padding([3, 7])
    .style(move |_, status| iced::widget::button::Style {
        background: matches!(status, iced::widget::button::Status::Hovered)
            .then_some(Background::Color(p.hover)),
        text_color: p.ink_soft,
        border: Border {
            radius: RADIUS_SM.into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .on_press(Message::ToggleMark(index, mark));
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(iced_agent_plugin::Role::Button, label, btn);
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
}

fn comment_button<'a>(
    index: usize,
    open_threads: usize,
    peer_count: usize,
    p: Palette,
) -> Element<'a, Message> {
    let label = if open_threads > 0 {
        format!("{open_threads}")
    } else {
        "Comment".to_string()
    };
    let mut inner = row![icons::view(Icon::Chat, 13.0, p.muted_3)]
        .spacing(5)
        .align_y(Alignment::Center);
    inner = inner.push(
        text(label)
            .font(SANS)
            .size(theme::LABEL)
            .color(p.muted_3),
    );
    if peer_count > 0 {
        inner = inner.push(
            text(format!("· {peer_count} here"))
                .font(MONO)
                .size(theme::CAPTION)
                .color(p.green),
        );
    }
    let btn = button(inner)
        .padding([4, 8])
        .style(move |_, status| iced::widget::button::Style {
            background: matches!(status, iced::widget::button::Status::Hovered)
                .then_some(Background::Color(p.hover)),
            text_color: p.muted_3,
            border: Border {
                radius: RADIUS_SM.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .on_press(Message::CommentOnBlock(index));
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(iced_agent_plugin::Role::Button, "Comment", btn);
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
}

fn drag_grip<'a>(index: usize, p: Palette) -> Element<'a, Message> {
    mouse_area(
        container(text("⠿").font(MONO).size(theme::LABEL).color(p.muted_2)).padding([4, 3]),
    )
    .on_press(Message::BeginBlockDrag(index))
    .into()
}

fn menu_trigger<'a>(index: usize, p: Palette) -> Element<'a, Message> {
    let btn = button(text("⋮").font(SANS_SEMIBOLD).size(theme::BODY).color(p.muted_2))
        .padding([2, 6])
        .style(move |_, status| iced::widget::button::Style {
            background: matches!(status, iced::widget::button::Status::Hovered)
                .then_some(Background::Color(p.hover)),
            text_color: p.muted_2,
            border: Border {
                radius: RADIUS_SM.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .on_press(Message::ToggleBlockMenu(index));
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(iced_agent_plugin::Role::Button, "Block actions", btn);
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
}

fn marker_glyph<'a>(glyph: &'a str, p: Palette) -> Element<'a, Message> {
    container(text(glyph).font(SANS).size(15.0).color(p.muted))
        .width(MARKER_WIDTH)
        .padding([5, 0])
        .align_x(Alignment::Center)
        .into()
}

fn checkbox_marker<'a>(index: usize, checked: bool, p: Palette) -> Element<'a, Message> {
    let btn = button(
        text(if checked { "☑" } else { "☐" })
            .font(SANS)
            .size(15.0)
            .color(if checked { p.filled } else { p.muted }),
    )
    .padding([3, 0])
    .width(MARKER_WIDTH)
    .style(move |_, _| iced::widget::button::Style {
        background: None,
        text_color: if checked { p.filled } else { p.muted },
        border: Border::default(),
        ..Default::default()
    })
    .on_press(Message::ToggleChecked(index));
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(
        iced_agent_plugin::Role::Button,
        if checked { "Uncheck" } else { "Check" },
        btn,
    );
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
}

fn chevron_marker<'a>(index: usize, collapsed: bool, p: Palette) -> Element<'a, Message> {
    let btn = button(
        text(if collapsed { "▸" } else { "▾" })
            .font(SANS)
            .size(13.0)
            .color(p.muted),
    )
    .padding([4, 0])
    .width(MARKER_WIDTH)
    .style(move |_, _| iced::widget::button::Style {
        background: None,
        text_color: p.muted,
        border: Border::default(),
        ..Default::default()
    })
    .on_press(Message::ToggleBlockCollapsed(index));
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(iced_agent_plugin::Role::Button, "Toggle children", btn);
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
}

fn add_block_affordance<'a>(p: Palette) -> Element<'a, Message> {
    let btn = button(
        row![
            icons::view(Icon::Plus, 13.0, p.muted_2),
            text("Add a block, or type ‘/’ for commands")
                .font(SANS)
                .size(theme::BODY)
                .color(p.muted_2),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([9, 6])
    .style(move |_, status| iced::widget::button::Style {
        background: matches!(status, iced::widget::button::Status::Hovered)
            .then_some(Background::Color(p.hover)),
        text_color: p.muted_2,
        border: Border {
            radius: RADIUS_SM.into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .on_press(Message::AddBlock(BlockKind::Paragraph));
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(iced_agent_plugin::Role::Button, "Add a block", btn);
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
}

fn drop_indicator<'a>(p: Palette) -> Element<'a, Message> {
    let _ = p;
    container(Space::new().height(2))
        .width(Length::Fill)
        .style(|_| iced::widget::container::Style {
            background: Some(Background::Color(theme::ACCENTS[0])),
            ..Default::default()
        })
        .into()
}

fn under_block<'a>(depth: usize, content: Element<'a, Message>) -> Element<'a, Message> {
    row![
        Space::new().width((depth as f32) * INDENT + GUTTER_WIDTH),
        content,
    ]
    .spacing(0)
    .into()
}

fn comment_composer<'a>(
    draft: &'a EditorState,
    label: &'a str,
    p: Palette,
) -> Element<'a, Message> {
    row![
        icons::view(Icon::Chat, 16.0, p.muted_3),
        compact_editor(draft, label, Message::CommentAction, p),
        outline_enabled(
            "Comment",
            Message::AddComment,
            !draft.text().trim().is_empty(),
            p
        ),
    ]
    .spacing(9)
    .align_y(Alignment::Center)
    .into()
}

fn presence_bar<'a>(presence: &'a [PagePresence], p: Palette) -> Element<'a, Message> {
    let mut avatars = row![].spacing(2).align_y(Alignment::Center);
    for peer in presence.iter().take(4) {
        avatars = avatars.push(avatar(&peer.peer, 22.0, p));
    }
    row![
        avatars,
        text(format!("{} editing", presence.len()))
            .font(MONO)
            .size(theme::CAPTION)
            .color(p.green),
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
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
            .size(theme::CAPTION)
            .color(if thread.resolved { p.green } else { p.amber }),
            text(excerpt.map_or_else(
                || format!("on {}", short(&thread.target)),
                |value| format!("“{}”", short(&value))
            ))
            .font(MONO)
            .size(theme::CAPTION)
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
                text(body).font(SANS).size(theme::BODY).color(p.ink_soft),
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
                    Element::from(danger_outline(
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
        .style(move |_| soft_panel(p.sunken, p.border_soft, RADIUS_SM))
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
            text("▱").font(SANS).size(theme::LABEL).color(p.muted_2),
            text(if page.title.is_empty() {
                "Untitled".into()
            } else {
                page.title.clone()
            })
            .font(SANS)
            .size(theme::LABEL)
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
    // A childless page reserves the chevron's width with plain space, never a
    // dead disabled dot masquerading as a control (D2).
    let toggle: Element<'static, Message> = if has_children {
        outline(
            if collapsed { "▸" } else { "▾" },
            Message::TogglePageCollapsed(page.id.clone()),
            p,
        )
    } else {
        Space::new().width(18).into()
    };
    row![
        Space::new().width((depth.min(8) * 12) as f32),
        toggle,
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
    let mut tabs = row![].spacing(4).padding([0, 10]).align_y(Alignment::Center);
    for id in &data.open_tabs {
        let title = data
            .pages
            .iter()
            .find(|page| &page.id == id)
            .map(|page| page.title.as_str())
            .filter(|title| !title.is_empty())
            .unwrap_or("Untitled");
        let selected = Some(id.as_str()) == active;
        let open = button(
            text(truncate_end(title, 24))
                .font(SANS)
                .size(theme::LABEL)
                .color(if selected { p.ink } else { p.ink_softer }),
        )
        .padding([7, 9])
        .style(move |_, _| iced::widget::button::Style {
            background: None,
            text_color: if selected { p.ink } else { p.ink_softer },
            border: Border::default(),
            ..Default::default()
        })
        .on_press(Message::OpenPage(id.clone()));
        #[cfg(all(feature = "agent", debug_assertions))]
        let open = iced_agent_plugin::sem(iced_agent_plugin::Role::Tab, title.to_owned(), open);
        let underline = container(Space::new().height(2))
            .width(Length::Fill)
            .style(move |_| iced::widget::container::Style {
                background: Some(Background::Color(if selected {
                    p.filled
                } else {
                    Color::TRANSPARENT
                })),
                ..Default::default()
            });
        tabs = tabs.push(
            container(
                column![
                    row![
                        open,
                        button(icons::view(Icon::Close, 11.0, p.muted))
                            .padding([6, 5])
                            .style(move |_, status| iced::widget::button::Style {
                                background: matches!(
                                    status,
                                    iced::widget::button::Status::Hovered
                                )
                                .then_some(Background::Color(p.hover)),
                                text_color: p.muted,
                                border: Border {
                                    radius: RADIUS_SM.into(),
                                    ..Default::default()
                                },
                                ..Default::default()
                            })
                            .on_press(Message::CloseTab(id.clone())),
                    ]
                    .align_y(Alignment::Center),
                    underline,
                ]
                .spacing(4),
            )
            .max_width(200)
            .style(move |_| iced::widget::container::Style {
                background: selected.then_some(Background::Color(p.paper)),
                ..Default::default()
            }),
        );
    }
    container(
        scrollable(tabs)
            .direction(scrollable::Direction::Horizontal(
                scrollable::Scrollbar::new(),
            ))
            .width(Length::Fill),
    )
    .width(Length::Fill)
    .height(36)
    .align_y(Alignment::Center)
    .style(move |_| surface(p.sidebar))
    .into()
}

fn no_page(p: Palette) -> Element<'static, Message> {
    ui::empty_state::empty_state(
        Some(icon_tile(Icon::Pages, 42.0, p)),
        "No page open",
        "Pick a page from the rail, or create one to start writing.",
        &theme::ui_for(&p),
    )
    .height(Length::Fill)
    .align_y(Alignment::Center)
    .into()
}

fn empty_state<'a>(title: &'a str, detail: &'a str, p: Palette) -> Element<'a, Message> {
    container(
        column![
            icon_tile(Icon::Pages, 38.0, p),
            text(title)
                .font(SANS_SEMIBOLD)
                .size(theme::BODY_LG)
                .color(p.ink),
            selectable_center(detail, p),
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

/// The empty-state detail line — selectable, because an error message shown
/// here is exactly the text a user needs to copy.
fn selectable_center<'a>(detail: &'a str, p: Palette) -> Element<'a, Message> {
    text_input("", detail)
        .font(SANS)
        .size(theme::BODY)
        .padding(0)
        .style(move |_, _| iced::widget::text_input::Style {
            background: Background::Color(Color::TRANSPARENT),
            border: Border::default(),
            icon: p.muted,
            placeholder: p.muted,
            value: p.muted,
            selection: theme::ACCENTS[0],
        })
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
        .size(theme::BODY)
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
    ui::input::input(placeholder, value, &theme::ui_for(&p)).on_input(on_input)
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
        .font(SANS_SEMIBOLD)
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
    let t = theme::ui_for(&p);
    let label = label.to_string();
    let button = ui::button::button(label.clone(), &t)
        .variant(ui::button::ButtonVariant::Outline)
        .size(ui::button::ButtonSize::Small)
        .disabled(!enabled)
        .on_press(message)
        .into_widget();
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::Sem::new(iced_agent_plugin::Role::Button, label, button)
        .disabled(!enabled)
        .into();
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    button.into()
}

/// Neutral-destructive button in the danger triad (danger ink / danger-soft
/// hover / danger-border), for `Delete page` and the block actions menu.
fn danger_outline<'a>(label: impl ToString, message: Message, p: Palette) -> Element<'a, Message> {
    let t = theme::ui_for(&p);
    let label = label.to_string();
    let btn = ui::button::button(label.clone(), &t)
        .variant(ui::button::ButtonVariant::DestructiveOutline)
        .size(ui::button::ButtonSize::Small)
        .on_press(message)
        .into_widget();
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(iced_agent_plugin::Role::Button, label, btn);
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
}

fn destructive_confirmation(
    copy: String,
    confirm: Message,
    cancel: Message,
    p: Palette,
) -> Element<'static, Message> {
    let delete = ui::button::button("Delete", &theme::ui_for(&p))
        .variant(ui::button::ButtonVariant::Destructive)
        .size(ui::button::ButtonSize::Small)
        .on_press(confirm)
        .into_widget();
    #[cfg(all(feature = "agent", debug_assertions))]
    let delete = iced_agent_plugin::sem(iced_agent_plugin::Role::Button, "Delete", delete);
    container(
        column![
            text(copy).font(SANS).size(theme::BODY).color(p.ink_soft),
            // Cancel left, destructive confirm right.
            row![outline("Cancel", cancel, p), delete].spacing(6),
        ]
        .spacing(8),
    )
    .padding(11)
    .style(move |_| soft_panel(p.danger_soft, p.danger_border, RADIUS_MD))
    .into()
}

fn filled_chip(icon: Icon, size: f32, p: Palette) -> Element<'static, Message> {
    container(icons::view(icon, size * 0.58, p.on_filled))
        .width(size)
        .height(size)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(move |_| iced::widget::container::Style {
            background: Some(Background::Color(p.filled)),
            border: Border {
                radius: RADIUS_SM.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

fn ghost_icon(
    icon: Icon,
    size: f32,
    message: Message,
    label: &'static str,
    p: Palette,
) -> Element<'static, Message> {
    let btn = button(
        container(icons::view(icon, size, p.muted_3))
            .padding([5, 6])
            .align_x(Alignment::Center)
            .align_y(Alignment::Center),
    )
    .padding(0)
    .style(move |_, status| iced::widget::button::Style {
        background: matches!(status, iced::widget::button::Status::Hovered)
            .then_some(Background::Color(p.hover)),
        text_color: p.muted_3,
        border: Border {
            radius: RADIUS_SM.into(),
            ..Default::default()
        },
        ..Default::default()
    })
    .on_press(message);
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(iced_agent_plugin::Role::Button, label, btn);
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    {
        let _ = label;
        btn.into()
    }
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
    ui::separator::horizontal(&theme::ui_for(&p)).into()
}

fn notice<'a>(copy: &'a str, p: Palette) -> Element<'a, Message> {
    let t = theme::ui_for(&p);
    ui::alert::alert(
        text(copy)
            .size(t.typography.sm)
            .color(t.palette.muted_foreground),
        ui::alert::AlertVariant::Default,
        &t,
    )
    .into()
}

fn notice_owned(copy: String, p: Palette) -> Element<'static, Message> {
    let t = theme::ui_for(&p);
    ui::alert::alert(
        text(copy)
            .size(t.typography.sm)
            .color(t.palette.muted_foreground),
        ui::alert::AlertVariant::Default,
        &t,
    )
    .into()
}

/// Page error — selectable, so the user can copy exactly what went wrong.
fn selectable_error<'a>(copy: &'a str, p: Palette) -> Element<'a, Message> {
    container(
        text_input("", copy)
            .font(SANS)
            .size(theme::BODY)
            .padding(0)
            .style(move |_, _| iced::widget::text_input::Style {
                background: Background::Color(Color::TRANSPARENT),
                border: Border::default(),
                icon: p.danger,
                placeholder: p.danger,
                value: p.danger,
                selection: theme::ACCENTS[0],
            }),
    )
    .padding(9)
    .style(move |_| soft_panel(p.danger_soft, p.danger_border, RADIUS_SM))
    .into()
}

fn panel(color: Color, border: Color) -> iced::widget::container::Style {
    soft_panel(color, border, RADIUS_SM)
}

fn soft_panel(color: Color, border: Color, radius: f32) -> iced::widget::container::Style {
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

/// Background-only container style — no border box. Section separation is a
/// dedicated 1px `divider`, never a four-side `Border`.
fn surface(color: Color) -> iced::widget::container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(color)),
        ..Default::default()
    }
}

/// Full-height side-panel frame: background plus a hairline edge against the
/// window. (iced `Border` is uniform; at full size this reads as the panel's
/// right/bottom rule, not a box around shrink content.)
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

fn crumb_button(title: &str, id: String, p: Palette) -> Element<'static, Message> {
    let title = title.to_owned();
    let btn = button(text(title.clone()).font(SANS).size(theme::BODY).color(p.muted))
        .padding([3, 5])
        .style(move |_, status| iced::widget::button::Style {
            background: matches!(status, iced::widget::button::Status::Hovered)
                .then_some(Background::Color(p.hover)),
            text_color: if matches!(status, iced::widget::button::Status::Hovered) {
                p.ink
            } else {
                p.muted
            },
            border: Border {
                radius: RADIUS_SM.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .on_press(Message::OpenPage(id));
    #[cfg(all(feature = "agent", debug_assertions))]
    return iced_agent_plugin::sem(iced_agent_plugin::Role::Button, title, btn);
    #[cfg(not(all(feature = "agent", debug_assertions)))]
    btn.into()
}

fn truncate_end(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        value.to_owned()
    } else {
        format!(
            "{}…",
            value.chars().take(max.saturating_sub(1)).collect::<String>()
        )
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
