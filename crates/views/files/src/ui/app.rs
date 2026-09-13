use ducktape_view_guest::{kit as native, wire};
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum AppTheme {
    App,
    AppDark,
}
#[derive(Default)]
struct DerivedCache {
    refusal: ::std::cell::OnceCell<String>,
    loading: ::std::cell::OnceCell<bool>,
    draft_here: ::std::cell::OnceCell<bool>,
    draft_parked: ::std::cell::OnceCell<bool>,
    edit_context: ::std::cell::OnceCell<String>,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Tone {
    Light,
    Dark,
}
#[allow(dead_code)]
pub(crate) struct FilesScreenState {
    history_open: bool,
}
impl ::std::default::Default for FilesScreenState {
    fn default() -> Self {
        Self { history_open: false }
    }
}
#[cfg(test)]
#[allow(non_camel_case_types, dead_code)]
#[derive(Clone)]
pub(crate) struct FilesScreenStateSnapshot {
    pub(crate) history_open: bool,
}
#[cfg(test)]
#[allow(dead_code)]
impl FilesView {
    pub(crate) fn test_state_files_screen(
        &self,
        scope: &str,
    ) -> Option<FilesScreenStateSnapshot> {
        let view = |state: &FilesScreenState| FilesScreenStateSnapshot {
            history_open: state.history_open.clone(),
        };
        self.files_screen_states.get(scope).map(view)
    }
    pub(crate) fn test_message_files_screen_fs_toggle_history(scope: String) -> Message {
        Message::FilesScreenFsToggleHistory(scope)
    }
}
#[allow(dead_code)]
pub struct FilesView {
    pub(crate) active_palette: AppTheme,
    pub(crate) connected: bool,
    pub(crate) dark: bool,
    pub(crate) chain: String,
    pub(crate) generation: i64,
    pub(crate) route_serial: i64,
    pub(crate) path: String,
    pub(crate) listed: bool,
    pub(crate) entries: Vec<crate::host::FsEntry>,
    pub(crate) directories: Vec<crate::host::FsEntry>,
    pub(crate) history: Vec<crate::host::FsSnapshot>,
    pub(crate) omitted: i64,
    pub(crate) diff_omitted: i64,
    pub(crate) preview_path: String,
    pub(crate) preview_entry: crate::host::FsEntry,
    pub(crate) preview_base: String,
    pub(crate) preview_text: String,
    pub(crate) preview_display_text: String,
    pub(crate) preview_clipped: bool,
    pub(crate) preview_truncated: bool,
    pub(crate) preview_binary: bool,
    pub(crate) preview_picture: bool,
    pub(crate) preview_width: i64,
    pub(crate) preview_height: i64,
    pub(crate) delete_target: String,
    pub(crate) diff_from: String,
    pub(crate) diff: Vec<crate::host::FsDiffEntry>,
    pub(crate) acting: bool,
    pub(crate) saving: bool,
    pub(crate) notice: String,
    pub(crate) new_name: String,
    pub(crate) draft: ::ducktape_view_guest::Editor,
    pub(crate) editing: bool,
    pub(crate) draft_chain: String,
    pub(crate) draft_path: String,
    pub(crate) draft_base: String,
    pub(crate) draft_id: i64,
    pub(crate) sent: bool,
    pub(crate) viewport_width: f64,
    pub(crate) viewport_height: f64,
    pub(crate) tree_width: f64,
    pub(crate) preview_pane_height: f64,
    pub(crate) object_width: f64,
    pub(crate) derived: DerivedCache,
    pub(crate) preview_text_revision: u64,
    pub(crate) files_screen_states: ::std::collections::HashMap<
        String,
        FilesScreenState,
    >,
    pub(crate) files_screen_initial: FilesScreenState,
}
impl ::std::fmt::Debug for FilesView {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.write_str("FilesView")
    }
}
#[derive(Clone)]
pub enum Message {
    TreeResized(f64, f64),
    PreviewResized(f64, f64),
    ObjectResized(f64, f64),
    ViewportChanged(f64, f64),
    SessionArrived(crate::host::SessionItem),
    RouteTo(String),
    ListingArrived(crate::host::ListingItem),
    PreviewArrived(crate::host::PreviewItem),
    DiffArrived(crate::host::DiffItem),
    ActDone(crate::host::ActItem),
    OpenDirAt(String),
    OpenFileAt(String),
    MkdirSubmit,
    NewFileSubmit,
    ArmDeleteAt(String),
    DisarmDeleteNow,
    DeleteSubmit,
    CloseDiffNow,
    ShowDiffOf(String),
    BeginEdit(String),
    CancelEdit(String),
    DiscardDraft(i64),
    SaveEdit(String),
    OpenLinkAt(String),
    FilesScreenFsToggleHistory(String),
    NewNameChanged(String),
    EditDraft(::ducktape_view_guest::EditorDocumentUpdate),
    DraftTransaction(::ducktape_view_guest::EditorTransaction<Message>),
    Ignore,
}
impl ::std::fmt::Debug for Message {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.write_str("Message")
    }
}
#[allow(unused_parens)]
impl FilesView {
    #[must_use]
    fn derived_refusal(&self) -> &String {
        self.derived
            .refusal
            .get_or_init(|| crate::host::write_refusal(
                ::std::convert::AsRef::as_ref(&(self.path)),
            ))
    }
    fn derived_loading(&self) -> &bool {
        self.derived
            .loading
            .get_or_init(|| {
                ((self.acting || self.saving) || (self.connected && (!self.listed)))
            })
    }
    fn derived_draft_here(&self) -> &bool {
        self.derived
            .draft_here
            .get_or_init(|| {
                ((self.editing && (self.draft_path == self.preview_path))
                    && (self.draft_chain == self.chain))
            })
    }
    fn derived_draft_parked(&self) -> &bool {
        self.derived
            .draft_parked
            .get_or_init(|| (self.editing && (!(*self.derived_draft_here()))))
    }
    fn derived_edit_context(&self) -> &String {
        self.derived
            .edit_context
            .get_or_init(|| {
                crate::host::edit_token(
                    ::std::convert::AsRef::as_ref(&(self.chain)),
                    ::std::convert::AsRef::as_ref(&(self.preview_path)),
                    ::std::convert::AsRef::as_ref(&(self.preview_base)),
                    self.draft_id,
                )
            })
    }
}
#[allow(unused_parens)]
impl FilesView {
    fn state() -> Self {
        Self {
            active_palette: AppTheme::App,
            connected: false,
            dark: false,
            chain: "".to_owned(),
            generation: 0,
            route_serial: 0,
            path: "/shared".to_owned(),
            listed: false,
            entries: Vec::new(),
            directories: Vec::new(),
            history: Vec::new(),
            omitted: 0,
            diff_omitted: 0,
            preview_path: "".to_owned(),
            preview_entry: crate::host::no_fs_entry(),
            preview_base: "".to_owned(),
            preview_text: "".to_owned(),
            preview_display_text: "".to_owned(),
            preview_clipped: false,
            preview_truncated: false,
            preview_binary: false,
            preview_picture: false,
            preview_width: 0,
            preview_height: 0,
            delete_target: "".to_owned(),
            diff_from: "".to_owned(),
            diff: Vec::new(),
            acting: false,
            saving: false,
            notice: "".to_owned(),
            new_name: "".to_owned(),
            draft: ::ducktape_view_guest::Editor::new("".to_owned()),
            editing: false,
            draft_chain: "".to_owned(),
            draft_path: "".to_owned(),
            draft_base: "".to_owned(),
            draft_id: 0,
            sent: false,
            viewport_width: 1280.0,
            viewport_height: 700.0,
            tree_width: 206.0,
            preview_pane_height: 300.0,
            object_width: 306.0,
            derived: ::std::default::Default::default(),
            preview_text_revision: ::ducktape_view_guest::rev::seed(),
            files_screen_states: ::std::collections::HashMap::new(),
            files_screen_initial: ::std::default::Default::default(),
        }
    }
    pub(crate) fn boot() -> (Self, ::ducktape_view_guest::Task<Message>) {
        (Self::state(), ::ducktape_view_guest::Task::none())
    }
    pub(crate) const PREFERRED_WINDOW_SIZE: &'static str = "none";
    pub(crate) const SNAPSHOT_SCHEMA: &'static str = "524542bd8f5b55e48d5274087dd4997a644784746056385b50157aadfe239bba";
    pub(crate) fn snapshot(&self) -> Result<Vec<u8>, String> {
        wire::Snapshot {
            schema: String::from(Self::SNAPSHOT_SCHEMA),
            state: wire::SnapshotValue::Record {
                name: String::from("FilesView"),
                fields: vec![
                    (String::from("active_palette"), match & self.active_palette {
                    AppTheme::App => ::ducktape_view_guest::wire::SnapshotValue::Record {
                    name : String::from("AppTheme"), fields : vec![(String::from("app"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit,)], },
                    AppTheme::AppDark => {
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("AppTheme"), fields : vec![(String::from("app_dark"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit,)], } } },),
                    (String::from("connected"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .connected)),), (String::from("dark"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self.dark)),),
                    (String::from("chain"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.chain),),), (String::from("generation"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .generation)),), (String::from("route_serial"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .route_serial)),), (String::from("path"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.path),),), (String::from("listed"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .listed)),), (String::from("entries"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self.entries)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("FsEntry"), fields : ::std::vec![(String::from("key"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item).key))),
                    (String::from("path"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).path))), (String::from("name"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).name))), (String::from("kind"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).kind))), (String::from("size"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item).size))),
                    (String::from("object"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).object)))], }).collect(),),), (String::from("directories"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self.directories)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("FsEntry"), fields : ::std::vec![(String::from("key"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item).key))),
                    (String::from("path"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).path))), (String::from("name"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).name))), (String::from("kind"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).kind))), (String::from("size"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item).size))),
                    (String::from("object"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).object)))], }).collect(),),), (String::from("history"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self.history)
                    .iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("FsSnapshot"), fields : ::std::vec![(String::from("id"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).id))), (String::from("short_id"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).short_id))), (String::from("author"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).author))), (String::from("height"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (item)
                    .height))), (String::from("message"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).message)))], }).collect(),),), (String::from("omitted"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .omitted)),), (String::from("diff_omitted"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .diff_omitted)),), (String::from("preview_path"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.preview_path),),), (String::from("preview_entry"),
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("FsEntry"), fields : ::std::vec![(String::from("key"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (& self
                    .preview_entry).key))), (String::from("path"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& self.preview_entry).path))), (String::from("name"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& self.preview_entry).name))), (String::from("kind"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& self.preview_entry).kind))), (String::from("size"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& (& self
                    .preview_entry).size))), (String::from("object"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (& self.preview_entry).object)))], },),
                    (String::from("preview_base"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.preview_base),),), (String::from("preview_text"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.preview_text),),), (String::from("preview_display_text"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.preview_display_text),),), (String::from("preview_clipped"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .preview_clipped)),), (String::from("preview_truncated"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .preview_truncated),),), (String::from("preview_binary"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .preview_binary)),), (String::from("preview_picture"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .preview_picture)),), (String::from("preview_width"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .preview_width)),), (String::from("preview_height"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .preview_height)),), (String::from("delete_target"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.delete_target),),), (String::from("diff_from"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.diff_from),),), (String::from("diff"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self.diff).iter()
                    .map(| item | ::ducktape_view_guest::wire::SnapshotValue::Record {
                    name : String::from("FsDiffEntry"), fields :
                    ::std::vec![(String::from("path"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).path))), (String::from("kind"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).kind)))], }).collect(),),), (String::from("acting"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .acting)),), (String::from("saving"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .saving)),), (String::from("notice"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.notice),),), (String::from("new_name"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.new_name),),), (String::from("draft"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bytes((& self.draft)
                    .snapshot()),), (String::from("editing"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .editing)),), (String::from("draft_chain"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.draft_chain),),), (String::from("draft_path"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.draft_path),),), (String::from("draft_base"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.draft_base),),), (String::from("draft_id"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .draft_id)),), (String::from("sent"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self.sent)),),
                    (String::from("viewport_width"),
                    ::ducktape_view_guest::wire::SnapshotValue::F64(* (& self
                    .viewport_width)),), (String::from("viewport_height"),
                    ::ducktape_view_guest::wire::SnapshotValue::F64(* (& self
                    .viewport_height)),), (String::from("tree_width"),
                    ::ducktape_view_guest::wire::SnapshotValue::F64(* (& self
                    .tree_width)),), (String::from("preview_pane_height"),
                    ::ducktape_view_guest::wire::SnapshotValue::F64(* (& self
                    .preview_pane_height),),), (String::from("object_width"),
                    ::ducktape_view_guest::wire::SnapshotValue::F64(* (& self
                    .object_width)),), (String::from("files_screen_states"), { let values
                    = & self.files_screen_states; let mut scopes = values.keys()
                    .collect::< Vec < _ >> (); scopes.sort();
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("FilesScreen instances"), fields : scopes.into_iter()
                    .map(| scope | { let component = & values[scope]; (scope.clone(),
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("FilesScreen"), fields :
                    vec![(String::from("history_open"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& component
                    .history_open),),)], },) }).collect(), } }),
                    (String::from("files_screen_initial"), { let component = & self
                    .files_screen_initial;
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("FilesScreen"), fields :
                    vec![(String::from("history_open"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& component
                    .history_open),),)], } }),
                ],
            },
        }
            .encode()
    }
    pub(crate) fn restore(bytes: &[u8]) -> Result<Self, String> {
        let snapshot = wire::Snapshot::decode(bytes)?;
        if snapshot.schema != Self::SNAPSHOT_SCHEMA {
            return Err(String::from("snapshot schema mismatch"));
        }
        let value = snapshot.state;
        ((|| {
            let wire::SnapshotValue::Record { name, fields } = value else {
                return None;
            };
            if name != "FilesView" || fields.len() != 45 {
                return None;
            }
            let mut fields = fields.into_iter();
            let (name, value) = fields.next()?;
            if name != "active_palette" {
                return None;
            }
            let active_palette: AppTheme = ((|| {
                let wire::SnapshotValue::Record { name, fields } = value else {
                    return None;
                };
                if name != "AppTheme" || fields.len() != 1 {
                    return None;
                }
                let (variant, payload) = fields.into_iter().next()?;
                match variant.as_str() {
                    "app" => {
                        matches!(
                            payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                        )
                            .then_some(AppTheme::App)
                    }
                    "app_dark" => {
                        matches!(
                            payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                        )
                            .then_some(AppTheme::AppDark)
                    }
                    _ => None,
                }
            })())?;
            let (name, value) = fields.next()?;
            if name != "connected" {
                return None;
            }
            let connected: bool = (match value {
                wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "dark" {
                return None;
            }
            let dark: bool = (match value {
                wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "chain" {
                return None;
            }
            let chain: String = (match value {
                wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "generation" {
                return None;
            }
            let generation: i64 = (match value {
                wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "route_serial" {
                return None;
            }
            let route_serial: i64 = (match value {
                wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "path" {
                return None;
            }
            let path: String = (match value {
                wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "listed" {
                return None;
            }
            let listed: bool = (match value {
                wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "entries" {
                return None;
            }
            let entries: Vec<crate::host::FsEntry> = (match value {
                wire::SnapshotValue::List(items) => {
                    items
                        .into_iter()
                        .map(|item| {
                            (|| {
                                let wire::SnapshotValue::Record { name, fields } = item
                                else {
                                    return None;
                                };
                                if name != "FsEntry" || fields.len() != 6 {
                                    return None;
                                }
                                let mut fields = fields.into_iter();
                                let (name, field_0) = fields.next()?;
                                if name != "key" {
                                    return None;
                                }
                                let (name, field_1) = fields.next()?;
                                if name != "path" {
                                    return None;
                                }
                                let (name, field_2) = fields.next()?;
                                if name != "name" {
                                    return None;
                                }
                                let (name, field_3) = fields.next()?;
                                if name != "kind" {
                                    return None;
                                }
                                let (name, field_4) = fields.next()?;
                                if name != "size" {
                                    return None;
                                }
                                let (name, field_5) = fields.next()?;
                                if name != "object" {
                                    return None;
                                }
                                Some(crate::host::FsEntry {
                                    key: (match field_0 {
                                        wire::SnapshotValue::I64(item) => Some(item),
                                        _ => None,
                                    })?,
                                    path: (match field_1 {
                                        wire::SnapshotValue::Str(item) => Some(item),
                                        _ => None,
                                    })?,
                                    name: (match field_2 {
                                        wire::SnapshotValue::Str(item) => Some(item),
                                        _ => None,
                                    })?,
                                    kind: (match field_3 {
                                        wire::SnapshotValue::Str(item) => Some(item),
                                        _ => None,
                                    })?,
                                    size: (match field_4 {
                                        wire::SnapshotValue::I64(item) => Some(item),
                                        _ => None,
                                    })?,
                                    object: (match field_5 {
                                        wire::SnapshotValue::Str(item) => Some(item),
                                        _ => None,
                                    })?,
                                })
                            })()
                        })
                        .collect::<Option<Vec<_>>>()
                }
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "directories" {
                return None;
            }
            let directories: Vec<crate::host::FsEntry> = (match value {
                wire::SnapshotValue::List(items) => {
                    items
                        .into_iter()
                        .map(|item| {
                            (|| {
                                let wire::SnapshotValue::Record { name, fields } = item
                                else {
                                    return None;
                                };
                                if name != "FsEntry" || fields.len() != 6 {
                                    return None;
                                }
                                let mut fields = fields.into_iter();
                                let (name, field_0) = fields.next()?;
                                if name != "key" {
                                    return None;
                                }
                                let (name, field_1) = fields.next()?;
                                if name != "path" {
                                    return None;
                                }
                                let (name, field_2) = fields.next()?;
                                if name != "name" {
                                    return None;
                                }
                                let (name, field_3) = fields.next()?;
                                if name != "kind" {
                                    return None;
                                }
                                let (name, field_4) = fields.next()?;
                                if name != "size" {
                                    return None;
                                }
                                let (name, field_5) = fields.next()?;
                                if name != "object" {
                                    return None;
                                }
                                Some(crate::host::FsEntry {
                                    key: (match field_0 {
                                        wire::SnapshotValue::I64(item) => Some(item),
                                        _ => None,
                                    })?,
                                    path: (match field_1 {
                                        wire::SnapshotValue::Str(item) => Some(item),
                                        _ => None,
                                    })?,
                                    name: (match field_2 {
                                        wire::SnapshotValue::Str(item) => Some(item),
                                        _ => None,
                                    })?,
                                    kind: (match field_3 {
                                        wire::SnapshotValue::Str(item) => Some(item),
                                        _ => None,
                                    })?,
                                    size: (match field_4 {
                                        wire::SnapshotValue::I64(item) => Some(item),
                                        _ => None,
                                    })?,
                                    object: (match field_5 {
                                        wire::SnapshotValue::Str(item) => Some(item),
                                        _ => None,
                                    })?,
                                })
                            })()
                        })
                        .collect::<Option<Vec<_>>>()
                }
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "history" {
                return None;
            }
            let history: Vec<crate::host::FsSnapshot> = (match value {
                wire::SnapshotValue::List(items) => {
                    items
                        .into_iter()
                        .map(|item| {
                            (|| {
                                let wire::SnapshotValue::Record { name, fields } = item
                                else {
                                    return None;
                                };
                                if name != "FsSnapshot" || fields.len() != 5 {
                                    return None;
                                }
                                let mut fields = fields.into_iter();
                                let (name, field_0) = fields.next()?;
                                if name != "id" {
                                    return None;
                                }
                                let (name, field_1) = fields.next()?;
                                if name != "short_id" {
                                    return None;
                                }
                                let (name, field_2) = fields.next()?;
                                if name != "author" {
                                    return None;
                                }
                                let (name, field_3) = fields.next()?;
                                if name != "height" {
                                    return None;
                                }
                                let (name, field_4) = fields.next()?;
                                if name != "message" {
                                    return None;
                                }
                                Some(crate::host::FsSnapshot {
                                    id: (match field_0 {
                                        wire::SnapshotValue::Str(item) => Some(item),
                                        _ => None,
                                    })?,
                                    short_id: (match field_1 {
                                        wire::SnapshotValue::Str(item) => Some(item),
                                        _ => None,
                                    })?,
                                    author: (match field_2 {
                                        wire::SnapshotValue::Str(item) => Some(item),
                                        _ => None,
                                    })?,
                                    height: (match field_3 {
                                        wire::SnapshotValue::I64(item) => Some(item),
                                        _ => None,
                                    })?,
                                    message: (match field_4 {
                                        wire::SnapshotValue::Str(item) => Some(item),
                                        _ => None,
                                    })?,
                                })
                            })()
                        })
                        .collect::<Option<Vec<_>>>()
                }
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "omitted" {
                return None;
            }
            let omitted: i64 = (match value {
                wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "diff_omitted" {
                return None;
            }
            let diff_omitted: i64 = (match value {
                wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "preview_path" {
                return None;
            }
            let preview_path: String = (match value {
                wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "preview_entry" {
                return None;
            }
            let preview_entry: crate::host::FsEntry = ((|| {
                let wire::SnapshotValue::Record { name, fields } = value else {
                    return None;
                };
                if name != "FsEntry" || fields.len() != 6 {
                    return None;
                }
                let mut fields = fields.into_iter();
                let (name, field_0) = fields.next()?;
                if name != "key" {
                    return None;
                }
                let (name, field_1) = fields.next()?;
                if name != "path" {
                    return None;
                }
                let (name, field_2) = fields.next()?;
                if name != "name" {
                    return None;
                }
                let (name, field_3) = fields.next()?;
                if name != "kind" {
                    return None;
                }
                let (name, field_4) = fields.next()?;
                if name != "size" {
                    return None;
                }
                let (name, field_5) = fields.next()?;
                if name != "object" {
                    return None;
                }
                Some(crate::host::FsEntry {
                    key: (match field_0 {
                        wire::SnapshotValue::I64(item) => Some(item),
                        _ => None,
                    })?,
                    path: (match field_1 {
                        wire::SnapshotValue::Str(item) => Some(item),
                        _ => None,
                    })?,
                    name: (match field_2 {
                        wire::SnapshotValue::Str(item) => Some(item),
                        _ => None,
                    })?,
                    kind: (match field_3 {
                        wire::SnapshotValue::Str(item) => Some(item),
                        _ => None,
                    })?,
                    size: (match field_4 {
                        wire::SnapshotValue::I64(item) => Some(item),
                        _ => None,
                    })?,
                    object: (match field_5 {
                        wire::SnapshotValue::Str(item) => Some(item),
                        _ => None,
                    })?,
                })
            })())?;
            let (name, value) = fields.next()?;
            if name != "preview_base" {
                return None;
            }
            let preview_base: String = (match value {
                wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "preview_text" {
                return None;
            }
            let preview_text: String = (match value {
                wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "preview_display_text" {
                return None;
            }
            let preview_display_text: String = (match value {
                wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "preview_clipped" {
                return None;
            }
            let preview_clipped: bool = (match value {
                wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "preview_truncated" {
                return None;
            }
            let preview_truncated: bool = (match value {
                wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "preview_binary" {
                return None;
            }
            let preview_binary: bool = (match value {
                wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "preview_picture" {
                return None;
            }
            let preview_picture: bool = (match value {
                wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "preview_width" {
                return None;
            }
            let preview_width: i64 = (match value {
                wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "preview_height" {
                return None;
            }
            let preview_height: i64 = (match value {
                wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "delete_target" {
                return None;
            }
            let delete_target: String = (match value {
                wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "diff_from" {
                return None;
            }
            let diff_from: String = (match value {
                wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "diff" {
                return None;
            }
            let diff: Vec<crate::host::FsDiffEntry> = (match value {
                wire::SnapshotValue::List(items) => {
                    items
                        .into_iter()
                        .map(|item| {
                            (|| {
                                let wire::SnapshotValue::Record { name, fields } = item
                                else {
                                    return None;
                                };
                                if name != "FsDiffEntry" || fields.len() != 2 {
                                    return None;
                                }
                                let mut fields = fields.into_iter();
                                let (name, field_0) = fields.next()?;
                                if name != "path" {
                                    return None;
                                }
                                let (name, field_1) = fields.next()?;
                                if name != "kind" {
                                    return None;
                                }
                                Some(crate::host::FsDiffEntry {
                                    path: (match field_0 {
                                        wire::SnapshotValue::Str(item) => Some(item),
                                        _ => None,
                                    })?,
                                    kind: (match field_1 {
                                        wire::SnapshotValue::Str(item) => Some(item),
                                        _ => None,
                                    })?,
                                })
                            })()
                        })
                        .collect::<Option<Vec<_>>>()
                }
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "acting" {
                return None;
            }
            let acting: bool = (match value {
                wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "saving" {
                return None;
            }
            let saving: bool = (match value {
                wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "notice" {
                return None;
            }
            let notice: String = (match value {
                wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "new_name" {
                return None;
            }
            let new_name: String = (match value {
                wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "draft" {
                return None;
            }
            let draft: ::ducktape_view_guest::Editor = (match value {
                wire::SnapshotValue::Bytes(bytes) => {
                    ::ducktape_view_guest::Editor::restore(&bytes)
                }
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "editing" {
                return None;
            }
            let editing: bool = (match value {
                wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "draft_chain" {
                return None;
            }
            let draft_chain: String = (match value {
                wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "draft_path" {
                return None;
            }
            let draft_path: String = (match value {
                wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "draft_base" {
                return None;
            }
            let draft_base: String = (match value {
                wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "draft_id" {
                return None;
            }
            let draft_id: i64 = (match value {
                wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "sent" {
                return None;
            }
            let sent: bool = (match value {
                wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "viewport_width" {
                return None;
            }
            let viewport_width: f64 = (match value {
                wire::SnapshotValue::F64(item) if item.is_finite() => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "viewport_height" {
                return None;
            }
            let viewport_height: f64 = (match value {
                wire::SnapshotValue::F64(item) if item.is_finite() => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "tree_width" {
                return None;
            }
            let tree_width: f64 = (match value {
                wire::SnapshotValue::F64(item) if item.is_finite() => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "preview_pane_height" {
                return None;
            }
            let preview_pane_height: f64 = (match value {
                wire::SnapshotValue::F64(item) if item.is_finite() => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "object_width" {
                return None;
            }
            let object_width: f64 = (match value {
                wire::SnapshotValue::F64(item) if item.is_finite() => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "files_screen_states" {
                return None;
            }
            let files_screen_states: ::std::collections::HashMap<
                String,
                FilesScreenState,
            > = ((|| {
                let wire::SnapshotValue::Record { name, fields } = value else {
                    return None;
                };
                if name != "FilesScreen instances" {
                    return None;
                }
                let mut values = ::std::collections::HashMap::new();
                for (scope, value) in fields {
                    let component = ((|| {
                        let wire::SnapshotValue::Record { name, fields } = value else {
                            return None;
                        };
                        if name != "FilesScreen" || fields.len() != 1 {
                            return None;
                        }
                        let mut fields = fields.into_iter();
                        let (name, value) = fields.next()?;
                        if name != "history_open" {
                            return None;
                        }
                        let history_open: bool = (match value {
                            wire::SnapshotValue::Bool(item) => Some(item),
                            _ => None,
                        })?;
                        Some(FilesScreenState {
                            history_open: history_open,
                        })
                    })())?;
                    if values.insert(scope, component).is_some() {
                        return None;
                    }
                }
                Some(values)
            })())?;
            let (name, value) = fields.next()?;
            if name != "files_screen_initial" {
                return None;
            }
            let files_screen_initial: FilesScreenState = ((|| {
                let wire::SnapshotValue::Record { name, fields } = value else {
                    return None;
                };
                if name != "FilesScreen" || fields.len() != 1 {
                    return None;
                }
                let mut fields = fields.into_iter();
                let (name, value) = fields.next()?;
                if name != "history_open" {
                    return None;
                }
                let history_open: bool = (match value {
                    wire::SnapshotValue::Bool(item) => Some(item),
                    _ => None,
                })?;
                Some(FilesScreenState {
                    history_open: history_open,
                })
            })())?;
            Some(Self {
                active_palette: active_palette,
                connected: connected,
                dark: dark,
                chain: chain,
                generation: generation,
                route_serial: route_serial,
                path: path,
                listed: listed,
                entries: entries,
                directories: directories,
                history: history,
                omitted: omitted,
                diff_omitted: diff_omitted,
                preview_path: preview_path,
                preview_entry: preview_entry,
                preview_base: preview_base,
                preview_text: preview_text,
                preview_display_text: preview_display_text,
                preview_clipped: preview_clipped,
                preview_truncated: preview_truncated,
                preview_binary: preview_binary,
                preview_picture: preview_picture,
                preview_width: preview_width,
                preview_height: preview_height,
                delete_target: delete_target,
                diff_from: diff_from,
                diff: diff,
                acting: acting,
                saving: saving,
                notice: notice,
                new_name: new_name,
                draft: draft,
                editing: editing,
                draft_chain: draft_chain,
                draft_path: draft_path,
                draft_base: draft_base,
                draft_id: draft_id,
                sent: sent,
                viewport_width: viewport_width,
                viewport_height: viewport_height,
                tree_width: tree_width,
                preview_pane_height: preview_pane_height,
                object_width: object_width,
                derived: ::std::default::Default::default(),
                preview_text_revision: ::ducktape_view_guest::rev::seed(),
                files_screen_states: files_screen_states,
                files_screen_initial: files_screen_initial,
            })
        })())
            .ok_or_else(|| String::from("snapshot state mismatch"))
    }
}
#[allow(unused_parens)]
impl FilesView {
    pub(crate) fn subscription(&self) -> ::ducktape_view_guest::Subscription<Message> {
        ::ducktape_view_guest::Subscription::batch([
            crate::host::session().map(move |value| Message::SessionArrived(value)),
            if self.connected {
                ::ducktape_view_guest::Subscription::batch([
                    crate::host::listing(self.generation, self.path.to_owned())
                        .map(move |value| Message::ListingArrived(value)),
                ])
            } else {
                ::ducktape_view_guest::Subscription::none()
            },
            if (self.connected && (!(self.preview_path).is_empty())) {
                ::ducktape_view_guest::Subscription::batch([
                    crate::host::preview(self.generation, self.preview_path.to_owned())
                        .map(move |value| Message::PreviewArrived(value)),
                ])
            } else {
                ::ducktape_view_guest::Subscription::none()
            },
            if (self.connected && (!(self.diff_from).is_empty())) {
                ::ducktape_view_guest::Subscription::batch([
                    crate::host::diff(self.generation, self.diff_from.to_owned())
                        .map(move |value| Message::DiffArrived(value)),
                ])
            } else {
                ::ducktape_view_guest::Subscription::none()
            },
            crate::host::acts().map(move |value| Message::ActDone(value)),
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
                let (app, _) = FilesView::boot();
                let _ = app.view();
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
mod app_update;
mod app_view;
mod browser;
mod files;
mod kit;
