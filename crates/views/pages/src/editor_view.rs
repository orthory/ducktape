//! The Pages presentation consumer: native-compatible comment badges and an
//! explicit plain-editor notice when the complete presentation cannot be used.
use crate::{document_sync::CommentMark, editor_binding::MenuState};
use ui_lang_guest::{Editor, EditorStateView, wire};
use wire::editor_presentation::{EditorMargin, EditorPresentation, PresentationError};

/// The gap one line of the document holds open below itself, in whole pixels.
/// An inline comment card lives in it: the card is a stack layer over the
/// document, so the space it needs has to come out of the document's own
/// layout or the card would sit on top of the text it belongs to.
#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EditorReserve {
    pub line: i64,
    pub height: i64,
}

fn prepare(
    state: EditorStateView<'_>,
    menu: MenuState,
    dark: bool,
    commented: Vec<i64>,
    marks: Vec<CommentMark>,
    focused: bool,
    reserve: EditorReserve,
) -> Result<EditorPresentation, PresentationError> {
    let mut paint = crate::presentation::build(state, menu, dark, commented, focused, reserve)?;
    let lines = wire::editor_lines(state.text).count();
    paint.affordances.margin_label = "Open comments".into();
    // Saved anchors can lag unsaved line deletion. The native document also
    // hides badges outside its current line count; it never moves their target.
    paint.affordances.margins = marks
        .into_iter()
        .filter_map(|mark| {
            let line = u32::try_from(mark.line).ok()?;
            let count = u32::try_from(mark.count).ok()?;
            ((line as usize) < lines && count > 0).then_some(EditorMargin { line, count })
        })
        .collect();
    paint.validate(state.text)?;
    Ok(paint)
}

/// A bounded derived value, shared by the visible notice and highlighter. It
/// contains no second document text and is snapshotted with its canonical state.
#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PreparedPresentation {
    pub reference: Vec<u8>,
    pub data: Vec<u8>,
    pub notice: String,
}

pub fn empty_presentation() -> PreparedPresentation {
    PreparedPresentation::default()
}

pub fn no_reserve() -> EditorReserve {
    EditorReserve::default()
}

pub fn document_presentation(
    document: &Editor,
    menu: MenuState,
    dark: bool,
    commented: Vec<i64>,
    marks: Vec<CommentMark>,
    focused: bool,
    reserve: EditorReserve,
) -> PreparedPresentation {
    let state = document.state_view();
    let (paint, notice) = match prepare(state, menu, dark, commented, marks, focused, reserve) {
        Ok(paint) => (paint, String::new()),
        Err(_) => (EditorPresentation::default(), FORMAT_NOTICE.into()),
    };
    let mut data = wire::encode(&paint);
    let mut notice = notice;
    if data.len() > 1024 * 1024 {
        data = wire::encode(&EditorPresentation::default());
        notice = FORMAT_NOTICE.into();
    }
    PreparedPresentation {
        reference: reference(state),
        data,
        notice,
    }
}

const FORMAT_NOTICE: &str =
    "Formatting is unavailable for this document. Your text and undo history are preserved.";
fn reference(state: EditorStateView<'_>) -> Vec<u8> {
    wire::encode(&(
        state.reset,
        state.text_revision,
        state.revision,
        state.cursor,
    ))
}

pub fn presentation_notice(document: &Editor, prepared: &PreparedPresentation) -> String {
    if prepared.reference != reference(document.state_view()) && !prepared.data.is_empty() {
        return FORMAT_NOTICE.into();
    }
    prepared.notice.clone()
}

pub fn paint(state: EditorStateView<'_>, prepared: PreparedPresentation) -> EditorPresentation {
    if prepared.reference != reference(state) || prepared.data.is_empty() {
        return EditorPresentation::default();
    }
    wire::decode(&prepared.data).expect("prepared Pages presentation")
}
