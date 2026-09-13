use ducktape_view_guest::{Subscription, Task, wire};
type KeyRelease = wire::keyboard::KeyState;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum CommentsMode {
    Beside,
    Squeeze,
    Inline,
}
#[derive(serde::Serialize, serde::Deserialize)]
pub struct PagesView {
    pub(crate) pointer_y: f64,
    pub(crate) comment_anchor_y: f64,
    pub(crate) comment_anchor_line: i64,
    pub(crate) comments_card_height: f64,
    pub(crate) document_reserve: crate::editor_view::EditorReserve,
    pub(crate) document_focused: bool,
    pub(crate) focus_query: i64,
    pub(crate) document_paint: crate::editor_view::PreparedPresentation,
    #[serde(with = "document_snapshot")]
    pub(crate) document: ducktape_view_guest::Editor,
    pub(crate) document_history: crate::editor_binding::HistoryState,
    pub(crate) document_menu: crate::editor_binding::MenuState,
    pub(crate) document_error: String,
    pub(crate) document_dark: bool,
    pub(crate) document_commented: Vec<i64>,
    pub(crate) document_marks: Vec<crate::document_sync::CommentMark>,
    pub(crate) connected: bool,
    pub(crate) chain: String,
    pub(crate) route_serial: i64,
    pub(crate) register_serial: i64,
    pub(crate) loading: bool,
    pub(crate) busy: bool,
    pub(crate) host_error: String,
    pub(crate) page_link: String,
    pub(crate) pages: Vec<crate::host::PageItem>,
    pub(crate) blocks: Vec<crate::document_sync::PageBlock>,
    pub(crate) pages_viewport_width: f64,
    pub(crate) pages_viewport_height: f64,
    pub(crate) pages_pane_width: f64,
    pub(crate) sidebar_width: f64,
    pub(crate) page_menu_open: bool,
    pub(crate) page_create_open: bool,
    pub(crate) active_page: String,
    pub(crate) active_page_title: String,
    pub(crate) active_page_parent: String,
    pub(crate) page_searching: bool,
    pub(crate) page_search_hits: Vec<crate::host::PageSearchHit>,
    pub(crate) page_search_query: String,
    pub(crate) page_search_serial: i64,
    pub(crate) page_delete_armed: bool,
    pub(crate) autosave: String,
    pub(crate) page_refusal: String,
    pub(crate) subpages: Vec<crate::host::Subpage>,
    pub(crate) orphaned_comment_drafts: Vec<String>,
    pub(crate) block_comments_open: bool,
    pub(crate) scope_target: String,
    pub(crate) scope_pinned: bool,
    pub(crate) thread_total: i64,
    pub(crate) comment_rows: Vec<crate::host::PageCommentThreadRow>,
    pub(crate) threads_loading: bool,
    pub(crate) commented_hits: Vec<String>,
    pub(crate) reply_thread: String,
    pub(crate) expanded_threads: Vec<String>,
    pub(crate) resolved_open: bool,
    pub(crate) page_draft: String,
    pub(crate) page_search_draft: String,
    pub(crate) block_comment_draft: String,
    pub(crate) reply_draft: String,
    pub(crate) pending_page: String,
    pub(crate) pending_comment: String,
    pub(crate) page_saved_text: String,
    pub(crate) buffer_page: String,
    pub(crate) page_inflight_text: String,
    pub(crate) sent: bool,
}
impl ::std::fmt::Debug for PagesView {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("PagesView")
    }
}
#[derive(Clone)]
pub enum Message {
    SessionArrived(crate::host::SessionItem),
    CommentPointerMoved(f64, f64),
    ChoosePage(String),
    RegisterArrived(crate::host::RegisterItem),
    SearchArrived(crate::host::SearchItem),
    ActDone(crate::host::ActItem),
    SaveDone(crate::host::SaveItem),
    PageAutosaveTick,
    TogglePageCreate,
    CreatePageSubmit,
    ArmPageDelete,
    DisarmPageDelete,
    DeletePageSubmit,
    SearchPagesSubmit,
    ClearPageSearch,
    OpenPageSearchHit(String, String),
    UseOrphanedCommentDraft(String),
    DiscardOrphanedCommentDraft(String),
    ToggleBlockComments,
    CloseBlockComments,
    NarrowCommentScope(String),
    WidenCommentScope,
    ResolveThreadSubmit(String, bool),
    SelectReplyThread(String),
    ToggleThreadReplies(String),
    ToggleResolvedComments,
    PostThreadReply(String),
    PostBlockCommentSubmit,
    CopyToClipboard(String, String),
    DocumentPointerReleased(wire::mouse::Button),
    DocumentKeyReleased(KeyRelease),
    DocumentWindowFocused,
    DocumentWindowUnfocused,
    DocumentFocusChecked(i64, bool),
    SidebarResized(f64, f64),
    PagesViewportChanged(f64, f64),
    PagesPaneResized(f64, f64),
    CommentsCardMeasured(f64, f64),
    TogglePageMenu,
    ClosePageMenu,
    DocumentCommitted(crate::editor_binding::EditorUpdate),
    PageDraftChanged(String),
    SearchDraftChanged(String),
    ReplyDraftChanged(String),
    CommentDraftChanged(String),
    DocumentUpdated(::ducktape_view_guest::EditorDocumentUpdate),
    DocumentTransaction(::ducktape_view_guest::EditorTransaction<Message>),
}
impl ::std::fmt::Debug for Message {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Message")
    }
}
impl PagesView {
    fn initial_state() -> Self {
        Self {
            pointer_y: 0.0,
            comment_anchor_y: -1.0,
            comment_anchor_line: 0,
            comments_card_height: 0.0,
            document_reserve: crate::editor_view::no_reserve(),
            document_focused: false,
            focus_query: 0,
            document_paint: crate::editor_view::empty_presentation(),
            document: ::ducktape_view_guest::Editor::new("".to_owned()),
            document_history: crate::editor_binding::initial_history(),
            document_menu: crate::editor_binding::initial_menu(),
            document_error: "".to_owned(),
            document_dark: false,
            document_commented: Vec::new(),
            document_marks: Vec::new(),
            connected: false,
            chain: "".to_owned(),
            route_serial: 0,
            register_serial: 0,
            loading: false,
            busy: false,
            host_error: "".to_owned(),
            page_link: "".to_owned(),
            pages: Vec::new(),
            blocks: Vec::new(),
            pages_viewport_width: 1280.0,
            pages_viewport_height: 700.0,
            pages_pane_width: 1280.0,
            sidebar_width: 230.0,
            page_menu_open: false,
            page_create_open: false,
            active_page: "".to_owned(),
            active_page_title: "".to_owned(),
            active_page_parent: "".to_owned(),
            page_searching: false,
            page_search_hits: Vec::new(),
            page_search_query: "".to_owned(),
            page_search_serial: 0,
            page_delete_armed: false,
            autosave: "idle".to_owned(),
            page_refusal: "".to_owned(),
            subpages: Vec::new(),
            orphaned_comment_drafts: Vec::new(),
            block_comments_open: false,
            scope_target: "".to_owned(),
            scope_pinned: false,
            thread_total: 0,
            comment_rows: Vec::new(),
            threads_loading: false,
            commented_hits: Vec::new(),
            reply_thread: "".to_owned(),
            expanded_threads: Vec::new(),
            resolved_open: false,
            page_draft: "".to_owned(),
            page_search_draft: "".to_owned(),
            block_comment_draft: "".to_owned(),
            reply_draft: "".to_owned(),
            pending_page: "".to_owned(),
            pending_comment: "".to_owned(),
            page_saved_text: "".to_owned(),
            buffer_page: "".to_owned(),
            page_inflight_text: "".to_owned(),
            sent: false,
        }
    }
    pub(crate) fn boot() -> (Self, Task<Message>) {
        (Self::initial_state(), Task::none())
    }
    pub(crate) const PREFERRED_WINDOW_SIZE: &'static str = "none";
    const SNAPSHOT_SCHEMA: &'static str =
        "a8e6006820699ce3ada3eec58c7aa178e96d4b16c4d5476a8246c97381857e71";
    pub(crate) fn snapshot(&self) -> Result<Vec<u8>, String> {
        wire::Snapshot {
            schema: Self::SNAPSHOT_SCHEMA.into(),
            state: wire::SnapshotValue::Bytes(wire::encode(self)),
        }
        .encode()
    }

    pub(crate) fn restore(bytes: &[u8]) -> Result<Self, String> {
        let snapshot = wire::Snapshot::decode(bytes)?;
        let wire::SnapshotValue::Bytes(state) = snapshot.state else {
            return Err("invalid Pages snapshot".into());
        };
        if snapshot.schema != Self::SNAPSHOT_SCHEMA {
            return Err("invalid Pages snapshot schema".into());
        }
        wire::decode(&state)
    }
}

mod document_snapshot {
    use serde::{Deserialize, Serialize};

    pub fn serialize<S: serde::Serializer>(
        editor: &ducktape_view_guest::Editor,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        editor.snapshot().serialize(serializer)
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<ducktape_view_guest::Editor, D::Error> {
        let bytes = Vec::<u8>::deserialize(deserializer)?;
        ducktape_view_guest::Editor::restore(&bytes)
            .ok_or_else(|| serde::de::Error::custom("invalid document snapshot"))
    }
}

impl PagesView {
    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            ::ducktape_view_guest::mouse::observe(Subscription::filter_events(
                |event| match event {
                    wire::Event::Mouse {
                        event: wire::mouse::Event::CursorMoved { x, y },
                        ..
                    } => Some(Message::CommentPointerMoved(*x as f64, *y as f64)),
                    _ => None,
                },
            )),
            ::ducktape_view_guest::mouse::observe(Subscription::filter_events(
                |event| match event {
                    wire::Event::Mouse {
                        event: wire::mouse::Event::ButtonReleased(button),
                        ..
                    } => Some(Message::DocumentPointerReleased(*button)),
                    _ => None,
                },
            )),
            Subscription::filter_events(|event| match event {
                wire::Event::Keyboard {
                    event: wire::keyboard::Event::Release(key),
                    ..
                } => Some(Message::DocumentKeyReleased(key.clone())),
                _ => None,
            }),
            ::ducktape_view_guest::events::observe(
                Subscription::filter_events(|event| match event {
                    wire::Event::Observation {
                        event: wire::events::Event::Window(wire::events::Window::Focused),
                        ..
                    } => Some(Message::DocumentWindowFocused),
                    _ => None,
                }),
                wire::events::Interest {
                    focus: true,
                    ..Default::default()
                },
            ),
            ::ducktape_view_guest::events::observe(
                Subscription::filter_events(|event| match event {
                    wire::Event::Observation {
                        event: wire::events::Event::Window(wire::events::Window::Unfocused),
                        ..
                    } => Some(Message::DocumentWindowUnfocused),
                    _ => None,
                }),
                wire::events::Interest {
                    focus: true,
                    ..Default::default()
                },
            ),
            crate::host::session().map(move |value| Message::SessionArrived(value)),
            if self.connected {
                Subscription::batch([crate::host::register(
                    self.active_page.to_owned(),
                    self.register_serial,
                )
                .map(move |value| Message::RegisterArrived(value))])
            } else {
                Subscription::none()
            },
            if self.connected && (!(self.page_search_query).is_empty()) {
                Subscription::batch([crate::host::search(
                    self.page_search_query.to_owned(),
                    self.page_search_serial,
                )
                .map(move |value| Message::SearchArrived(value))])
            } else {
                Subscription::none()
            },
            crate::host::acts().map(move |value| Message::ActDone(value)),
            crate::host::saves().map(move |value| Message::SaveDone(value)),
            if (((self.connected && (!self.loading)) && (!self.busy))
                && (!(self.active_page).is_empty()))
                && (self.active_page == self.buffer_page)
            {
                Subscription::batch([::ducktape_view_guest::every(
                    ::std::time::Duration::from_millis(900),
                )
                .map(move |_value| Message::PageAutosaveTick)])
            } else {
                Subscription::none()
            },
        ])
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordinary_state_preserves_document_and_drafts_through_snapshot() {
        let (mut app, _) = PagesView::boot();
        app.document = ducktape_view_guest::Editor::new("# 제목\n\nDocument 🙂");
        app.page_draft = "새 페이지".into();
        app.block_comment_draft = "Unsent comment".into();
        app.reply_draft = "Unsent reply".into();
        app.expanded_threads = vec!["thread-a".into()];
        app.document_history.snapshot = vec![1, 2, 3];
        app.document_menu.snapshot = vec![4, 5];
        app.page_search_draft = "Unsubmitted search".into();
        app.orphaned_comment_drafts = vec!["Recovered draft".into()];
        app.pending_page = "Submitted title".into();
        app.pending_comment = "Submitted comment".into();
        app.page_inflight_text = "Submitted document".into();
        app.page_saved_text = "Acknowledged document".into();
        app.scope_target = "block-a".into();
        app.reply_thread = "thread-a".into();
        app.block_comments_open = true;
        app.pages_pane_width = 1060.;
        let snapshot = app.snapshot().unwrap();
        let restored = PagesView::restore(&snapshot).unwrap();
        assert_eq!(restored.snapshot().unwrap(), snapshot);
    }
    #[test]
    fn kit_composition_retains_editor_and_comment_routes_without_custom_control_faces() {
        let (mut app, _) = PagesView::boot();
        app.connected = true;
        app.active_page = "page-a".into();
        app.buffer_page = app.active_page.clone();
        app.page_create_open = true;
        app.block_comments_open = true;
        app.comment_rows = vec![crate::host::PageCommentThreadRow {
            thread: crate::host::PageCommentThread {
                id: "thread-a".into(),
                ..Default::default()
            },
            ..Default::default()
        }];
        app.reply_thread = "thread-a".into();
        let mut editor = false;
        let mut inputs = Vec::new();
        app.view().for_each_mut(&mut |node| match node {
            Node::Editor {
                key,
                editable,
                options,
                ..
            } => {
                assert_eq!(key, &format!("{PAGE_KEY}/document"));
                assert!(*editable);
                assert!(options.binding.is_some());
                assert!(options.presentation.is_some());
                assert_eq!(options.style, wire::InputStyle::default());
                editor = true;
            }
            Node::Input {
                key,
                options,
                style,
                ..
            } => {
                assert!(!options.disabled);
                assert_eq!(**style, wire::InputStyle::default());
                inputs.push(key.clone());
            }
            Node::Button { style, .. } => {
                let preset = style.preset;
                assert_eq!(
                    *style,
                    wire::ButtonStyle {
                        preset,
                        ..Default::default()
                    }
                );
            }
            _ => {}
        });
        assert!(editor);
        assert!(inputs.contains(&format!("{PAGE_KEY}/new-page")));
        assert!(inputs.contains(&format!("{PAGE_KEY}/page-search")));
        assert!(inputs.contains(&format!("{PAGE_KEY}/page-comment(page-a)")));
        assert!(inputs.contains(&format!("{PAGE_KEY}/thread-reply(thread-a)")));
        app.loading = true;
        let Node::Editor { editable, .. } = app.document_editor() else {
            unreachable!()
        };
        assert!(!editable, "loading must not edit the previous page");
    }
    #[test]
    fn measured_pane_changes_the_guest_document_width() {
        let (mut app, _) = PagesView::boot();
        app.connected = true;
        app.active_page = "page".into();
        app.block_comments_open = true;
        let _ = app.update(Message::PagesPaneResized(1060., 700.));
        assert_eq!(app.pages_pane_width, 1060.);
        let mut constrained = false;
        app.view().for_each_mut(&mut |node| {
            if let wire::Node::Container {
                max_width: Some(width),
                ..
            } = node
            {
                constrained |= *width == 688.;
            }
        });
        assert!(
            constrained,
            "the guest must publish the measured squeeze width"
        );
    }
    #[test]
    fn view_fits_default_stack() {
        ::std::thread::Builder::new()
            .stack_size(4 * 1024 * 1024)
            .spawn(|| {
                let (app, _) = PagesView::boot();
                let _ = app.view();
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
include!("app_update.rs");
include!("app_view.rs");
include!("kit.rs");
include!("pages.rs");
include!("rows.rs");
