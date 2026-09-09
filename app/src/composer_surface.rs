//! The chat composers as HOST SURFACES. The Chat tab is a module-owned view
//! (`crates/views/chat`), and the tree wire carries no rich editor — so the
//! view leaves a `chat_composer` slot per room and per thread, and the app
//! paints the composer there: the plate, the marks row, Send, the
//! failed-send banner, over `crate::editor`'s `RichTextEditor`.
//!
//! THE WORDS NEVER CROSS THE WIRE. Each slot is keyed by the composer's
//! scope (`composer_scope` / `thread_scope`, spelled by the view exactly as
//! `backend/model.rs` spells them), and the document for a scope lives here
//! for the life of the process — the retained-instance rule of
//! ducktape-ui#697: a draft never rides a room switch, and closing the rail
//! does not discard a reply. A submit leaves as the `composer` intent
//! ([`intent`]) carrying the kind, the trimmed body and a fresh operation
//! id; a refused or failed body comes back through [`unsent`], into the
//! banner of the box it was written in.
//!
//! `text_editor` borrows its `Content` for the widget's lifetime and the
//! rendered tree is `'static`, so — like the runtime's own host editor — the
//! widget here holds the shared document and builds the real composer
//! afresh inside every `Widget` method, over a lock held for that call
//! alone. The `Tree` state is the built composer's own, so focus and caret
//! carry over between calls as they would for a widget built once.
//!
//! THE TREE THE LAYOUT WAS MADE FOR. iced lays a tree out once and walks
//! that layout with every later call — the draw, an operation, the cursor
//! query, the event walk — so the document `build` reads may change only
//! where a layout follows before any walk: the runtime's build (`children`
//! or `diff`, then `layout`) and the event walk (`update`, then the
//! relayout it asks for). The app writes between frames, from a handler
//! ([`unsent`], [`roster`]) or a loader thread (the name directory), so its
//! writes never reach the document: a body queues in the slot's inbox, a
//! roster or directory moves its fact store's generation on, and the widget
//! takes them in ([`take_inputs`]) at exactly those two points. Every other
//! `Widget` method builds from the document as it was laid out. The lint
//! test at the bottom of this file holds that shape.
//!
//! THE MENTION MENU. An `@word` under the caret opens a list of the handles
//! it prefixes, above the editor inside the plate; the arrows walk it, Enter
//! or Tab (or a click) completes the word, Escape closes it for that word.
//! The handles are exactly the send's: [`MentionCandidates`] over the name
//! directory and the room's roster, which the app hands each room through
//! [`roster`] as it reads it — so a row offered is a mention that resolves.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer};
use iced::widget::text_editor::Content;
use iced::{Border, Color, Element, Event, Font, Length, Rectangle, Size, Vector, widget};
use ui_lang_runtime::view_tree::Surface;
use ui_lang_wire::SurfaceValue as Value;

use crate::backend::{ChatMember, MentionCandidates, names_at, names_generation, room_scope};
use crate::editor::{
    ComposerEvent, MentionQuery, MenuKey, apply_composer_event, complete_mention,
    composer_toggle_mark, mention_matches, mention_query, rich_composer,
};

/// One composer's state as the painted composer reads it: its words, the
/// unsent body its last failed send handed back, where the reader is in the
/// mention menu over the word under the caret, and the handles an `@` may
/// complete to, as last taken in.
#[derive(Default)]
struct Document {
    content: Content,
    failed: String,
    menu: MenuState,
    handles: Vec<String>,
}

/// What the app hands a composer between frames, held in the slot's inbox
/// until the widget takes it in where a layout follows.
enum Input {
    /// A refused or failed body, back to the box it was written in.
    Unsent { text: String, committed: bool },
}

/// The generations of the two fact stores a document's handles were
/// computed from; a store moved on means the handles are stale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Facts {
    names: u64,
    rosters: u64,
}

/// One scope's seat: the document the painted composer reads, the inbox
/// the app writes, and the facts the document's handles are current to —
/// `None` until the first take-in computes them.
struct Slot {
    scope: String,
    document: Document,
    inbox: Vec<Input>,
    facts: Option<Facts>,
}

/// The reader's position in the mention menu for ONE typed word: which row
/// is highlighted, and whether Escape closed the menu for it. Typing a
/// different word starts over — the menu is derived from the words, and
/// this is the only thing about it that is not.
#[derive(Default)]
struct MenuState {
    partial: String,
    selected: usize,
    dismissed: bool,
}

/// The mention menu as drawn: the word it completes, the handles it offers
/// and the highlighted one.
struct Menu {
    query: MentionQuery,
    matches: Vec<String>,
    selected: usize,
}

type Shared = Arc<Mutex<Slot>>;

thread_local! {
    // THE UI THREAD'S OWN. Every composer is painted and edited on the one
    // thread iced runs the app on, and a handler's `unsent` runs there too —
    // so the slots are that thread's, which also keeps every test thread's
    // rooms apart without a window in the key.
    //
    // ponytail: one slot per scope for the life of the thread, never
    // evicted — a scope is a room or a thread the reader typed in, which is
    // bounded by how many she visits; add an LRU if a long session shows it.
    static SLOTS: RefCell<HashMap<String, Shared>> = RefCell::default();
}

fn slot(scope: &str) -> Shared {
    SLOTS.with_borrow_mut(|slots| {
        slots
            .entry(scope.to_owned())
            .or_insert_with(|| {
                Arc::new(Mutex::new(Slot {
                    scope: scope.to_owned(),
                    document: Document::default(),
                    inbox: Vec::new(),
                    facts: None,
                }))
            })
            .clone()
    })
}

/// Each room's explicit roster, as the app last read it, under the room's
/// composer scope; a thread's composer reads its room's. Never evicted, for
/// the same reason the slots are not: a room the reader visited is a room
/// she may write in again, and its roster is still its roster. The
/// generation moves on every write, so a slot can tell its handles are
/// behind without comparing rosters.
#[derive(Default)]
struct Rosters {
    by_room: HashMap<String, Vec<ChatMember>>,
    generation: u64,
}

thread_local! {
    static ROSTERS: RefCell<Rosters> = RefCell::default();
}

/// The app's hand-off: the roster of the room under `scope`, for the
/// composers over it. Called wherever the app learns a room's members.
pub fn roster(scope: &str, members: &[ChatMember]) {
    ROSTERS.with_borrow_mut(|rosters| {
        rosters.by_room.insert(scope.to_owned(), members.to_vec());
        rosters.generation += 1;
    });
}

/// The generations the fact stores are at right now.
fn current_facts() -> Facts {
    Facts {
        names: names_generation(),
        rosters: ROSTERS.with_borrow(|rosters| rosters.generation),
    }
}

/// Who an `@` in this composer may complete to — the send's own rule over
/// the name directory and the room's roster, under the scope itself (a
/// room), else under the room a thread scope names — and the generations
/// the stores were read at.
fn handles(scope: &str) -> (Facts, Vec<String>) {
    let (rosters_generation, members) = ROSTERS.with_borrow(|rosters| {
        let members = rosters
            .by_room
            .get(scope)
            .or_else(|| rosters.by_room.get(&room_scope(scope)))
            .cloned()
            .unwrap_or_default();
        (rosters.generation, members)
    });
    let (names_generation, directory) = names_at();
    let handles = MentionCandidates::new(&directory, &members)
        .handles()
        .into_iter()
        .map(str::to_owned)
        .collect();
    let facts = Facts {
        names: names_generation,
        rosters: rosters_generation,
    };
    (facts, handles)
}

fn lock(slot: &Shared) -> MutexGuard<'_, Slot> {
    // A panic while the lock was held leaves the slot usable: the widget
    // only ever reads it here and applies whole interactions.
    slot.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The app's share-out: what a room's or thread's send let go of, handed
/// back to the box it came from. It waits in the inbox until the composer
/// next takes its inputs in; a committed body is not unsent, so it never
/// stashes — `remember_failed_draft` decides then, as it did in the view.
pub fn unsent(scope: &str, text: &str, committed: bool) {
    let slot = slot(scope);
    lock(&slot).inbox.push(Input::Unsent {
        text: text.to_owned(),
        committed,
    });
}

/// The app's inputs taken into the document — the handed-back bodies folded
/// into the stash, the handles recomputed when a fact store moved on — and
/// whether the document changed for it. This is the ONLY writer of the
/// document outside the reader's own interactions, and it runs only where
/// a layout follows before any walk of the tree: `children`, `diff`, and
/// `update` after its event walk.
fn take_inputs(slot: &mut Slot) -> bool {
    let inputs = std::mem::take(&mut slot.inbox);
    let stash_changed = fold_handbacks(&mut slot.document, inputs);
    let facts_stale = slot.facts != Some(current_facts());
    if facts_stale {
        let (facts, handles) = handles(&slot.scope);
        slot.document.handles = handles;
        slot.facts = Some(facts);
    }
    stash_changed || facts_stale
}

/// The handed-back bodies folded into the stash, in the order they
/// arrived; whether the stash changed.
fn fold_handbacks(document: &mut Document, inputs: Vec<Input>) -> bool {
    if inputs.is_empty() {
        return false;
    }
    let before = std::mem::take(&mut document.failed);
    let mut failed = before.clone();
    for input in inputs {
        match input {
            Input::Unsent { text, committed } => {
                failed =
                    crate::backend::remember_failed_draft(failed, "stash".into(), text, committed);
            }
        }
    }
    let changed = failed != before;
    document.failed = failed;
    changed
}

/// The `chat_composer` surface: `(scope, kind, compact, hint, blocked,
/// restore_blocked, failed_note)`, as the view declares it.
pub fn provider() -> Surface {
    Arc::new(|_key: &str, args: &[Value]| {
        let [
            Value::Str(scope),
            Value::Str(kind),
            Value::Bool(compact),
            Value::Str(hint),
            Value::Bool(blocked),
            Value::Bool(restore_blocked),
            Value::Str(failed_note),
        ] = args
        else {
            return widget::text("invalid chat_composer arguments").into();
        };
        Element::new(Composer {
            slot: slot(scope),
            scope: scope.clone(),
            kind: kind.clone(),
            compact: *compact,
            hint: hint.clone(),
            blocked: *blocked,
            restore_blocked: *restore_blocked,
            failed_note: failed_note.clone(),
        })
    })
}

/// The intent a surface event is, if it is one: a submit crosses as a
/// `composer` record — with the scope it was written in, so the handler
/// that takes it later can tell a box the reader has since left from the
/// one on screen; an edit is the surface's own business and is not.
pub fn intent(value: &Value) -> Option<crate::module_view::ModuleViewEvent> {
    let Value::Record { name, fields } = value else {
        return None;
    };
    if name != "composer" {
        return None;
    }
    let field = |wanted: &str| {
        fields.iter().find_map(|(key, value)| match value {
            Value::Str(text) if key == wanted => Some(text.as_str()),
            _ => None,
        })
    };
    let detail = serde_json::json!({
        "scope": field("scope")?,
        "kind": field("kind")?,
        "body": field("body")?,
        "id": field("id")?,
    });
    Some(crate::module_view::ModuleViewEvent {
        kind: "composer".into(),
        detail: detail.to_string(),
    })
}

/// What the reader did in the composer: an editor event (a keystroke, a
/// paste, a click, a chord, the Send button's synthetic Submit, a key the
/// mention menu claimed), a mark from the toolbar, a click on a mention
/// menu row, or the banner's two buttons.
#[derive(Clone, Debug)]
pub(crate) enum Interaction {
    Editor(ComposerEvent),
    Mark(&'static str),
    Pick(String),
    Restore,
    Dismiss,
}

struct Composer {
    slot: Shared,
    scope: String,
    kind: String,
    compact: bool,
    hint: String,
    blocked: bool,
    restore_blocked: bool,
    failed_note: String,
}

impl Composer {
    /// The child tree diffed against the element `document` builds. iced
    /// does this on a rebuild of the app's view and never on a relayout, so
    /// a document change inside `update` does it itself: the element the
    /// next layout builds is not the one the tree was last diffed against
    /// (the mention menu appears and disappears with the word under the
    /// caret), and a stale tree is a widget laid out with another's state.
    fn diff_document(&self, tree: &mut Tree, document: &Document) {
        tree.diff_children(&[self.build(document).as_widget()]);
    }

    /// The mention menu over the word under the caret, if one is showing:
    /// the word is a mention in progress, the reader has not closed the menu
    /// for it, and at least one of the document's handles prefixes it. The
    /// highlighted row is the reader's for the word she is typing and the
    /// first row for a new one.
    fn menu(&self, document: &Document) -> Option<Menu> {
        let query = mention_query(&document.content)?;
        let same_word = document.menu.partial == query.partial;
        if same_word && document.menu.dismissed {
            return None;
        }
        let handles: Vec<&str> = document.handles.iter().map(String::as_str).collect();
        let matches = mention_matches(&handles, &query.partial);
        if matches.is_empty() {
            return None;
        }
        let selected = if same_word {
            document.menu.selected.min(matches.len() - 1)
        } else {
            0
        };
        Some(Menu {
            query,
            matches,
            selected,
        })
    }

    /// One interaction applied to the document; a submit that goes through
    /// is the value published to the guest tree.
    fn apply(&self, document: &mut Document, interaction: Interaction) -> Option<Value> {
        match interaction {
            Interaction::Editor(ComposerEvent::Menu(key)) => {
                let menu = self.menu(document)?;
                let rows = menu.matches.len();
                let step = |from: usize, by: usize| (from + by) % rows;
                document.menu = match key {
                    MenuKey::Up => MenuState {
                        partial: menu.query.partial,
                        selected: step(menu.selected, rows - 1),
                        dismissed: false,
                    },
                    MenuKey::Down => MenuState {
                        partial: menu.query.partial,
                        selected: step(menu.selected, 1),
                        dismissed: false,
                    },
                    MenuKey::Dismiss => MenuState {
                        partial: menu.query.partial,
                        selected: 0,
                        dismissed: true,
                    },
                    MenuKey::Pick => {
                        let content = std::mem::take(&mut document.content);
                        document.content =
                            complete_mention(content, &menu.query, &menu.matches[menu.selected]);
                        MenuState::default()
                    }
                };
                None
            }
            Interaction::Pick(handle) => {
                let menu = self.menu(document)?;
                let content = std::mem::take(&mut document.content);
                document.content = complete_mention(content, &menu.query, &handle);
                document.menu = MenuState::default();
                None
            }
            Interaction::Editor(ComposerEvent::Submit) => {
                if self.blocked {
                    return None;
                }
                let body = document.content.text().trim().to_owned();
                if body.is_empty() {
                    return None;
                }
                document.content = Content::new();
                let prefix = if self.kind == "reply" {
                    "reply"
                } else {
                    "message"
                };
                Some(Value::Record {
                    name: "composer".into(),
                    fields: vec![
                        ("scope".into(), Value::Str(self.scope.clone())),
                        ("kind".into(), Value::Str(self.kind.clone())),
                        ("body".into(), Value::Str(body)),
                        (
                            "id".into(),
                            Value::Str(crate::backend::fresh_operation_id(prefix.into())),
                        ),
                    ],
                })
            }
            Interaction::Editor(event) => {
                let content = std::mem::take(&mut document.content);
                document.content = apply_composer_event(content, event);
                None
            }
            Interaction::Mark(glyph) => {
                if self.blocked {
                    return None;
                }
                let content = std::mem::take(&mut document.content);
                document.content = composer_toggle_mark(content, glyph.into());
                None
            }
            Interaction::Restore => {
                let empty = document.content.text().trim().is_empty();
                if self.restore_blocked || document.failed.is_empty() || !empty {
                    return None;
                }
                document.content = Content::with_text(&document.failed);
                document.failed.clear();
                None
            }
            Interaction::Dismiss => {
                document.failed.clear();
                None
            }
        }
    }

    /// The composer for this call, over the locked document: the
    /// failed-send banner (an empty slot when there is none, so the editor
    /// under it keeps its tree position and its caret), then the plate
    /// with the editor, the marks and Send — `ChatComposer` in
    /// components/chat.ice before the port, shape for shape.
    fn build<'a>(&'a self, document: &'a Document) -> Element<'a, Interaction> {
        let empty = document.content.text().trim().is_empty();
        let banner: Element<'a, Interaction> = if document.failed.is_empty() {
            widget::Space::new().into()
        } else {
            let restore = widget::button(widget::text("Restore").size(12.5))
                .padding([5, 9])
                .style(secondary_button)
                .on_press_maybe((!self.restore_blocked && empty).then_some(Interaction::Restore));
            let dismiss = widget::button(widget::text("×").size(14.0).center())
                .width(28)
                .height(28)
                .padding(0)
                .style(ghost_button)
                .on_press(Interaction::Dismiss);
            widget::container(
                widget::row![
                    widget::container(widget::Space::new())
                        .width(6)
                        .height(6)
                        .style(|theme| {
                            let tokens = crate::backend::app_tokens(theme);
                            widget::container::Style {
                                background: Some(tokens.palette.destructive_dot.into()),
                                border: Border::default().rounded(3.0),
                                ..Default::default()
                            }
                        }),
                    widget::text(self.failed_note.as_str())
                        .size(12.5)
                        .line_height(1.45)
                        .width(Length::Fill)
                        .style(|theme| widget::text::Style {
                            color: Some(crate::backend::app_tokens(theme).palette.destructive),
                        }),
                    restore,
                    dismiss,
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center)
                .width(Length::Fill),
            )
            .padding([11, 13])
            .width(Length::Fill)
            .style(|theme| {
                let tokens = crate::backend::app_tokens(theme);
                widget::container::Style {
                    background: Some(tokens.palette.destructive_background.into()),
                    border: Border {
                        color: tokens.palette.destructive_line,
                        width: 1.0,
                        radius: 9.0.into(),
                    },
                    ..Default::default()
                }
            })
            .into()
        };
        let menu = if self.blocked {
            None
        } else {
            self.menu(document)
        };
        let editor = rich_composer(
            &document.content,
            self.hint.clone(),
            self.blocked,
            menu.is_some(),
            44.0,
            150.0,
            10.0,
        )
        .map(Interaction::Editor);
        let suggestions: Element<'a, Interaction> = match menu {
            None => widget::Space::new().into(),
            Some(menu) => mention_menu(menu),
        };
        let mark = |label: Element<'a, Interaction>, glyph: &'static str| {
            widget::button(widget::container(label).center(Length::Fill))
                .width(26)
                .height(24)
                .padding(0)
                .style(ghost_button)
                .on_press_maybe((!self.blocked).then_some(Interaction::Mark(glyph)))
                .into()
        };
        let marks: Vec<Element<'a, Interaction>> = vec![
            mark(
                widget::text("B")
                    .size(12.5)
                    .font(Font {
                        weight: iced::font::Weight::Bold,
                        ..crate::Ducktape::default_font()
                    })
                    .into(),
                "bold",
            ),
            mark(
                widget::text("I")
                    .size(12.5)
                    .font(Font {
                        style: iced::font::Style::Italic,
                        ..crate::Ducktape::default_font()
                    })
                    .into(),
                "italic",
            ),
            widget::container(widget::Space::new())
                .width(1)
                .height(14)
                .style(|theme| widget::container::Style {
                    background: Some(crate::backend::app_tokens(theme).palette.border.into()),
                    ..Default::default()
                })
                .into(),
            mark(glyph("code-brackets"), "code"),
            mark(glyph("quote"), "quote"),
        ];
        let send = widget::button(widget::text("Send").size(12.5))
            .padding(if self.compact { [6, 11] } else { [7, 12] })
            .height(if self.compact { 28 } else { 29 })
            .style(primary_button)
            .on_press_maybe(
                (!self.blocked && !empty).then_some(Interaction::Editor(ComposerEvent::Submit)),
            );
        let mut tail = widget::row(marks)
            .spacing(2)
            .align_y(iced::Alignment::Center)
            .push(widget::Space::new().width(Length::Fill));
        if !self.compact {
            tail = tail
                .push(
                    widget::text("↵ send · ⇧↵ newline")
                        .size(10.5)
                        .font(Font {
                            family: iced::font::Family::Name("Geist Mono"),
                            weight: iced::font::Weight::Medium,
                            ..Font::DEFAULT
                        })
                        .style(|theme| widget::text::Style {
                            color: Some(crate::backend::app_tokens(theme).palette.muted_foreground),
                        }),
                )
                .push(widget::Space::new().width(8));
        }
        let tail = tail.push(send);
        let plate = widget::container(
            widget::column![
                suggestions,
                editor,
                widget::container(tail.width(Length::Fill)).padding(iced::Padding {
                    top: 0.0,
                    right: 8.0,
                    bottom: 8.0,
                    left: 8.0,
                }),
            ]
            .width(Length::Fill),
        )
        .width(Length::Fill)
        .clip(true)
        .style(|theme| {
            let tokens = crate::backend::app_tokens(theme);
            widget::container::Style {
                background: Some(tokens.palette.card.into()),
                border: Border {
                    color: tokens.palette.control_line,
                    width: 1.0,
                    radius: 12.0.into(),
                },
                shadow: tokens.elevation.popover,
                ..Default::default()
            }
        });
        widget::column![banner, plate]
            .spacing(10)
            .width(Length::Fill)
            .into()
    }
}

/// The mention menu's rows, one button per handle, the highlighted one on
/// the accent wash: a click completes the word the way Enter does.
fn mention_menu<'a>(menu: Menu) -> Element<'a, Interaction> {
    let rows: Vec<Element<'a, Interaction>> = menu
        .matches
        .into_iter()
        .enumerate()
        .map(|(index, handle)| {
            let highlighted = index == menu.selected;
            let label = widget::text(format!("@{handle}")).size(12.5).font(Font {
                family: iced::font::Family::Name("Geist Mono"),
                ..Font::DEFAULT
            });
            widget::button(label)
                .width(Length::Fill)
                .padding([5, 10])
                .style(move |theme, status| mention_row(theme, status, highlighted))
                .on_press(Interaction::Pick(handle))
                .into()
        })
        .collect();
    widget::container(
        widget::scrollable(widget::column(rows).width(Length::Fill)).height(Length::Shrink),
    )
    .max_height(168)
    .width(Length::Fill)
    .padding(iced::Padding {
        top: 6.0,
        right: 6.0,
        bottom: 0.0,
        left: 6.0,
    })
    .into()
}

fn mention_row(
    theme: &iced::Theme,
    status: widget::button::Status,
    highlighted: bool,
) -> widget::button::Style {
    let tokens = crate::backend::app_tokens(theme);
    let hovered = matches!(
        status,
        widget::button::Status::Hovered | widget::button::Status::Pressed
    );
    let background = if highlighted || hovered {
        Some(tokens.palette.accent.into())
    } else {
        None
    };
    widget::button::Style {
        background,
        text_color: tokens.palette.foreground,
        border: Border::default().rounded(6.0),
        ..Default::default()
    }
}

/// The toolbar glyphs, as the design crate ships them; the button's ink is
/// the svg's.
fn glyph<'a>(name: &str) -> Element<'a, Interaction> {
    widget::svg(widget::svg::Handle::from_memory(crate::backend::icon(name)))
        .width(13)
        .height(13)
        .style(|theme, _| widget::svg::Style {
            color: Some(crate::backend::app_tokens(theme).palette.muted_foreground),
        })
        .into()
}

fn primary_button(theme: &iced::Theme, status: widget::button::Status) -> widget::button::Style {
    let tokens = crate::backend::app_tokens(theme);
    let (background, text) = match status {
        widget::button::Status::Disabled => {
            (tokens.palette.disabled, tokens.palette.disabled_foreground)
        }
        widget::button::Status::Hovered | widget::button::Status::Pressed => (
            tokens.palette.primary_hover,
            tokens.palette.primary_foreground,
        ),
        widget::button::Status::Active => {
            (tokens.palette.primary, tokens.palette.primary_foreground)
        }
    };
    widget::button::Style {
        background: Some(background.into()),
        text_color: text,
        border: Border::default().rounded(tokens.radius.button),
        ..Default::default()
    }
}

fn secondary_button(theme: &iced::Theme, status: widget::button::Status) -> widget::button::Style {
    let tokens = crate::backend::app_tokens(theme);
    let wash = |alpha: f32| Color {
        a: alpha,
        ..tokens.palette.foreground
    };
    let background = match status {
        widget::button::Status::Disabled => wash(0.05),
        widget::button::Status::Hovered => wash(0.14),
        widget::button::Status::Pressed => wash(0.18),
        widget::button::Status::Active => wash(0.09),
    };
    widget::button::Style {
        background: Some(background.into()),
        text_color: match status {
            widget::button::Status::Disabled => tokens.palette.disabled_foreground,
            _ => tokens.palette.foreground,
        },
        border: Border {
            color: wash(0.11),
            width: 1.0,
            radius: 7.0.into(),
        },
        ..Default::default()
    }
}

fn ghost_button(theme: &iced::Theme, status: widget::button::Status) -> widget::button::Style {
    let tokens = crate::backend::app_tokens(theme);
    let wash = |alpha: f32| Color {
        a: alpha,
        ..tokens.palette.foreground
    };
    let (background, text) = match status {
        widget::button::Status::Disabled => {
            (Color::TRANSPARENT, tokens.palette.disabled_foreground)
        }
        widget::button::Status::Hovered => (wash(0.08), tokens.palette.foreground),
        widget::button::Status::Pressed => (wash(0.12), tokens.palette.foreground),
        widget::button::Status::Active => (Color::TRANSPARENT, tokens.palette.muted_foreground),
    };
    widget::button::Style {
        background: Some(background.into()),
        text_color: text,
        border: Border::default().rounded(6.0),
        ..Default::default()
    }
}

impl Widget<Value, iced::Theme, iced::Renderer> for Composer {
    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Fill,
            height: Length::Shrink,
        }
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<()>()
    }

    fn state(&self) -> tree::State {
        tree::State::None
    }

    fn children(&self) -> Vec<Tree> {
        let mut slot = lock(&self.slot);
        take_inputs(&mut slot);
        vec![Tree::new(self.build(&slot.document).as_widget())]
    }

    fn diff(&self, tree: &mut Tree) {
        let mut slot = lock(&self.slot);
        take_inputs(&mut slot);
        self.diff_document(tree, &slot.document);
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let slot = lock(&self.slot);
        self.build(&slot.document)
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let slot = lock(&self.slot);
        self.build(&slot.document).as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        let slot = lock(&self.slot);
        self.build(&slot.document).as_widget_mut().operate(
            &mut tree.children[0],
            layout,
            renderer,
            operation,
        );
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Value>,
        viewport: &Rectangle,
    ) {
        let mut slot = lock(&self.slot);
        let mut interactions = Vec::new();
        let mut local = Shell::new(&mut interactions);
        self.build(&slot.document).as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            &mut local,
            viewport,
        );
        if local.is_event_captured() {
            shell.capture_event();
        }
        if local.is_layout_invalid() {
            shell.invalidate_layout();
        }
        if local.are_widgets_invalid() {
            shell.invalidate_widgets();
        }
        match local.redraw_request() {
            iced::window::RedrawRequest::NextFrame => shell.request_redraw(),
            iced::window::RedrawRequest::At(at) => shell.request_redraw_at(at),
            iced::window::RedrawRequest::Wait => {}
        }
        shell.input_method_mut().merge(local.input_method());
        // The reader acted on the tree she saw, so her interactions land
        // before the app's inputs: a Restore restores the stash the banner
        // showed, and a body handed back meanwhile opens a banner of its own.
        let reader_acted = !interactions.is_empty();
        for interaction in interactions {
            if let Some(submitted) = self.apply(&mut slot.document, interaction) {
                shell.publish(submitted);
            }
        }
        let inputs_taken = take_inputs(&mut slot);
        let document_changed = reader_acted || inputs_taken;
        if !document_changed {
            return;
        }
        self.diff_document(tree, &slot.document);
        shell.invalidate_layout();
        shell.request_redraw();
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        let slot = lock(&self.slot);
        self.build(&slot.document).as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'a>(
        &'a mut self,
        _tree: &'a mut Tree,
        _layout: Layout<'a>,
        _renderer: &iced::Renderer,
        _viewport: &Rectangle,
        _translation: Vector,
    ) -> Option<overlay::Element<'a, Value, iced::Theme, iced::Renderer>> {
        // The composer has no overlay, and one could not outlive the lock.
        None
    }
}

/// The app suite's seat at the composer: one interaction on a scope's
/// document, exactly as the painted composer applies it; the words, the
/// menu and the stash as the painted composer reads them; and the next
/// frame's take-in of what the app wrote between frames.
#[cfg(test)]
pub(crate) mod testing {
    pub(crate) use super::Interaction;
    use super::*;

    /// A submit that goes through is the value the guest tree publishes.
    pub(crate) fn interact(
        scope: &str,
        kind: &str,
        blocked: bool,
        restore_blocked: bool,
        interaction: Interaction,
    ) -> Option<Value> {
        let composer = Composer {
            slot: slot(scope),
            scope: scope.into(),
            kind: kind.into(),
            compact: false,
            hint: String::new(),
            blocked,
            restore_blocked,
            failed_note: String::new(),
        };
        let slot = composer.slot.clone();
        let mut slot = lock(&slot);
        composer.apply(&mut slot.document, interaction)
    }

    /// The next frame's take-in: what the app handed the scope's composer
    /// between frames (a body, a roster, the directory), folded into the
    /// document the way the widget's own build point does it.
    pub(crate) fn settle(scope: &str) {
        let slot = slot(scope);
        take_inputs(&mut lock(&slot));
    }

    pub(crate) fn text(scope: &str) -> String {
        let slot = slot(scope);
        let slot = lock(&slot);
        slot.document.content.text()
    }

    /// The mention menu's rows over the scope's words, and the highlighted
    /// one — as the painted composer would draw them for an unblocked box.
    pub(crate) fn menu_rows(scope: &str) -> Option<(Vec<String>, usize)> {
        let composer = Composer {
            slot: slot(scope),
            scope: scope.into(),
            kind: "message".into(),
            compact: false,
            hint: String::new(),
            blocked: false,
            restore_blocked: false,
            failed_note: String::new(),
        };
        let slot = composer.slot.clone();
        let slot = lock(&slot);
        composer
            .menu(&slot.document)
            .map(|menu| (menu.matches, menu.selected))
    }

    pub(crate) fn failed(scope: &str) -> String {
        let slot = slot(scope);
        let slot = lock(&slot);
        slot.document.failed.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{BoundAccount, NameDirectory};
    use iced::widget::text_editor::{Action, Edit};
    use std::collections::BTreeMap;

    /// Type `text` into the scope's composer, one keystroke at a time, the
    /// way the editor delivers them.
    fn type_into(scope: &str, text: &str) {
        for c in text.chars() {
            testing::interact(
                scope,
                "message",
                false,
                false,
                Interaction::Editor(ComposerEvent::Apply(crate::editor::RichAction::Edit(
                    Action::Edit(Edit::Insert(c)),
                ))),
            );
        }
    }

    fn menu_key(scope: &str, key: MenuKey) {
        testing::interact(
            scope,
            "message",
            false,
            false,
            Interaction::Editor(ComposerEvent::Menu(key)),
        );
    }

    /// A directory naming two accounts.
    fn two_accounts() -> NameDirectory {
        let mut accounts = BTreeMap::new();
        accounts.insert(
            "aa".repeat(32),
            BoundAccount {
                number: 5,
                name: "ChiefDuck".into(),
            },
        );
        accounts.insert(
            "bb".repeat(32),
            BoundAccount {
                number: 6,
                name: "chi-ops".into(),
            },
        );
        NameDirectory::new(accounts)
    }

    /// A room's roster adding a member the directory does not know, whose
    /// handle is its key's.
    fn seat_roster(room: &str) {
        roster(
            room,
            &[ChatMember {
                key: "user:cafe0123".into(),
                label: "cafe".into(),
            }],
        );
    }

    /// The two-account directory seated and the room's roster handed over:
    /// the handles a send resolves.
    fn seat_directory_and_roster(room: &str) -> crate::backend::SeededNames {
        seat_roster(room);
        crate::backend::seed_names(two_accounts())
    }

    #[test]
    fn a_failed_send_stashes_into_the_box_it_came_from_and_a_committed_one_does_not() {
        unsent("net\u{1f}room-a", "hello", false);
        unsent("net\u{1f}room-a", "again", false);
        unsent("net\u{1f}room-b", "landed", true);
        assert_eq!(
            testing::failed("net\u{1f}room-a"),
            "",
            "a body handed back waits for the next frame"
        );
        testing::settle("net\u{1f}room-a");
        testing::settle("net\u{1f}room-b");
        assert_eq!(testing::failed("net\u{1f}room-a"), "hello\nagain");
        assert_eq!(testing::failed("net\u{1f}room-b"), "");
    }

    #[test]
    fn a_submit_is_the_composer_intent_and_an_edit_is_not() {
        let value = Value::Record {
            name: "composer".into(),
            fields: vec![
                ("scope".into(), Value::Str("net\u{1f}room-a#4".into())),
                ("kind".into(), Value::Str("reply".into())),
                ("body".into(), Value::Str("hi".into())),
                ("id".into(), Value::Str("reply-1".into())),
            ],
        };
        let event = intent(&value).expect("a submit");
        assert_eq!(event.kind, "composer");
        let detail: serde_json::Value = serde_json::from_str(&event.detail).expect("json");
        assert_eq!(detail["scope"], "net\u{1f}room-a#4");
        assert_eq!(detail["kind"], "reply");
        assert_eq!(detail["body"], "hi");
        assert_eq!(detail["id"], "reply-1");
        assert!(intent(&Value::Unit).is_none());
    }

    /// THE MENU IS A WIDGET THAT APPEARS UNDER A TREE DIFFED FOR ITS ABSENCE.
    /// iced lays a relayout out over the tree the last build diffed, so the
    /// composer diffs its own child after every interaction it applies: the
    /// `@` that opens the menu, typed through the painted widget, is followed
    /// by the relayout and the draw the window runs on it.
    #[test]
    fn the_menu_opening_under_the_caret_survives_the_relayout() {
        use iced::advanced::clipboard;
        use iced::keyboard;
        use iced_test::runtime::user_interface::{self, UserInterface};

        let room = "net\u{1f}mention-relayout";
        let _names = seat_directory_and_roster(room);
        let composer = Element::<Value>::new(Composer {
            slot: slot(room),
            scope: room.into(),
            kind: "message".into(),
            compact: false,
            hint: String::new(),
            blocked: false,
            restore_blocked: false,
            failed_note: String::new(),
        });
        let mut renderer = crate::frame_probe::headless_renderer();
        let size = Size::new(600.0, 300.0);
        let mut clipboard = clipboard::Null;
        let mut published: Vec<Value> = Vec::new();
        let mut ui =
            UserInterface::build(composer, size, user_interface::Cache::new(), &mut renderer);

        // a press on the editor's first line focuses it
        let position = iced::Point::new(200.0, 30.0);
        let cursor = mouse::Cursor::Available(position);
        ui.update(
            &[
                Event::Mouse(mouse::Event::CursorMoved { position }),
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            ],
            cursor,
            &mut renderer,
            &mut clipboard,
            &mut published,
        );
        // the `@` opens the menu inside this update
        ui.update(
            &[Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Character("@".into()),
                modified_key: keyboard::Key::Character("@".into()),
                physical_key: keyboard::key::Physical::Code(keyboard::key::Code::Digit2),
                location: keyboard::Location::Standard,
                modifiers: keyboard::Modifiers::SHIFT,
                text: Some("@".into()),
                repeat: false,
            })],
            cursor,
            &mut renderer,
            &mut clipboard,
            &mut published,
        );
        assert_eq!(
            testing::text(room),
            "@",
            "the press focused the editor and the key typed"
        );
        assert_eq!(
            testing::menu_rows(room).map(|(rows, _)| rows.len()),
            Some(3),
            "the menu is open over the word"
        );
        assert!(published.is_empty(), "no submit");

        let mut ui = ui.relayout(size, &mut renderer);
        ui.draw(
            &mut renderer,
            &iced::Theme::Dark,
            &renderer::Style::default(),
            cursor,
        );
    }

    /// THE MENU OFFERS WHAT THE SEND RESOLVES, AND ONLY WHILE A MENTION IS
    /// BEING TYPED. The rows are the directory's handles and the room's keyed
    /// members, prefix-matched to the word under the caret; a word that names
    /// nobody, or a caret that has left the word, shows nothing.
    #[test]
    fn the_mention_menu_follows_the_word_under_the_caret() {
        let room = "net\u{1f}mention-room";
        let _names = seat_directory_and_roster(room);
        testing::settle(room);
        type_into(room, "hello @");
        assert_eq!(
            testing::menu_rows(room),
            Some((
                vec!["cafe0123".into(), "chi-ops".into(), "chiefduck".into()],
                0
            ))
        );
        type_into(room, "CH");
        assert_eq!(
            testing::menu_rows(room),
            Some((vec!["chi-ops".into(), "chiefduck".into()], 0))
        );
        type_into(room, "x");
        assert_eq!(testing::menu_rows(room), None, "no handle starts with chx");
        type_into(room, " and @c");
        assert!(
            testing::menu_rows(room).is_some(),
            "a new word reopens the menu"
        );
        // a thread over the room reads the room's roster
        let thread = format!("{room}#12");
        testing::settle(&thread);
        type_into(&thread, "@ca");
        assert_eq!(
            testing::menu_rows(&thread),
            Some((vec!["cafe0123".into()], 0))
        );
        // an `@` glued to a word is an address or a decoration, not a mention
        type_into(room, " mail@ch");
        assert_eq!(testing::menu_rows(room), None);
    }

    /// The arrows walk the rows and wrap; Enter completes the highlighted
    /// handle in place and leaves the caret after a space; Escape closes the
    /// menu for that word and typing on reopens it.
    #[test]
    fn the_menu_keys_walk_pick_and_dismiss() {
        let room = "net\u{1f}mention-keys";
        let _names = seat_directory_and_roster(room);
        testing::settle(room);
        type_into(room, "@ch");
        menu_key(room, MenuKey::Down);
        assert_eq!(testing::menu_rows(room).map(|(_, at)| at), Some(1));
        menu_key(room, MenuKey::Down);
        assert_eq!(testing::menu_rows(room).map(|(_, at)| at), Some(0), "wraps");
        menu_key(room, MenuKey::Up);
        assert_eq!(testing::menu_rows(room).map(|(_, at)| at), Some(1));
        menu_key(room, MenuKey::Pick);
        assert_eq!(testing::text(room).trim_end(), "@chiefduck");
        assert_eq!(
            testing::menu_rows(room),
            None,
            "a completed word is not a query"
        );
        type_into(room, "and @c");
        assert_eq!(testing::text(room).trim_end(), "@chiefduck and @c");

        menu_key(room, MenuKey::Dismiss);
        assert_eq!(
            testing::menu_rows(room),
            None,
            "escape closes the menu for this word"
        );
        type_into(room, "h");
        assert!(testing::menu_rows(room).is_some(), "typing on reopens it");

        // a click on a row completes like Enter does
        testing::interact(
            room,
            "message",
            false,
            false,
            Interaction::Pick("chi-ops".into()),
        );
        assert_eq!(testing::text(room).trim_end(), "@chiefduck and @chi-ops");
    }

    /// The menu only ever completes the word under the caret: a completion
    /// mid-line replaces exactly the typed `@partial`.
    #[test]
    fn a_completion_replaces_only_the_typed_word() {
        let room = "net\u{1f}mention-midline";
        let _names = seat_directory_and_roster(room);
        testing::settle(room);
        type_into(room, "ping @chi please");
        // put the caret right after "@chi"
        for _ in 0.." please".len() {
            testing::interact(
                room,
                "message",
                false,
                false,
                Interaction::Editor(ComposerEvent::Apply(crate::editor::RichAction::Edit(
                    Action::Move(iced::widget::text_editor::Motion::Left),
                ))),
            );
        }
        assert!(
            testing::menu_rows(room).is_some(),
            "the caret ends the word again"
        );
        menu_key(room, MenuKey::Pick);
        assert_eq!(testing::text(room).trim_end(), "ping @chi-ops  please");
    }

    /// THE APP WRITES THE COMPOSER'S INPUTS BETWEEN FRAMES. A roster, a
    /// directory or a handed-back body lands while a mention is being typed:
    /// the tree the window laid out has no menu and no banner, and the tree
    /// the next build would make has both. Every consumer of that layout
    /// still walks the tree that made it — the operation the runtime runs,
    /// the draw, the cursor query — and the menu and the banner open at the
    /// next update, under the relayout that update runs.
    #[test]
    fn inputs_the_app_writes_between_frames_wait_for_the_next_update() {
        use iced::advanced::clipboard;
        use iced::keyboard;
        use iced_test::runtime::user_interface::{self, UserInterface};

        struct Texts(Vec<String>);
        impl Operation for Texts {
            fn traverse(&mut self, visit: &mut dyn FnMut(&mut dyn Operation)) {
                visit(self);
            }
            fn text(&mut self, _: Option<&iced::widget::Id>, _: Rectangle, text: &str) {
                self.0.push(text.to_owned());
            }
        }
        fn texts(
            ui: &mut UserInterface<'_, Value, iced::Theme, iced::Renderer>,
            renderer: &iced::Renderer,
        ) -> Vec<String> {
            let mut probe = Texts(Vec::new());
            ui.operate(renderer, &mut probe);
            probe.0
        }

        // the directory is the process's: this test's turn on it starts
        // empty, so the first build has nobody to complete to
        let room = "net\u{1f}mention-late-inputs";
        let names = crate::backend::seed_names(NameDirectory::empty());
        let composer = Element::<Value>::new(Composer {
            slot: slot(room),
            scope: room.into(),
            kind: "message".into(),
            compact: false,
            hint: String::new(),
            blocked: false,
            restore_blocked: false,
            failed_note: "not sent".into(),
        });
        let mut renderer = crate::frame_probe::headless_renderer();
        let size = Size::new(600.0, 300.0);
        let mut clipboard = clipboard::Null;
        let mut published: Vec<Value> = Vec::new();
        let mut ui =
            UserInterface::build(composer, size, user_interface::Cache::new(), &mut renderer);
        let position = iced::Point::new(200.0, 30.0);
        let cursor = mouse::Cursor::Available(position);
        ui.update(
            &[
                Event::Mouse(mouse::Event::CursorMoved { position }),
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
                Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Character("@".into()),
                    modified_key: keyboard::Key::Character("@".into()),
                    physical_key: keyboard::key::Physical::Code(keyboard::key::Code::Digit2),
                    location: keyboard::Location::Standard,
                    modifiers: keyboard::Modifiers::SHIFT,
                    text: Some("@".into()),
                    repeat: false,
                }),
            ],
            cursor,
            &mut renderer,
            &mut clipboard,
            &mut published,
        );
        assert_eq!(testing::text(room), "@");
        let before = texts(&mut ui, &renderer);
        assert!(
            !before
                .iter()
                .any(|text| text.starts_with('@') || text == "Restore"),
            "nobody to complete to yet, nothing handed back: {before:?}"
        );

        // between frames: the roster and the directory arrive, and a send
        // hands its body back
        seat_roster(room);
        names.seat(two_accounts());
        unsent(room, "lost words", false);

        // every consumer of the laid-out tree still walks that tree
        let after_write = texts(&mut ui, &renderer);
        assert_eq!(
            after_write, before,
            "the operation sees the tree that was laid out"
        );
        ui.draw(
            &mut renderer,
            &iced::Theme::Dark,
            &renderer::Style::default(),
            cursor,
        );

        // the next update takes the inputs in and relayouts over them
        ui.update(
            &[Event::Window(iced::window::Event::RedrawRequested(
                std::time::Instant::now(),
            ))],
            cursor,
            &mut renderer,
            &mut clipboard,
            &mut published,
        );
        let after_update = texts(&mut ui, &renderer);
        assert!(
            after_update.iter().any(|text| text == "@cafe0123"),
            "the menu opened over the word: {after_update:?}"
        );
        assert!(
            after_update.iter().any(|text| text == "Restore"),
            "the banner offers the handed-back body: {after_update:?}"
        );
        ui.draw(
            &mut renderer,
            &iced::Theme::Dark,
            &renderer::Style::default(),
            cursor,
        );
        assert!(published.is_empty(), "no submit");
    }

    /// THE SHAPE THE PANIC CANNOT COME BACK THROUGH. A composer's document
    /// changes only where a layout follows before any walk of the tree: the
    /// runtime's build (`children`, `diff`) and the event walk (`update`,
    /// after the walk and before the diff that asks for the relayout). The
    /// walks over the laid-out tree never take the app's inputs in, and the
    /// element builders read the document alone — never a fact store, never
    /// the slot. Both host composers hold the shape; an edit that breaks it
    /// fails here, not under a reader's `@`.
    #[test]
    fn only_a_build_point_takes_the_apps_inputs_in() {
        fn method<'a>(widget_impl: &'a str, name: &str) -> &'a str {
            let start = [format!("\n    fn {name}("), format!("\n    fn {name}<")]
                .iter()
                .find_map(|head| widget_impl.find(head.as_str()))
                .unwrap_or_else(|| panic!("the widget impl has `fn {name}`"));
            let rest = &widget_impl[start + 1..];
            let end = rest[4..].find("\n    fn ").map_or(rest.len(), |at| at + 4);
            &rest[..end]
        }
        fn builder<'a>(source: &'a str, name: &str) -> &'a str {
            let start = source
                .find(&format!("    fn {name}"))
                .unwrap_or_else(|| panic!("a `fn {name}` builder"));
            let rest = &source[start..];
            let end = rest.find("\n    }\n").expect("the builder ends");
            &rest[..end]
        }
        let sources = [
            (
                "composer_surface.rs",
                include_str!("composer_surface.rs"),
                &["build", "menu"][..],
            ),
            (
                "shell_composer.rs",
                include_str!("shell_composer.rs"),
                &["build"][..],
            ),
        ];
        for (file, source, builders) in sources {
            let widget_impl = source
                .split("\nimpl Widget<")
                .nth(1)
                .unwrap_or_else(|| panic!("{file}: one `impl Widget`"));
            let widget_impl = &widget_impl[..widget_impl.find("\n}\n").expect("the impl ends")];
            for name in ["children", "diff", "update"] {
                assert!(
                    method(widget_impl, name).contains("take_inputs("),
                    "{file}: `{name}` builds a tree a layout follows, so it takes the app's inputs in"
                );
            }
            for name in ["layout", "draw", "operate", "mouse_interaction", "overlay"] {
                assert!(
                    !method(widget_impl, name).contains("take_inputs("),
                    "{file}: `{name}` walks the laid-out tree, so it never changes the document"
                );
            }
            let update = method(widget_impl, "update");
            let walk = update.find(".update(").expect("the event walk");
            let take = update.find("take_inputs(").expect("the take-in");
            let diff = update.find("diff_document(").expect("the diff");
            let take_in_lands_between_the_walk_and_the_diff = walk < take && take < diff;
            assert!(
                take_in_lands_between_the_walk_and_the_diff,
                "{file}: `update` takes the inputs in after the walk and before the diff"
            );
            for name in builders {
                let body = builder(source, name);
                for store in [
                    "names(",
                    "names_at(",
                    "ROSTERS",
                    "handles(",
                    "lock(",
                    "slot(",
                ] {
                    assert!(
                        !body.contains(store),
                        "{file}: `{name}` reads `{store}`; a builder reads the document alone"
                    );
                }
            }
        }
    }

    #[test]
    fn a_room_scope_is_its_own_and_a_thread_scope_names_its_room() {
        assert_eq!(room_scope("net\u{1f}general"), "net\u{1f}general");
        assert_eq!(room_scope("net\u{1f}general#42"), "net\u{1f}general");
        assert_eq!(
            room_scope("net\u{1f}forge:playground:1#7"),
            "net\u{1f}forge:playground:1"
        );
        // a `#` that is not a thread tail stays
        assert_eq!(room_scope("net\u{1f}room#x"), "net\u{1f}room#x");
    }
}
