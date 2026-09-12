#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum AppTheme {
    App,
}
#[derive(Clone, Copy)]
struct Palette {
    name: &'static str,
    colors: [::ducktape_view_guest::wire::Rgba; 4],
}
#[allow(dead_code)]
pub struct PagesEditorFixture {
    pub(crate) formatting_notice: String,
    pub(crate) document: ::ducktape_view_guest::Editor,
    pub(crate) history: crate::editor_binding::HistoryState,
    pub(crate) menu: crate::editor_binding::MenuState,
    pub(crate) source: crate::fixture_source::DocumentSource,
    pub(crate) installed_source: Vec<u8>,
    pub(crate) load_error: String,
    pub(crate) paint_dark: bool,
    pub(crate) commented: Vec<i64>,
}
impl ::std::fmt::Debug for PagesEditorFixture {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("PagesEditorFixture")
    }
}
#[derive(Clone)]
pub enum Message {
    Load,
    DocumentArrived(crate::fixture_source::DocumentItem),
    Committed(crate::editor_binding::EditorUpdate),
    DocumentUpdated(::ducktape_view_guest::EditorDocumentUpdate),
    DocumentTransaction(::ducktape_view_guest::EditorTransaction<Message>),
}
impl ::std::fmt::Debug for Message {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Message")
    }
}
#[allow(unused_parens)]
impl PagesEditorFixture {
    fn palette(&self) -> Palette {
        Palette {
            name: "app",
            colors: [
                ::ducktape_view_guest::wire::Rgba([
                    255.0 / 255.0,
                    255.0 / 255.0,
                    255.0 / 255.0,
                    1.000000,
                ]),
                ::ducktape_view_guest::wire::Rgba([
                    0.0 / 255.0,
                    0.0 / 255.0,
                    0.0 / 255.0,
                    1.000000,
                ]),
                ::ducktape_view_guest::wire::Rgba([
                    255.0 / 255.0,
                    0.0 / 255.0,
                    0.0 / 255.0,
                    1.000000,
                ]),
                ::ducktape_view_guest::wire::Rgba([
                    255.0 / 255.0,
                    0.0 / 255.0,
                    255.0 / 255.0,
                    1.000000,
                ]),
            ],
        }
    }
}
#[allow(unused_parens)]
impl PagesEditorFixture {
    fn initial_state() -> Self {
        Self {
            formatting_notice: "".to_owned(),
            document: ::ducktape_view_guest::Editor::new("- 한글".to_owned()),
            history: crate::editor_binding::initial_history(),
            menu: crate::editor_binding::initial_menu(),
            source: crate::fixture_source::empty_source(),
            installed_source: ::std::vec![],
            load_error: "".to_owned(),
            paint_dark: false,
            commented: Vec::new(),
        }
    }
    pub(crate) fn boot() -> (Self, ducktape_view_guest::Task<Message>) {
        (Self::initial_state(), ducktape_view_guest::Task::none())
    }
    pub(crate) const PREFERRED_WINDOW_SIZE: &'static str = "none";
    pub(crate) const SNAPSHOT_SCHEMA: &'static str =
        "d58bf2b798ab09bc7584235aa6d369a976f67b3396fb454eef1417f7f456836b";
    pub(crate) fn snapshot(&self) -> Result<Vec<u8>, String> {
        ::ducktape_view_guest::wire::Snapshot {
            schema: String::from(Self::SNAPSHOT_SCHEMA),
            state: ::ducktape_view_guest::wire::SnapshotValue::Record {
                name: String::from("PagesEditorFixture"),
                fields: vec![
                    (
                        String::from("formatting_notice"),
                        ::ducktape_view_guest::wire::SnapshotValue::Str(
                            ::std::string::ToString::to_string(&self.formatting_notice),
                        ),
                    ),
                    (
                        String::from("document"),
                        ::ducktape_view_guest::wire::SnapshotValue::Bytes(
                            (&self.document).snapshot(),
                        ),
                    ),
                    (
                        String::from("history"),
                        ::ducktape_view_guest::wire::SnapshotValue::Record {
                            name: String::from("HistoryState"),
                            fields: ::std::vec![(
                                String::from("snapshot"),
                                ::ducktape_view_guest::wire::SnapshotValue::Bytes(
                                    (&(&self.history).snapshot).clone()
                                )
                            )],
                        },
                    ),
                    (
                        String::from("menu"),
                        ::ducktape_view_guest::wire::SnapshotValue::Record {
                            name: String::from("MenuState"),
                            fields: ::std::vec![(
                                String::from("snapshot"),
                                ::ducktape_view_guest::wire::SnapshotValue::Bytes(
                                    (&(&self.menu).snapshot).clone()
                                )
                            )],
                        },
                    ),
                    (
                        String::from("source"),
                        ::ducktape_view_guest::wire::SnapshotValue::Record {
                            name: String::from("DocumentSource"),
                            fields: ::std::vec![(
                                String::from("reference"),
                                ::ducktape_view_guest::wire::SnapshotValue::Bytes(
                                    (&(&self.source).reference).clone()
                                )
                            )],
                        },
                    ),
                    (
                        String::from("installed_source"),
                        ::ducktape_view_guest::wire::SnapshotValue::Bytes(
                            (&self.installed_source).clone(),
                        ),
                    ),
                    (
                        String::from("load_error"),
                        ::ducktape_view_guest::wire::SnapshotValue::Str(
                            ::std::string::ToString::to_string(&self.load_error),
                        ),
                    ),
                    (
                        String::from("paint_dark"),
                        ::ducktape_view_guest::wire::SnapshotValue::Bool(*(&self.paint_dark)),
                    ),
                    (
                        String::from("commented"),
                        ::ducktape_view_guest::wire::SnapshotValue::List(
                            (&self.commented)
                                .iter()
                                .map(|item| {
                                    ::ducktape_view_guest::wire::SnapshotValue::I64(*(item))
                                })
                                .collect(),
                        ),
                    ),
                ],
            },
        }
        .encode()
    }
    pub(crate) fn restore(bytes: &[u8]) -> Result<Self, String> {
        let snapshot = ::ducktape_view_guest::wire::Snapshot::decode(bytes)?;
        if snapshot.schema != Self::SNAPSHOT_SCHEMA {
            return Err(String::from("snapshot schema mismatch"));
        }
        let value = snapshot.state;
        ((|| {
            let ::ducktape_view_guest::wire::SnapshotValue::Record {
                name: name,
                fields: fields,
            } = value
            else {
                return None;
            };
            if name != "PagesEditorFixture" || fields.len() != 9 {
                return None;
            }
            let mut fields = fields.into_iter();
            let (name, value) = fields.next()?;
            if name != "formatting_notice" {
                return None;
            }
            let formatting_notice: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "document" {
                return None;
            }
            let document: ::ducktape_view_guest::Editor = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bytes(bytes) => {
                    ::ducktape_view_guest::Editor::restore(&bytes)
                }
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "history" {
                return None;
            }
            let history: crate::editor_binding::HistoryState = ((|| {
                let ::ducktape_view_guest::wire::SnapshotValue::Record {
                    name: name,
                    fields: fields,
                } = value
                else {
                    return None;
                };
                if name != "HistoryState" || fields.len() != 1 {
                    return None;
                }
                let mut fields = fields.into_iter();
                let (name, field_0) = fields.next()?;
                if name != "snapshot" {
                    return None;
                }
                Some(crate::editor_binding::HistoryState {
                    snapshot: (match field_0 {
                        ::ducktape_view_guest::wire::SnapshotValue::Bytes(item) => Some(item),
                        _ => None,
                    })?,
                })
            })())?;
            let (name, value) = fields.next()?;
            if name != "menu" {
                return None;
            }
            let menu: crate::editor_binding::MenuState = ((|| {
                let ::ducktape_view_guest::wire::SnapshotValue::Record {
                    name: name,
                    fields: fields,
                } = value
                else {
                    return None;
                };
                if name != "MenuState" || fields.len() != 1 {
                    return None;
                }
                let mut fields = fields.into_iter();
                let (name, field_0) = fields.next()?;
                if name != "snapshot" {
                    return None;
                }
                Some(crate::editor_binding::MenuState {
                    snapshot: (match field_0 {
                        ::ducktape_view_guest::wire::SnapshotValue::Bytes(item) => Some(item),
                        _ => None,
                    })?,
                })
            })())?;
            let (name, value) = fields.next()?;
            if name != "source" {
                return None;
            }
            let source: crate::fixture_source::DocumentSource = ((|| {
                let ::ducktape_view_guest::wire::SnapshotValue::Record {
                    name: name,
                    fields: fields,
                } = value
                else {
                    return None;
                };
                if name != "DocumentSource" || fields.len() != 1 {
                    return None;
                }
                let mut fields = fields.into_iter();
                let (name, field_0) = fields.next()?;
                if name != "reference" {
                    return None;
                }
                Some(crate::fixture_source::DocumentSource {
                    reference: (match field_0 {
                        ::ducktape_view_guest::wire::SnapshotValue::Bytes(item) => Some(item),
                        _ => None,
                    })?,
                })
            })())?;
            let (name, value) = fields.next()?;
            if name != "installed_source" {
                return None;
            }
            let installed_source: Vec<u8> = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bytes(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "load_error" {
                return None;
            }
            let load_error: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "paint_dark" {
                return None;
            }
            let paint_dark: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "commented" {
                return None;
            }
            let commented: Vec<i64> = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::List(items) => items
                    .into_iter()
                    .map(|item| match item {
                        ::ducktape_view_guest::wire::SnapshotValue::I64(item) => Some(item),
                        _ => None,
                    })
                    .collect::<Option<Vec<_>>>(),
                _ => None,
            })?;
            Some(Self {
                formatting_notice,
                document,
                history,
                menu,
                source,
                installed_source,
                load_error,
                paint_dark,
                commented,
            })
        })())
        .ok_or_else(|| String::from("snapshot state mismatch"))
    }
}
#[allow(unused_parens)]
impl PagesEditorFixture {
    fn subscription(&self) -> ::ducktape_view_guest::Subscription<Message> {
        ::ducktape_view_guest::Subscription::batch([
            if ((!(self.source.reference).is_empty())
                && (self.source.reference != self.installed_source))
            {
                ::ducktape_view_guest::Subscription::batch([
                    crate::fixture_source::document_source(self.source.clone())
                        .map(move |value| Message::DocumentArrived(value)),
                ])
            } else {
                ::ducktape_view_guest::Subscription::none()
            },
        ])
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn view_fits_default_stack() {
        ::std::thread::Builder::new()
            .stack_size(4 * 1024 * 1024)
            .spawn(|| {
                let (app, _) = PagesEditorFixture::boot();
                let _ = app.view();
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
include!("app_update.rs");
include!("app_view.rs");
