//! Settings as a module-owned view: this device's preferences and the account
//! this key speaks for, from the facts the desktop app pushes. Every act — a
//! theme, a rename, a minted ticket, the signing seat — leaves as an intent
//! the app signs.
pub mod host;
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum AppTheme {
    App,
    AppDark,
}

#[derive(Default)]
struct DerivedCache {
    account_keys: ::std::cell::OnceCell<i64>,
}
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum SettingsPane {
    General,
    Network,
    Account,
    Security,
}
#[allow(dead_code)]
pub(crate) struct SettingsScreenState {
    key_pw: String,
    settings_pane: SettingsPane,
}
impl ::std::default::Default for SettingsScreenState {
    fn default() -> Self {
        Self {
            key_pw: "".to_owned(),
            settings_pane: SettingsPane::General,
        }
    }
}
#[allow(dead_code)]
pub struct SettingsView {
    pub(crate) active_palette: AppTheme,
    pub(crate) connected: bool,
    pub(crate) loading: bool,
    pub(crate) status: String,
    pub(crate) busy: bool,
    pub(crate) recovering: bool,
    pub(crate) appearance: String,
    pub(crate) desktop_notifications: bool,
    pub(crate) unlocked: bool,
    pub(crate) seat_key: String,
    pub(crate) account_name: String,
    pub(crate) network_name: String,
    pub(crate) connected_rpc: String,
    pub(crate) account_ceremony_phase: String,
    pub(crate) account_ceremony_qr: String,
    pub(crate) account_ceremony_detail: String,
    pub(crate) account_ceremony_left: String,
    pub(crate) settings_key_state: String,
    pub(crate) settings_key_path: String,
    pub(crate) account_number: String,
    pub(crate) account_exists: bool,
    pub(crate) account_busy: bool,
    pub(crate) account_ticket: String,
    pub(crate) connection_serial: i64,
    pub(crate) tier: String,
    pub(crate) admin: bool,
    pub(crate) members_line: String,
    pub(crate) members_answered: bool,
    pub(crate) account_key_rows: Vec<crate::host::AccountKeyRow>,
    pub(crate) renaming_to: String,
    pub(crate) account_name_draft: String,
    pub(crate) account_create_draft: String,
    pub(crate) account_key_draft: String,
    pub(crate) account_key_label_draft: String,
    pub(crate) account_join_draft: String,
    pub(crate) host_error: String,
    pub(crate) sent: bool,
    pub(crate) derived: DerivedCache,
    pub(crate) settings_screen_states: ::std::collections::HashMap<String, SettingsScreenState>,
    pub(crate) settings_screen_initial: SettingsScreenState,
}
impl ::std::fmt::Debug for SettingsView {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.write_str("SettingsView")
    }
}
#[derive(Clone)]
pub enum Message {
    SessionArrived(crate::host::SessionItem),
    StandingArrived(crate::host::StandingItem),
    KeysArrived(crate::host::KeysItem),
    ShowTab(String),
    Reconnect,
    SwitchNetwork,
    SettingsUnlockSubmit(String),
    LockSession,
    AccountRenameSubmit,
    AccountCreateSubmit,
    AccountKeyAddSubmit,
    AccountKeyJoinSubmit,
    AccountKeyRemove(String),
    AccountPasskeySubmit,
    AccountPasskeyDesktop,
    AccountCeremonyCancel,
    AccountWalletSubmit,
    AccountLoginSubmit,
    CopyToClipboard(String, String),
    SetAppearanceLight,
    SetAppearanceDark,
    SetDesktopNotifications(bool),
    PickSettingsPane(String, SettingsPane),
    EditSettingsPassword(String, String),
    BindAccountNameDraft(String),
    BindAccountCreateDraft(String),
    BindAccountJoinDraft(String),
    BindAccountKeyDraft(String),
    BindAccountKeyLabelDraft(String),
}
impl ::std::fmt::Debug for Message {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.write_str("Message")
    }
}
#[allow(unused_parens)]
impl SettingsView {
    #[must_use]
    fn derived_account_keys(&self) -> &i64 {
        self.derived
            .account_keys
            .get_or_init(|| ((self.account_key_rows).len() as i64))
    }
}

#[allow(unused_parens)]
impl SettingsView {
    fn state() -> Self {
        Self {
            active_palette: AppTheme::App,
            connected: false,
            loading: false,
            status: "".to_owned(),
            busy: false,
            recovering: false,
            appearance: "system".to_owned(),
            desktop_notifications: true,
            unlocked: false,
            seat_key: "".to_owned(),
            account_name: "".to_owned(),
            network_name: "".to_owned(),
            connected_rpc: "".to_owned(),
            account_ceremony_phase: "".to_owned(),
            account_ceremony_qr: "".to_owned(),
            account_ceremony_detail: "".to_owned(),
            account_ceremony_left: "".to_owned(),
            settings_key_state: "".to_owned(),
            settings_key_path: "".to_owned(),
            account_number: "".to_owned(),
            account_exists: false,
            account_busy: false,
            account_ticket: "".to_owned(),
            connection_serial: 0,
            tier: "".to_owned(),
            admin: false,
            members_line: "".to_owned(),
            members_answered: false,
            account_key_rows: Vec::new(),
            renaming_to: "".to_owned(),
            account_name_draft: "".to_owned(),
            account_create_draft: "".to_owned(),
            account_key_draft: "".to_owned(),
            account_key_label_draft: "".to_owned(),
            account_join_draft: "".to_owned(),
            host_error: "".to_owned(),
            sent: false,
            derived: ::std::default::Default::default(),
            settings_screen_states: ::std::collections::HashMap::new(),
            settings_screen_initial: ::std::default::Default::default(),
        }
    }
    pub(crate) fn boot() -> (Self, ::ducktape_view_guest::Task<Message>) {
        (Self::state(), ::ducktape_view_guest::Task::none())
    }
    pub(crate) const PREFERRED_WINDOW_SIZE: &'static str = "none";
    #[allow(clippy::too_many_arguments)]
    fn restore_state(
        active_palette: AppTheme,
        connected: bool,
        loading: bool,
        status: String,
        busy: bool,
        recovering: bool,
        appearance: String,
        desktop_notifications: bool,
        unlocked: bool,
        seat_key: String,
        account_name: String,
        network_name: String,
        connected_rpc: String,
        account_ceremony_phase: String,
        account_ceremony_qr: String,
        account_ceremony_detail: String,
        account_ceremony_left: String,
        settings_key_state: String,
        settings_key_path: String,
        account_number: String,
        account_exists: bool,
        account_busy: bool,
        account_ticket: String,
        connection_serial: i64,
        tier: String,
        admin: bool,
        members_line: String,
        members_answered: bool,
        account_key_rows: Vec<crate::host::AccountKeyRow>,
        renaming_to: String,
        account_name_draft: String,
        account_create_draft: String,
        account_key_draft: String,
        account_key_label_draft: String,
        account_join_draft: String,
        host_error: String,
        sent: bool,
        settings_screen_states: ::std::collections::HashMap<String, SettingsScreenState>,
        settings_screen_initial: SettingsScreenState,
    ) -> Self {
        Self {
            active_palette: active_palette,
            connected: connected,
            loading: loading,
            status: status,
            busy: busy,
            recovering: recovering,
            appearance: appearance,
            desktop_notifications: desktop_notifications,
            unlocked: unlocked,
            seat_key: seat_key,
            account_name: account_name,
            network_name: network_name,
            connected_rpc: connected_rpc,
            account_ceremony_phase: account_ceremony_phase,
            account_ceremony_qr: account_ceremony_qr,
            account_ceremony_detail: account_ceremony_detail,
            account_ceremony_left: account_ceremony_left,
            settings_key_state: settings_key_state,
            settings_key_path: settings_key_path,
            account_number: account_number,
            account_exists: account_exists,
            account_busy: account_busy,
            account_ticket: account_ticket,
            connection_serial: connection_serial,
            tier: tier,
            admin: admin,
            members_line: members_line,
            members_answered: members_answered,
            account_key_rows: account_key_rows,
            renaming_to: renaming_to,
            account_name_draft: account_name_draft,
            account_create_draft: account_create_draft,
            account_key_draft: account_key_draft,
            account_key_label_draft: account_key_label_draft,
            account_join_draft: account_join_draft,
            host_error: host_error,
            sent: sent,
            derived: ::std::default::Default::default(),
            settings_screen_states: settings_screen_states,
            settings_screen_initial: settings_screen_initial,
        }
    }
    pub(crate) const SNAPSHOT_SCHEMA: &'static str =
        "f8c4c32da46fa9482206b251b5b9979dfe6115d847f004e01847b0543779d57c";
    pub(crate) fn snapshot(&self) -> Result<Vec<u8>, String> {
        ::ducktape_view_guest::wire::Snapshot {
            schema: String::from(Self::SNAPSHOT_SCHEMA),
            state: ::ducktape_view_guest::wire::SnapshotValue::Record {
                name: String::from("SettingsView"),
                fields: vec![
                    (String::from("active_palette"), match & self.active_palette {
                    AppTheme::App => ::ducktape_view_guest::wire::SnapshotValue::Record {
                    name : String::from("AppTheme"), fields : vec![(String::from("app"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] },
                    AppTheme::AppDark =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("AppTheme"), fields : vec![(String::from("app_dark"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] } }),
                    (String::from("connected"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .connected))), (String::from("loading"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .loading))), (String::from("status"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.status))), (String::from("busy"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self.busy))),
                    (String::from("recovering"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .recovering))), (String::from("appearance"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.appearance))), (String::from("desktop_notifications"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .desktop_notifications))), (String::from("unlocked"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .unlocked))), (String::from("seat_key"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.seat_key))), (String::from("account_name"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.account_name))), (String::from("network_name"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.network_name))), (String::from("connected_rpc"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.connected_rpc))), (String::from("account_ceremony_phase"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.account_ceremony_phase))), (String::from("account_ceremony_qr"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.account_ceremony_qr))),
                    (String::from("account_ceremony_detail"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.account_ceremony_detail))),
                    (String::from("account_ceremony_left"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.account_ceremony_left))), (String::from("settings_key_state"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.settings_key_state))), (String::from("settings_key_path"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.settings_key_path))), (String::from("account_number"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.account_number))), (String::from("account_exists"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .account_exists))), (String::from("account_busy"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .account_busy))), (String::from("account_ticket"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.account_ticket))), (String::from("connection_serial"),
                    ::ducktape_view_guest::wire::SnapshotValue::I64(* (& self
                    .connection_serial))), (String::from("tier"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.tier))), (String::from("admin"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self.admin))),
                    (String::from("members_line"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.members_line))), (String::from("members_answered"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self
                    .members_answered))), (String::from("account_key_rows"),
                    ::ducktape_view_guest::wire::SnapshotValue::List((& self
                    .account_key_rows).iter().map(| item |
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("AccountKeyRow"), fields :
                    ::std::vec![(String::from("scheme"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).scheme))), (String::from("pubkey"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).pubkey))), (String::from("label"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    (item).label)))] }).collect())), (String::from("renaming_to"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.renaming_to))), (String::from("account_name_draft"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.account_name_draft))), (String::from("account_create_draft"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.account_create_draft))), (String::from("account_key_draft"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.account_key_draft))), (String::from("account_key_label_draft"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.account_key_label_draft))), (String::from("account_join_draft"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.account_join_draft))), (String::from("host_error"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    self.host_error))), (String::from("sent"),
                    ::ducktape_view_guest::wire::SnapshotValue::Bool(* (& self.sent))),
                    (String::from("settings_screen_states"), { let values = & self
                    .settings_screen_states; let mut scopes = values.keys().collect:: <
                    Vec < _ > > (); scopes.sort();
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("SettingsScreen instances"), fields : scopes.into_iter()
                    .map(| scope | { let component = & values[scope]; (scope.clone(),
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("SettingsScreen"), fields :
                    vec![(String::from("key_pw"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    component.key_pw))), (String::from("settings_pane"), match &
                    component.settings_pane { SettingsPane::General =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("SettingsPane"), fields : vec![(String::from("general"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] },
                    SettingsPane::Network =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("SettingsPane"), fields : vec![(String::from("network"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] },
                    SettingsPane::Account =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("SettingsPane"), fields : vec![(String::from("account"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] },
                    SettingsPane::Security =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("SettingsPane"), fields :
                    vec![(String::from("security"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] } })] }) })
                    .collect() } }), (String::from("settings_screen_initial"), { let
                    component = & self.settings_screen_initial;
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("SettingsScreen"), fields :
                    vec![(String::from("key_pw"),
                    ::ducktape_view_guest::wire::SnapshotValue::Str(::std::string::ToString::to_string(&
                    component.key_pw))), (String::from("settings_pane"), match &
                    component.settings_pane { SettingsPane::General =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("SettingsPane"), fields : vec![(String::from("general"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] },
                    SettingsPane::Network =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("SettingsPane"), fields : vec![(String::from("network"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] },
                    SettingsPane::Account =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("SettingsPane"), fields : vec![(String::from("account"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] },
                    SettingsPane::Security =>
                    ::ducktape_view_guest::wire::SnapshotValue::Record { name :
                    String::from("SettingsPane"), fields :
                    vec![(String::from("security"),
                    ::ducktape_view_guest::wire::SnapshotValue::Unit)] } })] } })
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
            if name != "SettingsView" || fields.len() != 39 {
                return None;
            }
            let mut fields = fields.into_iter();
            let (name, value) = fields.next()?;
            if name != "active_palette" {
                return None;
            }
            let active_palette: AppTheme = ((|| {
                let ::ducktape_view_guest::wire::SnapshotValue::Record {
                    name: name,
                    fields: fields,
                } = value
                else {
                    return None;
                };
                if name != "AppTheme" || fields.len() != 1 {
                    return None;
                }
                let (variant, payload) = fields.into_iter().next()?;
                match variant.as_str() {
                    "app" => matches!(payload, ::ducktape_view_guest::wire::SnapshotValue::Unit)
                        .then_some(AppTheme::App),
                    "app_dark" => {
                        matches!(payload, ::ducktape_view_guest::wire::SnapshotValue::Unit)
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
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "loading" {
                return None;
            }
            let loading: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "status" {
                return None;
            }
            let status: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "busy" {
                return None;
            }
            let busy: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "recovering" {
                return None;
            }
            let recovering: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "appearance" {
                return None;
            }
            let appearance: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "desktop_notifications" {
                return None;
            }
            let desktop_notifications: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "unlocked" {
                return None;
            }
            let unlocked: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "seat_key" {
                return None;
            }
            let seat_key: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "account_name" {
                return None;
            }
            let account_name: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "network_name" {
                return None;
            }
            let network_name: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "connected_rpc" {
                return None;
            }
            let connected_rpc: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "account_ceremony_phase" {
                return None;
            }
            let account_ceremony_phase: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "account_ceremony_qr" {
                return None;
            }
            let account_ceremony_qr: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "account_ceremony_detail" {
                return None;
            }
            let account_ceremony_detail: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "account_ceremony_left" {
                return None;
            }
            let account_ceremony_left: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "settings_key_state" {
                return None;
            }
            let settings_key_state: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "settings_key_path" {
                return None;
            }
            let settings_key_path: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "account_number" {
                return None;
            }
            let account_number: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "account_exists" {
                return None;
            }
            let account_exists: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "account_busy" {
                return None;
            }
            let account_busy: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "account_ticket" {
                return None;
            }
            let account_ticket: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "connection_serial" {
                return None;
            }
            let connection_serial: i64 = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::I64(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "tier" {
                return None;
            }
            let tier: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "admin" {
                return None;
            }
            let admin: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "members_line" {
                return None;
            }
            let members_line: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "members_answered" {
                return None;
            }
            let members_answered: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "account_key_rows" {
                return None;
            }
            let account_key_rows: Vec<crate::host::AccountKeyRow> = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::List(items) => items
                    .into_iter()
                    .map(|item| {
                        (|| {
                            let ::ducktape_view_guest::wire::SnapshotValue::Record {
                                name: name,
                                fields: fields,
                            } = item
                            else {
                                return None;
                            };
                            if name != "AccountKeyRow" || fields.len() != 3 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, field_0) = fields.next()?;
                            if name != "scheme" {
                                return None;
                            }
                            let (name, field_1) = fields.next()?;
                            if name != "pubkey" {
                                return None;
                            }
                            let (name, field_2) = fields.next()?;
                            if name != "label" {
                                return None;
                            }
                            Some(crate::host::AccountKeyRow {
                                scheme: (match field_0 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                pubkey: (match field_1 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                label: (match field_2 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                            })
                        })()
                    })
                    .collect::<Option<Vec<_>>>(),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "renaming_to" {
                return None;
            }
            let renaming_to: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "account_name_draft" {
                return None;
            }
            let account_name_draft: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "account_create_draft" {
                return None;
            }
            let account_create_draft: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "account_key_draft" {
                return None;
            }
            let account_key_draft: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "account_key_label_draft" {
                return None;
            }
            let account_key_label_draft: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "account_join_draft" {
                return None;
            }
            let account_join_draft: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "host_error" {
                return None;
            }
            let host_error: String = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "sent" {
                return None;
            }
            let sent: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            let (name, value) = fields.next()?;
            if name != "settings_screen_states" {
                return None;
            }
            let settings_screen_states: ::std::collections::HashMap<String, SettingsScreenState> =
                ((|| {
                    let ::ducktape_view_guest::wire::SnapshotValue::Record {
                        name: name,
                        fields: fields,
                    } = value
                    else {
                        return None;
                    };
                    if name != "SettingsScreen instances" {
                        return None;
                    }
                    let mut values = ::std::collections::HashMap::new();
                    for (scope, value) in fields {
                        let component = ((|| {
                            let ::ducktape_view_guest::wire::SnapshotValue::Record {
                                name: name,
                                fields: fields,
                            } = value
                            else {
                                return None;
                            };
                            if name != "SettingsScreen" || fields.len() != 2 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, value) = fields.next()?;
                            if name != "key_pw" {
                                return None;
                            }
                            let key_pw: String = (match value {
                                ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                                _ => None,
                            })?;
                            let (name, value) = fields.next()?;
                            if name != "settings_pane" {
                                return None;
                            }
                            let settings_pane: SettingsPane = ((|| {
                                let ::ducktape_view_guest::wire::SnapshotValue::Record {
                                    name: name,
                                    fields: fields,
                                } = value
                                else {
                                    return None;
                                };
                                if name != "SettingsPane" || fields.len() != 1 {
                                    return None;
                                }
                                let (variant, payload) = fields.into_iter().next()?;
                                match variant.as_str() {
                                    "general" => matches!(
                                        payload,
                                        ::ducktape_view_guest::wire::SnapshotValue::Unit
                                    )
                                    .then_some(SettingsPane::General),
                                    "network" => matches!(
                                        payload,
                                        ::ducktape_view_guest::wire::SnapshotValue::Unit
                                    )
                                    .then_some(SettingsPane::Network),
                                    "account" => matches!(
                                        payload,
                                        ::ducktape_view_guest::wire::SnapshotValue::Unit
                                    )
                                    .then_some(SettingsPane::Account),
                                    "security" => matches!(
                                        payload,
                                        ::ducktape_view_guest::wire::SnapshotValue::Unit
                                    )
                                    .then_some(SettingsPane::Security),
                                    _ => None,
                                }
                            })())?;
                            Some(SettingsScreenState {
                                key_pw: key_pw,
                                settings_pane: settings_pane,
                            })
                        })())?;
                        if values.insert(scope, component).is_some() {
                            return None;
                        }
                    }
                    Some(values)
                })())?;
            let (name, value) = fields.next()?;
            if name != "settings_screen_initial" {
                return None;
            }
            let settings_screen_initial: SettingsScreenState = ((|| {
                let ::ducktape_view_guest::wire::SnapshotValue::Record {
                    name: name,
                    fields: fields,
                } = value
                else {
                    return None;
                };
                if name != "SettingsScreen" || fields.len() != 2 {
                    return None;
                }
                let mut fields = fields.into_iter();
                let (name, value) = fields.next()?;
                if name != "key_pw" {
                    return None;
                }
                let key_pw: String = (match value {
                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => Some(item),
                    _ => None,
                })?;
                let (name, value) = fields.next()?;
                if name != "settings_pane" {
                    return None;
                }
                let settings_pane: SettingsPane = ((|| {
                    let ::ducktape_view_guest::wire::SnapshotValue::Record {
                        name: name,
                        fields: fields,
                    } = value
                    else {
                        return None;
                    };
                    if name != "SettingsPane" || fields.len() != 1 {
                        return None;
                    }
                    let (variant, payload) = fields.into_iter().next()?;
                    match variant.as_str() {
                        "general" => {
                            matches!(payload, ::ducktape_view_guest::wire::SnapshotValue::Unit)
                                .then_some(SettingsPane::General)
                        }
                        "network" => {
                            matches!(payload, ::ducktape_view_guest::wire::SnapshotValue::Unit)
                                .then_some(SettingsPane::Network)
                        }
                        "account" => {
                            matches!(payload, ::ducktape_view_guest::wire::SnapshotValue::Unit)
                                .then_some(SettingsPane::Account)
                        }
                        "security" => {
                            matches!(payload, ::ducktape_view_guest::wire::SnapshotValue::Unit)
                                .then_some(SettingsPane::Security)
                        }
                        _ => None,
                    }
                })())?;
                Some(SettingsScreenState {
                    key_pw: key_pw,
                    settings_pane: settings_pane,
                })
            })())?;
            Some(Self::restore_state(
                active_palette,
                connected,
                loading,
                status,
                busy,
                recovering,
                appearance,
                desktop_notifications,
                unlocked,
                seat_key,
                account_name,
                network_name,
                connected_rpc,
                account_ceremony_phase,
                account_ceremony_qr,
                account_ceremony_detail,
                account_ceremony_left,
                settings_key_state,
                settings_key_path,
                account_number,
                account_exists,
                account_busy,
                account_ticket,
                connection_serial,
                tier,
                admin,
                members_line,
                members_answered,
                account_key_rows,
                renaming_to,
                account_name_draft,
                account_create_draft,
                account_key_draft,
                account_key_label_draft,
                account_join_draft,
                host_error,
                sent,
                settings_screen_states,
                settings_screen_initial,
            ))
        })())
        .ok_or_else(|| String::from("snapshot state mismatch"))
    }
}
#[allow(unused_parens)]
impl SettingsView {
    fn subscription(&self) -> ::ducktape_view_guest::Subscription<Message> {
        ::ducktape_view_guest::Subscription::batch([
            crate::host::session().map(move |value| Message::SessionArrived(value)),
            if self.connected {
                ::ducktape_view_guest::Subscription::batch([crate::host::standing(
                    self.connection_serial,
                )
                .map(move |value| Message::StandingArrived(value))])
            } else {
                ::ducktape_view_guest::Subscription::none()
            },
            if (self.connected && (!(self.seat_key).is_empty())) {
                ::ducktape_view_guest::Subscription::batch([crate::host::account_keys(
                    self.connection_serial,
                    self.seat_key.to_owned(),
                )
                .map(move |value| Message::KeysArrived(value))])
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
                let (app, _) = SettingsView::boot();
                let _ = app.view();
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
impl SettingsView {
    #[allow(clippy::assign_op_pattern)]
    pub(crate) fn update(&mut self, message: Message) -> ::ducktape_view_guest::Task<Message> {
        match message {
            Message::SessionArrived(item) => self.on_session_arrived(item),
            Message::StandingArrived(item) => self.on_standing_arrived(item),
            Message::KeysArrived(item) => self.on_keys_arrived(item),
            Message::ShowTab(tab) => self.on_show_tab(tab),
            Message::Reconnect => self.on_reconnect(),
            Message::SwitchNetwork => self.on_switch_network(),
            Message::SettingsUnlockSubmit(pw) => self.on_settings_unlock_submit(pw),
            Message::LockSession => self.on_lock_session(),
            Message::AccountRenameSubmit => self.on_account_rename_submit(),
            Message::AccountCreateSubmit => self.on_account_create_submit(),
            Message::AccountKeyAddSubmit => self.on_account_key_add_submit(),
            Message::AccountKeyJoinSubmit => self.on_account_key_join_submit(),
            Message::AccountKeyRemove(pubkey) => self.on_account_key_remove(pubkey),
            Message::AccountPasskeySubmit => self.on_account_passkey_submit(),
            Message::AccountPasskeyDesktop => self.on_account_passkey_desktop(),
            Message::AccountCeremonyCancel => self.on_account_ceremony_cancel(),
            Message::AccountWalletSubmit => self.on_account_wallet_submit(),
            Message::AccountLoginSubmit => self.on_account_login_submit(),
            Message::CopyToClipboard(text, label) => self.on_copy_to_clipboard(text, label),
            Message::SetAppearanceLight => self.on_set_appearance_light(),
            Message::SetAppearanceDark => self.on_set_appearance_dark(),
            Message::SetDesktopNotifications(enabled) => self.on_set_desktop_notifications(enabled),
            Message::PickSettingsPane(scope, picked) => self.on_pick_settings_pane(scope, picked),
            Message::EditSettingsPassword(scope, value) => {
                self.on_edit_settings_password(scope, value)
            }
            Message::BindAccountNameDraft(value) => self.on_bind_account_name_draft(value),
            Message::BindAccountCreateDraft(value) => self.on_bind_account_create_draft(value),
            Message::BindAccountJoinDraft(value) => self.on_bind_account_join_draft(value),
            Message::BindAccountKeyDraft(value) => self.on_bind_account_key_draft(value),
            Message::BindAccountKeyLabelDraft(value) => self.on_bind_account_key_label_draft(value),
        }
    }
    fn on_session_arrived(
        &mut self,
        item: crate::host::SessionItem,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.host_error = item.error.to_owned();
            }
            if (!(item.error).is_empty()) {
                return ::ducktape_view_guest::Task::none();
            }
            let next = item.next.clone();
            {
                self.connection_serial = crate::host::connection_serial_after(
                    self.connected,
                    next.connected,
                    self.connection_serial,
                );
            }
            {
                self.connected = next.connected;
            }
            {
                self.loading = next.loading;
            }
            {
                self.status = next.status.to_owned();
            }
            {
                self.busy = next.busy;
            }
            {
                self.recovering = next.recovering;
            }
            {
                self.appearance = next.appearance.to_owned();
            }
            {
                self.desktop_notifications = next.desktop_notifications;
            }
            {
                self.unlocked = next.unlocked;
            }
            {
                self.seat_key = next.seat_key.to_owned();
            }
            {
                self.account_name = next.account_name.to_owned();
            }
            {
                self.network_name = next.network_name.to_owned();
            }
            {
                self.connected_rpc = next.connected_rpc.to_owned();
            }
            {
                self.account_ceremony_phase = next.account_ceremony_phase.to_owned();
            }
            {
                self.account_ceremony_qr = next.account_ceremony_qr.to_owned();
            }
            {
                self.account_ceremony_detail = next.account_ceremony_detail.to_owned();
            }
            {
                self.account_ceremony_left = next.account_ceremony_left.to_owned();
            }
            {
                self.settings_key_state = next.settings_key_state.to_owned();
            }
            {
                self.settings_key_path = next.settings_key_path.to_owned();
            }
            {
                self.account_busy = next.account_busy;
            }
            {
                self.account_ticket = next.account_ticket.to_owned();
            }
            let renamed = crate::host::renamed_to(
                ::std::convert::AsRef::as_ref(&(next.account_name)),
                ::std::convert::AsRef::as_ref(&(self.renaming_to)),
            );
            {
                self.renaming_to = crate::host::keep_draft(
                    renamed,
                    ::std::convert::AsRef::as_ref(&(self.renaming_to)),
                );
            }
            {
                self.account_name_draft = crate::host::keep_draft(
                    renamed,
                    ::std::convert::AsRef::as_ref(&(self.account_name_draft)),
                );
            }
            let founded = (next.account_exists && (!self.account_exists));
            {
                self.account_exists = next.account_exists;
            }
            {
                self.account_number = next.account_number.to_owned();
            }
            {
                self.account_create_draft = crate::host::keep_draft(
                    founded,
                    ::std::convert::AsRef::as_ref(&(self.account_create_draft)),
                );
            }
            {
                self.account_join_draft = crate::host::keep_draft(
                    founded,
                    ::std::convert::AsRef::as_ref(&(self.account_join_draft)),
                );
            }
            let minted = (!(next.account_ticket).is_empty());
            {
                self.account_key_draft = crate::host::keep_draft(
                    minted,
                    ::std::convert::AsRef::as_ref(&(self.account_key_draft)),
                );
            }
            {
                self.account_key_label_draft = crate::host::keep_draft(
                    minted,
                    ::std::convert::AsRef::as_ref(&(self.account_key_label_draft)),
                );
            }
            {
                self.active_palette = AppTheme::App;
            }
            if (!next.dark) {
                return ::ducktape_view_guest::Task::none();
            }
            {
                self.active_palette = AppTheme::AppDark;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_standing_arrived(
        &mut self,
        item: crate::host::StandingItem,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.host_error = item.error.to_owned();
            }
            {
                self.members_answered = item.answered;
            }
            if (!(item.error).is_empty()) {
                return ::ducktape_view_guest::Task::none();
            }
            {
                self.tier = item.next.tier.to_owned();
            }
            {
                self.admin = item.next.admin;
            }
            {
                self.members_line = item.next.members_line.to_owned();
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_keys_arrived(
        &mut self,
        item: crate::host::KeysItem,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.host_error = item.error.to_owned();
            }
            if (!(item.error).is_empty()) {
                return ::ducktape_view_guest::Task::none();
            }
            {
                self.account_key_rows = item.rows.clone();
                self.derived.account_keys.take();
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_show_tab(&mut self, tab: String) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.sent = crate::host::open_tab(::std::convert::AsRef::as_ref(&(tab)));
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_reconnect(&mut self) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.sent = crate::host::reconnect_network();
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_switch_network(&mut self) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.sent = crate::host::switch_workspace();
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_settings_unlock_submit(&mut self, pw: String) -> ::ducktape_view_guest::Task<Message> {
        {
            if (self.busy || (pw).is_empty()) {
                return ::ducktape_view_guest::Task::none();
            }
            {
                self.sent = crate::host::unlock(::std::convert::AsRef::as_ref(&(pw)));
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_lock_session(&mut self) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.sent = crate::host::lock();
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_account_rename_submit(&mut self) -> ::ducktape_view_guest::Task<Message> {
        {
            if (self.account_busy || ((self.account_name_draft).trim().to_owned()).is_empty()) {
                return ::ducktape_view_guest::Task::none();
            }
            {
                self.renaming_to = (self.account_name_draft).trim().to_owned();
            }
            {
                self.sent = crate::host::rename_account(::std::convert::AsRef::as_ref(
                    &((self.account_name_draft).trim().to_owned()),
                ));
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_account_create_submit(&mut self) -> ::ducktape_view_guest::Task<Message> {
        {
            if ((self.account_busy || (!self.unlocked))
                || ((self.account_create_draft).trim().to_owned()).is_empty())
            {
                return ::ducktape_view_guest::Task::none();
            }
            {
                self.sent = crate::host::create_account(::std::convert::AsRef::as_ref(
                    &((self.account_create_draft).trim().to_owned()),
                ));
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_account_key_add_submit(&mut self) -> ::ducktape_view_guest::Task<Message> {
        {
            if ((self.account_busy || (!self.unlocked))
                || ((self.account_key_draft).trim().to_owned()).is_empty())
            {
                return ::ducktape_view_guest::Task::none();
            }
            {
                self.sent = crate::host::mint_ticket(
                    ::std::convert::AsRef::as_ref(&((self.account_key_draft).trim().to_owned())),
                    ::std::convert::AsRef::as_ref(
                        &((self.account_key_label_draft).trim().to_owned()),
                    ),
                );
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_account_key_join_submit(&mut self) -> ::ducktape_view_guest::Task<Message> {
        {
            if ((self.account_busy || (!self.unlocked))
                || ((self.account_join_draft).trim().to_owned()).is_empty())
            {
                return ::ducktape_view_guest::Task::none();
            }
            {
                self.sent = crate::host::join_account(::std::convert::AsRef::as_ref(
                    &((self.account_join_draft).trim().to_owned()),
                ));
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_account_key_remove(&mut self, pubkey: String) -> ::ducktape_view_guest::Task<Message> {
        {
            if ((self.account_busy || (!self.unlocked)) || ((*self.derived_account_keys()) <= 1)) {
                return ::ducktape_view_guest::Task::none();
            }
            {
                self.sent = crate::host::remove_key(::std::convert::AsRef::as_ref(&(pubkey)));
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_account_passkey_submit(&mut self) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.sent = crate::host::add_passkey(::std::convert::AsRef::as_ref(
                    &((self.account_key_label_draft).trim().to_owned()),
                ));
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_account_passkey_desktop(&mut self) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.sent = crate::host::add_passkey_here(::std::convert::AsRef::as_ref(
                    &((self.account_key_label_draft).trim().to_owned()),
                ));
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_account_ceremony_cancel(&mut self) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.sent = crate::host::cancel_ceremony();
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_account_wallet_submit(&mut self) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.sent = crate::host::link_wallet(::std::convert::AsRef::as_ref(
                    &((self.account_key_label_draft).trim().to_owned()),
                ));
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_account_login_submit(&mut self) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.sent = crate::host::login();
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_copy_to_clipboard(
        &mut self,
        text: String,
        label: String,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.sent = crate::host::copy(
                    ::std::convert::AsRef::as_ref(&(text)),
                    ::std::convert::AsRef::as_ref(&(label)),
                );
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_set_appearance_light(&mut self) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.sent = crate::host::set_light();
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_set_appearance_dark(&mut self) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.sent = crate::host::set_dark();
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_set_desktop_notifications(
        &mut self,
        enabled: bool,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.sent = crate::host::set_notifications(enabled);
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_pick_settings_pane(
        &mut self,
        scope: String,
        picked: SettingsPane,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            let local = self
                .settings_screen_states
                .entry(scope.clone())
                .or_insert_with(|| SettingsScreenState {
                    key_pw: self.settings_screen_initial.key_pw.clone(),
                    settings_pane: self.settings_screen_initial.settings_pane.clone(),
                });
            {
                local.settings_pane = picked.clone();
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_edit_settings_password(
        &mut self,
        scope: String,
        value: String,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            let local =
                self.settings_screen_states
                    .entry(scope)
                    .or_insert_with(|| SettingsScreenState {
                        key_pw: self.settings_screen_initial.key_pw.clone(),
                        settings_pane: self.settings_screen_initial.settings_pane.clone(),
                    });
            {
                local.key_pw = value;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_bind_account_name_draft(
        &mut self,
        value: String,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.account_name_draft = value;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_bind_account_create_draft(
        &mut self,
        value: String,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.account_create_draft = value;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_bind_account_join_draft(
        &mut self,
        value: String,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.account_join_draft = value;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_bind_account_key_draft(&mut self, value: String) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.account_key_draft = value;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_bind_account_key_label_draft(
        &mut self,
        value: String,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.account_key_label_draft = value;
            }
            ::ducktape_view_guest::Task::none()
        }
    }
}

const SETTINGS_SCOPE: &str = "SettingsView/root/settings";

fn settings_action(
    key: impl Into<String>,
    label: &str,
    message: Message,
    enabled: bool,
) -> ducktape_view_guest::wire::Node {
    use ducktape_view_guest::{kit, slots, wire};
    kit::button(
        key,
        label,
        enabled.then(|| slots::message(message)),
        wire::ButtonPreset::Secondary,
    )
}

fn settings_input(
    key: &str,
    placeholder: &str,
    value: &str,
    route: fn(String) -> Message,
) -> ducktape_view_guest::wire::Node {
    use ducktape_view_guest::{kit, slots};
    kit::input(
        key,
        placeholder,
        value,
        slots::handler::<String, Message>(Box::new(move |text| Some(route(text)))),
        None,
    )
}

impl SettingsView {
    pub(crate) fn view(&self) -> ducktape_view_guest::wire::Node {
        use ducktape_view_guest::{kit, wire};
        let state = self
            .settings_screen_states
            .get(SETTINGS_SCOPE)
            .unwrap_or(&self.settings_screen_initial);
        let tabs = [
            ("general", "General", SettingsPane::General),
            ("network", "Network", SettingsPane::Network),
            ("account", "Account", SettingsPane::Account),
            ("security", "Security", SettingsPane::Security),
        ];
        let mut content = vec![
            kit::heading("settings/title", "Settings"),
            kit::row(
                "settings/tabs",
                tabs.into_iter().map(|(key, label, pane)| {
                    let mut tab = settings_action(
                        format!("settings/tab/{key}"),
                        label,
                        Message::PickSettingsPane(SETTINGS_SCOPE.into(), pane),
                        true,
                    );
                    if let wire::Node::Button { checked, .. } = &mut tab {
                        *checked = Some(state.settings_pane == pane);
                    }
                    tab
                }),
            ),
        ];
        if !self.host_error.is_empty() {
            content.push(kit::text("settings/error", &self.host_error));
        }
        content.push(match state.settings_pane {
            SettingsPane::General => self.general_settings(),
            SettingsPane::Network => self.network_settings(),
            SettingsPane::Account => self.account_settings(),
            SettingsPane::Security => self.security_settings(state),
        });
        kit::scroll(
            "settings",
            kit::padded(
                kit::column("settings/content", content),
                wire::Edges::all(22.),
            ),
        )
    }

    fn general_settings(&self) -> ducktape_view_guest::wire::Node {
        use ducktape_view_guest::{kit, wire};
        let choices = [
            ("light", "Light", Message::SetAppearanceLight),
            ("dark", "Dark", Message::SetAppearanceDark),
        ];
        let appearance = kit::row(
            "settings/appearance",
            choices.into_iter().map(|(value, label, message)| {
                let mut button =
                    settings_action(format!("settings/appearance/{value}"), label, message, true);
                if let wire::Node::Button { checked, .. } = &mut button {
                    *checked = Some(self.appearance == value);
                }
                button
            }),
        );
        let notifications = kit::row(
            "settings/notifications",
            [(true, "On"), (false, "Off")]
                .into_iter()
                .map(|(enabled, label)| {
                    let mut button = settings_action(
                        format!("settings/notifications/{enabled}"),
                        label,
                        Message::SetDesktopNotifications(enabled),
                        true,
                    );
                    if let wire::Node::Button { checked, .. } = &mut button {
                        *checked = Some(self.desktop_notifications == enabled);
                    }
                    button
                }),
        );
        kit::column(
            "settings/general",
            [
                kit::heading("settings/theme-title", "Theme"),
                appearance,
                kit::text(
                    "settings/appearance-source",
                    if self.appearance == "system" {
                        "Following the system appearance."
                    } else {
                        "Pinned for this device."
                    },
                ),
                kit::heading(
                    "settings/notification-title",
                    "Mentions and direct messages",
                ),
                kit::text(
                    "settings/notification-help",
                    if self.desktop_notifications {
                        "A desktop banner when you are named, or written to directly."
                    } else {
                        "Silent — the bell is the only notice."
                    },
                ),
                notifications,
            ],
        )
    }

    fn network_settings(&self) -> ducktape_view_guest::wire::Node {
        use ducktape_view_guest::kit;
        kit::column(
            "settings/network",
            [
                kit::heading("settings/network-name", &self.network_name),
                kit::text("settings/network-status", &self.status),
                kit::text("settings/network-rpc", &self.connected_rpc),
                kit::row(
                    "settings/members",
                    [
                        kit::text("settings/members-title", "Members"),
                        kit::text("settings/members-count", &self.members_line),
                        settings_action(
                            "settings/manage-members",
                            "manage",
                            Message::ShowTab("members".into()),
                            true,
                        ),
                    ],
                ),
                kit::row(
                    "settings/node",
                    [
                        kit::text("settings/node-title", "Node"),
                        settings_action(
                            "settings/view-node",
                            "view",
                            Message::ShowTab("node".into()),
                            true,
                        ),
                    ],
                ),
                settings_action(
                    "settings/reconnect",
                    "Reconnect",
                    Message::Reconnect,
                    !self.loading && (!self.busy || self.recovering),
                ),
                settings_action(
                    "settings/switch",
                    "Switch network",
                    Message::SwitchNetwork,
                    !self.busy,
                ),
            ],
        )
    }

    fn account_settings(&self) -> ducktape_view_guest::wire::Node {
        use ducktape_view_guest::{kit, wire};
        let available = !self.account_busy && self.unlocked;
        let mut content = vec![
            kit::heading("settings/identity-title", "YOUR IDENTITY"),
            kit::text(
                "settings/account-name",
                if self.account_name.is_empty() {
                    "(unnamed)"
                } else {
                    &self.account_name
                },
            ),
            kit::text(
                "settings/standing",
                if self.tier.is_empty() && self.members_answered {
                    "standing unknown"
                } else {
                    &self.tier
                },
            ),
            kit::text("settings/account-number", &self.account_number),
            kit::text("settings/seat", &self.seat_key),
            kit::text("settings/seat-label", "keypair on this device"),
            settings_input(
                "settings/rename-draft",
                "rename account…",
                &self.account_name_draft,
                Message::BindAccountNameDraft,
            ),
            settings_action(
                "settings/rename",
                "Rename",
                Message::AccountRenameSubmit,
                available && !self.account_name_draft.trim().is_empty(),
            ),
        ];
        if !self.account_exists {
            content.extend([
                settings_input(
                    "settings/create-draft",
                    "name your account…",
                    &self.account_create_draft,
                    Message::BindAccountCreateDraft,
                ),
                settings_action(
                    "settings/create",
                    "Create account",
                    Message::AccountCreateSubmit,
                    available && !self.account_create_draft.trim().is_empty(),
                ),
                settings_input(
                    "settings/join-draft",
                    "paste a ticket from a member device…",
                    &self.account_join_draft,
                    Message::BindAccountJoinDraft,
                ),
                settings_action(
                    "settings/join",
                    "Join",
                    Message::AccountKeyJoinSubmit,
                    available && !self.account_join_draft.trim().is_empty(),
                ),
                kit::text(
                    "settings/login-help",
                    "…or let a passkey from another device admit this one.",
                ),
                settings_action(
                    "settings/login",
                    "Log in with a passkey",
                    Message::AccountLoginSubmit,
                    available,
                ),
            ]);
        } else {
            content.push(kit::text(
                "settings/key-count",
                format!("{} keys", self.account_key_rows.len()),
            ));
            content.push(settings_action(
                "settings/copy-number",
                "Copy number",
                Message::CopyToClipboard(
                    self.account_number.clone(),
                    "Account number copied".into(),
                ),
                !self.account_number.is_empty(),
            ));
            content.push(kit::heading("settings/keys-title", "ACCOUNT KEYS"));
            for row in &self.account_key_rows {
                let key = format!("settings/key/{}/{}", row.scheme, row.pubkey);
                content.push(kit::column(
                    &key,
                    [
                        kit::text(
                            format!("{key}/label"),
                            if row.label.is_empty() {
                                "(unlabeled)"
                            } else {
                                &row.label
                            },
                        ),
                        kit::text(format!("{key}/scheme"), &row.scheme),
                        kit::text(format!("{key}/value"), &row.pubkey),
                        settings_action(
                            format!("{key}/remove"),
                            "Remove",
                            Message::AccountKeyRemove(row.pubkey.clone()),
                            available && self.account_key_rows.len() > 1,
                        ),
                    ],
                ));
            }
            content.extend([
                kit::heading("settings/add-device", "Add a device"),
                settings_input(
                    "settings/key-draft",
                    "paste its ed25519 key (hex)…",
                    &self.account_key_draft,
                    Message::BindAccountKeyDraft,
                ),
                settings_input(
                    "settings/key-label-draft",
                    "label…",
                    &self.account_key_label_draft,
                    Message::BindAccountKeyLabelDraft,
                ),
                settings_action(
                    "settings/mint",
                    "Mint ticket",
                    Message::AccountKeyAddSubmit,
                    available && !self.account_key_draft.trim().is_empty(),
                ),
                kit::text("settings/passkey-help", "…or a passkey:"),
                settings_action(
                    "settings/passkey-phone",
                    "On your phone",
                    Message::AccountPasskeySubmit,
                    available,
                ),
                settings_action(
                    "settings/passkey-browser",
                    "In this browser",
                    Message::AccountPasskeyDesktop,
                    available,
                ),
                settings_action(
                    "settings/wallet",
                    "Link a wallet",
                    Message::AccountWalletSubmit,
                    available,
                ),
            ]);
            if !self.account_ticket.is_empty() {
                content.push(kit::text(
                    "settings/ticket-help",
                    "Ticket minted — paste it on the other device.",
                ));
                content.push(settings_action(
                    "settings/copy-ticket",
                    "Copy ticket",
                    Message::CopyToClipboard(self.account_ticket.clone(), "Ticket copied".into()),
                    true,
                ));
            }
        }
        match self.account_ceremony_phase.as_str() {
            "show_qr" => {
                content.push(wire::Node::Qr {
                    key: "settings/ceremony-qr".into(),
                    code: wire::Qr {
                        payload: Some(self.account_ceremony_qr.as_bytes().to_vec()),
                        correction: Some(wire::QrCorrection::Medium),
                        size: Some(wire::QrSize::Cell(3.)),
                        version: None,
                        cell: None,
                        background: None,
                    },
                });
                content.push(kit::text(
                    "settings/ceremony-detail",
                    &self.account_ceremony_detail,
                ));
                content.push(kit::text(
                    "settings/ceremony-left",
                    &self.account_ceremony_left,
                ));
                content.push(settings_action(
                    "settings/ceremony-cancel",
                    "Cancel",
                    Message::AccountCeremonyCancel,
                    true,
                ));
            }
            "working" => {
                content.push(kit::text(
                    "settings/ceremony-detail",
                    &self.account_ceremony_detail,
                ));
                content.push(settings_action(
                    "settings/ceremony-cancel",
                    "Cancel",
                    Message::AccountCeremonyCancel,
                    true,
                ));
            }
            _ => {}
        }
        kit::column("settings/account", content)
    }

    fn security_settings(&self, state: &SettingsScreenState) -> ducktape_view_guest::wire::Node {
        use ducktape_view_guest::{kit, slots, wire};
        let mut content = vec![
            kit::heading("settings/security-title", "IDENTITY KEY"),
            kit::text("settings/key-state-label", "Key state"),
            kit::text("settings/key-state", &self.settings_key_state),
            kit::text("settings/key-path-label", "Key path"),
            kit::text("settings/key-path", &self.settings_key_path),
        ];
        if self.unlocked {
            content.push(kit::text(
                "settings/signing",
                "Signing unlocked for this session.",
            ));
            content.push(settings_action(
                "settings/lock",
                "Lock",
                Message::LockSession,
                true,
            ));
        } else {
            let mut password = kit::input(
                "settings/password",
                "unlock signing…",
                &state.key_pw,
                slots::handler::<String, Message>(Box::new(|text| {
                    Some(Message::EditSettingsPassword(SETTINGS_SCOPE.into(), text))
                })),
                Some(slots::message(Message::SettingsUnlockSubmit(
                    state.key_pw.clone(),
                ))),
            );
            if let wire::Node::Input { secure, .. } = &mut password {
                *secure = true;
            }
            content.push(password);
            content.push(settings_action(
                "settings/unlock",
                "Unlock",
                Message::SettingsUnlockSubmit(state.key_pw.clone()),
                !self.busy && !state.key_pw.is_empty(),
            ));
        }
        kit::column("settings/security", content)
    }
}

ducktape_view_guest::export_app!(
    SettingsView,
    "Settings",
    "This device's preferences, the account this key speaks for, and the workspace's lifecycle.",
    ["settings"]
);
