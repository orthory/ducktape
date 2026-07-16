//! Capability-free Pages editor.
//!
//! The node wire stays in `screen_service`; this module owns the complete
//! Pages editing contract, including UTF-16 ranges and IME-aware draft state.

use iced::widget::text_editor;

use crate::view_api::{Resource, fresh_id};

mod view;

pub use view::view;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageMeta {
    pub id: String,
    pub title: String,
    pub parent: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    Paragraph,
    Heading1,
    Heading2,
    Heading3,
    Bulleted,
    Numbered,
    Todo,
    Toggle,
    Quote,
    Code,
    Callout,
    Divider,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineMark {
    Bold,
    Italic,
    Underline,
    Strikethrough,
    Code,
}

impl InlineMark {
    const fn label(self) -> &'static str {
        match self {
            Self::Bold => "B",
            Self::Italic => "I",
            Self::Underline => "U",
            Self::Strikethrough => "S",
            Self::Code => "Code",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelativeAnchor {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpanMark {
    pub start: usize,
    pub end: usize,
    pub kind: InlineMark,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageBlock {
    pub id: String,
    pub kind: BlockKind,
    pub text: String,
    pub depth: usize,
    pub checked: bool,
    pub parent: String,
    pub children: Vec<String>,
    pub marks: Vec<SpanMark>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageComment {
    pub id: String,
    pub author: String,
    pub author_key: Option<String>,
    pub text: String,
    pub deleted: bool,
    pub edited: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageCommentThread {
    pub id: String,
    pub target: String,
    pub anchor: Option<RelativeAnchor>,
    pub resolved: bool,
    pub comments: Vec<PageComment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PagePresence {
    pub peer: String,
    pub block: Option<String>,
    pub anchor: usize,
    pub head: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageDocument {
    pub id: String,
    pub title: String,
    pub ancestry: Vec<PageMeta>,
    pub blocks: Vec<PageBlock>,
    pub page_comments: usize,
    pub comment_threads: Vec<PageCommentThread>,
    pub presence: Vec<PagePresence>,
    pub self_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PagesData {
    pub pages: Vec<PageMeta>,
    pub open_tabs: Vec<String>,
    pub document: Option<PageDocument>,
}

#[derive(Debug, Clone)]
struct EditorState {
    content: text_editor::Content,
    committed: String,
}

impl EditorState {
    fn new(text: &str) -> Self {
        Self {
            content: text_editor::Content::with_text(text),
            committed: text.to_owned(),
        }
    }

    fn text(&self) -> String {
        self.content.text()
    }

    fn dirty(&self) -> bool {
        self.text() != self.committed
    }

    fn clear(&mut self) {
        self.content = text_editor::Content::new();
        self.committed.clear();
    }
}

impl Default for EditorState {
    fn default() -> Self {
        Self::new("")
    }
}

impl PartialEq for EditorState {
    fn eq(&self, other: &Self) -> bool {
        self.text() == other.text() && self.committed == other.committed
    }
}

impl Eq for EditorState {}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CommentTarget {
    New {
        target: String,
        anchor: Option<RelativeAnchor>,
    },
    Reply {
        thread: String,
        target: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageEdit {
    pub block: String,
    pub before: String,
    pub after: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub data: Resource<PagesData>,
    pub query: String,
    pub error: Option<String>,
    pub collapsed_pages: Vec<String>,
    pub collapsed_blocks: Vec<String>,
    pub slash_for: Option<usize>,
    pub paste_dropped: usize,
    pub focused_block: Option<String>,
    pub undo: Vec<PageEdit>,
    pub redo: Vec<PageEdit>,
    pub edit_generation: u64,
    pub dirty_block: Option<(String, u64)>,
    pub dragging_block: Option<usize>,
    pub drag_hover: Option<usize>,
    pub pending_block_delete: Option<String>,
    pub pending_page_delete: bool,
    // Which block the cursor is over (reveals its hover gutter) and which
    // block's actions menu is open. Both are id-keyed, so a stale id after a
    // reorder simply matches nothing.
    pub hovered_block: Option<String>,
    pub menu_open_block: Option<String>,
    editors: Vec<(String, EditorState)>,
    comment_draft: EditorState,
    comment_target: Option<CommentTarget>,
    editing_comment: Option<(String, EditorState)>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            data: Resource::Loading,
            query: String::new(),
            error: None,
            collapsed_pages: Vec::new(),
            collapsed_blocks: Vec::new(),
            slash_for: None,
            paste_dropped: 0,
            focused_block: None,
            undo: Vec::new(),
            redo: Vec::new(),
            edit_generation: 0,
            dirty_block: None,
            dragging_block: None,
            drag_hover: None,
            pending_block_delete: None,
            pending_page_delete: false,
            hovered_block: None,
            menu_open_block: None,
            editors: Vec::new(),
            comment_draft: EditorState::default(),
            comment_target: None,
            editing_comment: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    QueryChanged(String),
    NewPage,
    DismissError,
    Refresh,
    OpenPage(String),
    OpenPageAt { page: String, block: String },
    CloseTab(String),
    TitleChanged(String),
    CommitTitle,
    BlockAction(usize, text_editor::Action),
    BlockEnter(usize),
    BlockBackspace(usize),
    CommitBlockIf { block: String, generation: u64 },
    Undo,
    Redo,
    SetBlockKind(usize, BlockKind),
    ToggleChecked(usize),
    RequestRemoveBlock(usize),
    ConfirmRemoveBlock,
    CancelRemoveBlock,
    AddBlock(BlockKind),
    CreateChildPage,
    RequestDeletePage,
    ConfirmDeletePage,
    CancelDeletePage,
    TogglePageCollapsed(String),
    ToggleBlockCollapsed(usize),
    ApplySlash(usize, BlockKind),
    ToggleMark(usize, InlineMark),
    MoveBlock(usize, BlockMove),
    MoveFocusedBlock(BlockMove),
    RemoveEmptyFocusedBlock,
    ActivateFocusedBlock,
    CycleTab(bool),
    CloseActiveTab,
    BeginBlockDrag(usize),
    HoverBlock(usize),
    BlockRowExited(usize),
    ToggleBlockMenu(usize),
    DropDraggedBlock,
    PasteFromClipboard(usize),
    PasteBlocks(usize, String),
    CommentOnBlock(usize),
    ReplyToThread(String, String),
    CommentAction(text_editor::Action),
    AddComment,
    ResolveComment(String, bool),
    DeleteComment(String),
    BeginCommentEdit(String, String),
    CommentEditAction(text_editor::Action),
    CommitCommentEdit,
    CancelCommentEdit,
    // Re-parenting a page is a rail-tree drag gesture (not yet ported); the
    // per-document parent picker was removed (G6). The effect/command plumbing
    // below stays so that drag can emit this once the rail tree grows it.
    #[allow(dead_code)]
    SetPageParent(Option<String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockMove {
    Up,
    Down,
    Indent,
    Outdent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    CreatePage {
        parent: Option<String>,
    },
    LoadPages {
        active: Option<String>,
        open_tabs: Vec<String>,
    },
    LoadPage(String),
    RenamePage {
        page: String,
        title: String,
    },
    SaveBlock {
        page: String,
        block: PageBlock,
    },
    SetBlockKind {
        block: String,
        kind: BlockKind,
    },
    ApplySlash {
        block: String,
        kind: BlockKind,
        text: String,
    },
    SetBlockChecked {
        block: String,
        checked: bool,
    },
    RemoveBlock(String),
    AddBlock {
        page: String,
        kind: BlockKind,
    },
    SplitBlock {
        page: String,
        left: PageBlock,
        right: PageBlock,
        thread_moves: Vec<ThreadMove>,
    },
    MergeBlock {
        page: String,
        destination: PageBlock,
        source: PageBlock,
        thread_moves: Vec<ThreadMove>,
    },
    DeletePage(String),
    SetPageParent {
        page: String,
        parent: Option<String>,
    },
    SetSpanMark {
        block: String,
        start: usize,
        end: usize,
        kind: InlineMark,
        active: bool,
    },
    MoveBlock {
        block: String,
        parent: String,
        after: Option<String>,
    },
    PasteBlocks {
        parent: String,
        after: Option<String>,
        blocks: Vec<(BlockKind, String, bool)>,
    },
    ReadClipboard(usize),
    FocusBlock(String),
    CommitAfter {
        block: String,
        generation: u64,
    },
    AddComment {
        thread: String,
        comment: String,
        target: String,
        anchor: Option<RelativeAnchor>,
        text: String,
    },
    ResolveComment {
        thread: String,
        resolved: bool,
    },
    DeleteComment(String),
    EditComment {
        comment: String,
        text: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadMove {
    pub thread: String,
    pub target: String,
    pub anchor: Option<RelativeAnchor>,
}

impl State {
    #[cfg(test)]
    pub fn document(&self) -> Option<&PageDocument> {
        page_document(self)
    }

    pub fn loaded(&mut self, result: Result<Option<PagesData>, String>) {
        match result {
            Ok(Some(mut data)) => {
                if let Some(document) = &mut data.document {
                    self.merge_document(document);
                }
                self.data = Resource::Ready(data);
            }
            Ok(None) => self.data = Resource::Empty,
            Err(error) => self.data = Resource::Error(error),
        }
    }

    pub fn document_loaded(&mut self, result: Result<PageDocument, String>) -> Option<Effect> {
        let mut document = match result {
            Ok(document) => document,
            Err(error) => {
                self.error = Some(error);
                return None;
            }
        };
        self.merge_document(&mut document);
        let focus = self
            .focused_block
            .clone()
            .filter(|id| document.blocks.iter().any(|block| &block.id == id));
        if let Resource::Ready(data) = &mut self.data {
            if !data.open_tabs.contains(&document.id) {
                data.open_tabs.push(document.id.clone());
            }
            data.document = Some(document);
        }
        if let Some(block) = focus {
            reveal_page_block(self, &block);
            Some(Effect::FocusBlock(block))
        } else {
            None
        }
    }

    fn merge_document(&mut self, incoming: &mut PageDocument) {
        let old_editors = std::mem::take(&mut self.editors);
        let mut next = Vec::with_capacity(incoming.blocks.len());
        let mut text_rebases = Vec::new();
        for block in &mut incoming.blocks {
            let mut editor = old_editors
                .iter()
                .find(|(id, _)| id == &block.id)
                .map(|(_, editor)| editor.clone())
                .unwrap_or_else(|| EditorState::new(&block.text));
            let draft = editor.text();
            if block.text == draft {
                editor.committed.clone_from(&block.text);
            } else if editor.dirty() {
                let server_text = block.text.clone();
                block.marks = rebase_marks(&server_text, &draft, &block.marks);
                block.text = draft.clone();
                text_rebases.push((block.id.clone(), server_text, draft));
            } else {
                editor = EditorState::new(&block.text);
            }
            next.push((block.id.clone(), editor));
        }
        for thread in &mut incoming.comment_threads {
            if let Some((_, old, new)) = text_rebases.iter().find(|(id, _, _)| id == &thread.target)
                && let Some(anchor) = thread.anchor
            {
                thread.anchor = Some(rebase_range(old, new, anchor));
            }
        }
        self.editors = next;
    }

    pub fn cursor_presence(&self) -> (Option<String>, usize, usize) {
        let Some(id) = self.focused_block.as_ref() else {
            return (None, 0, 0);
        };
        let Some((_, editor)) = self.editors.iter().find(|(block, _)| block == id) else {
            return (Some(id.clone()), 0, 0);
        };
        let (anchor, head) = editor_cursor_utf16(editor);
        (Some(id.clone()), anchor, head)
    }
}

pub fn update(state: &mut State, message: Message) -> Option<Effect> {
    state.error = None;
    match message {
        Message::QueryChanged(value) => {
            state.query = value;
            None
        }
        Message::NewPage => Some(begin_page_create(state, None)),
        Message::DismissError => {
            state.error = None;
            None
        }
        Message::Refresh => {
            let (active, open_tabs) = location(state);
            Some(Effect::LoadPages { active, open_tabs })
        }
        Message::OpenPage(id) => {
            reset_page_transients(state);
            if let Resource::Ready(data) = &mut state.data
                && !data.open_tabs.contains(&id)
            {
                data.open_tabs.push(id.clone());
            }
            Some(Effect::LoadPage(id))
        }
        Message::OpenPageAt { page, block } => {
            reset_page_transients(state);
            state.focused_block = Some(block);
            if let Resource::Ready(data) = &mut state.data
                && !data.open_tabs.contains(&page)
            {
                data.open_tabs.push(page.clone());
            }
            Some(Effect::LoadPage(page))
        }
        Message::CloseTab(id) => {
            if let Resource::Ready(data) = &mut state.data {
                data.open_tabs.retain(|tab| tab != &id);
                if data
                    .document
                    .as_ref()
                    .is_some_and(|document| document.id == id)
                {
                    data.document = None;
                }
            }
            None
        }
        Message::TitleChanged(value) => {
            page_document_mut(state)?.title = value;
            None
        }
        Message::CommitTitle => {
            let document = page_document(state)?;
            Some(Effect::RenamePage {
                page: document.id.clone(),
                title: document.title.clone(),
            })
        }
        Message::BlockAction(index, action) => edit_block(state, index, action),
        Message::BlockEnter(index) => enter_block(state, index),
        Message::BlockBackspace(index) => backspace_block(state, index),
        Message::CommitBlockIf { block, generation } => {
            if state.dirty_block.as_ref() != Some(&(block.clone(), generation)) {
                return None;
            }
            let index = page_document(state)?
                .blocks
                .iter()
                .position(|candidate| candidate.id == block)?;
            commit_block(state, index)
        }
        Message::Undo => apply_page_edit(state, false),
        Message::Redo => apply_page_edit(state, true),
        Message::SetBlockKind(index, kind) => {
            state.menu_open_block = None;
            let block = page_document(state)?.blocks.get(index)?;
            Some(Effect::SetBlockKind {
                block: block.id.clone(),
                kind,
            })
        }
        Message::ToggleChecked(index) => {
            let block = page_document(state)?.blocks.get(index)?;
            Some(Effect::SetBlockChecked {
                block: block.id.clone(),
                checked: !block.checked,
            })
        }
        Message::RequestRemoveBlock(index) => {
            state.menu_open_block = None;
            state.pending_block_delete = Some(page_document(state)?.blocks.get(index)?.id.clone());
            None
        }
        Message::ConfirmRemoveBlock => {
            Some(Effect::RemoveBlock(state.pending_block_delete.take()?))
        }
        Message::CancelRemoveBlock => {
            state.pending_block_delete = None;
            None
        }
        Message::AddBlock(kind) => Some(Effect::AddBlock {
            page: page_document(state)?.id.clone(),
            kind,
        }),
        Message::CreateChildPage => {
            let parent = page_document(state)?.id.clone();
            Some(begin_page_create(state, Some(parent)))
        }
        Message::RequestDeletePage => {
            page_document(state)?;
            state.pending_page_delete = true;
            None
        }
        Message::ConfirmDeletePage => {
            state.pending_page_delete = false;
            Some(Effect::DeletePage(page_document(state)?.id.clone()))
        }
        Message::CancelDeletePage => {
            state.pending_page_delete = false;
            None
        }
        Message::TogglePageCollapsed(id) => {
            toggle_id(&mut state.collapsed_pages, &id);
            None
        }
        Message::ToggleBlockCollapsed(index) => {
            let id = page_document(state)?.blocks.get(index)?.id.clone();
            toggle_id(&mut state.collapsed_blocks, &id);
            None
        }
        Message::ApplySlash(index, kind) => {
            let block = page_document(state)?.blocks.get(index)?.id.clone();
            set_editor_text(state, &block, "", true);
            if let Some(value) = page_document_mut(state)?.blocks.get_mut(index) {
                value.kind = kind;
                value.text.clear();
                value.marks.clear();
            }
            state.slash_for = None;
            state.dirty_block = None;
            Some(Effect::ApplySlash {
                block,
                kind,
                text: String::new(),
            })
        }
        Message::ToggleMark(index, kind) => toggle_mark(state, index, kind),
        Message::MoveBlock(index, movement) => move_block(state, index, movement),
        Message::MoveFocusedBlock(movement) => {
            let focused = state.focused_block.as_deref()?;
            let index = page_document(state)?
                .blocks
                .iter()
                .position(|block| block.id == focused)?;
            move_block(state, index, movement)
        }
        Message::RemoveEmptyFocusedBlock => remove_empty_focused(state),
        Message::ActivateFocusedBlock => activate_focused(state),
        Message::CycleTab(next) => cycle_tab(state, next),
        Message::CloseActiveTab => close_active_tab(state),
        Message::BeginBlockDrag(index) => {
            page_document(state)?.blocks.get(index)?;
            state.dragging_block = Some(index);
            state.drag_hover = Some(index);
            None
        }
        Message::HoverBlock(index) => {
            if let Some(id) = page_document(state)
                .and_then(|document| document.blocks.get(index))
                .map(|block| block.id.clone())
            {
                state.hovered_block = Some(id);
            }
            if state.dragging_block.is_some() {
                state.drag_hover = Some(index);
            }
            None
        }
        Message::BlockRowExited(index) => {
            let id = page_document(state)
                .and_then(|document| document.blocks.get(index))
                .map(|block| block.id.clone());
            // Only the row we're actually leaving clears the flag, so an
            // enter/exit pair while crossing between two rows can't blank the
            // gutter on the row we just entered.
            if id.is_some() && state.hovered_block == id {
                state.hovered_block = None;
            }
            None
        }
        Message::ToggleBlockMenu(index) => {
            let id = page_document(state)?.blocks.get(index)?.id.clone();
            state.menu_open_block = if state.menu_open_block.as_deref() == Some(&id) {
                None
            } else {
                Some(id)
            };
            None
        }
        Message::DropDraggedBlock => drop_dragged(state),
        Message::PasteFromClipboard(index) => Some(Effect::ReadClipboard(index)),
        Message::PasteBlocks(index, text) => paste(state, index, &text),
        Message::CommentOnBlock(index) => begin_block_comment(state, index),
        Message::ReplyToThread(thread, target) => {
            state.comment_target = Some(CommentTarget::Reply { thread, target });
            state.comment_draft.clear();
            None
        }
        Message::CommentAction(action) => {
            state.comment_draft.content.perform(action);
            None
        }
        Message::AddComment => add_comment(state),
        Message::ResolveComment(thread, resolved) => {
            Some(Effect::ResolveComment { thread, resolved })
        }
        Message::DeleteComment(comment) => Some(Effect::DeleteComment(comment)),
        Message::BeginCommentEdit(comment, text) => {
            state.editing_comment = Some((comment, EditorState::new(&text)));
            None
        }
        Message::CommentEditAction(action) => {
            state.editing_comment.as_mut()?.1.content.perform(action);
            None
        }
        Message::CommitCommentEdit => {
            let (comment, draft) = state.editing_comment.take()?;
            Some(Effect::EditComment {
                comment,
                text: nonempty(&draft.text())?,
            })
        }
        Message::CancelCommentEdit => {
            state.editing_comment = None;
            None
        }
        Message::SetPageParent(parent) => Some(Effect::SetPageParent {
            page: page_document(state)?.id.clone(),
            parent,
        }),
    }
}

fn edit_block(state: &mut State, index: usize, action: text_editor::Action) -> Option<Effect> {
    let (id, before) = page_document(state)
        .and_then(|document| document.blocks.get(index))
        .map(|block| (block.id.clone(), block.text.clone()))?;
    state.focused_block = Some(id.clone());
    let editor = editor_mut(state, &id, &before);
    editor.content.perform(action);
    let after = editor.text();
    if before == after {
        return None;
    }
    let marks = {
        let block = page_document(state)?.blocks.get(index)?;
        rebase_marks(&before, &after, &block.marks)
    };
    if let Some(block) = page_document_mut(state)?.blocks.get_mut(index) {
        block.text.clone_from(&after);
        block.marks = marks;
    }
    if let Some(document) = page_document_mut(state) {
        for thread in &mut document.comment_threads {
            if thread.target == id {
                thread.anchor = thread
                    .anchor
                    .map(|anchor| rebase_range(&before, &after, anchor));
            }
        }
    }
    state.slash_for = after.starts_with('/').then_some(index);
    state.undo.push(PageEdit {
        block: id.clone(),
        before,
        after,
    });
    if state.undo.len() > 128 {
        state.undo.remove(0);
    }
    state.redo.clear();
    state.edit_generation = state.edit_generation.wrapping_add(1);
    let generation = state.edit_generation;
    state.dirty_block = Some((id.clone(), generation));
    state.slash_for.is_none().then_some(Effect::CommitAfter {
        block: id,
        generation,
    })
}

fn enter_block(state: &mut State, index: usize) -> Option<Effect> {
    let block = page_document(state)?.blocks.get(index)?.clone();
    state.focused_block = Some(block.id.clone());
    if state.slash_for == Some(index) {
        let kind = slash_options(&block.text).first().copied()?;
        return update(state, Message::ApplySlash(index, kind));
    }
    if block.kind == BlockKind::Code {
        return edit_block(
            state,
            index,
            text_editor::Action::Edit(text_editor::Edit::Enter),
        );
    }
    if block.text.is_empty() && empty_enter_exits(block.kind) {
        return Some(Effect::SetBlockKind {
            block: block.id,
            kind: BlockKind::Paragraph,
        });
    }
    let (selection_start, selection_end) = editor_selection_bytes(state, &block.id, &block.text)
        .unwrap_or_else(|| {
            let end = block.text.len();
            (end, end)
        });
    let left_text = block.text[..selection_start].to_owned();
    let right_text = block.text[selection_end..].to_owned();
    let split_utf16 = utf16_len(&block.text[..selection_start]);
    let selection_end_utf16 = utf16_len(&block.text[..selection_end]);
    let left_marks = split_marks(&block.marks, split_utf16).0;
    let right_marks = split_marks(&block.marks, selection_end_utf16).1;
    let id = local_id("block");
    let right = PageBlock {
        id: id.clone(),
        kind: continuation_kind(block.kind),
        text: right_text,
        depth: block.depth,
        checked: false,
        parent: block.parent.clone(),
        children: Vec::new(),
        marks: right_marks,
    };
    let mut left = block.clone();
    left.text = left_text;
    left.marks = left_marks;
    let thread_moves = split_thread_moves(page_document(state)?, &block, &left, &right);
    set_editor_text(state, &left.id, &left.text, true);
    set_editor_text(state, &right.id, &right.text, true);
    let document = page_document_mut(state)?;
    document.blocks[index].clone_from(&left);
    document.blocks.insert(index + 1, right.clone());
    state.dirty_block = None;
    state.focused_block = Some(id);
    Some(Effect::SplitBlock {
        page: page_document(state)?.id.clone(),
        left,
        right,
        thread_moves,
    })
}

fn backspace_block(state: &mut State, index: usize) -> Option<Effect> {
    let block = page_document(state)?.blocks.get(index)?.clone();
    state.focused_block = Some(block.id.clone());
    let selection = editor_selection_bytes(state, &block.id, &block.text);
    if selection.is_some_and(|(start, end)| start != end || start != 0) {
        return edit_block(
            state,
            index,
            text_editor::Action::Edit(text_editor::Edit::Backspace),
        );
    }
    let cursor = editor_cursor_byte(state, &block.id, &block.text).unwrap_or(block.text.len());
    if cursor != 0 {
        return edit_block(
            state,
            index,
            text_editor::Action::Edit(text_editor::Edit::Backspace),
        );
    }
    if block.text.is_empty() {
        if block.children.is_empty() {
            return Some(Effect::RemoveBlock(block.id));
        }
        state.pending_block_delete = Some(block.id);
        return None;
    }
    let previous_index = (0..index).rev().find(|candidate| {
        page_document(state).is_some_and(|document| {
            document.blocks[*candidate].kind != BlockKind::Divider
                && !block_hidden_by_collapse(
                    document,
                    &state.collapsed_blocks,
                    &document.blocks[*candidate],
                )
        })
    });
    if let Some(above) = index.checked_sub(1)
        && page_document(state)?.blocks.get(above)?.kind == BlockKind::Divider
    {
        let divider = page_document(state)?.blocks.get(above)?;
        if divider.children.is_empty() {
            return Some(Effect::RemoveBlock(divider.id.clone()));
        }
        state.pending_block_delete = Some(divider.id.clone());
        return None;
    }
    let previous_index = previous_index?;
    let source = block;
    let previous = page_document(state)?.blocks.get(previous_index)?.clone();
    let seam = utf16_len(&previous.text);
    let mut destination = previous.clone();
    destination.text.push_str(&source.text);
    destination.marks = merge_marks(&previous.marks, &source.marks, seam);
    let thread_moves = merge_thread_moves(page_document(state)?, &source, &destination, seam);
    set_editor_text(state, &destination.id, &destination.text, true);
    if let Some(editor) = state
        .editors
        .iter_mut()
        .find(|(id, _)| id == &destination.id)
        .map(|(_, editor)| editor)
    {
        let byte = byte_for_utf16(&destination.text, seam);
        editor.content.move_to(text_editor::Cursor {
            position: position_for_byte(&destination.text, byte),
            selection: None,
        });
    }
    if let Some(model) = page_document_mut(state)?.blocks.get_mut(previous_index) {
        model.clone_from(&destination);
    }
    state.focused_block = Some(destination.id.clone());
    state.dirty_block = None;
    Some(Effect::MergeBlock {
        page: page_document(state)?.id.clone(),
        destination,
        source,
        thread_moves,
    })
}

fn commit_block(state: &mut State, index: usize) -> Option<Effect> {
    let (page, block) = page_document(state).and_then(|document| {
        document
            .blocks
            .get(index)
            .map(|block| (document.id.clone(), block.clone()))
    })?;
    if state
        .dirty_block
        .as_ref()
        .is_some_and(|(id, _)| id == &block.id)
    {
        state.dirty_block = None;
    }
    Some(Effect::SaveBlock { page, block })
}

fn toggle_mark(state: &mut State, index: usize, kind: InlineMark) -> Option<Effect> {
    let block = page_document(state)?.blocks.get(index)?;
    let (start, end) = editor_selection_utf16(state, &block.id, &block.text)?;
    if start == end {
        return None;
    }
    let active = !range_has_mark(&block.marks, RelativeAnchor { start, end }, kind);
    Some(Effect::SetSpanMark {
        block: block.id.clone(),
        start,
        end,
        kind,
        active,
    })
}

fn begin_block_comment(state: &mut State, index: usize) -> Option<Effect> {
    let block = page_document(state)?.blocks.get(index)?;
    let anchor = editor_selection_utf16(state, &block.id, &block.text)
        .filter(|(start, end)| start < end)
        .map(|(start, end)| RelativeAnchor { start, end });
    state.comment_target = Some(CommentTarget::New {
        target: block.id.clone(),
        anchor,
    });
    state.comment_draft.clear();
    None
}

fn add_comment(state: &mut State) -> Option<Effect> {
    let text = nonempty(&state.comment_draft.text())?;
    let default_target = page_document(state)?.id.clone();
    let target = state.comment_target.take().unwrap_or(CommentTarget::New {
        target: default_target,
        anchor: None,
    });
    state.comment_draft.clear();
    match target {
        CommentTarget::New { target, anchor } => Some(Effect::AddComment {
            thread: local_id("comment-thread"),
            comment: local_id("comment"),
            target,
            anchor,
            text,
        }),
        CommentTarget::Reply { thread, target } => Some(Effect::AddComment {
            thread,
            comment: local_id("comment"),
            target,
            anchor: None,
            text,
        }),
    }
}

fn apply_page_edit(state: &mut State, redo: bool) -> Option<Effect> {
    let edit = if redo {
        state.redo.pop()?
    } else {
        state.undo.pop()?
    };
    let value = if redo {
        edit.after.clone()
    } else {
        edit.before.clone()
    };
    let index = page_document(state)?
        .blocks
        .iter()
        .position(|block| block.id == edit.block)?;
    // Undo/redo replaces the block text; its inline marks and comment anchors
    // must be rebased onto the restored text too, exactly as the forward
    // `edit_block` path does. Otherwise a mark's UTF-16 range can exceed the
    // reverted text length and the node silently rejects the commit.
    let current = page_document(state)?.blocks.get(index)?.text.clone();
    set_editor_text(state, &edit.block, &value, true);
    let marks = {
        let block = page_document(state)?.blocks.get(index)?;
        rebase_marks(&current, &value, &block.marks)
    };
    if let Some(block) = page_document_mut(state)?.blocks.get_mut(index) {
        block.text.clone_from(&value);
        block.marks = marks;
    }
    if let Some(document) = page_document_mut(state) {
        for thread in &mut document.comment_threads {
            if thread.target == edit.block {
                thread.anchor = thread
                    .anchor
                    .map(|anchor| rebase_range(&current, &value, anchor));
            }
        }
    }
    if redo {
        state.undo.push(edit);
    } else {
        state.redo.push(edit);
    }
    commit_block(state, index)
}

fn move_block(state: &State, index: usize, movement: BlockMove) -> Option<Effect> {
    let document = page_document(state)?;
    let (parent, after) = block_move_target(document, index, movement)?;
    Some(Effect::MoveBlock {
        block: document.blocks[index].id.clone(),
        parent,
        after,
    })
}

fn remove_empty_focused(state: &mut State) -> Option<Effect> {
    let focused = state.focused_block.as_deref()?;
    let block = page_document(state)?
        .blocks
        .iter()
        .find(|block| block.id == focused)?;
    if !block.text.is_empty() {
        return None;
    }
    if block.children.is_empty() {
        Some(Effect::RemoveBlock(block.id.clone()))
    } else {
        state.pending_block_delete = Some(block.id.clone());
        None
    }
}

fn activate_focused(state: &mut State) -> Option<Effect> {
    let focused = state.focused_block.as_deref()?;
    let (id, kind, checked, index) = page_document(state)?
        .blocks
        .iter()
        .enumerate()
        .find(|(_, block)| block.id == focused)
        .map(|(index, block)| (block.id.clone(), block.kind, block.checked, index))?;
    match kind {
        BlockKind::Todo => Some(Effect::SetBlockChecked {
            block: id,
            checked: !checked,
        }),
        BlockKind::Toggle => {
            let id = page_document(state)?.blocks[index].id.clone();
            toggle_id(&mut state.collapsed_blocks, &id);
            None
        }
        _ => None,
    }
}

fn cycle_tab(state: &mut State, next: bool) -> Option<Effect> {
    let data = match &mut state.data {
        Resource::Ready(data) if !data.open_tabs.is_empty() => data,
        _ => return None,
    };
    let current = data
        .document
        .as_ref()
        .and_then(|document| data.open_tabs.iter().position(|tab| tab == &document.id))
        .unwrap_or(0);
    let index = if next {
        (current + 1) % data.open_tabs.len()
    } else {
        (current + data.open_tabs.len() - 1) % data.open_tabs.len()
    };
    Some(Effect::LoadPage(data.open_tabs[index].clone()))
}

fn close_active_tab(state: &mut State) -> Option<Effect> {
    let data = match &mut state.data {
        Resource::Ready(data) => data,
        _ => return None,
    };
    let active = data.document.as_ref()?.id.clone();
    let index = data.open_tabs.iter().position(|tab| tab == &active)?;
    data.open_tabs.remove(index);
    if let Some(next) = data
        .open_tabs
        .get(index.min(data.open_tabs.len().saturating_sub(1)))
        .cloned()
    {
        Some(Effect::LoadPage(next))
    } else {
        data.document = None;
        None
    }
}

fn drop_dragged(state: &mut State) -> Option<Effect> {
    let from = state.dragging_block.take()?;
    let target = state.drag_hover.take()?;
    if from == target {
        return None;
    }
    let document = page_document(state)?;
    let (parent, after) = block_drop_target(document, from, target)?;
    Some(Effect::MoveBlock {
        block: document.blocks.get(from)?.id.clone(),
        parent,
        after,
    })
}

fn paste(state: &mut State, index: usize, text: &str) -> Option<Effect> {
    let anchor = page_document(state)?.blocks.get(index)?;
    let parent = anchor.parent.clone();
    let after = Some(anchor.id.clone());
    let (blocks, dropped) = paste_blocks(text, 60);
    state.paste_dropped = dropped;
    (!blocks.is_empty()).then_some(Effect::PasteBlocks {
        parent,
        after,
        blocks,
    })
}

fn split_thread_moves(
    document: &PageDocument,
    original: &PageBlock,
    left: &PageBlock,
    right: &PageBlock,
) -> Vec<ThreadMove> {
    let left_len = utf16_len(&left.text);
    document
        .comment_threads
        .iter()
        .filter(|thread| thread.target == original.id)
        .filter_map(|thread| {
            let anchor = thread.anchor?;
            let anchor = rebase_range(
                &original.text,
                &format!("{}{}", left.text, right.text),
                anchor,
            );
            (anchor.start >= left_len).then(|| ThreadMove {
                thread: thread.id.clone(),
                target: right.id.clone(),
                anchor: Some(RelativeAnchor {
                    start: anchor.start.saturating_sub(left_len),
                    end: anchor.end.saturating_sub(left_len),
                }),
            })
        })
        .collect()
}

fn merge_thread_moves(
    document: &PageDocument,
    source: &PageBlock,
    destination: &PageBlock,
    seam: usize,
) -> Vec<ThreadMove> {
    document
        .comment_threads
        .iter()
        .filter(|thread| thread.target == source.id)
        .map(|thread| ThreadMove {
            thread: thread.id.clone(),
            target: destination.id.clone(),
            anchor: thread.anchor.map(|anchor| RelativeAnchor {
                start: anchor.start + seam,
                end: anchor.end + seam,
            }),
        })
        .collect()
}

fn editor_mut<'a>(state: &'a mut State, id: &str, text: &str) -> &'a mut EditorState {
    if let Some(index) = state.editors.iter().position(|(block, _)| block == id) {
        return &mut state.editors[index].1;
    }
    state.editors.push((id.to_owned(), EditorState::new(text)));
    &mut state.editors.last_mut().unwrap().1
}

fn set_editor_text(state: &mut State, id: &str, text: &str, committed: bool) {
    let editor = editor_mut(state, id, text);
    editor.content = text_editor::Content::with_text(text);
    if committed {
        editor.committed = text.to_owned();
    }
}

fn editor_selection_bytes(state: &State, id: &str, fallback: &str) -> Option<(usize, usize)> {
    let editor = state
        .editors
        .iter()
        .find(|(block, _)| block == id)
        .map(|(_, editor)| editor)?;
    let cursor = editor.content.cursor();
    let selection = cursor.selection?;
    let text = editor.text();
    let a = byte_for_position(&text, cursor.position);
    let b = byte_for_position(&text, selection);
    Some((a.min(b).min(fallback.len()), a.max(b).min(fallback.len())))
}

fn editor_cursor_byte(state: &State, id: &str, fallback: &str) -> Option<usize> {
    let editor = state
        .editors
        .iter()
        .find(|(block, _)| block == id)
        .map(|(_, editor)| editor)?;
    Some(byte_for_position(&editor.text(), editor.content.cursor().position).min(fallback.len()))
}

fn editor_selection_utf16(state: &State, id: &str, text: &str) -> Option<(usize, usize)> {
    let (start, end) = editor_selection_bytes(state, id, text)?;
    Some((utf16_len(&text[..start]), utf16_len(&text[..end])))
}

fn editor_cursor_utf16(editor: &EditorState) -> (usize, usize) {
    let text = editor.text();
    let cursor = editor.content.cursor();
    let head = utf16_len(&text[..byte_for_position(&text, cursor.position)]);
    let anchor = cursor
        .selection
        .map(|position| utf16_len(&text[..byte_for_position(&text, position)]))
        .unwrap_or(head);
    (anchor, head)
}

fn byte_for_position(text: &str, position: text_editor::Position) -> usize {
    let mut offset = 0;
    for (line, part) in text.split('\n').enumerate() {
        if line == position.line {
            let mut column = position.column.min(part.len());
            while column > 0 && !part.is_char_boundary(column) {
                column -= 1;
            }
            return offset + column;
        }
        offset += part.len() + 1;
    }
    text.len()
}

fn position_for_byte(text: &str, byte: usize) -> text_editor::Position {
    let byte = byte.min(text.len());
    let before = &text[..byte];
    let line = before.bytes().filter(|byte| *byte == b'\n').count();
    let column = before
        .rsplit_once('\n')
        .map_or(before.len(), |(_, line)| line.len());
    text_editor::Position { line, column }
}

fn byte_for_utf16(text: &str, wanted: usize) -> usize {
    let mut units = 0;
    for (byte, ch) in text.char_indices() {
        if units >= wanted {
            return byte;
        }
        units += ch.len_utf16();
    }
    text.len()
}

fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

#[derive(Debug, Clone, Copy)]
struct TextEdit {
    start: usize,
    old_end: usize,
    new_end: usize,
}

fn edit_between(old: &str, new: &str) -> Option<TextEdit> {
    if old == new {
        return None;
    }
    let old_chars = old.char_indices().collect::<Vec<_>>();
    let new_chars = new.char_indices().collect::<Vec<_>>();
    let mut prefix = 0;
    while prefix < old_chars.len()
        && prefix < new_chars.len()
        && old_chars[prefix].1 == new_chars[prefix].1
    {
        prefix += 1;
    }
    let mut suffix = 0;
    while suffix < old_chars.len().saturating_sub(prefix)
        && suffix < new_chars.len().saturating_sub(prefix)
        && old_chars[old_chars.len() - 1 - suffix].1 == new_chars[new_chars.len() - 1 - suffix].1
    {
        suffix += 1;
    }
    let old_start_byte = old_chars.get(prefix).map_or(old.len(), |(byte, _)| *byte);
    let new_start_byte = new_chars.get(prefix).map_or(new.len(), |(byte, _)| *byte);
    let old_end_byte = old_chars
        .get(old_chars.len().saturating_sub(suffix))
        .map_or(old.len(), |(byte, _)| *byte);
    let new_end_byte = new_chars
        .get(new_chars.len().saturating_sub(suffix))
        .map_or(new.len(), |(byte, _)| *byte);
    Some(TextEdit {
        start: utf16_len(&old[..old_start_byte]),
        old_end: utf16_len(&old[..old_end_byte]),
        new_end: utf16_len(&new[..new_end_byte.max(new_start_byte)]),
    })
}

fn rebase_range(old: &str, new: &str, range: RelativeAnchor) -> RelativeAnchor {
    let Some(edit) = edit_between(old, new) else {
        return range;
    };
    let length = utf16_len(new);
    if range.start == range.end {
        let position = if range.start <= edit.start {
            range.start
        } else if range.start >= edit.old_end {
            shift_after(range.start, edit)
        } else {
            edit.new_end
        };
        let position = position.min(length);
        return RelativeAnchor {
            start: position,
            end: position,
        };
    }
    let start = if range.start < edit.start {
        range.start
    } else if range.start >= edit.old_end {
        shift_after(range.start, edit)
    } else {
        edit.start
    }
    .min(length);
    let end = if range.end <= edit.start {
        range.end
    } else if range.end >= edit.old_end {
        shift_after(range.end, edit)
    } else {
        edit.new_end
    }
    .min(length)
    .max(start);
    RelativeAnchor { start, end }
}

fn shift_after(position: usize, edit: TextEdit) -> usize {
    if edit.new_end >= edit.old_end {
        position.saturating_add(edit.new_end - edit.old_end)
    } else {
        position.saturating_sub(edit.old_end - edit.new_end)
    }
}

fn normalize_marks(mut marks: Vec<SpanMark>) -> Vec<SpanMark> {
    marks.retain(|mark| mark.start < mark.end);
    marks.sort_by_key(|mark| (mark.kind as u8, mark.start, mark.end));
    let mut output: Vec<SpanMark> = Vec::new();
    for mark in marks {
        if let Some(last) = output.last_mut()
            && last.kind == mark.kind
            && mark.start <= last.end
        {
            last.end = last.end.max(mark.end);
        } else {
            output.push(mark);
        }
    }
    output
}

fn rebase_marks(old: &str, new: &str, marks: &[SpanMark]) -> Vec<SpanMark> {
    normalize_marks(
        marks
            .iter()
            .map(|mark| {
                let range = rebase_range(
                    old,
                    new,
                    RelativeAnchor {
                        start: mark.start,
                        end: mark.end,
                    },
                );
                SpanMark {
                    start: range.start,
                    end: range.end,
                    kind: mark.kind,
                }
            })
            .collect(),
    )
}

fn range_has_mark(marks: &[SpanMark], range: RelativeAnchor, kind: InlineMark) -> bool {
    let mut covered = range.start;
    for mark in normalize_marks(
        marks
            .iter()
            .filter(|mark| mark.kind == kind)
            .cloned()
            .collect(),
    ) {
        if mark.end <= covered {
            continue;
        }
        if mark.start > covered {
            return false;
        }
        covered = mark.end;
        if covered >= range.end {
            return true;
        }
    }
    false
}

fn split_marks(marks: &[SpanMark], at: usize) -> (Vec<SpanMark>, Vec<SpanMark>) {
    let left = normalize_marks(
        marks
            .iter()
            .filter(|mark| mark.start < at)
            .map(|mark| SpanMark {
                end: mark.end.min(at),
                ..mark.clone()
            })
            .collect(),
    );
    let right = normalize_marks(
        marks
            .iter()
            .filter(|mark| mark.end > at)
            .map(|mark| SpanMark {
                start: mark.start.max(at) - at,
                end: mark.end - at,
                kind: mark.kind,
            })
            .collect(),
    );
    (left, right)
}

fn merge_marks(left: &[SpanMark], right: &[SpanMark], offset: usize) -> Vec<SpanMark> {
    normalize_marks(
        left.iter()
            .cloned()
            .chain(right.iter().map(|mark| SpanMark {
                start: mark.start + offset,
                end: mark.end + offset,
                kind: mark.kind,
            }))
            .collect(),
    )
}

fn page_document(state: &State) -> Option<&PageDocument> {
    match &state.data {
        Resource::Ready(data) => data.document.as_ref(),
        _ => None,
    }
}

fn page_document_mut(state: &mut State) -> Option<&mut PageDocument> {
    match &mut state.data {
        Resource::Ready(data) => data.document.as_mut(),
        _ => None,
    }
}

fn location(state: &State) -> (Option<String>, Vec<String>) {
    match &state.data {
        Resource::Ready(data) => (
            data.document.as_ref().map(|document| document.id.clone()),
            data.open_tabs.clone(),
        ),
        _ => (None, Vec::new()),
    }
}

fn begin_page_create(state: &mut State, parent: Option<String>) -> Effect {
    let id = fresh_id("page");
    let document = PageDocument {
        id: id.clone(),
        title: "Untitled".into(),
        ancestry: Vec::new(),
        blocks: Vec::new(),
        page_comments: 0,
        comment_threads: Vec::new(),
        presence: Vec::new(),
        self_key: None,
    };
    reset_page_transients(state);
    match &mut state.data {
        Resource::Ready(data) => {
            data.open_tabs.push(id.clone());
            data.document = Some(document);
        }
        _ => {
            state.data = Resource::Ready(PagesData {
                pages: Vec::new(),
                open_tabs: vec![id.clone()],
                document: Some(document),
            });
        }
    }
    // ponytail: reuse the forwarded Command slot; give CreatePage typed fields
    // when the screen/command boundary can change together.
    Effect::CreatePage {
        parent: Some(format!("{id}\0{}", parent.unwrap_or_default())),
    }
}

fn reset_page_transients(state: &mut State) {
    state.focused_block = None;
    state.pending_block_delete = None;
    state.pending_page_delete = false;
    state.comment_target = None;
    state.hovered_block = None;
    state.menu_open_block = None;
}

fn reveal_page_block(state: &mut State, id: &str) {
    let Some(document) = page_document(state) else {
        return;
    };
    let mut parents = Vec::new();
    let mut cursor = id;
    while let Some(block) = document.blocks.iter().find(|block| block.id == cursor) {
        if block.parent == document.id || parents.len() >= document.blocks.len() {
            break;
        }
        parents.push(block.parent.clone());
        cursor = &block.parent;
    }
    state.collapsed_blocks.retain(|id| !parents.contains(id));
}

fn toggle_id(ids: &mut Vec<String>, id: &str) {
    if let Some(index) = ids.iter().position(|candidate| candidate == id) {
        ids.remove(index);
    } else {
        ids.push(id.to_owned());
    }
}

fn nonempty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

fn local_id(prefix: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!(
        "{prefix}-{nanos:x}-{:x}",
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

fn block_move_target(
    document: &PageDocument,
    index: usize,
    movement: BlockMove,
) -> Option<(String, Option<String>)> {
    let block = document.blocks.get(index)?;
    let siblings = document
        .blocks
        .iter()
        .filter(|candidate| candidate.parent == block.parent)
        .collect::<Vec<_>>();
    let position = siblings
        .iter()
        .position(|candidate| candidate.id == block.id)?;
    match movement {
        BlockMove::Up if position > 0 => Some((
            block.parent.clone(),
            position
                .checked_sub(2)
                .and_then(|index| siblings.get(index))
                .map(|block| block.id.clone()),
        )),
        BlockMove::Down => siblings
            .get(position + 1)
            .map(|sibling| (block.parent.clone(), Some(sibling.id.clone()))),
        BlockMove::Indent if position > 0 => {
            let parent = siblings[position - 1];
            Some((parent.id.clone(), parent.children.last().cloned()))
        }
        BlockMove::Outdent if block.parent != document.id => {
            let parent = document
                .blocks
                .iter()
                .find(|candidate| candidate.id == block.parent)?;
            Some((parent.parent.clone(), Some(parent.id.clone())))
        }
        _ => None,
    }
}

fn block_drop_target(
    document: &PageDocument,
    from: usize,
    target: usize,
) -> Option<(String, Option<String>)> {
    let moved = document.blocks.get(from)?;
    let target = document.blocks.get(target)?;
    let mut cursor = target.parent.as_str();
    while cursor != document.id {
        if cursor == moved.id {
            return None;
        }
        cursor = document
            .blocks
            .iter()
            .find(|block| block.id == cursor)?
            .parent
            .as_str();
    }
    let after = document
        .blocks
        .iter()
        .filter(|block| block.parent == target.parent && block.id != moved.id)
        .take_while(|block| block.id != target.id)
        .last()
        .map(|block| block.id.clone());
    Some((target.parent.clone(), after))
}

fn block_descendant_count(document: &PageDocument, root: &str) -> usize {
    let mut pending = document
        .blocks
        .iter()
        .find(|block| block.id == root)
        .map(|block| block.children.clone())
        .unwrap_or_default();
    let mut visited = Vec::new();
    while let Some(id) = pending.pop() {
        if visited.contains(&id) || visited.len() >= document.blocks.len() {
            continue;
        }
        visited.push(id.clone());
        if let Some(block) = document.blocks.iter().find(|block| block.id == id) {
            pending.extend(block.children.iter().cloned());
        }
    }
    visited.len()
}

fn paste_blocks(text: &str, limit: usize) -> (Vec<(BlockKind, String, bool)>, usize) {
    let lines = text
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    let blocks = lines
        .iter()
        .take(limit)
        .map(|line| paste_block(line))
        .collect();
    (blocks, lines.len().saturating_sub(limit))
}

fn paste_block(line: &str) -> (BlockKind, String, bool) {
    for (prefix, kind) in [
        ("### ", BlockKind::Heading3),
        ("## ", BlockKind::Heading2),
        ("# ", BlockKind::Heading1),
        ("- [ ] ", BlockKind::Todo),
        ("- [x] ", BlockKind::Todo),
        ("- [X] ", BlockKind::Todo),
        ("- ", BlockKind::Bulleted),
        ("> ", BlockKind::Quote),
    ] {
        if let Some(text) = line.strip_prefix(prefix) {
            return (kind, text.to_owned(), matches!(prefix, "- [x] " | "- [X] "));
        }
    }
    if line == "---" {
        (BlockKind::Divider, String::new(), false)
    } else {
        (BlockKind::Paragraph, line.to_owned(), false)
    }
}

fn block_hidden_by_collapse(
    document: &PageDocument,
    collapsed: &[String],
    block: &PageBlock,
) -> bool {
    let mut parent = block.parent.as_str();
    let mut depth = 0;
    while parent != document.id {
        if collapsed.iter().any(|id| id == parent) {
            return true;
        }
        let Some(next) = document.blocks.iter().find(|block| block.id == parent) else {
            break;
        };
        parent = &next.parent;
        depth += 1;
        if depth >= document.blocks.len() {
            break;
        }
    }
    false
}

const fn block_kind_label(kind: BlockKind) -> &'static str {
    match kind {
        BlockKind::Paragraph => "¶",
        BlockKind::Heading1 => "H1",
        BlockKind::Heading2 => "H2",
        BlockKind::Heading3 => "H3",
        BlockKind::Bulleted => "•",
        BlockKind::Numbered => "1.",
        BlockKind::Todo => "☐",
        BlockKind::Toggle => "▸",
        BlockKind::Quote => "❯",
        BlockKind::Code => "</>",
        BlockKind::Callout => "!",
        BlockKind::Divider => "—",
    }
}

const fn all_block_kinds() -> [BlockKind; 12] {
    [
        BlockKind::Paragraph,
        BlockKind::Heading1,
        BlockKind::Heading2,
        BlockKind::Heading3,
        BlockKind::Bulleted,
        BlockKind::Numbered,
        BlockKind::Todo,
        BlockKind::Toggle,
        BlockKind::Quote,
        BlockKind::Code,
        BlockKind::Callout,
        BlockKind::Divider,
    ]
}

fn slash_options(value: &str) -> Vec<BlockKind> {
    let query = value.trim_start_matches('/').trim().to_ascii_lowercase();
    all_block_kinds()
        .into_iter()
        .filter(|kind| {
            query.is_empty()
                || block_kind_label(*kind)
                    .to_ascii_lowercase()
                    .contains(&query)
        })
        .collect()
}

const fn continuation_kind(kind: BlockKind) -> BlockKind {
    match kind {
        BlockKind::Bulleted | BlockKind::Numbered | BlockKind::Todo => kind,
        _ => BlockKind::Paragraph,
    }
}

const fn empty_enter_exits(kind: BlockKind) -> bool {
    matches!(
        kind,
        BlockKind::Bulleted
            | BlockKind::Numbered
            | BlockKind::Todo
            | BlockKind::Quote
            | BlockKind::Code
            | BlockKind::Callout
            | BlockKind::Toggle
    )
}

fn block_placeholder(kind: BlockKind) -> &'static str {
    match kind {
        BlockKind::Heading1 => "Heading 1",
        BlockKind::Heading2 => "Heading 2",
        BlockKind::Heading3 => "Heading 3",
        BlockKind::Bulleted | BlockKind::Numbered => "List item",
        BlockKind::Todo => "To-do",
        BlockKind::Toggle => "Toggle",
        BlockKind::Quote => "Quote",
        BlockKind::Code => "Code",
        BlockKind::Callout => "Callout",
        BlockKind::Divider => "Divider",
        BlockKind::Paragraph => "Write, or press '/' for commands",
    }
}

pub fn page_block_input_id(block: &str) -> String {
    format!("page-block-{block}")
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::view::page_depth;
    use super::*;
    use crate::theme;

    fn block(id: &str, text: &str) -> PageBlock {
        PageBlock {
            id: id.into(),
            kind: BlockKind::Paragraph,
            text: text.into(),
            depth: 0,
            checked: false,
            parent: "p".into(),
            children: Vec::new(),
            marks: Vec::new(),
        }
    }

    fn document(blocks: Vec<PageBlock>) -> PageDocument {
        PageDocument {
            id: "p".into(),
            title: "Page".into(),
            ancestry: vec![PageMeta {
                id: "p".into(),
                title: "Page".into(),
                parent: None,
            }],
            blocks,
            page_comments: 0,
            comment_threads: Vec::new(),
            presence: Vec::new(),
            self_key: None,
        }
    }

    fn state_with(document: PageDocument) -> State {
        let mut state = State::default();
        state.loaded(Ok(Some(PagesData {
            pages: vec![PageMeta {
                id: "p".into(),
                title: "Page".into(),
                parent: None,
            }],
            open_tabs: vec!["p".into()],
            document: Some(document),
        })));
        state
    }

    fn select(state: &mut State, id: &str, start: usize, end: usize) {
        let editor = state
            .editors
            .iter_mut()
            .find(|(block, _)| block == id)
            .unwrap();
        let text = editor.1.text();
        editor.1.content.move_to(text_editor::Cursor {
            position: position_for_byte(&text, end),
            selection: Some(position_for_byte(&text, start)),
        });
    }

    #[test]
    fn text_editor_keeps_multiline_ime_text() {
        let mut state = state_with(document(vec![block("a", "")]));
        update(
            &mut state,
            Message::BlockAction(
                0,
                text_editor::Action::Edit(text_editor::Edit::Paste(Arc::new("안녕\nducks".into()))),
            ),
        );
        assert_eq!(page_document(&state).unwrap().blocks[0].text, "안녕\nducks");
    }

    #[test]
    fn enter_splits_at_real_caret_and_moves_utf16_ranges() {
        let mut doc = document(vec![block("a", "a😀b")]);
        doc.blocks[0].marks = vec![SpanMark {
            start: 1,
            end: 4,
            kind: InlineMark::Bold,
        }];
        doc.comment_threads = vec![PageCommentThread {
            id: "t".into(),
            target: "a".into(),
            anchor: Some(RelativeAnchor { start: 3, end: 4 }),
            resolved: false,
            comments: Vec::new(),
        }];
        let mut state = state_with(doc);
        select(&mut state, "a", "a😀".len(), "a😀".len());
        let Some(Effect::SplitBlock {
            left,
            right,
            thread_moves,
            ..
        }) = update(&mut state, Message::BlockEnter(0))
        else {
            panic!("expected split")
        };
        assert_eq!(left.text, "a😀");
        assert_eq!(right.text, "b");
        assert_eq!(
            left.marks,
            vec![SpanMark {
                start: 1,
                end: 3,
                kind: InlineMark::Bold
            }]
        );
        assert_eq!(
            right.marks,
            vec![SpanMark {
                start: 0,
                end: 1,
                kind: InlineMark::Bold
            }]
        );
        assert_eq!(
            thread_moves[0].anchor,
            Some(RelativeAnchor { start: 0, end: 1 })
        );
    }

    #[test]
    fn code_enter_is_a_newline_not_a_block_split() {
        let mut code = block("a", "let x = 1;");
        code.kind = BlockKind::Code;
        let mut state = state_with(document(vec![code]));
        select(&mut state, "a", 3, 3);
        let effect = update(&mut state, Message::BlockEnter(0));
        assert!(matches!(effect, Some(Effect::CommitAfter { .. })));
        assert_eq!(
            page_document(&state).unwrap().blocks[0].text,
            "let\n x = 1;"
        );
    }

    #[test]
    fn offset_zero_backspace_merges_marks_and_comment_anchor() {
        let left = block("a", "A😀");
        let mut right = block("b", "tail");
        right.marks = vec![SpanMark {
            start: 0,
            end: 4,
            kind: InlineMark::Italic,
        }];
        let mut doc = document(vec![left, right]);
        doc.comment_threads = vec![PageCommentThread {
            id: "t".into(),
            target: "b".into(),
            anchor: Some(RelativeAnchor { start: 0, end: 2 }),
            resolved: false,
            comments: Vec::new(),
        }];
        let mut state = state_with(doc);
        select(&mut state, "b", 0, 0);
        let Some(Effect::MergeBlock {
            destination,
            source,
            thread_moves,
            ..
        }) = update(&mut state, Message::BlockBackspace(1))
        else {
            panic!("expected merge")
        };
        assert_eq!(source.id, "b");
        assert_eq!(destination.text, "A😀tail");
        assert_eq!(
            destination.marks[0],
            SpanMark {
                start: 3,
                end: 7,
                kind: InlineMark::Italic
            }
        );
        assert_eq!(
            thread_moves[0].anchor,
            Some(RelativeAnchor { start: 3, end: 5 })
        );
    }

    #[test]
    fn selected_range_drives_mark_and_comment_anchor_exactly() {
        let mut state = state_with(document(vec![block("a", "first second")]));
        select(&mut state, "a", 0, 5);
        assert_eq!(
            update(&mut state, Message::ToggleMark(0, InlineMark::Bold)),
            Some(Effect::SetSpanMark {
                block: "a".into(),
                start: 0,
                end: 5,
                kind: InlineMark::Bold,
                active: true,
            })
        );
        update(&mut state, Message::CommentOnBlock(0));
        update(
            &mut state,
            Message::CommentAction(text_editor::Action::Edit(text_editor::Edit::Paste(
                Arc::new("tighten".into()),
            ))),
        );
        let Some(Effect::AddComment {
            target,
            anchor,
            text,
            ..
        }) = update(&mut state, Message::AddComment)
        else {
            panic!("expected comment")
        };
        assert_eq!(target, "a");
        assert_eq!(anchor, Some(RelativeAnchor { start: 0, end: 5 }));
        assert_eq!(text, "tighten");
    }

    #[test]
    fn comment_replies_and_edits_keep_multiline_content() {
        let mut state = state_with(document(vec![block("a", "text")]));
        update(
            &mut state,
            Message::ReplyToThread("thread-1".into(), "a".into()),
        );
        update(
            &mut state,
            Message::CommentAction(text_editor::Action::Edit(text_editor::Edit::Paste(
                Arc::new("first\nsecond".into()),
            ))),
        );
        let Some(Effect::AddComment {
            thread,
            anchor,
            text,
            ..
        }) = update(&mut state, Message::AddComment)
        else {
            panic!("expected reply")
        };
        assert_eq!(thread, "thread-1");
        assert_eq!(anchor, None);
        assert_eq!(text, "first\nsecond");

        update(
            &mut state,
            Message::BeginCommentEdit("comment-1".into(), "old".into()),
        );
        update(
            &mut state,
            Message::CommentEditAction(text_editor::Action::SelectAll),
        );
        update(
            &mut state,
            Message::CommentEditAction(text_editor::Action::Edit(text_editor::Edit::Paste(
                Arc::new("new\nbody".into()),
            ))),
        );
        assert_eq!(
            update(&mut state, Message::CommitCommentEdit),
            Some(Effect::EditComment {
                comment: "comment-1".into(),
                text: "new\nbody".into(),
            })
        );
    }

    #[test]
    fn presence_uses_live_utf16_selection() {
        let mut state = state_with(document(vec![block("a", "a😀b")]));
        state.focused_block = Some("a".into());
        select(&mut state, "a", 1, "a😀".len());
        assert_eq!(state.cursor_presence(), (Some("a".into()), 1, 3));
    }

    #[test]
    fn reload_preserves_dirty_multiline_draft_and_rebases_ranges() {
        let mut original = block("a", "abcd");
        original.marks = vec![SpanMark {
            start: 2,
            end: 4,
            kind: InlineMark::Bold,
        }];
        let mut state = state_with(document(vec![original]));
        select(&mut state, "a", 1, 1);
        update(
            &mut state,
            Message::BlockAction(0, text_editor::Action::Edit(text_editor::Edit::Insert('X'))),
        );
        let mut fresh = document(vec![block("a", "abcd")]);
        fresh.blocks[0].marks = vec![SpanMark {
            start: 2,
            end: 4,
            kind: InlineMark::Bold,
        }];
        state.document_loaded(Ok(fresh));
        let block = &page_document(&state).unwrap().blocks[0];
        assert_eq!(block.text, "aXbcd");
        assert_eq!(
            block.marks,
            vec![SpanMark {
                start: 3,
                end: 5,
                kind: InlineMark::Bold
            }]
        );
    }

    #[test]
    fn range_rebase_uses_utf16_not_utf8_bytes() {
        assert_eq!(
            rebase_range("a😀b", "aX😀b", RelativeAnchor { start: 3, end: 4 }),
            RelativeAnchor { start: 4, end: 5 },
        );
    }

    #[test]
    fn page_tree_depth_is_bounded_for_malformed_cycles() {
        let pages = vec![
            PageMeta {
                id: "root".into(),
                title: "Root".into(),
                parent: None,
            },
            PageMeta {
                id: "child".into(),
                title: "Child".into(),
                parent: Some("root".into()),
            },
            PageMeta {
                id: "grandchild".into(),
                title: "Grandchild".into(),
                parent: Some("child".into()),
            },
        ];
        assert_eq!(page_depth(&pages, "root"), 0);
        assert_eq!(page_depth(&pages, "grandchild"), 2);
        let cycle = vec![PageMeta {
            id: "loop".into(),
            title: "Loop".into(),
            parent: Some("loop".into()),
        }];
        assert_eq!(page_depth(&cycle, "loop"), cycle.len());
    }

    #[test]
    fn drag_drop_places_before_target_and_rejects_descendants() {
        let mut doc = document(vec![block("a", "a"), block("b", "b"), block("c", "c")]);
        assert_eq!(
            block_drop_target(&doc, 2, 1),
            Some(("p".into(), Some("a".into())))
        );
        doc.blocks[0].children.push("b".into());
        doc.blocks[1].parent = "a".into();
        assert_eq!(block_drop_target(&doc, 0, 1), None);
    }

    #[test]
    fn populated_view_constructs_in_both_design_palettes() {
        let state = state_with(document(vec![block("a", "안녕\nPages")]));
        for mode in [theme::Mode::Light, theme::Mode::Dark] {
            let _ = view(&state, *theme::palette(mode));
        }
    }
}
