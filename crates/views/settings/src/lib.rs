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
#[derive(Clone, Copy)]
struct Palette {
    colors: [::ducktape_view_guest::wire::Rgba; 128],
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
    pub(crate) settings_screen_states: ::std::collections::HashMap<
        String,
        SettingsScreenState,
    >,
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
        self.derived.account_keys.get_or_init(|| ((self.account_key_rows).len() as i64))
    }
}
#[allow(unused_parens)]
impl SettingsView {
    fn palette(&self) -> Palette {
        match self.active_palette.clone() {
            AppTheme::App => {
                Palette {
                    colors: [
                        ::ducktape_view_guest::wire::Rgba([
                            58.0 / 255.0,
                            56.0 / 255.0,
                            51.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            212.0 / 255.0,
                            210.0 / 255.0,
                            202.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            253.0 / 255.0,
                            253.0 / 255.0,
                            251.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            255.0 / 255.0,
                            255.0 / 255.0,
                            255.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            44.0 / 255.0,
                            43.0 / 255.0,
                            39.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            107.0 / 255.0,
                            105.0 / 255.0,
                            98.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            246.0 / 255.0,
                            245.0 / 255.0,
                            242.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            38.0 / 255.0,
                            37.0 / 255.0,
                            31.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            50.0 / 255.0,
                            47.0 / 255.0,
                            40.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            255.0 / 255.0,
                            255.0 / 255.0,
                            255.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            236.0 / 255.0,
                            235.0 / 255.0,
                            230.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            179.0 / 255.0,
                            177.0 / 255.0,
                            168.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            255.0 / 255.0,
                            255.0 / 255.0,
                            255.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            94.0 / 255.0,
                            92.0 / 255.0,
                            85.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            243.0 / 255.0,
                            242.0 / 255.0,
                            239.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            63.0 / 255.0,
                            62.0 / 255.0,
                            57.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            160.0 / 255.0,
                            90.0 / 255.0,
                            60.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            255.0 / 255.0,
                            255.0 / 255.0,
                            255.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            249.0 / 255.0,
                            241.0 / 255.0,
                            234.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            231.0 / 255.0,
                            210.0 / 255.0,
                            196.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            184.0 / 255.0,
                            84.0 / 255.0,
                            76.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            255.0 / 255.0,
                            255.0 / 255.0,
                            255.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            253.0 / 255.0,
                            244.0 / 255.0,
                            243.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            239.0 / 255.0,
                            214.0 / 255.0,
                            211.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            224.0 / 255.0,
                            101.0 / 255.0,
                            92.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            95.0 / 255.0,
                            158.0 / 255.0,
                            116.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            21.0 / 255.0,
                            20.0 / 255.0,
                            16.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            238.0 / 255.0,
                            245.0 / 255.0,
                            240.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            207.0 / 255.0,
                            227.0 / 255.0,
                            215.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            92.0 / 255.0,
                            180.0 / 255.0,
                            95.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            160.0 / 255.0,
                            123.0 / 255.0,
                            50.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            21.0 / 255.0,
                            20.0 / 255.0,
                            16.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            251.0 / 255.0,
                            244.0 / 255.0,
                            230.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            236.0 / 255.0,
                            220.0 / 255.0,
                            174.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            227.0 / 255.0,
                            180.0 / 255.0,
                            67.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            210.0 / 255.0,
                            208.0 / 255.0,
                            199.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            79.0 / 255.0,
                            77.0 / 255.0,
                            71.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            38.0 / 255.0,
                            37.0 / 255.0,
                            31.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            243.0 / 255.0,
                            241.0 / 255.0,
                            234.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            231.0 / 255.0,
                            230.0 / 255.0,
                            226.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            224.0 / 255.0,
                            223.0 / 255.0,
                            215.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            138.0 / 255.0,
                            137.0 / 255.0,
                            131.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            38.0 / 255.0,
                            37.0 / 255.0,
                            31.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            253.0 / 255.0,
                            252.0 / 255.0,
                            250.0 / 255.0,
                            0.501961,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            253.0 / 255.0,
                            252.0 / 255.0,
                            250.0 / 255.0,
                            0.619608,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            253.0 / 255.0,
                            252.0 / 255.0,
                            250.0 / 255.0,
                            0.858824,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            40.0 / 255.0,
                            38.0 / 255.0,
                            34.0 / 255.0,
                            0.129412,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            40.0 / 255.0,
                            38.0 / 255.0,
                            34.0 / 255.0,
                            0.219608,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            40.0 / 255.0,
                            38.0 / 255.0,
                            34.0 / 255.0,
                            0.301961,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            40.0 / 255.0,
                            38.0 / 255.0,
                            34.0 / 255.0,
                            0.219608,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            40.0 / 255.0,
                            38.0 / 255.0,
                            34.0 / 255.0,
                            0.101961,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            227.0 / 255.0,
                            225.0 / 255.0,
                            217.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            236.0 / 255.0,
                            234.0 / 255.0,
                            227.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            250.0 / 255.0,
                            250.0 / 255.0,
                            248.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            251.0 / 255.0,
                            251.0 / 255.0,
                            249.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            243.0 / 255.0,
                            242.0 / 255.0,
                            239.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            236.0 / 255.0,
                            235.0 / 255.0,
                            230.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            248.0 / 255.0,
                            247.0 / 255.0,
                            243.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            240.0 / 255.0,
                            239.0 / 255.0,
                            234.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            214.0 / 255.0,
                            212.0 / 255.0,
                            204.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            239.0 / 255.0,
                            238.0 / 255.0,
                            233.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            9.0 / 255.0,
                            11.0 / 255.0,
                            14.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            36.0 / 255.0,
                            42.0 / 255.0,
                            51.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            236.0 / 255.0,
                            233.0 / 255.0,
                            225.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            236.0 / 255.0,
                            214.0 / 255.0,
                            208.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            253.0 / 255.0,
                            246.0 / 255.0,
                            244.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            163.0 / 255.0,
                            82.0 / 255.0,
                            72.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            143.0 / 255.0,
                            70.0 / 255.0,
                            61.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            50.0 / 255.0,
                            47.0 / 255.0,
                            40.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            58.0 / 255.0,
                            57.0 / 255.0,
                            52.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            154.0 / 255.0,
                            152.0 / 255.0,
                            143.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            167.0 / 255.0,
                            165.0 / 255.0,
                            155.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            179.0 / 255.0,
                            177.0 / 255.0,
                            168.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            189.0 / 255.0,
                            187.0 / 255.0,
                            177.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            203.0 / 255.0,
                            201.0 / 255.0,
                            191.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            123.0 / 255.0,
                            167.0 / 255.0,
                            140.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            95.0 / 255.0,
                            122.0 / 255.0,
                            158.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            238.0 / 255.0,
                            242.0 / 255.0,
                            247.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            218.0 / 255.0,
                            226.0 / 255.0,
                            236.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            127.0 / 255.0,
                            154.0 / 255.0,
                            184.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            163.0 / 255.0,
                            82.0 / 255.0,
                            72.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            251.0 / 255.0,
                            236.0 / 255.0,
                            234.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            236.0 / 255.0,
                            207.0 / 255.0,
                            201.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            207.0 / 255.0,
                            106.0 / 255.0,
                            94.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            251.0 / 255.0,
                            248.0 / 255.0,
                            240.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            40.0 / 255.0,
                            38.0 / 255.0,
                            34.0 / 255.0,
                            0.341176,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            247.0 / 255.0,
                            246.0 / 255.0,
                            242.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            250.0 / 255.0,
                            249.0 / 255.0,
                            246.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            252.0 / 255.0,
                            251.0 / 255.0,
                            249.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            251.0 / 255.0,
                            250.0 / 255.0,
                            247.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            253.0 / 255.0,
                            248.0 / 255.0,
                            243.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            240.0 / 255.0,
                            236.0 / 255.0,
                            225.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            244.0 / 255.0,
                            231.0 / 255.0,
                            200.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            217.0 / 255.0,
                            216.0 / 255.0,
                            208.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            213.0 / 255.0,
                            211.0 / 255.0,
                            202.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            182.0 / 255.0,
                            180.0 / 255.0,
                            168.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            200.0 / 255.0,
                            198.0 / 255.0,
                            188.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            194.0 / 255.0,
                            192.0 / 255.0,
                            182.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            208.0 / 255.0,
                            206.0 / 255.0,
                            196.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            220.0 / 255.0,
                            219.0 / 255.0,
                            212.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            122.0 / 255.0,
                            120.0 / 255.0,
                            114.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            126.0 / 255.0,
                            158.0 / 255.0,
                            136.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            102.0 / 255.0,
                            100.0 / 255.0,
                            94.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            122.0 / 255.0,
                            111.0 / 255.0,
                            158.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            241.0 / 255.0,
                            237.0 / 255.0,
                            245.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            221.0 / 255.0,
                            210.0 / 255.0,
                            230.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            240.0 / 255.0,
                            245.0 / 255.0,
                            241.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            220.0 / 255.0,
                            235.0 / 255.0,
                            224.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            238.0 / 255.0,
                            246.0 / 255.0,
                            239.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            225.0 / 255.0,
                            239.0 / 255.0,
                            227.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            47.0 / 255.0,
                            107.0 / 255.0,
                            65.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            251.0 / 255.0,
                            238.0 / 255.0,
                            236.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            244.0 / 255.0,
                            221.0 / 255.0,
                            216.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            161.0 / 255.0,
                            67.0 / 255.0,
                            56.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            246.0 / 255.0,
                            243.0 / 255.0,
                            249.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            74.0 / 255.0,
                            72.0 / 255.0,
                            67.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            224.0 / 255.0,
                            145.0 / 255.0,
                            138.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            160.0 / 255.0,
                            138.0 / 255.0,
                            90.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            95.0 / 255.0,
                            138.0 / 255.0,
                            114.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            237.0 / 255.0,
                            244.0 / 255.0,
                            239.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            122.0 / 255.0,
                            111.0 / 255.0,
                            158.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            241.0 / 255.0,
                            239.0 / 255.0,
                            247.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            74.0 / 255.0,
                            72.0 / 255.0,
                            67.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            242.0 / 255.0,
                            241.0 / 255.0,
                            237.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            185.0 / 255.0,
                            113.0 / 255.0,
                            78.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            250.0 / 255.0,
                            240.0 / 255.0,
                            233.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            192.0 / 255.0,
                            138.0 / 255.0,
                            62.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            250.0 / 255.0,
                            243.0 / 255.0,
                            230.0 / 255.0,
                            1.000000,
                        ]),
                    ],
                }
            }
            AppTheme::AppDark => {
                Palette {
                    colors: [
                        ::ducktape_view_guest::wire::Rgba([
                            212.0 / 255.0,
                            210.0 / 255.0,
                            202.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            69.0 / 255.0,
                            68.0 / 255.0,
                            60.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            27.0 / 255.0,
                            26.0 / 255.0,
                            22.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            34.0 / 255.0,
                            33.0 / 255.0,
                            29.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            232.0 / 255.0,
                            230.0 / 255.0,
                            223.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            168.0 / 255.0,
                            166.0 / 255.0,
                            156.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            38.0 / 255.0,
                            37.0 / 255.0,
                            31.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            232.0 / 255.0,
                            230.0 / 255.0,
                            223.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            244.0 / 255.0,
                            242.0 / 255.0,
                            234.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            27.0 / 255.0,
                            26.0 / 255.0,
                            22.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            51.0 / 255.0,
                            50.0 / 255.0,
                            44.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            107.0 / 255.0,
                            106.0 / 255.0,
                            97.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            42.0 / 255.0,
                            41.0 / 255.0,
                            37.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            181.0 / 255.0,
                            179.0 / 255.0,
                            169.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            46.0 / 255.0,
                            45.0 / 255.0,
                            39.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            207.0 / 255.0,
                            205.0 / 255.0,
                            196.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            201.0 / 255.0,
                            138.0 / 255.0,
                            99.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            27.0 / 255.0,
                            26.0 / 255.0,
                            22.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            51.0 / 255.0,
                            38.0 / 255.0,
                            29.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            74.0 / 255.0,
                            56.0 / 255.0,
                            43.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            217.0 / 255.0,
                            123.0 / 255.0,
                            114.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            27.0 / 255.0,
                            26.0 / 255.0,
                            22.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            51.0 / 255.0,
                            33.0 / 255.0,
                            31.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            77.0 / 255.0,
                            47.0 / 255.0,
                            44.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            224.0 / 255.0,
                            101.0 / 255.0,
                            92.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            127.0 / 255.0,
                            184.0 / 255.0,
                            148.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            21.0 / 255.0,
                            20.0 / 255.0,
                            16.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            30.0 / 255.0,
                            42.0 / 255.0,
                            34.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            50.0 / 255.0,
                            71.0 / 255.0,
                            58.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            92.0 / 255.0,
                            180.0 / 255.0,
                            95.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            212.0 / 255.0,
                            169.0 / 255.0,
                            78.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            21.0 / 255.0,
                            20.0 / 255.0,
                            16.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            46.0 / 255.0,
                            39.0 / 255.0,
                            23.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            77.0 / 255.0,
                            63.0 / 255.0,
                            34.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            227.0 / 255.0,
                            180.0 / 255.0,
                            67.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            58.0 / 255.0,
                            57.0 / 255.0,
                            49.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            207.0 / 255.0,
                            205.0 / 255.0,
                            196.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            243.0 / 255.0,
                            241.0 / 255.0,
                            234.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            38.0 / 255.0,
                            37.0 / 255.0,
                            31.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            53.0 / 255.0,
                            52.0 / 255.0,
                            46.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            59.0 / 255.0,
                            58.0 / 255.0,
                            51.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            133.0 / 255.0,
                            131.0 / 255.0,
                            123.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            232.0 / 255.0,
                            230.0 / 255.0,
                            223.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            27.0 / 255.0,
                            26.0 / 255.0,
                            22.0 / 255.0,
                            0.501961,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            27.0 / 255.0,
                            26.0 / 255.0,
                            22.0 / 255.0,
                            0.619608,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            27.0 / 255.0,
                            26.0 / 255.0,
                            22.0 / 255.0,
                            0.858824,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            0.0 / 255.0,
                            0.0 / 255.0,
                            0.0 / 255.0,
                            0.250980,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            0.0 / 255.0,
                            0.0 / 255.0,
                            0.0 / 255.0,
                            0.349020,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            0.0 / 255.0,
                            0.0 / 255.0,
                            0.0 / 255.0,
                            0.450980,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            0.0 / 255.0,
                            0.0 / 255.0,
                            0.0 / 255.0,
                            0.349020,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            0.0 / 255.0,
                            0.0 / 255.0,
                            0.0 / 255.0,
                            0.149020,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            18.0 / 255.0,
                            17.0 / 255.0,
                            16.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            25.0 / 255.0,
                            24.0 / 255.0,
                            21.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            32.0 / 255.0,
                            31.0 / 255.0,
                            27.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            30.0 / 255.0,
                            29.0 / 255.0,
                            25.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            42.0 / 255.0,
                            41.0 / 255.0,
                            37.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            49.0 / 255.0,
                            48.0 / 255.0,
                            43.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            36.0 / 255.0,
                            35.0 / 255.0,
                            30.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            40.0 / 255.0,
                            39.0 / 255.0,
                            34.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            14.0 / 255.0,
                            13.0 / 255.0,
                            11.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            44.0 / 255.0,
                            43.0 / 255.0,
                            38.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            9.0 / 255.0,
                            11.0 / 255.0,
                            14.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            36.0 / 255.0,
                            42.0 / 255.0,
                            51.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            48.0 / 255.0,
                            47.0 / 255.0,
                            41.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            77.0 / 255.0,
                            47.0 / 255.0,
                            44.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            42.0 / 255.0,
                            29.0 / 255.0,
                            27.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            194.0 / 255.0,
                            90.0 / 255.0,
                            79.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            211.0 / 255.0,
                            104.0 / 255.0,
                            92.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            244.0 / 255.0,
                            242.0 / 255.0,
                            234.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            220.0 / 255.0,
                            218.0 / 255.0,
                            210.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            143.0 / 255.0,
                            141.0 / 255.0,
                            132.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            124.0 / 255.0,
                            122.0 / 255.0,
                            113.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            107.0 / 255.0,
                            106.0 / 255.0,
                            97.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            96.0 / 255.0,
                            95.0 / 255.0,
                            86.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            85.0 / 255.0,
                            84.0 / 255.0,
                            76.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            123.0 / 255.0,
                            167.0 / 255.0,
                            140.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            127.0 / 255.0,
                            154.0 / 255.0,
                            184.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            30.0 / 255.0,
                            37.0 / 255.0,
                            48.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            48.0 / 255.0,
                            62.0 / 255.0,
                            82.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            127.0 / 255.0,
                            154.0 / 255.0,
                            184.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            211.0 / 255.0,
                            104.0 / 255.0,
                            92.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            48.0 / 255.0,
                            31.0 / 255.0,
                            28.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            77.0 / 255.0,
                            47.0 / 255.0,
                            44.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            207.0 / 255.0,
                            106.0 / 255.0,
                            94.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            42.0 / 255.0,
                            37.0 / 255.0,
                            23.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            0.0 / 255.0,
                            0.0 / 255.0,
                            0.0 / 255.0,
                            0.501961,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            32.0 / 255.0,
                            31.0 / 255.0,
                            26.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            35.0 / 255.0,
                            34.0 / 255.0,
                            29.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            38.0 / 255.0,
                            37.0 / 255.0,
                            32.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            38.0 / 255.0,
                            36.0 / 255.0,
                            24.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            42.0 / 255.0,
                            34.0 / 255.0,
                            27.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            53.0 / 255.0,
                            50.0 / 255.0,
                            42.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            69.0 / 255.0,
                            58.0 / 255.0,
                            30.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            63.0 / 255.0,
                            62.0 / 255.0,
                            54.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            69.0 / 255.0,
                            68.0 / 255.0,
                            60.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            110.0 / 255.0,
                            109.0 / 255.0,
                            99.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            91.0 / 255.0,
                            90.0 / 255.0,
                            82.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            98.0 / 255.0,
                            97.0 / 255.0,
                            90.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            74.0 / 255.0,
                            73.0 / 255.0,
                            65.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            51.0 / 255.0,
                            50.0 / 255.0,
                            44.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            163.0 / 255.0,
                            161.0 / 255.0,
                            152.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            126.0 / 255.0,
                            158.0 / 255.0,
                            136.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            157.0 / 255.0,
                            155.0 / 255.0,
                            146.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            168.0 / 255.0,
                            154.0 / 255.0,
                            201.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            42.0 / 255.0,
                            38.0 / 255.0,
                            51.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            68.0 / 255.0,
                            60.0 / 255.0,
                            87.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            30.0 / 255.0,
                            42.0 / 255.0,
                            34.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            50.0 / 255.0,
                            71.0 / 255.0,
                            58.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            29.0 / 255.0,
                            42.0 / 255.0,
                            32.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            36.0 / 255.0,
                            53.0 / 255.0,
                            42.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            143.0 / 255.0,
                            201.0 / 255.0,
                            162.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            47.0 / 255.0,
                            31.0 / 255.0,
                            28.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            61.0 / 255.0,
                            39.0 / 255.0,
                            35.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            222.0 / 255.0,
                            139.0 / 255.0,
                            127.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            38.0 / 255.0,
                            35.0 / 255.0,
                            48.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            46.0 / 255.0,
                            45.0 / 255.0,
                            40.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            160.0 / 255.0,
                            92.0 / 255.0,
                            85.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            192.0 / 255.0,
                            168.0 / 255.0,
                            110.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            127.0 / 255.0,
                            184.0 / 255.0,
                            148.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            30.0 / 255.0,
                            42.0 / 255.0,
                            34.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            168.0 / 255.0,
                            154.0 / 255.0,
                            201.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            42.0 / 255.0,
                            38.0 / 255.0,
                            51.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            207.0 / 255.0,
                            205.0 / 255.0,
                            196.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            46.0 / 255.0,
                            45.0 / 255.0,
                            40.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            208.0 / 255.0,
                            144.0 / 255.0,
                            104.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            51.0 / 255.0,
                            38.0 / 255.0,
                            29.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            212.0 / 255.0,
                            169.0 / 255.0,
                            78.0 / 255.0,
                            1.000000,
                        ]),
                        ::ducktape_view_guest::wire::Rgba([
                            46.0 / 255.0,
                            39.0 / 255.0,
                            23.0 / 255.0,
                            1.000000,
                        ]),
                    ],
                }
            }
        }
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
    pub(crate) const SNAPSHOT_SCHEMA: &'static str = "f8c4c32da46fa9482206b251b5b9979dfe6115d847f004e01847b0543779d57c";
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
            } = value else {
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
                } = value else {
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
                ::ducktape_view_guest::wire::SnapshotValue::List(items) => {
                    items
                        .into_iter()
                        .map(|item| (|| {
                            let ::ducktape_view_guest::wire::SnapshotValue::Record {
                                name: name,
                                fields: fields,
                            } = item else {
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
                        })())
                        .collect::<Option<Vec<_>>>()
                }
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
            let settings_screen_states: ::std::collections::HashMap<
                String,
                SettingsScreenState,
            > = ((|| {
                let ::ducktape_view_guest::wire::SnapshotValue::Record {
                    name: name,
                    fields: fields,
                } = value else {
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
                        } = value else {
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
                            ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                Some(item)
                            }
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
                            } = value else {
                                return None;
                            };
                            if name != "SettingsPane" || fields.len() != 1 {
                                return None;
                            }
                            let (variant, payload) = fields.into_iter().next()?;
                            match variant.as_str() {
                                "general" => {
                                    matches!(
                                        payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                                    )
                                        .then_some(SettingsPane::General)
                                }
                                "network" => {
                                    matches!(
                                        payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                                    )
                                        .then_some(SettingsPane::Network)
                                }
                                "account" => {
                                    matches!(
                                        payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                                    )
                                        .then_some(SettingsPane::Account)
                                }
                                "security" => {
                                    matches!(
                                        payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                                    )
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
                } = value else {
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
                    } = value else {
                        return None;
                    };
                    if name != "SettingsPane" || fields.len() != 1 {
                        return None;
                    }
                    let (variant, payload) = fields.into_iter().next()?;
                    match variant.as_str() {
                        "general" => {
                            matches!(
                                payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                            )
                                .then_some(SettingsPane::General)
                        }
                        "network" => {
                            matches!(
                                payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                            )
                                .then_some(SettingsPane::Network)
                        }
                        "account" => {
                            matches!(
                                payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                            )
                                .then_some(SettingsPane::Account)
                        }
                        "security" => {
                            matches!(
                                payload, ::ducktape_view_guest::wire::SnapshotValue::Unit
                            )
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
            Some(
                Self::restore_state(
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
                ),
            )
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
                ::ducktape_view_guest::Subscription::batch([
                    crate::host::standing(self.connection_serial)
                        .map(move |value| Message::StandingArrived(value)),
                ])
            } else {
                ::ducktape_view_guest::Subscription::none()
            },
            if (self.connected && (!(self.seat_key).is_empty())) {
                ::ducktape_view_guest::Subscription::batch([
                    crate::host::account_keys(
                            self.connection_serial,
                            self.seat_key.to_owned(),
                        )
                        .map(move |value| Message::KeysArrived(value)),
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
    pub(crate) fn update(
        &mut self,
        message: Message,
    ) -> ::ducktape_view_guest::Task<Message> {
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
            Message::CopyToClipboard(text, label) => {
                self.on_copy_to_clipboard(text, label)
            }
            Message::SetAppearanceLight => self.on_set_appearance_light(),
            Message::SetAppearanceDark => self.on_set_appearance_dark(),
            Message::SetDesktopNotifications(enabled) => {
                self.on_set_desktop_notifications(enabled)
            }
            Message::PickSettingsPane(scope, picked) => {
                self.on_pick_settings_pane(scope, picked)
            }
            Message::EditSettingsPassword(scope, value) => {
                self.on_edit_settings_password(scope, value)
            }
            Message::BindAccountNameDraft(value) => {
                self.on_bind_account_name_draft(value)
            }
            Message::BindAccountCreateDraft(value) => {
                self.on_bind_account_create_draft(value)
            }
            Message::BindAccountJoinDraft(value) => {
                self.on_bind_account_join_draft(value)
            }
            Message::BindAccountKeyDraft(value) => self.on_bind_account_key_draft(value),
            Message::BindAccountKeyLabelDraft(value) => {
                self.on_bind_account_key_label_draft(value)
            }
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
    fn on_settings_unlock_submit(
        &mut self,
        pw: String,
    ) -> ::ducktape_view_guest::Task<Message> {
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
            if (self.account_busy
                || ((self.account_name_draft).trim().to_owned()).is_empty())
            {
                return ::ducktape_view_guest::Task::none();
            }
            {
                self.renaming_to = (self.account_name_draft).trim().to_owned();
            }
            {
                self.sent = crate::host::rename_account(
                    ::std::convert::AsRef::as_ref(
                        &((self.account_name_draft).trim().to_owned()),
                    ),
                );
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
                self.sent = crate::host::create_account(
                    ::std::convert::AsRef::as_ref(
                        &((self.account_create_draft).trim().to_owned()),
                    ),
                );
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
                    ::std::convert::AsRef::as_ref(
                        &((self.account_key_draft).trim().to_owned()),
                    ),
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
                self.sent = crate::host::join_account(
                    ::std::convert::AsRef::as_ref(
                        &((self.account_join_draft).trim().to_owned()),
                    ),
                );
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_account_key_remove(
        &mut self,
        pubkey: String,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            if ((self.account_busy || (!self.unlocked))
                || ((*self.derived_account_keys()) <= 1))
            {
                return ::ducktape_view_guest::Task::none();
            }
            {
                self.sent = crate::host::remove_key(
                    ::std::convert::AsRef::as_ref(&(pubkey)),
                );
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_account_passkey_submit(&mut self) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.sent = crate::host::add_passkey(
                    ::std::convert::AsRef::as_ref(
                        &((self.account_key_label_draft).trim().to_owned()),
                    ),
                );
            }
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_account_passkey_desktop(&mut self) -> ::ducktape_view_guest::Task<Message> {
        {
            {
                self.sent = crate::host::add_passkey_here(
                    ::std::convert::AsRef::as_ref(
                        &((self.account_key_label_draft).trim().to_owned()),
                    ),
                );
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
                self.sent = crate::host::link_wallet(
                    ::std::convert::AsRef::as_ref(
                        &((self.account_key_label_draft).trim().to_owned()),
                    ),
                );
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
            let local = self
                .settings_screen_states
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
    fn on_bind_account_key_draft(
        &mut self,
        value: String,
    ) -> ::ducktape_view_guest::Task<Message> {
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
impl SettingsView {
    pub(crate) fn view(&self) -> ::ducktape_view_guest::wire::Node {
        let palette = self.palette();
        {
            let node_scope = format!("{}/root", "SettingsView");
            ::ducktape_view_guest::wire::Node::Container {
                shadow: ::ducktape_view_guest::wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: false,
                key: node_scope.clone(),
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                height: Some(::ducktape_view_guest::wire::Length::Fill),
                padding: None,
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[2]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: None,
                snap: None,
                content: Box::new({
                    let node_scope = format!("{}/settings", node_scope);
                    self.render_settings_screen(palette, node_scope.clone())
                }),
            }
        }
    }
}
impl SettingsView {
    pub(crate) fn render_tab_label_0(
        &self,
        palette: Palette,
        use_scope: String,
        ctx_0: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                        if ((*self
                            .settings_screen_states
                            .get(&ctx_0)
                            .map_or(
                                &self.settings_screen_initial.settings_pane,
                                |local| &local.settings_pane,
                            )) == SettingsPane::General)
                        {
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:65", use_scope),
                                    size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[7]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("General".to_owned()).to_string(),
                                });
                        }
                        if (!((*self
                            .settings_screen_states
                            .get(&ctx_0)
                            .map_or(
                                &self.settings_screen_initial.settings_pane,
                                |local| &local.settings_pane,
                            )) == SettingsPane::General))
                        {
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:72", use_scope),
                                    size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[71]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("General".to_owned()).to_string(),
                                });
                        }
                        if (0 > 0) {
                            children
                                .push(::ducktape_view_guest::wire::Node::Container {
                                    shadow: ::ducktape_view_guest::wire::Shadow {
                                        color: None,
                                        x: None,
                                        y: None,
                                        blur: None,
                                    },
                                    max_width: None,
                                    max_height: None,
                                    clip: false,
                                    key: format!("{}/@container:79", use_scope),
                                    width: None,
                                    height: None,
                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                        top: (1.0) as f32,
                                        right: (7.0) as f32,
                                        bottom: (1.0) as f32,
                                        left: (7.0) as f32,
                                    }),
                                    align_x: None,
                                    align_y: None,
                                    background: (Some(palette.colors[55]))
                                        .map(::ducktape_view_guest::wire::Background::Color),
                                    border: Some(::ducktape_view_guest::wire::Border {
                                        color: None,
                                        width: None,
                                        radius: Some([
                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                        ]),
                                    }),
                                    snap: None,
                                    content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                        options: ::ducktape_view_guest::wire::TextOptions {
                                            height: None,
                                            align_y: None,
                                            line_height: None,
                                            shaping: None,
                                            wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                            tracking: 0.0f32,
                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                    "Geist Mono".into(),
                                                ),
                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                            }),
                                        },
                                        key: format!("{}/@text:85", use_scope),
                                        size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                        color: Some(palette.colors[71]),
                                        font: ::ducktape_view_guest::wire::Font {
                                            monospace: false,
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        },
                                        width: None,
                                        align_x: None,
                                        content: (0).to_string(),
                                    }),
                                });
                        }
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:58", use_scope),
                            wrap: None,
                            axis: ::ducktape_view_guest::wire::Axis::Row,
                            spacing: Some((7.0) as f32),
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (10.0) as f32,
                                right: (0.0) as f32,
                                bottom: (10.0) as f32,
                                left: (0.0) as f32,
                            }),
                            width: None,
                            height: None,
                            align: Some(::ducktape_view_guest::wire::AlignX::Center),
                            background: None,
                            border: None,
                            children: children,
                        }
                    });
                if ((*self
                    .settings_screen_states
                    .get(&ctx_0)
                    .map_or(
                        &self.settings_screen_initial.settings_pane,
                        |local| &local.settings_pane,
                    )) == SettingsPane::General)
                {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:92", use_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((2.0) as f32),
                            ),
                            padding: None,
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[7]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                            }),
                        });
                }
                if (!((*self
                    .settings_screen_states
                    .get(&ctx_0)
                    .map_or(
                        &self.settings_screen_initial.settings_pane,
                        |local| &local.settings_pane,
                    )) == SettingsPane::General))
                {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:99", use_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((2.0) as f32),
                            ),
                            padding: None,
                            align_x: None,
                            align_y: None,
                            background: (Some(
                                ::ducktape_view_guest::wire::Rgba([
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.000000,
                                ]),
                            ))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                            }),
                        });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: None,
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_tab_label_1(
        &self,
        palette: Palette,
        use_scope: String,
        ctx_0: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                        if ((*self
                            .settings_screen_states
                            .get(&ctx_0)
                            .map_or(
                                &self.settings_screen_initial.settings_pane,
                                |local| &local.settings_pane,
                            )) == SettingsPane::Network)
                        {
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:65", use_scope),
                                    size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[7]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("Network".to_owned()).to_string(),
                                });
                        }
                        if (!((*self
                            .settings_screen_states
                            .get(&ctx_0)
                            .map_or(
                                &self.settings_screen_initial.settings_pane,
                                |local| &local.settings_pane,
                            )) == SettingsPane::Network))
                        {
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:72", use_scope),
                                    size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[71]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("Network".to_owned()).to_string(),
                                });
                        }
                        if (0 > 0) {
                            children
                                .push(::ducktape_view_guest::wire::Node::Container {
                                    shadow: ::ducktape_view_guest::wire::Shadow {
                                        color: None,
                                        x: None,
                                        y: None,
                                        blur: None,
                                    },
                                    max_width: None,
                                    max_height: None,
                                    clip: false,
                                    key: format!("{}/@container:79", use_scope),
                                    width: None,
                                    height: None,
                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                        top: (1.0) as f32,
                                        right: (7.0) as f32,
                                        bottom: (1.0) as f32,
                                        left: (7.0) as f32,
                                    }),
                                    align_x: None,
                                    align_y: None,
                                    background: (Some(palette.colors[55]))
                                        .map(::ducktape_view_guest::wire::Background::Color),
                                    border: Some(::ducktape_view_guest::wire::Border {
                                        color: None,
                                        width: None,
                                        radius: Some([
                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                        ]),
                                    }),
                                    snap: None,
                                    content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                        options: ::ducktape_view_guest::wire::TextOptions {
                                            height: None,
                                            align_y: None,
                                            line_height: None,
                                            shaping: None,
                                            wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                            tracking: 0.0f32,
                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                    "Geist Mono".into(),
                                                ),
                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                            }),
                                        },
                                        key: format!("{}/@text:85", use_scope),
                                        size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                        color: Some(palette.colors[71]),
                                        font: ::ducktape_view_guest::wire::Font {
                                            monospace: false,
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        },
                                        width: None,
                                        align_x: None,
                                        content: (0).to_string(),
                                    }),
                                });
                        }
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:58", use_scope),
                            wrap: None,
                            axis: ::ducktape_view_guest::wire::Axis::Row,
                            spacing: Some((7.0) as f32),
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (10.0) as f32,
                                right: (0.0) as f32,
                                bottom: (10.0) as f32,
                                left: (0.0) as f32,
                            }),
                            width: None,
                            height: None,
                            align: Some(::ducktape_view_guest::wire::AlignX::Center),
                            background: None,
                            border: None,
                            children: children,
                        }
                    });
                if ((*self
                    .settings_screen_states
                    .get(&ctx_0)
                    .map_or(
                        &self.settings_screen_initial.settings_pane,
                        |local| &local.settings_pane,
                    )) == SettingsPane::Network)
                {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:92", use_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((2.0) as f32),
                            ),
                            padding: None,
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[7]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                            }),
                        });
                }
                if (!((*self
                    .settings_screen_states
                    .get(&ctx_0)
                    .map_or(
                        &self.settings_screen_initial.settings_pane,
                        |local| &local.settings_pane,
                    )) == SettingsPane::Network))
                {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:99", use_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((2.0) as f32),
                            ),
                            padding: None,
                            align_x: None,
                            align_y: None,
                            background: (Some(
                                ::ducktape_view_guest::wire::Rgba([
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.000000,
                                ]),
                            ))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                            }),
                        });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: None,
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_tab_label_2(
        &self,
        palette: Palette,
        use_scope: String,
        ctx_0: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                        if ((*self
                            .settings_screen_states
                            .get(&ctx_0)
                            .map_or(
                                &self.settings_screen_initial.settings_pane,
                                |local| &local.settings_pane,
                            )) == SettingsPane::Account)
                        {
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:65", use_scope),
                                    size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[7]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("Account".to_owned()).to_string(),
                                });
                        }
                        if (!((*self
                            .settings_screen_states
                            .get(&ctx_0)
                            .map_or(
                                &self.settings_screen_initial.settings_pane,
                                |local| &local.settings_pane,
                            )) == SettingsPane::Account))
                        {
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:72", use_scope),
                                    size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[71]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("Account".to_owned()).to_string(),
                                });
                        }
                        if ((*self.derived_account_keys()) > 0) {
                            children
                                .push(::ducktape_view_guest::wire::Node::Container {
                                    shadow: ::ducktape_view_guest::wire::Shadow {
                                        color: None,
                                        x: None,
                                        y: None,
                                        blur: None,
                                    },
                                    max_width: None,
                                    max_height: None,
                                    clip: false,
                                    key: format!("{}/@container:79", use_scope),
                                    width: None,
                                    height: None,
                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                        top: (1.0) as f32,
                                        right: (7.0) as f32,
                                        bottom: (1.0) as f32,
                                        left: (7.0) as f32,
                                    }),
                                    align_x: None,
                                    align_y: None,
                                    background: (Some(palette.colors[55]))
                                        .map(::ducktape_view_guest::wire::Background::Color),
                                    border: Some(::ducktape_view_guest::wire::Border {
                                        color: None,
                                        width: None,
                                        radius: Some([
                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                        ]),
                                    }),
                                    snap: None,
                                    content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                        options: ::ducktape_view_guest::wire::TextOptions {
                                            height: None,
                                            align_y: None,
                                            line_height: None,
                                            shaping: None,
                                            wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                            tracking: 0.0f32,
                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                    "Geist Mono".into(),
                                                ),
                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                            }),
                                        },
                                        key: format!("{}/@text:85", use_scope),
                                        size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                        color: Some(palette.colors[71]),
                                        font: ::ducktape_view_guest::wire::Font {
                                            monospace: false,
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        },
                                        width: None,
                                        align_x: None,
                                        content: ((*self.derived_account_keys())).to_string(),
                                    }),
                                });
                        }
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:58", use_scope),
                            wrap: None,
                            axis: ::ducktape_view_guest::wire::Axis::Row,
                            spacing: Some((7.0) as f32),
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (10.0) as f32,
                                right: (0.0) as f32,
                                bottom: (10.0) as f32,
                                left: (0.0) as f32,
                            }),
                            width: None,
                            height: None,
                            align: Some(::ducktape_view_guest::wire::AlignX::Center),
                            background: None,
                            border: None,
                            children: children,
                        }
                    });
                if ((*self
                    .settings_screen_states
                    .get(&ctx_0)
                    .map_or(
                        &self.settings_screen_initial.settings_pane,
                        |local| &local.settings_pane,
                    )) == SettingsPane::Account)
                {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:92", use_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((2.0) as f32),
                            ),
                            padding: None,
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[7]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                            }),
                        });
                }
                if (!((*self
                    .settings_screen_states
                    .get(&ctx_0)
                    .map_or(
                        &self.settings_screen_initial.settings_pane,
                        |local| &local.settings_pane,
                    )) == SettingsPane::Account))
                {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:99", use_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((2.0) as f32),
                            ),
                            padding: None,
                            align_x: None,
                            align_y: None,
                            background: (Some(
                                ::ducktape_view_guest::wire::Rgba([
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.000000,
                                ]),
                            ))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                            }),
                        });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: None,
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_tab_label_3(
        &self,
        palette: Palette,
        use_scope: String,
        ctx_0: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push({
                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                        if ((*self
                            .settings_screen_states
                            .get(&ctx_0)
                            .map_or(
                                &self.settings_screen_initial.settings_pane,
                                |local| &local.settings_pane,
                            )) == SettingsPane::Security)
                        {
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:65", use_scope),
                                    size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[7]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("Security".to_owned()).to_string(),
                                });
                        }
                        if (!((*self
                            .settings_screen_states
                            .get(&ctx_0)
                            .map_or(
                                &self.settings_screen_initial.settings_pane,
                                |local| &local.settings_pane,
                            )) == SettingsPane::Security))
                        {
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:72", use_scope),
                                    size: Some(((13.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[71]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("Security".to_owned()).to_string(),
                                });
                        }
                        if (0 > 0) {
                            children
                                .push(::ducktape_view_guest::wire::Node::Container {
                                    shadow: ::ducktape_view_guest::wire::Shadow {
                                        color: None,
                                        x: None,
                                        y: None,
                                        blur: None,
                                    },
                                    max_width: None,
                                    max_height: None,
                                    clip: false,
                                    key: format!("{}/@container:79", use_scope),
                                    width: None,
                                    height: None,
                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                        top: (1.0) as f32,
                                        right: (7.0) as f32,
                                        bottom: (1.0) as f32,
                                        left: (7.0) as f32,
                                    }),
                                    align_x: None,
                                    align_y: None,
                                    background: (Some(palette.colors[55]))
                                        .map(::ducktape_view_guest::wire::Background::Color),
                                    border: Some(::ducktape_view_guest::wire::Border {
                                        color: None,
                                        width: None,
                                        radius: Some([
                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                            ((9.0) as f32).max(0.0).min(f32::MAX),
                                        ]),
                                    }),
                                    snap: None,
                                    content: Box::new(::ducktape_view_guest::wire::Node::Text {
                                        options: ::ducktape_view_guest::wire::TextOptions {
                                            height: None,
                                            align_y: None,
                                            line_height: None,
                                            shaping: None,
                                            wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                            tracking: 0.0f32,
                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                    "Geist Mono".into(),
                                                ),
                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                            }),
                                        },
                                        key: format!("{}/@text:85", use_scope),
                                        size: Some(((10.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                        color: Some(palette.colors[71]),
                                        font: ::ducktape_view_guest::wire::Font {
                                            monospace: false,
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                        },
                                        width: None,
                                        align_x: None,
                                        content: (0).to_string(),
                                    }),
                                });
                        }
                        ::ducktape_view_guest::wire::Node::Linear {
                            max_width: None,
                            clip: false,
                            key: format!("{}/@layout:58", use_scope),
                            wrap: None,
                            axis: ::ducktape_view_guest::wire::Axis::Row,
                            spacing: Some((7.0) as f32),
                            padding: Some(::ducktape_view_guest::wire::Edges {
                                top: (10.0) as f32,
                                right: (0.0) as f32,
                                bottom: (10.0) as f32,
                                left: (0.0) as f32,
                            }),
                            width: None,
                            height: None,
                            align: Some(::ducktape_view_guest::wire::AlignX::Center),
                            background: None,
                            border: None,
                            children: children,
                        }
                    });
                if ((*self
                    .settings_screen_states
                    .get(&ctx_0)
                    .map_or(
                        &self.settings_screen_initial.settings_pane,
                        |local| &local.settings_pane,
                    )) == SettingsPane::Security)
                {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:92", use_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((2.0) as f32),
                            ),
                            padding: None,
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[7]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                            }),
                        });
                }
                if (!((*self
                    .settings_screen_states
                    .get(&ctx_0)
                    .map_or(
                        &self.settings_screen_initial.settings_pane,
                        |local| &local.settings_pane,
                    )) == SettingsPane::Security))
                {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:99", use_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((2.0) as f32),
                            ),
                            padding: None,
                            align_x: None,
                            align_y: None,
                            background: (Some(
                                ::ducktape_view_guest::wire::Rgba([
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.0 / 255.0,
                                    0.000000,
                                ]),
                            ))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                            }),
                        });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: None,
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_group_label_4(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Text {
                options: ::ducktape_view_guest::wire::TextOptions {
                    height: None,
                    align_y: None,
                    line_height: None,
                    shaping: None,
                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                    tracking: 0.0f32,
                    font: Some(::ducktape_view_guest::wire::NamedFont {
                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                            "Geist Mono".into(),
                        ),
                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                    }),
                },
                key: node_scope.clone(),
                size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                color: Some(palette.colors[73]),
                font: ::ducktape_view_guest::wire::Font {
                    monospace: false,
                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                },
                width: None,
                align_x: None,
                content: ("APPEARANCE".to_owned()).to_string(),
            }
        }
    }
    pub(crate) fn render_group_label_5(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Text {
                options: ::ducktape_view_guest::wire::TextOptions {
                    height: None,
                    align_y: None,
                    line_height: None,
                    shaping: None,
                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                    tracking: 0.0f32,
                    font: Some(::ducktape_view_guest::wire::NamedFont {
                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                            "Geist Mono".into(),
                        ),
                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                    }),
                },
                key: node_scope.clone(),
                size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                color: Some(palette.colors[73]),
                font: ::ducktape_view_guest::wire::Font {
                    monospace: false,
                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                },
                width: None,
                align_x: None,
                content: ("NOTIFICATIONS".to_owned()).to_string(),
            }
        }
    }
    pub(crate) fn render_group_label_6(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Text {
                options: ::ducktape_view_guest::wire::TextOptions {
                    height: None,
                    align_y: None,
                    line_height: None,
                    shaping: None,
                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                    tracking: 0.0f32,
                    font: Some(::ducktape_view_guest::wire::NamedFont {
                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                            "Geist Mono".into(),
                        ),
                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                    }),
                },
                key: node_scope.clone(),
                size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                color: Some(palette.colors[73]),
                font: ::ducktape_view_guest::wire::Font {
                    monospace: false,
                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                },
                width: None,
                align_x: None,
                content: ("NETWORK".to_owned()).to_string(),
            }
        }
    }
    pub(crate) fn render_key_value_row_7(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push(::ducktape_view_guest::wire::Node::Container {
                        shadow: ::ducktape_view_guest::wire::Shadow {
                            color: None,
                            x: None,
                            y: None,
                            blur: None,
                        },
                        max_width: None,
                        max_height: None,
                        clip: false,
                        key: format!("{}/@container:26", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        padding: Some(::ducktape_view_guest::wire::Edges {
                            top: (13.0) as f32,
                            right: (15.0) as f32,
                            bottom: (13.0) as f32,
                            left: (15.0) as f32,
                        }),
                        align_x: None,
                        align_y: None,
                        background: (None)
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: None,
                        snap: None,
                        content: Box::new({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:36", use_scope),
                                    size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[15]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("Workspace".to_owned()).to_string(),
                                });
                            children
                                .push(::ducktape_view_guest::wire::Node::Space {
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    height: None,
                                });
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist Mono".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Medium,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:42", use_scope),
                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[13]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: (self.network_name.to_owned()).to_string(),
                                });
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:31", use_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Row,
                                spacing: Some((10.0) as f32),
                                padding: None,
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                background: None,
                                border: None,
                                children: children,
                            }
                        }),
                    });
                {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:49", use_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                            ),
                            padding: None,
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[55]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                            }),
                        });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_key_value_row_8(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push(::ducktape_view_guest::wire::Node::Container {
                        shadow: ::ducktape_view_guest::wire::Shadow {
                            color: None,
                            x: None,
                            y: None,
                            blur: None,
                        },
                        max_width: None,
                        max_height: None,
                        clip: false,
                        key: format!("{}/@container:26", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        padding: Some(::ducktape_view_guest::wire::Edges {
                            top: (13.0) as f32,
                            right: (15.0) as f32,
                            bottom: (13.0) as f32,
                            left: (15.0) as f32,
                        }),
                        align_x: None,
                        align_y: None,
                        background: (None)
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: None,
                        snap: None,
                        content: Box::new({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:36", use_scope),
                                    size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[15]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("Endpoint".to_owned()).to_string(),
                                });
                            children
                                .push(::ducktape_view_guest::wire::Node::Space {
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    height: None,
                                });
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist Mono".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Medium,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:42", use_scope),
                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[13]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: (self.connected_rpc.to_owned()).to_string(),
                                });
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:31", use_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Row,
                                spacing: Some((10.0) as f32),
                                padding: None,
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                background: None,
                                border: None,
                                children: children,
                            }
                        }),
                    });
                {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:49", use_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                            ),
                            padding: None,
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[55]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                            }),
                        });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_badge_destructive_9(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Container {
                shadow: ::ducktape_view_guest::wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: false,
                key: node_scope.clone(),
                width: None,
                height: None,
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (3.0) as f32,
                    right: (7.0) as f32,
                    bottom: (3.0) as f32,
                    left: (7.0) as f32,
                }),
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[22]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[23]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:154", use_scope),
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((6.0) as f32),
                            ),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((6.0) as f32),
                            ),
                            padding: None,
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[24]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: Some(::ducktape_view_guest::wire::Border {
                                color: None,
                                width: None,
                                radius: Some([
                                    ((3.0) as f32).max(0.0).min(f32::MAX),
                                    ((3.0) as f32).max(0.0).min(f32::MAX),
                                    ((3.0) as f32).max(0.0).min(f32::MAX),
                                    ((3.0) as f32).max(0.0).min(f32::MAX),
                                ]),
                            }),
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                            }),
                        });
                    children
                        .push(::ducktape_view_guest::wire::Node::Text {
                            options: ::ducktape_view_guest::wire::TextOptions {
                                height: None,
                                align_y: None,
                                line_height: Some(
                                    ::ducktape_view_guest::wire::LineHeight::Relative(1.35f32),
                                ),
                                shaping: None,
                                wrapping: None,
                                tracking: 0.0f32,
                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                        "Geist".into(),
                                    ),
                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:161", use_scope),
                            size: Some(9.0f32),
                            color: Some(palette.colors[4]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                            },
                            width: None,
                            align_x: None,
                            content: (self.status.to_owned()).to_string(),
                        });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:153", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Row,
                        spacing: Some((5.0) as f32),
                        padding: None,
                        width: None,
                        height: None,
                        align: Some(::ducktape_view_guest::wire::AlignX::Center),
                        background: None,
                        border: None,
                        children: children,
                    }
                }),
            }
        }
    }
    pub(crate) fn render_badge_success_10(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Container {
                shadow: ::ducktape_view_guest::wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: false,
                key: node_scope.clone(),
                width: None,
                height: None,
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (3.0) as f32,
                    right: (7.0) as f32,
                    bottom: (3.0) as f32,
                    left: (7.0) as f32,
                }),
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[27]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[28]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:173", use_scope),
                            width: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((6.0) as f32),
                            ),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((6.0) as f32),
                            ),
                            padding: None,
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[29]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: Some(::ducktape_view_guest::wire::Border {
                                color: None,
                                width: None,
                                radius: Some([
                                    ((3.0) as f32).max(0.0).min(f32::MAX),
                                    ((3.0) as f32).max(0.0).min(f32::MAX),
                                    ((3.0) as f32).max(0.0).min(f32::MAX),
                                    ((3.0) as f32).max(0.0).min(f32::MAX),
                                ]),
                            }),
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                            }),
                        });
                    children
                        .push(::ducktape_view_guest::wire::Node::Text {
                            options: ::ducktape_view_guest::wire::TextOptions {
                                height: None,
                                align_y: None,
                                line_height: Some(
                                    ::ducktape_view_guest::wire::LineHeight::Relative(1.35f32),
                                ),
                                shaping: None,
                                wrapping: None,
                                tracking: 0.0f32,
                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                        "Geist".into(),
                                    ),
                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:180", use_scope),
                            size: Some(9.0f32),
                            color: Some(palette.colors[4]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                            },
                            width: None,
                            align_x: None,
                            content: (self.status.to_owned()).to_string(),
                        });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:172", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Row,
                        spacing: Some((5.0) as f32),
                        padding: None,
                        width: None,
                        height: None,
                        align: Some(::ducktape_view_guest::wire::AlignX::Center),
                        background: None,
                        border: None,
                        children: children,
                    }
                }),
            }
        }
    }
    pub(crate) fn render_group_card_11(
        &self,
        palette: Palette,
        use_scope: String,
        ctx_0: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Container {
                shadow: ::ducktape_view_guest::wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: true,
                key: node_scope.clone(),
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                height: None,
                padding: None,
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[3]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[63]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    children
                        .push({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            children
                                .push(
                                    self
                                        .render_key_value_row_7(
                                            palette,
                                            format!("{}/KeyValueRow@802", use_scope),
                                        ),
                                );
                            children
                                .push(
                                    self
                                        .render_key_value_row_8(
                                            palette,
                                            format!("{}/KeyValueRow@807", use_scope),
                                        ),
                                );
                            children
                                .push(::ducktape_view_guest::wire::Node::Container {
                                    shadow: ::ducktape_view_guest::wire::Shadow {
                                        color: None,
                                        x: None,
                                        y: None,
                                        blur: None,
                                    },
                                    max_width: None,
                                    max_height: None,
                                    clip: false,
                                    key: format!("{}/@container:273", use_scope),
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    height: None,
                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                        top: (13.0) as f32,
                                        right: (15.0) as f32,
                                        bottom: (13.0) as f32,
                                        left: (15.0) as f32,
                                    }),
                                    align_x: None,
                                    align_y: None,
                                    background: (None)
                                        .map(::ducktape_view_guest::wire::Background::Color),
                                    border: None,
                                    snap: None,
                                    content: Box::new({
                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                        children
                                            .push(::ducktape_view_guest::wire::Node::Text {
                                                options: ::ducktape_view_guest::wire::TextOptions {
                                                    height: None,
                                                    align_y: None,
                                                    line_height: None,
                                                    shaping: None,
                                                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                    tracking: 0.0f32,
                                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                            "Geist".into(),
                                                        ),
                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                    }),
                                                },
                                                key: format!("{}/@text:283", use_scope),
                                                size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                color: Some(palette.colors[15]),
                                                font: ::ducktape_view_guest::wire::Font {
                                                    monospace: false,
                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                },
                                                width: None,
                                                align_x: None,
                                                content: ("Members".to_owned()).to_string(),
                                            });
                                        children
                                            .push(::ducktape_view_guest::wire::Node::Space {
                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                height: None,
                                            });
                                        children
                                            .push(::ducktape_view_guest::wire::Node::Text {
                                                options: ::ducktape_view_guest::wire::TextOptions {
                                                    height: None,
                                                    align_y: None,
                                                    line_height: None,
                                                    shaping: None,
                                                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                    tracking: 0.0f32,
                                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                            "Geist Mono".into(),
                                                        ),
                                                        weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                    }),
                                                },
                                                key: format!("{}/@text:289", use_scope),
                                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                color: Some(palette.colors[13]),
                                                font: ::ducktape_view_guest::wire::Font {
                                                    monospace: false,
                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                },
                                                width: None,
                                                align_x: None,
                                                content: (self.members_line.to_owned()).to_string(),
                                            });
                                        children
                                            .push(::ducktape_view_guest::wire::Node::Button {
                                                checked: None,
                                                expanded: None,
                                                description: None,
                                                key: format!("{}/@button:295", use_scope),
                                                content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                                    String::from("manage"),
                                                ),
                                                label: None,
                                                on_press: Some(
                                                    ::ducktape_view_guest::slots::message(
                                                        Message::ShowTab("members".to_owned()),
                                                    ),
                                                ),
                                                width: None,
                                                height: None,
                                                padding: Some(
                                                    ::ducktape_view_guest::wire::Edges::all((0.0) as f32),
                                                ),
                                                style: ::ducktape_view_guest::wire::ButtonStyle {
                                                    preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                    recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                        base: ::ducktape_view_guest::wire::Face {
                                                            background: Some(
                                                                ::ducktape_view_guest::wire::Rgba([
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.000000,
                                                                ]),
                                                            ),
                                                            text: Some(palette.colors[4]),
                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                color: None,
                                                                width: None,
                                                                radius: Some([8.0; 4]),
                                                            }),
                                                        },
                                                        hover_background: Some(palette.colors[14]),
                                                        pressed_background: Some(palette.colors[39]),
                                                        disabled_background: None,
                                                        disabled_text: None,
                                                        disabled_opacity: Some(0.5f32),
                                                        focus_ring: Some(palette.colors[42]),
                                                        text_size: Some(11.0f32),
                                                        line_height: Some(1.35f32),
                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                "Geist".into(),
                                                            ),
                                                            weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                        }),
                                                    }),
                                                    active: ::ducktape_view_guest::wire::Face {
                                                        background: Some(
                                                            ::ducktape_view_guest::wire::Rgba([
                                                                0.0 / 255.0,
                                                                0.0 / 255.0,
                                                                0.0 / 255.0,
                                                                0.000000,
                                                            ]),
                                                        ),
                                                        text: Some(palette.colors[16]),
                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                            color: Some(
                                                                ::ducktape_view_guest::wire::Rgba([
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.000000,
                                                                ]),
                                                            ),
                                                            width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                            radius: Some([
                                                                ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                ((6.0) as f32).max(0.0).min(f32::MAX),
                                                            ]),
                                                        }),
                                                    },
                                                    hovered: Some(::ducktape_view_guest::wire::Face {
                                                        background: Some(palette.colors[55]),
                                                        text: Some(palette.colors[16]),
                                                        border: None,
                                                    }),
                                                    pressed: Some(::ducktape_view_guest::wire::Face {
                                                        background: Some(palette.colors[56]),
                                                        text: Some(palette.colors[16]),
                                                        border: None,
                                                    }),
                                                    disabled: None,
                                                },
                                            });
                                        ::ducktape_view_guest::wire::Node::Linear {
                                            max_width: None,
                                            clip: false,
                                            key: format!("{}/@layout:278", use_scope),
                                            wrap: None,
                                            axis: ::ducktape_view_guest::wire::Axis::Row,
                                            spacing: Some((10.0) as f32),
                                            padding: None,
                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                            height: None,
                                            align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                            background: None,
                                            border: None,
                                            children: children,
                                        }
                                    }),
                                });
                            children
                                .push(::ducktape_view_guest::wire::Node::Container {
                                    shadow: ::ducktape_view_guest::wire::Shadow {
                                        color: None,
                                        x: None,
                                        y: None,
                                        blur: None,
                                    },
                                    max_width: None,
                                    max_height: None,
                                    clip: false,
                                    key: format!("{}/@container:302", use_scope),
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    height: None,
                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                        top: (13.0) as f32,
                                        right: (15.0) as f32,
                                        bottom: (13.0) as f32,
                                        left: (15.0) as f32,
                                    }),
                                    align_x: None,
                                    align_y: None,
                                    background: (None)
                                        .map(::ducktape_view_guest::wire::Background::Color),
                                    border: None,
                                    snap: None,
                                    content: Box::new({
                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                        children
                                            .push(::ducktape_view_guest::wire::Node::Text {
                                                options: ::ducktape_view_guest::wire::TextOptions {
                                                    height: None,
                                                    align_y: None,
                                                    line_height: None,
                                                    shaping: None,
                                                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                    tracking: 0.0f32,
                                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                            "Geist".into(),
                                                        ),
                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                    }),
                                                },
                                                key: format!("{}/@text:312", use_scope),
                                                size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                color: Some(palette.colors[15]),
                                                font: ::ducktape_view_guest::wire::Font {
                                                    monospace: false,
                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                },
                                                width: None,
                                                align_x: None,
                                                content: ("Node".to_owned()).to_string(),
                                            });
                                        children
                                            .push(::ducktape_view_guest::wire::Node::Space {
                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                height: None,
                                            });
                                        children
                                            .push(::ducktape_view_guest::wire::Node::Text {
                                                options: ::ducktape_view_guest::wire::TextOptions {
                                                    height: None,
                                                    align_y: None,
                                                    line_height: None,
                                                    shaping: None,
                                                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                    tracking: 0.0f32,
                                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                            "Geist Mono".into(),
                                                        ),
                                                        weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                    }),
                                                },
                                                key: format!("{}/@text:318", use_scope),
                                                size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                color: Some(palette.colors[13]),
                                                font: ::ducktape_view_guest::wire::Font {
                                                    monospace: false,
                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                },
                                                width: None,
                                                align_x: None,
                                                content: (self.status.to_owned()).to_string(),
                                            });
                                        children
                                            .push(::ducktape_view_guest::wire::Node::Button {
                                                checked: None,
                                                expanded: None,
                                                description: None,
                                                key: format!("{}/@button:324", use_scope),
                                                content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                                    String::from("view"),
                                                ),
                                                label: None,
                                                on_press: Some(
                                                    ::ducktape_view_guest::slots::message(
                                                        Message::ShowTab("node".to_owned()),
                                                    ),
                                                ),
                                                width: None,
                                                height: None,
                                                padding: Some(
                                                    ::ducktape_view_guest::wire::Edges::all((0.0) as f32),
                                                ),
                                                style: ::ducktape_view_guest::wire::ButtonStyle {
                                                    preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                    recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                        base: ::ducktape_view_guest::wire::Face {
                                                            background: Some(
                                                                ::ducktape_view_guest::wire::Rgba([
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.000000,
                                                                ]),
                                                            ),
                                                            text: Some(palette.colors[4]),
                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                color: None,
                                                                width: None,
                                                                radius: Some([8.0; 4]),
                                                            }),
                                                        },
                                                        hover_background: Some(palette.colors[14]),
                                                        pressed_background: Some(palette.colors[39]),
                                                        disabled_background: None,
                                                        disabled_text: None,
                                                        disabled_opacity: Some(0.5f32),
                                                        focus_ring: Some(palette.colors[42]),
                                                        text_size: Some(11.0f32),
                                                        line_height: Some(1.35f32),
                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                "Geist".into(),
                                                            ),
                                                            weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                        }),
                                                    }),
                                                    active: ::ducktape_view_guest::wire::Face {
                                                        background: Some(
                                                            ::ducktape_view_guest::wire::Rgba([
                                                                0.0 / 255.0,
                                                                0.0 / 255.0,
                                                                0.0 / 255.0,
                                                                0.000000,
                                                            ]),
                                                        ),
                                                        text: Some(palette.colors[16]),
                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                            color: Some(
                                                                ::ducktape_view_guest::wire::Rgba([
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.000000,
                                                                ]),
                                                            ),
                                                            width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                            radius: Some([
                                                                ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                ((6.0) as f32).max(0.0).min(f32::MAX),
                                                                ((6.0) as f32).max(0.0).min(f32::MAX),
                                                            ]),
                                                        }),
                                                    },
                                                    hovered: Some(::ducktape_view_guest::wire::Face {
                                                        background: Some(palette.colors[55]),
                                                        text: Some(palette.colors[16]),
                                                        border: None,
                                                    }),
                                                    pressed: Some(::ducktape_view_guest::wire::Face {
                                                        background: Some(palette.colors[56]),
                                                        text: Some(palette.colors[16]),
                                                        border: None,
                                                    }),
                                                    disabled: None,
                                                },
                                            });
                                        ::ducktape_view_guest::wire::Node::Linear {
                                            max_width: None,
                                            clip: false,
                                            key: format!("{}/@layout:307", use_scope),
                                            wrap: None,
                                            axis: ::ducktape_view_guest::wire::Axis::Row,
                                            spacing: Some((10.0) as f32),
                                            padding: None,
                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                            height: None,
                                            align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                            background: None,
                                            border: None,
                                            children: children,
                                        }
                                    }),
                                });
                            children
                                .push(::ducktape_view_guest::wire::Node::Container {
                                    shadow: ::ducktape_view_guest::wire::Shadow {
                                        color: None,
                                        x: None,
                                        y: None,
                                        blur: None,
                                    },
                                    max_width: None,
                                    max_height: None,
                                    clip: false,
                                    key: format!("{}/@container:334", use_scope),
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    height: None,
                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                        top: (13.0) as f32,
                                        right: (15.0) as f32,
                                        bottom: (13.0) as f32,
                                        left: (15.0) as f32,
                                    }),
                                    align_x: None,
                                    align_y: None,
                                    background: (None)
                                        .map(::ducktape_view_guest::wire::Background::Color),
                                    border: None,
                                    snap: None,
                                    content: Box::new({
                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                        if crate::host::connection_degraded(
                                            ::std::convert::AsRef::as_ref(&(self.status)),
                                        ) {
                                            children
                                                .push(
                                                    self
                                                        .render_badge_destructive_9(
                                                            palette,
                                                            format!("{}/Badge.Destructive@886", use_scope),
                                                        ),
                                                );
                                        }
                                        if (!crate::host::connection_degraded(
                                            ::std::convert::AsRef::as_ref(&(self.status)),
                                        )) {
                                            children
                                                .push(
                                                    self
                                                        .render_badge_success_10(
                                                            palette,
                                                            format!("{}/Badge.Success@888", use_scope),
                                                        ),
                                                );
                                        }
                                        children
                                            .push(::ducktape_view_guest::wire::Node::Space {
                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                height: None,
                                            });
                                        children
                                            .push(::ducktape_view_guest::wire::Node::Button {
                                                checked: None,
                                                expanded: None,
                                                description: None,
                                                key: format!("{}/@button:349", use_scope),
                                                content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                                    String::from("Reconnect"),
                                                ),
                                                label: None,
                                                on_press: if ((self.loading
                                                    || (self.busy && (!self.recovering))))
                                                {
                                                    None
                                                } else {
                                                    Some(
                                                            ::ducktape_view_guest::slots::message(Message::Reconnect),
                                                        )
                                                },
                                                width: None,
                                                height: None,
                                                padding: Some(
                                                    ::ducktape_view_guest::wire::Edges::all((6.0) as f32),
                                                ),
                                                style: ::ducktape_view_guest::wire::ButtonStyle {
                                                    preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                    recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                        base: ::ducktape_view_guest::wire::Face {
                                                            background: Some(palette.colors[12]),
                                                            text: Some(palette.colors[13]),
                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                color: Some(palette.colors[40]),
                                                                width: Some(1.0),
                                                                radius: Some([9.0; 4]),
                                                            }),
                                                        },
                                                        hover_background: Some(palette.colors[14]),
                                                        pressed_background: Some(palette.colors[6]),
                                                        disabled_background: None,
                                                        disabled_text: None,
                                                        disabled_opacity: Some(0.5f32),
                                                        focus_ring: Some(palette.colors[42]),
                                                        text_size: Some(12.5f32),
                                                        line_height: None,
                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                "Geist".into(),
                                                            ),
                                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                        }),
                                                    }),
                                                    active: ::ducktape_view_guest::wire::Face::default(),
                                                    hovered: None,
                                                    pressed: None,
                                                    disabled: None,
                                                },
                                            });
                                        children
                                            .push(::ducktape_view_guest::wire::Node::Button {
                                                checked: None,
                                                expanded: None,
                                                description: None,
                                                key: format!("{}/@button:354", use_scope),
                                                content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                                    String::from("Switch network"),
                                                ),
                                                label: None,
                                                on_press: if (self.busy) {
                                                    None
                                                } else {
                                                    Some(
                                                            ::ducktape_view_guest::slots::message(
                                                                Message::SwitchNetwork,
                                                            ),
                                                        )
                                                },
                                                width: None,
                                                height: None,
                                                padding: Some(
                                                    ::ducktape_view_guest::wire::Edges::all((6.0) as f32),
                                                ),
                                                style: ::ducktape_view_guest::wire::ButtonStyle {
                                                    preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                    recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                        base: ::ducktape_view_guest::wire::Face {
                                                            background: Some(palette.colors[12]),
                                                            text: Some(palette.colors[13]),
                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                color: Some(palette.colors[40]),
                                                                width: Some(1.0),
                                                                radius: Some([9.0; 4]),
                                                            }),
                                                        },
                                                        hover_background: Some(palette.colors[14]),
                                                        pressed_background: Some(palette.colors[6]),
                                                        disabled_background: None,
                                                        disabled_text: None,
                                                        disabled_opacity: Some(0.5f32),
                                                        focus_ring: Some(palette.colors[42]),
                                                        text_size: Some(12.5f32),
                                                        line_height: None,
                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                "Geist".into(),
                                                            ),
                                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                        }),
                                                    }),
                                                    active: ::ducktape_view_guest::wire::Face::default(),
                                                    hovered: None,
                                                    pressed: None,
                                                    disabled: None,
                                                },
                                            });
                                        ::ducktape_view_guest::wire::Node::Linear {
                                            max_width: None,
                                            clip: false,
                                            key: format!("{}/@layout:339", use_scope),
                                            wrap: None,
                                            axis: ::ducktape_view_guest::wire::Axis::Row,
                                            spacing: Some((9.0) as f32),
                                            padding: None,
                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                            height: None,
                                            align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                            background: None,
                                            border: None,
                                            children: children,
                                        }
                                    }),
                                });
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:260", use_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Column,
                                spacing: None,
                                padding: None,
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                align: None,
                                background: None,
                                border: None,
                                children: children,
                            }
                        });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:21", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Column,
                        spacing: None,
                        padding: None,
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        align: None,
                        background: None,
                        border: None,
                        children: children,
                    }
                }),
            }
        }
    }
    pub(crate) fn render_group_label_12(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Text {
                options: ::ducktape_view_guest::wire::TextOptions {
                    height: None,
                    align_y: None,
                    line_height: None,
                    shaping: None,
                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                    tracking: 0.0f32,
                    font: Some(::ducktape_view_guest::wire::NamedFont {
                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                            "Geist Mono".into(),
                        ),
                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                    }),
                },
                key: node_scope.clone(),
                size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                color: Some(palette.colors[73]),
                font: ::ducktape_view_guest::wire::Font {
                    monospace: false,
                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                },
                width: None,
                align_x: None,
                content: ("YOUR IDENTITY".to_owned()).to_string(),
            }
        }
    }
    pub(crate) fn render_person_avatar_13(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Container {
                shadow: ::ducktape_view_guest::wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: false,
                key: node_scope.clone(),
                width: Some(::ducktape_view_guest::wire::Length::Fixed((40.0) as f32)),
                height: Some(::ducktape_view_guest::wire::Length::Fixed((40.0) as f32)),
                padding: None,
                align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                align_y: Some(::ducktape_view_guest::wire::AlignY::Center),
                background: (Some(palette.colors[35]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: None,
                    width: None,
                    radius: Some([
                        (((40.0 / 2.0)) as f32).max(0.0).min(f32::MAX),
                        (((40.0 / 2.0)) as f32).max(0.0).min(f32::MAX),
                        (((40.0 / 2.0)) as f32).max(0.0).min(f32::MAX),
                        (((40.0 / 2.0)) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new(::ducktape_view_guest::wire::Node::Text {
                    options: ::ducktape_view_guest::wire::TextOptions {
                        height: None,
                        align_y: None,
                        line_height: None,
                        shaping: None,
                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                        tracking: 0.0f32,
                        font: Some(::ducktape_view_guest::wire::NamedFont {
                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                "Geist".into(),
                            ),
                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                        }),
                    },
                    key: format!("{}/@text:117", use_scope),
                    size: Some(((14.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                    color: Some(palette.colors[5]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                    },
                    width: None,
                    align_x: None,
                    content: (crate::host::initial_of(
                            ::std::convert::AsRef::as_ref(&(self.account_name)),
                        )
                        .to_owned())
                        .to_string(),
                }),
            }
        }
    }
    pub(crate) fn render_badge_secondary_14(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Container {
                shadow: ::ducktape_view_guest::wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: false,
                key: node_scope.clone(),
                width: None,
                height: None,
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (3.0) as f32,
                    right: (7.0) as f32,
                    bottom: (3.0) as f32,
                    left: (7.0) as f32,
                }),
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[7]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: None,
                    width: None,
                    radius: Some([
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new(::ducktape_view_guest::wire::Node::Text {
                    options: ::ducktape_view_guest::wire::TextOptions {
                        height: None,
                        align_y: None,
                        line_height: Some(
                            ::ducktape_view_guest::wire::LineHeight::Relative(1.35f32),
                        ),
                        shaping: None,
                        wrapping: None,
                        tracking: 0.0f32,
                        font: Some(::ducktape_view_guest::wire::NamedFont {
                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                "Geist".into(),
                            ),
                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                        }),
                    },
                    key: format!("{}/@text:131", use_scope),
                    size: Some(9.0f32),
                    color: Some(palette.colors[9]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                    },
                    width: None,
                    align_x: None,
                    content: (self.tier.to_owned()).to_string(),
                }),
            }
        }
    }
    pub(crate) fn render_badge_outline_15(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Container {
                shadow: ::ducktape_view_guest::wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: false,
                key: node_scope.clone(),
                width: None,
                height: None,
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (3.0) as f32,
                    right: (7.0) as f32,
                    bottom: (3.0) as f32,
                    left: (7.0) as f32,
                }),
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[3]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[40]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new(::ducktape_view_guest::wire::Node::Text {
                    options: ::ducktape_view_guest::wire::TextOptions {
                        height: None,
                        align_y: None,
                        line_height: Some(
                            ::ducktape_view_guest::wire::LineHeight::Relative(1.35f32),
                        ),
                        shaping: None,
                        wrapping: None,
                        tracking: 0.0f32,
                        font: Some(::ducktape_view_guest::wire::NamedFont {
                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                "Geist".into(),
                            ),
                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                        }),
                    },
                    key: format!("{}/@text:142", use_scope),
                    size: Some(9.0f32),
                    color: Some(palette.colors[13]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                    },
                    width: None,
                    align_x: None,
                    content: (self.tier.to_owned()).to_string(),
                }),
            }
        }
    }
    pub(crate) fn render_badge_outline_16(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Container {
                shadow: ::ducktape_view_guest::wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: false,
                key: node_scope.clone(),
                width: None,
                height: None,
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (3.0) as f32,
                    right: (7.0) as f32,
                    bottom: (3.0) as f32,
                    left: (7.0) as f32,
                }),
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[3]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[40]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new(::ducktape_view_guest::wire::Node::Text {
                    options: ::ducktape_view_guest::wire::TextOptions {
                        height: None,
                        align_y: None,
                        line_height: Some(
                            ::ducktape_view_guest::wire::LineHeight::Relative(1.35f32),
                        ),
                        shaping: None,
                        wrapping: None,
                        tracking: 0.0f32,
                        font: Some(::ducktape_view_guest::wire::NamedFont {
                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                "Geist".into(),
                            ),
                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                        }),
                    },
                    key: format!("{}/@text:142", use_scope),
                    size: Some(9.0f32),
                    color: Some(palette.colors[13]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                    },
                    width: None,
                    align_x: None,
                    content: ("standing unknown".to_owned()).to_string(),
                }),
            }
        }
    }
    pub(crate) fn render_settings_loading(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                if (self.account_ceremony_phase == "show_qr") {
                    children
                        .push({
                            let node_scope = format!("{}/plate-qr", node_scope);
                            ::ducktape_view_guest::wire::Node::Qr {
                                key: node_scope.clone(),
                                code: ::ducktape_view_guest::wire::Qr {
                                    payload: Some(
                                        ::std::convert::AsRef::<
                                            [u8],
                                        >::as_ref(&(self.account_ceremony_qr))
                                            .to_vec(),
                                    ),
                                    version: None,
                                    correction: Some(
                                        ::ducktape_view_guest::wire::QrCorrection::Medium,
                                    ),
                                    size: Some(
                                        ::ducktape_view_guest::wire::QrSize::Cell((3.0) as f32),
                                    ),
                                    cell: None,
                                    background: None,
                                },
                            }
                        });
                    children
                        .push(::ducktape_view_guest::wire::Node::Text {
                            options: ::ducktape_view_guest::wire::TextOptions {
                                height: None,
                                align_y: None,
                                line_height: None,
                                shaping: None,
                                wrapping: None,
                                tracking: 0.0f32,
                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                        "Geist".into(),
                                    ),
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:195", use_scope),
                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[71]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            align_x: Some(::ducktape_view_guest::wire::AlignX::Center),
                            content: (self.account_ceremony_detail.to_owned())
                                .to_string(),
                        });
                    children
                        .push({
                            let node_scope = format!("{}/plate-left", node_scope);
                            ::ducktape_view_guest::wire::Node::Text {
                                options: ::ducktape_view_guest::wire::TextOptions {
                                    height: None,
                                    align_y: None,
                                    line_height: None,
                                    shaping: None,
                                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                    tracking: 0.0f32,
                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                            "Geist Mono".into(),
                                        ),
                                        weight: ::ducktape_view_guest::wire::Weight::Medium,
                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                    }),
                                },
                                key: node_scope.clone(),
                                size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                color: Some(palette.colors[72]),
                                font: ::ducktape_view_guest::wire::Font {
                                    monospace: false,
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                },
                                width: None,
                                align_x: None,
                                content: (self.account_ceremony_left.to_owned()).to_string(),
                            }
                        });
                    children
                        .push({
                            let node_scope = format!("{}/plate-cancel", node_scope);
                            ::ducktape_view_guest::wire::Node::Button {
                                checked: None,
                                expanded: None,
                                description: None,
                                key: node_scope.clone(),
                                content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                    String::from("Cancel"),
                                ),
                                label: None,
                                on_press: Some(
                                    ::ducktape_view_guest::slots::message(
                                        Message::AccountCeremonyCancel,
                                    ),
                                ),
                                width: None,
                                height: None,
                                padding: Some(
                                    ::ducktape_view_guest::wire::Edges::all((5.0) as f32),
                                ),
                                style: ::ducktape_view_guest::wire::ButtonStyle {
                                    preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                    recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                        base: ::ducktape_view_guest::wire::Face {
                                            background: Some(palette.colors[12]),
                                            text: Some(palette.colors[13]),
                                            border: Some(::ducktape_view_guest::wire::Border {
                                                color: Some(palette.colors[40]),
                                                width: Some(1.0),
                                                radius: Some([9.0; 4]),
                                            }),
                                        },
                                        hover_background: Some(palette.colors[14]),
                                        pressed_background: Some(palette.colors[6]),
                                        disabled_background: None,
                                        disabled_text: None,
                                        disabled_opacity: Some(0.5f32),
                                        focus_ring: Some(palette.colors[42]),
                                        text_size: Some(12.5f32),
                                        line_height: None,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    }),
                                    active: ::ducktape_view_guest::wire::Face::default(),
                                    hovered: None,
                                    pressed: None,
                                    disabled: None,
                                },
                            }
                        });
                }
                if (self.account_ceremony_phase == "working") {
                    children
                        .push(::ducktape_view_guest::wire::Node::Text {
                            options: ::ducktape_view_guest::wire::TextOptions {
                                height: None,
                                align_y: None,
                                line_height: None,
                                shaping: None,
                                wrapping: None,
                                tracking: 0.0f32,
                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                        "Geist".into(),
                                    ),
                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                }),
                            },
                            key: format!("{}/@text:212", use_scope),
                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                            color: Some(palette.colors[71]),
                            font: ::ducktape_view_guest::wire::Font {
                                monospace: false,
                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                            },
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            align_x: None,
                            content: (self.account_ceremony_detail.to_owned())
                                .to_string(),
                        });
                    children
                        .push({
                            let node_scope = format!(
                                "{}/plate-cancel-working", node_scope
                            );
                            ::ducktape_view_guest::wire::Node::Button {
                                checked: None,
                                expanded: None,
                                description: None,
                                key: node_scope.clone(),
                                content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                    String::from("Cancel"),
                                ),
                                label: None,
                                on_press: Some(
                                    ::ducktape_view_guest::slots::message(
                                        Message::AccountCeremonyCancel,
                                    ),
                                ),
                                width: None,
                                height: None,
                                padding: Some(
                                    ::ducktape_view_guest::wire::Edges::all((5.0) as f32),
                                ),
                                style: ::ducktape_view_guest::wire::ButtonStyle {
                                    preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                    recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                        base: ::ducktape_view_guest::wire::Face {
                                            background: Some(palette.colors[12]),
                                            text: Some(palette.colors[13]),
                                            border: Some(::ducktape_view_guest::wire::Border {
                                                color: Some(palette.colors[40]),
                                                width: Some(1.0),
                                                radius: Some([9.0; 4]),
                                            }),
                                        },
                                        hover_background: Some(palette.colors[14]),
                                        pressed_background: Some(palette.colors[6]),
                                        disabled_background: None,
                                        disabled_text: None,
                                        disabled_opacity: Some(0.5f32),
                                        focus_ring: Some(palette.colors[42]),
                                        text_size: Some(12.5f32),
                                        line_height: None,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    }),
                                    active: ::ducktape_view_guest::wire::Face::default(),
                                    hovered: None,
                                    pressed: None,
                                    disabled: None,
                                },
                            }
                        });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: Some((8.0) as f32),
                    padding: None,
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                    align: Some(::ducktape_view_guest::wire::AlignX::Center),
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_group_label_18(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Text {
                options: ::ducktape_view_guest::wire::TextOptions {
                    height: None,
                    align_y: None,
                    line_height: None,
                    shaping: None,
                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                    tracking: 0.0f32,
                    font: Some(::ducktape_view_guest::wire::NamedFont {
                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                            "Geist Mono".into(),
                        ),
                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                    }),
                },
                key: node_scope.clone(),
                size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                color: Some(palette.colors[73]),
                font: ::ducktape_view_guest::wire::Font {
                    monospace: false,
                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                },
                width: None,
                align_x: None,
                content: ("ACCOUNT KEYS".to_owned()).to_string(),
            }
        }
    }
    pub(crate) fn render_badge_outline_19(
        &self,
        palette: Palette,
        use_scope: String,
        arg_0: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Container {
                shadow: ::ducktape_view_guest::wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: false,
                key: node_scope.clone(),
                width: None,
                height: None,
                padding: Some(::ducktape_view_guest::wire::Edges {
                    top: (3.0) as f32,
                    right: (7.0) as f32,
                    bottom: (3.0) as f32,
                    left: (7.0) as f32,
                }),
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[3]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[40]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                        ((5.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new(::ducktape_view_guest::wire::Node::Text {
                    options: ::ducktape_view_guest::wire::TextOptions {
                        height: None,
                        align_y: None,
                        line_height: Some(
                            ::ducktape_view_guest::wire::LineHeight::Relative(1.35f32),
                        ),
                        shaping: None,
                        wrapping: None,
                        tracking: 0.0f32,
                        font: Some(::ducktape_view_guest::wire::NamedFont {
                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                "Geist".into(),
                            ),
                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                        }),
                    },
                    key: format!("{}/@text:142", use_scope),
                    size: Some(9.0f32),
                    color: Some(palette.colors[13]),
                    font: ::ducktape_view_guest::wire::Font {
                        monospace: false,
                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                    },
                    width: None,
                    align_x: None,
                    content: (arg_0.to_owned()).to_string(),
                }),
            }
        }
    }
    pub(crate) fn render_group_card_20(
        &self,
        palette: Palette,
        use_scope: String,
        ctx_0: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Container {
                shadow: ::ducktape_view_guest::wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: true,
                key: node_scope.clone(),
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                height: None,
                padding: None,
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[3]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[63]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    children
                        .push({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            for (index, row) in self.account_key_rows.iter().enumerate()
                            {
                                let for_scope = format!(
                                    "{}/@for:1150({})", use_scope, index
                                );
                                children
                                    .push(::ducktape_view_guest::wire::Node::Container {
                                        shadow: ::ducktape_view_guest::wire::Shadow {
                                            color: None,
                                            x: None,
                                            y: None,
                                            blur: None,
                                        },
                                        max_width: None,
                                        max_height: None,
                                        clip: false,
                                        key: format!("{}/@container:610", for_scope),
                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                        height: None,
                                        padding: Some(::ducktape_view_guest::wire::Edges {
                                            top: (11.0) as f32,
                                            right: (15.0) as f32,
                                            bottom: (11.0) as f32,
                                            left: (15.0) as f32,
                                        }),
                                        align_x: None,
                                        align_y: None,
                                        background: (None)
                                            .map(::ducktape_view_guest::wire::Background::Color),
                                        border: None,
                                        snap: None,
                                        content: Box::new({
                                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                            children
                                                .push(
                                                    self
                                                        .render_badge_outline_19(
                                                            palette,
                                                            format!("{}/Badge.Outline@1161", for_scope),
                                                            row.scheme.to_owned(),
                                                        ),
                                                );
                                            children
                                                .push({
                                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                    if (!(row.label).is_empty()) {
                                                        children
                                                            .push(::ducktape_view_guest::wire::Node::Text {
                                                                options: ::ducktape_view_guest::wire::TextOptions {
                                                                    height: None,
                                                                    align_y: None,
                                                                    line_height: None,
                                                                    shaping: None,
                                                                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                                    tracking: 0.0f32,
                                                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                            "Geist".into(),
                                                                        ),
                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                    }),
                                                                },
                                                                key: format!("{}/@text:627", for_scope),
                                                                size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                color: Some(palette.colors[4]),
                                                                font: ::ducktape_view_guest::wire::Font {
                                                                    monospace: false,
                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                },
                                                                width: None,
                                                                align_x: None,
                                                                content: (row.label.to_owned()).to_string(),
                                                            });
                                                    }
                                                    if (row.label).is_empty() {
                                                        children
                                                            .push(::ducktape_view_guest::wire::Node::Text {
                                                                options: ::ducktape_view_guest::wire::TextOptions {
                                                                    height: None,
                                                                    align_y: None,
                                                                    line_height: None,
                                                                    shaping: None,
                                                                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                                    tracking: 0.0f32,
                                                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                            "Geist".into(),
                                                                        ),
                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                    }),
                                                                },
                                                                key: format!("{}/@text:633", for_scope),
                                                                size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                color: Some(palette.colors[5]),
                                                                font: ::ducktape_view_guest::wire::Font {
                                                                    monospace: false,
                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                },
                                                                width: None,
                                                                align_x: None,
                                                                content: ("(unlabeled)".to_owned()).to_string(),
                                                            });
                                                    }
                                                    children
                                                        .push(::ducktape_view_guest::wire::Node::Text {
                                                            options: ::ducktape_view_guest::wire::TextOptions {
                                                                height: None,
                                                                align_y: None,
                                                                line_height: None,
                                                                shaping: None,
                                                                wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                                tracking: 0.0f32,
                                                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                        "Geist Mono".into(),
                                                                    ),
                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                }),
                                                            },
                                                            key: format!("{}/@text:638", for_scope),
                                                            size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                            color: Some(palette.colors[72]),
                                                            font: ::ducktape_view_guest::wire::Font {
                                                                monospace: false,
                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                            },
                                                            width: None,
                                                            align_x: None,
                                                            content: (row.pubkey.to_owned()).to_string(),
                                                        });
                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                        max_width: None,
                                                        clip: true,
                                                        key: format!("{}/@layout:621", for_scope),
                                                        wrap: None,
                                                        axis: ::ducktape_view_guest::wire::Axis::Column,
                                                        spacing: Some((2.0) as f32),
                                                        padding: None,
                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                        height: None,
                                                        align: None,
                                                        background: None,
                                                        border: None,
                                                        children: children,
                                                    }
                                                });
                                            children
                                                .push(::ducktape_view_guest::wire::Node::Button {
                                                    checked: None,
                                                    expanded: None,
                                                    description: None,
                                                    key: format!("{}/@button:644", for_scope),
                                                    content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                                        String::from("Remove"),
                                                    ),
                                                    label: None,
                                                    on_press: if (((self.account_busy || (!self.unlocked))
                                                        || ((*self.derived_account_keys()) <= 1)))
                                                    {
                                                        None
                                                    } else {
                                                        Some(
                                                                ::ducktape_view_guest::slots::message(
                                                                    Message::AccountKeyRemove(row.pubkey.to_owned()),
                                                                ),
                                                            )
                                                    },
                                                    width: None,
                                                    height: None,
                                                    padding: Some(
                                                        ::ducktape_view_guest::wire::Edges::all((5.0) as f32),
                                                    ),
                                                    style: ::ducktape_view_guest::wire::ButtonStyle {
                                                        preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                        recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                            base: ::ducktape_view_guest::wire::Face {
                                                                background: Some(palette.colors[12]),
                                                                text: Some(palette.colors[13]),
                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                    color: Some(palette.colors[40]),
                                                                    width: Some(1.0),
                                                                    radius: Some([9.0; 4]),
                                                                }),
                                                            },
                                                            hover_background: Some(palette.colors[14]),
                                                            pressed_background: Some(palette.colors[6]),
                                                            disabled_background: None,
                                                            disabled_text: None,
                                                            disabled_opacity: Some(0.5f32),
                                                            focus_ring: Some(palette.colors[42]),
                                                            text_size: Some(12.5f32),
                                                            line_height: None,
                                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                    "Geist".into(),
                                                                ),
                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                            }),
                                                        }),
                                                        active: ::ducktape_view_guest::wire::Face::default(),
                                                        hovered: None,
                                                        pressed: None,
                                                        disabled: None,
                                                    },
                                                });
                                            ::ducktape_view_guest::wire::Node::Linear {
                                                max_width: None,
                                                clip: false,
                                                key: format!("{}/@layout:615", for_scope),
                                                wrap: None,
                                                axis: ::ducktape_view_guest::wire::Axis::Row,
                                                spacing: Some((10.0) as f32),
                                                padding: None,
                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                height: None,
                                                align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                background: None,
                                                border: None,
                                                children: children,
                                            }
                                        }),
                                    });
                            }
                            children
                                .push(::ducktape_view_guest::wire::Node::Container {
                                    shadow: ::ducktape_view_guest::wire::Shadow {
                                        color: None,
                                        x: None,
                                        y: None,
                                        blur: None,
                                    },
                                    max_width: None,
                                    max_height: None,
                                    clip: false,
                                    key: format!("{}/@container:651", use_scope),
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    height: None,
                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                        top: (13.0) as f32,
                                        right: (15.0) as f32,
                                        bottom: (13.0) as f32,
                                        left: (15.0) as f32,
                                    }),
                                    align_x: None,
                                    align_y: None,
                                    background: (None)
                                        .map(::ducktape_view_guest::wire::Background::Color),
                                    border: None,
                                    snap: None,
                                    content: Box::new({
                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                        children
                                            .push(::ducktape_view_guest::wire::Node::Text {
                                                options: ::ducktape_view_guest::wire::TextOptions {
                                                    height: None,
                                                    align_y: None,
                                                    line_height: None,
                                                    shaping: None,
                                                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                    tracking: 0.0f32,
                                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                            "Geist".into(),
                                                        ),
                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                    }),
                                                },
                                                key: format!("{}/@text:657", use_scope),
                                                size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                color: Some(palette.colors[15]),
                                                font: ::ducktape_view_guest::wire::Font {
                                                    monospace: false,
                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                },
                                                width: None,
                                                align_x: None,
                                                content: ("Add a device".to_owned()).to_string(),
                                            });
                                        children
                                            .push({
                                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                children
                                                    .push({
                                                        let node_scope = format!("{}/account-key", node_scope);
                                                        ::ducktape_view_guest::wire::Node::Input {
                                                            options: ::ducktape_view_guest::wire::InputOptions {
                                                                label: ("Other device's public key".to_owned()).to_string(),
                                                                description: None,
                                                                disabled: self.account_busy,
                                                                padding: Some(
                                                                    ::ducktape_view_guest::wire::Edges::all((5.0) as f32),
                                                                ),
                                                                text_size: Some((12.5) as f32),
                                                                line_height: Some((1.2) as f32),
                                                                align: None,
                                                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                        "Geist".into(),
                                                                    ),
                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                }),
                                                            },
                                                            key: node_scope.clone(),
                                                            placeholder: String::from(
                                                                "paste its ed25519 key (hex)…".to_owned(),
                                                            ),
                                                            value: (self.account_key_draft).to_string(),
                                                            on_input: ::ducktape_view_guest::slots::handler::<
                                                                String,
                                                                Message,
                                                            >(
                                                                Box::new({
                                                                    let route = Message::BindAccountKeyDraft
                                                                        as fn(String) -> Message;
                                                                    move |sent: String| Some(route(sent))
                                                                }),
                                                            ),
                                                            on_submit: None,
                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                            secure: (false),
                                                            style: Box::new(::ducktape_view_guest::wire::InputStyle {
                                                                utility: ::ducktape_view_guest::wire::InputFace {
                                                                    background: Some(palette.colors[3]),
                                                                    border: Some(::ducktape_view_guest::wire::Border {
                                                                        color: Some(palette.colors[39]),
                                                                        width: Some(1f32),
                                                                        radius: Some([10f32; 4]),
                                                                    }),
                                                                    ..Default::default()
                                                                },
                                                                focus_border: Some(palette.colors[42]),
                                                                focused_hovered: None,
                                                                active: ::ducktape_view_guest::wire::InputFace {
                                                                    icon: None,
                                                                    background: Some(palette.colors[55]),
                                                                    border: Some(::ducktape_view_guest::wire::Border {
                                                                        color: Some({
                                                                            let mut color = palette.colors[4];
                                                                            color.0[3] = 0.160000;
                                                                            color
                                                                        }),
                                                                        width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                        radius: Some([
                                                                            ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                            ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                            ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                            ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                        ]),
                                                                    }),
                                                                    value: Some(palette.colors[4]),
                                                                    placeholder: Some(palette.colors[5]),
                                                                    selection: Some({
                                                                        let mut color = palette.colors[4];
                                                                        color.0[3] = 0.180000;
                                                                        color
                                                                    }),
                                                                },
                                                                hovered: Some(::ducktape_view_guest::wire::InputFace {
                                                                    icon: None,
                                                                    background: Some(palette.colors[55]),
                                                                    border: Some(::ducktape_view_guest::wire::Border {
                                                                        color: Some({
                                                                            let mut color = palette.colors[4];
                                                                            color.0[3] = 0.210000;
                                                                            color
                                                                        }),
                                                                        width: None,
                                                                        radius: None,
                                                                    }),
                                                                    value: None,
                                                                    placeholder: None,
                                                                    selection: None,
                                                                }),
                                                                focused: None,
                                                                disabled: Some(::ducktape_view_guest::wire::InputFace {
                                                                    icon: None,
                                                                    background: Some({
                                                                        let mut color = palette.colors[6];
                                                                        color.0[3] = 0.540000;
                                                                        color
                                                                    }),
                                                                    border: None,
                                                                    value: Some(palette.colors[5]),
                                                                    placeholder: None,
                                                                    selection: None,
                                                                }),
                                                            }),
                                                        }
                                                    });
                                                children
                                                    .push({
                                                        let node_scope = format!(
                                                            "{}/account-key-label", node_scope
                                                        );
                                                        ::ducktape_view_guest::wire::Node::Input {
                                                            options: ::ducktape_view_guest::wire::InputOptions {
                                                                label: ("Key label".to_owned()).to_string(),
                                                                description: None,
                                                                disabled: self.account_busy,
                                                                padding: Some(
                                                                    ::ducktape_view_guest::wire::Edges::all((5.0) as f32),
                                                                ),
                                                                text_size: Some((12.5) as f32),
                                                                line_height: Some((1.2) as f32),
                                                                align: None,
                                                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                        "Geist".into(),
                                                                    ),
                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                }),
                                                            },
                                                            key: node_scope.clone(),
                                                            placeholder: String::from("label…".to_owned()),
                                                            value: (self.account_key_label_draft).to_string(),
                                                            on_input: ::ducktape_view_guest::slots::handler::<
                                                                String,
                                                                Message,
                                                            >(
                                                                Box::new({
                                                                    let route = Message::BindAccountKeyLabelDraft
                                                                        as fn(String) -> Message;
                                                                    move |sent: String| Some(route(sent))
                                                                }),
                                                            ),
                                                            on_submit: None,
                                                            width: Some(
                                                                ::ducktape_view_guest::wire::Length::Fixed((120.0) as f32),
                                                            ),
                                                            secure: (false),
                                                            style: Box::new(::ducktape_view_guest::wire::InputStyle {
                                                                utility: ::ducktape_view_guest::wire::InputFace {
                                                                    background: Some(palette.colors[3]),
                                                                    border: Some(::ducktape_view_guest::wire::Border {
                                                                        color: Some(palette.colors[39]),
                                                                        width: Some(1f32),
                                                                        radius: Some([10f32; 4]),
                                                                    }),
                                                                    ..Default::default()
                                                                },
                                                                focus_border: Some(palette.colors[42]),
                                                                focused_hovered: None,
                                                                active: ::ducktape_view_guest::wire::InputFace {
                                                                    icon: None,
                                                                    background: Some(palette.colors[55]),
                                                                    border: Some(::ducktape_view_guest::wire::Border {
                                                                        color: Some({
                                                                            let mut color = palette.colors[4];
                                                                            color.0[3] = 0.160000;
                                                                            color
                                                                        }),
                                                                        width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                        radius: Some([
                                                                            ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                            ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                            ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                            ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                        ]),
                                                                    }),
                                                                    value: Some(palette.colors[4]),
                                                                    placeholder: Some(palette.colors[5]),
                                                                    selection: Some({
                                                                        let mut color = palette.colors[4];
                                                                        color.0[3] = 0.180000;
                                                                        color
                                                                    }),
                                                                },
                                                                hovered: Some(::ducktape_view_guest::wire::InputFace {
                                                                    icon: None,
                                                                    background: Some(palette.colors[55]),
                                                                    border: Some(::ducktape_view_guest::wire::Border {
                                                                        color: Some({
                                                                            let mut color = palette.colors[4];
                                                                            color.0[3] = 0.210000;
                                                                            color
                                                                        }),
                                                                        width: None,
                                                                        radius: None,
                                                                    }),
                                                                    value: None,
                                                                    placeholder: None,
                                                                    selection: None,
                                                                }),
                                                                focused: None,
                                                                disabled: Some(::ducktape_view_guest::wire::InputFace {
                                                                    icon: None,
                                                                    background: Some({
                                                                        let mut color = palette.colors[6];
                                                                        color.0[3] = 0.540000;
                                                                        color
                                                                    }),
                                                                    border: None,
                                                                    value: Some(palette.colors[5]),
                                                                    placeholder: None,
                                                                    selection: None,
                                                                }),
                                                            }),
                                                        }
                                                    });
                                                children
                                                    .push(::ducktape_view_guest::wire::Node::Button {
                                                        checked: None,
                                                        expanded: None,
                                                        description: None,
                                                        key: format!("{}/@button:693", use_scope),
                                                        content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                                            String::from("Mint ticket"),
                                                        ),
                                                        label: None,
                                                        on_press: if (((self.account_busy || (!self.unlocked))
                                                            || ((self.account_key_draft).trim().to_owned()).is_empty()))
                                                        {
                                                            None
                                                        } else {
                                                            Some(
                                                                    ::ducktape_view_guest::slots::message(
                                                                        Message::AccountKeyAddSubmit,
                                                                    ),
                                                                )
                                                        },
                                                        width: None,
                                                        height: None,
                                                        padding: Some(
                                                            ::ducktape_view_guest::wire::Edges::all((5.0) as f32),
                                                        ),
                                                        style: ::ducktape_view_guest::wire::ButtonStyle {
                                                            preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                            recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                                base: ::ducktape_view_guest::wire::Face {
                                                                    background: Some(palette.colors[12]),
                                                                    text: Some(palette.colors[13]),
                                                                    border: Some(::ducktape_view_guest::wire::Border {
                                                                        color: Some(palette.colors[40]),
                                                                        width: Some(1.0),
                                                                        radius: Some([9.0; 4]),
                                                                    }),
                                                                },
                                                                hover_background: Some(palette.colors[14]),
                                                                pressed_background: Some(palette.colors[6]),
                                                                disabled_background: None,
                                                                disabled_text: None,
                                                                disabled_opacity: Some(0.5f32),
                                                                focus_ring: Some(palette.colors[42]),
                                                                text_size: Some(12.5f32),
                                                                line_height: None,
                                                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                        "Geist".into(),
                                                                    ),
                                                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                }),
                                                            }),
                                                            active: ::ducktape_view_guest::wire::Face::default(),
                                                            hovered: None,
                                                            pressed: None,
                                                            disabled: None,
                                                        },
                                                    });
                                                ::ducktape_view_guest::wire::Node::Linear {
                                                    max_width: None,
                                                    clip: false,
                                                    key: format!("{}/@layout:662", use_scope),
                                                    wrap: None,
                                                    axis: ::ducktape_view_guest::wire::Axis::Row,
                                                    spacing: Some((7.0) as f32),
                                                    padding: None,
                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                    height: None,
                                                    align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                    background: None,
                                                    border: None,
                                                    children: children,
                                                }
                                            });
                                        children
                                            .push({
                                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                children
                                                    .push(::ducktape_view_guest::wire::Node::Text {
                                                        options: ::ducktape_view_guest::wire::TextOptions {
                                                            height: None,
                                                            align_y: None,
                                                            line_height: None,
                                                            shaping: None,
                                                            wrapping: None,
                                                            tracking: 0.0f32,
                                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                    "Geist".into(),
                                                                ),
                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                            }),
                                                        },
                                                        key: format!("{}/@text:707", use_scope),
                                                        size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                        color: Some(palette.colors[71]),
                                                        font: ::ducktape_view_guest::wire::Font {
                                                            monospace: false,
                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                        },
                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                        align_x: None,
                                                        content: ("…or a passkey:".to_owned()).to_string(),
                                                    });
                                                children
                                                    .push({
                                                        let node_scope = format!(
                                                            "{}/account-passkey-qr", node_scope
                                                        );
                                                        ::ducktape_view_guest::wire::Node::Button {
                                                            checked: None,
                                                            expanded: None,
                                                            description: None,
                                                            key: node_scope.clone(),
                                                            content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                                                String::from("On your phone"),
                                                            ),
                                                            label: None,
                                                            on_press: if ((self.account_busy || (!self.unlocked))) {
                                                                None
                                                            } else {
                                                                Some(
                                                                        ::ducktape_view_guest::slots::message(
                                                                            Message::AccountPasskeySubmit,
                                                                        ),
                                                                    )
                                                            },
                                                            width: None,
                                                            height: None,
                                                            padding: Some(
                                                                ::ducktape_view_guest::wire::Edges::all((5.0) as f32),
                                                            ),
                                                            style: ::ducktape_view_guest::wire::ButtonStyle {
                                                                preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                                recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                                    base: ::ducktape_view_guest::wire::Face {
                                                                        background: Some(palette.colors[12]),
                                                                        text: Some(palette.colors[13]),
                                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                                            color: Some(palette.colors[40]),
                                                                            width: Some(1.0),
                                                                            radius: Some([9.0; 4]),
                                                                        }),
                                                                    },
                                                                    hover_background: Some(palette.colors[14]),
                                                                    pressed_background: Some(palette.colors[6]),
                                                                    disabled_background: None,
                                                                    disabled_text: None,
                                                                    disabled_opacity: Some(0.5f32),
                                                                    focus_ring: Some(palette.colors[42]),
                                                                    text_size: Some(12.5f32),
                                                                    line_height: None,
                                                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                            "Geist".into(),
                                                                        ),
                                                                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                    }),
                                                                }),
                                                                active: ::ducktape_view_guest::wire::Face::default(),
                                                                hovered: None,
                                                                pressed: None,
                                                                disabled: None,
                                                            },
                                                        }
                                                    });
                                                children
                                                    .push(::ducktape_view_guest::wire::Node::Button {
                                                        checked: None,
                                                        expanded: None,
                                                        description: None,
                                                        key: format!("{}/@button:717", use_scope),
                                                        content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                                            String::from("In this browser"),
                                                        ),
                                                        label: None,
                                                        on_press: if ((self.account_busy || (!self.unlocked))) {
                                                            None
                                                        } else {
                                                            Some(
                                                                    ::ducktape_view_guest::slots::message(
                                                                        Message::AccountPasskeyDesktop,
                                                                    ),
                                                                )
                                                        },
                                                        width: None,
                                                        height: None,
                                                        padding: Some(
                                                            ::ducktape_view_guest::wire::Edges::all((5.0) as f32),
                                                        ),
                                                        style: ::ducktape_view_guest::wire::ButtonStyle {
                                                            preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                            recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                                base: ::ducktape_view_guest::wire::Face {
                                                                    background: Some(palette.colors[12]),
                                                                    text: Some(palette.colors[13]),
                                                                    border: Some(::ducktape_view_guest::wire::Border {
                                                                        color: Some(palette.colors[40]),
                                                                        width: Some(1.0),
                                                                        radius: Some([9.0; 4]),
                                                                    }),
                                                                },
                                                                hover_background: Some(palette.colors[14]),
                                                                pressed_background: Some(palette.colors[6]),
                                                                disabled_background: None,
                                                                disabled_text: None,
                                                                disabled_opacity: Some(0.5f32),
                                                                focus_ring: Some(palette.colors[42]),
                                                                text_size: Some(12.5f32),
                                                                line_height: None,
                                                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                        "Geist".into(),
                                                                    ),
                                                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                }),
                                                            }),
                                                            active: ::ducktape_view_guest::wire::Face::default(),
                                                            hovered: None,
                                                            pressed: None,
                                                            disabled: None,
                                                        },
                                                    });
                                                children
                                                    .push(::ducktape_view_guest::wire::Node::Button {
                                                        checked: None,
                                                        expanded: None,
                                                        description: None,
                                                        key: format!("{}/@button:722", use_scope),
                                                        content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                                            String::from("Link a wallet"),
                                                        ),
                                                        label: None,
                                                        on_press: if ((self.account_busy || (!self.unlocked))) {
                                                            None
                                                        } else {
                                                            Some(
                                                                    ::ducktape_view_guest::slots::message(
                                                                        Message::AccountWalletSubmit,
                                                                    ),
                                                                )
                                                        },
                                                        width: None,
                                                        height: None,
                                                        padding: Some(
                                                            ::ducktape_view_guest::wire::Edges::all((5.0) as f32),
                                                        ),
                                                        style: ::ducktape_view_guest::wire::ButtonStyle {
                                                            preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                            recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                                base: ::ducktape_view_guest::wire::Face {
                                                                    background: Some(palette.colors[12]),
                                                                    text: Some(palette.colors[13]),
                                                                    border: Some(::ducktape_view_guest::wire::Border {
                                                                        color: Some(palette.colors[40]),
                                                                        width: Some(1.0),
                                                                        radius: Some([9.0; 4]),
                                                                    }),
                                                                },
                                                                hover_background: Some(palette.colors[14]),
                                                                pressed_background: Some(palette.colors[6]),
                                                                disabled_background: None,
                                                                disabled_text: None,
                                                                disabled_opacity: Some(0.5f32),
                                                                focus_ring: Some(palette.colors[42]),
                                                                text_size: Some(12.5f32),
                                                                line_height: None,
                                                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                        "Geist".into(),
                                                                    ),
                                                                    weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                }),
                                                            }),
                                                            active: ::ducktape_view_guest::wire::Face::default(),
                                                            hovered: None,
                                                            pressed: None,
                                                            disabled: None,
                                                        },
                                                    });
                                                ::ducktape_view_guest::wire::Node::Linear {
                                                    max_width: None,
                                                    clip: false,
                                                    key: format!("{}/@layout:702", use_scope),
                                                    wrap: None,
                                                    axis: ::ducktape_view_guest::wire::Axis::Row,
                                                    spacing: Some((7.0) as f32),
                                                    padding: None,
                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                    height: None,
                                                    align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                    background: None,
                                                    border: None,
                                                    children: children,
                                                }
                                            });
                                        children
                                            .push({
                                                let node_scope = format!("{}/account-ceremony", node_scope);
                                                self.render_settings_loading(palette, node_scope.clone())
                                            });
                                        if (!(self.account_ticket).is_empty()) {
                                            children
                                                .push({
                                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                    children
                                                        .push(::ducktape_view_guest::wire::Node::Text {
                                                            options: ::ducktape_view_guest::wire::TextOptions {
                                                                height: None,
                                                                align_y: None,
                                                                line_height: None,
                                                                shaping: None,
                                                                wrapping: None,
                                                                tracking: 0.0f32,
                                                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                        "Geist".into(),
                                                                    ),
                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                }),
                                                            },
                                                            key: format!("{}/@text:741", use_scope),
                                                            size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                            color: Some(palette.colors[71]),
                                                            font: ::ducktape_view_guest::wire::Font {
                                                                monospace: false,
                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                            },
                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                            align_x: None,
                                                            content: ("Ticket minted — paste it on the other device."
                                                                .to_owned())
                                                                .to_string(),
                                                        });
                                                    children
                                                        .push(::ducktape_view_guest::wire::Node::Button {
                                                            checked: None,
                                                            expanded: None,
                                                            description: None,
                                                            key: format!("{}/@button:746", use_scope),
                                                            content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                                                String::from("Copy ticket"),
                                                            ),
                                                            label: None,
                                                            on_press: Some(
                                                                ::ducktape_view_guest::slots::message(
                                                                    Message::CopyToClipboard(
                                                                        self.account_ticket.to_owned(),
                                                                        "Ticket copied".to_owned(),
                                                                    ),
                                                                ),
                                                            ),
                                                            width: None,
                                                            height: None,
                                                            padding: Some(
                                                                ::ducktape_view_guest::wire::Edges::all((5.0) as f32),
                                                            ),
                                                            style: ::ducktape_view_guest::wire::ButtonStyle {
                                                                preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                                recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                                    base: ::ducktape_view_guest::wire::Face {
                                                                        background: Some(palette.colors[12]),
                                                                        text: Some(palette.colors[13]),
                                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                                            color: Some(palette.colors[40]),
                                                                            width: Some(1.0),
                                                                            radius: Some([9.0; 4]),
                                                                        }),
                                                                    },
                                                                    hover_background: Some(palette.colors[14]),
                                                                    pressed_background: Some(palette.colors[6]),
                                                                    disabled_background: None,
                                                                    disabled_text: None,
                                                                    disabled_opacity: Some(0.5f32),
                                                                    focus_ring: Some(palette.colors[42]),
                                                                    text_size: Some(12.5f32),
                                                                    line_height: None,
                                                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                            "Geist".into(),
                                                                        ),
                                                                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                    }),
                                                                }),
                                                                active: ::ducktape_view_guest::wire::Face::default(),
                                                                hovered: None,
                                                                pressed: None,
                                                                disabled: None,
                                                            },
                                                        });
                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                        max_width: None,
                                                        clip: false,
                                                        key: format!("{}/@layout:736", use_scope),
                                                        wrap: None,
                                                        axis: ::ducktape_view_guest::wire::Axis::Row,
                                                        spacing: Some((7.0) as f32),
                                                        padding: None,
                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                        height: None,
                                                        align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                        background: None,
                                                        border: None,
                                                        children: children,
                                                    }
                                                });
                                        }
                                        ::ducktape_view_guest::wire::Node::Linear {
                                            max_width: None,
                                            clip: false,
                                            key: format!("{}/@layout:656", use_scope),
                                            wrap: None,
                                            axis: ::ducktape_view_guest::wire::Axis::Column,
                                            spacing: Some((7.0) as f32),
                                            padding: None,
                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                            height: None,
                                            align: None,
                                            background: None,
                                            border: None,
                                            children: children,
                                        }
                                    }),
                                });
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:608", use_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Column,
                                spacing: None,
                                padding: None,
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                align: None,
                                background: None,
                                border: None,
                                children: children,
                            }
                        });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:21", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Column,
                        spacing: None,
                        padding: None,
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        align: None,
                        background: None,
                        border: None,
                        children: children,
                    }
                }),
            }
        }
    }
    pub(crate) fn render_group_label_21(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Text {
                options: ::ducktape_view_guest::wire::TextOptions {
                    height: None,
                    align_y: None,
                    line_height: None,
                    shaping: None,
                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                    tracking: 0.0f32,
                    font: Some(::ducktape_view_guest::wire::NamedFont {
                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                            "Geist Mono".into(),
                        ),
                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                    }),
                },
                key: node_scope.clone(),
                size: Some(((9.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                color: Some(palette.colors[73]),
                font: ::ducktape_view_guest::wire::Font {
                    monospace: false,
                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                },
                width: None,
                align_x: None,
                content: ("IDENTITY KEY".to_owned()).to_string(),
            }
        }
    }
    pub(crate) fn render_key_value_row_22(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push(::ducktape_view_guest::wire::Node::Container {
                        shadow: ::ducktape_view_guest::wire::Shadow {
                            color: None,
                            x: None,
                            y: None,
                            blur: None,
                        },
                        max_width: None,
                        max_height: None,
                        clip: false,
                        key: format!("{}/@container:26", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        padding: Some(::ducktape_view_guest::wire::Edges {
                            top: (13.0) as f32,
                            right: (15.0) as f32,
                            bottom: (13.0) as f32,
                            left: (15.0) as f32,
                        }),
                        align_x: None,
                        align_y: None,
                        background: (None)
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: None,
                        snap: None,
                        content: Box::new({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:36", use_scope),
                                    size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[15]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("Key state".to_owned()).to_string(),
                                });
                            children
                                .push(::ducktape_view_guest::wire::Node::Space {
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    height: None,
                                });
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist Mono".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Medium,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:42", use_scope),
                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[13]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: (self.settings_key_state.to_owned()).to_string(),
                                });
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:31", use_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Row,
                                spacing: Some((10.0) as f32),
                                padding: None,
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                background: None,
                                border: None,
                                children: children,
                            }
                        }),
                    });
                {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:49", use_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                            ),
                            padding: None,
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[55]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                            }),
                        });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_key_value_row_23(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            {
                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                children
                    .push(::ducktape_view_guest::wire::Node::Container {
                        shadow: ::ducktape_view_guest::wire::Shadow {
                            color: None,
                            x: None,
                            y: None,
                            blur: None,
                        },
                        max_width: None,
                        max_height: None,
                        clip: false,
                        key: format!("{}/@container:26", use_scope),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        padding: Some(::ducktape_view_guest::wire::Edges {
                            top: (13.0) as f32,
                            right: (15.0) as f32,
                            bottom: (13.0) as f32,
                            left: (15.0) as f32,
                        }),
                        align_x: None,
                        align_y: None,
                        background: (None)
                            .map(::ducktape_view_guest::wire::Background::Color),
                        border: None,
                        snap: None,
                        content: Box::new({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:36", use_scope),
                                    size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[15]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("Key path".to_owned()).to_string(),
                                });
                            children
                                .push(::ducktape_view_guest::wire::Node::Space {
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    height: None,
                                });
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist Mono".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Medium,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:42", use_scope),
                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[13]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: (self.settings_key_path.to_owned()).to_string(),
                                });
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:31", use_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Row,
                                spacing: Some((10.0) as f32),
                                padding: None,
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                background: None,
                                border: None,
                                children: children,
                            }
                        }),
                    });
                {
                    children
                        .push(::ducktape_view_guest::wire::Node::Container {
                            shadow: ::ducktape_view_guest::wire::Shadow {
                                color: None,
                                x: None,
                                y: None,
                                blur: None,
                            },
                            max_width: None,
                            max_height: None,
                            clip: false,
                            key: format!("{}/@container:49", use_scope),
                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                            height: Some(
                                ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                            ),
                            padding: None,
                            align_x: None,
                            align_y: None,
                            background: (Some(palette.colors[55]))
                                .map(::ducktape_view_guest::wire::Background::Color),
                            border: None,
                            snap: None,
                            content: Box::new(::ducktape_view_guest::wire::Node::Space {
                                width: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                                height: Some(
                                    ::ducktape_view_guest::wire::Length::Fixed((1.0) as f32),
                                ),
                            }),
                        });
                }
                ::ducktape_view_guest::wire::Node::Linear {
                    max_width: None,
                    clip: false,
                    key: node_scope.clone(),
                    wrap: None,
                    axis: ::ducktape_view_guest::wire::Axis::Column,
                    spacing: None,
                    padding: None,
                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                    height: None,
                    align: None,
                    background: None,
                    border: None,
                    children: children,
                }
            }
        }
    }
    pub(crate) fn render_group_card_24(
        &self,
        palette: Palette,
        use_scope: String,
        ctx_0: String,
    ) -> ::ducktape_view_guest::wire::Node {
        {
            let node_scope = format!("{}/root", use_scope);
            ::ducktape_view_guest::wire::Node::Container {
                shadow: ::ducktape_view_guest::wire::Shadow {
                    color: None,
                    x: None,
                    y: None,
                    blur: None,
                },
                max_width: None,
                max_height: None,
                clip: true,
                key: node_scope.clone(),
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                height: None,
                padding: None,
                align_x: None,
                align_y: None,
                background: (Some(palette.colors[3]))
                    .map(::ducktape_view_guest::wire::Background::Color),
                border: Some(::ducktape_view_guest::wire::Border {
                    color: Some(palette.colors[63]),
                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                    radius: Some([
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                        ((11.0) as f32).max(0.0).min(f32::MAX),
                    ]),
                }),
                snap: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    children
                        .push({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            children
                                .push(
                                    self
                                        .render_key_value_row_22(
                                            palette,
                                            format!("{}/KeyValueRow@1297", use_scope),
                                        ),
                                );
                            children
                                .push(
                                    self
                                        .render_key_value_row_23(
                                            palette,
                                            format!("{}/KeyValueRow@1302", use_scope),
                                        ),
                                );
                            children
                                .push(::ducktape_view_guest::wire::Node::Container {
                                    shadow: ::ducktape_view_guest::wire::Shadow {
                                        color: None,
                                        x: None,
                                        y: None,
                                        blur: None,
                                    },
                                    max_width: None,
                                    max_height: None,
                                    clip: false,
                                    key: format!("{}/@container:769", use_scope),
                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                    height: None,
                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                        top: (13.0) as f32,
                                        right: (15.0) as f32,
                                        bottom: (13.0) as f32,
                                        left: (15.0) as f32,
                                    }),
                                    align_x: None,
                                    align_y: None,
                                    background: (None)
                                        .map(::ducktape_view_guest::wire::Background::Color),
                                    border: None,
                                    snap: None,
                                    content: Box::new({
                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                        if (!self.unlocked) {
                                            children
                                                .push({
                                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                    children
                                                        .push({
                                                            let node_scope = format!("{}/key-password", node_scope);
                                                            ::ducktape_view_guest::wire::Node::Input {
                                                                options: ::ducktape_view_guest::wire::InputOptions {
                                                                    label: ("Key password".to_owned()).to_string(),
                                                                    description: None,
                                                                    disabled: self.busy,
                                                                    padding: Some(
                                                                        ::ducktape_view_guest::wire::Edges::all((6.2) as f32),
                                                                    ),
                                                                    text_size: Some((13.0) as f32),
                                                                    line_height: Some((1.2) as f32),
                                                                    align: None,
                                                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                            "Geist".into(),
                                                                        ),
                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                    }),
                                                                },
                                                                key: node_scope.clone(),
                                                                placeholder: String::from("unlock signing…".to_owned()),
                                                                value: (self
                                                                    .settings_screen_states
                                                                    .get(&ctx_0)
                                                                    .map_or_else(
                                                                        || self.settings_screen_initial.key_pw.clone(),
                                                                        |state| state.key_pw.clone(),
                                                                    ))
                                                                    .to_string(),
                                                                on_input: ::ducktape_view_guest::slots::handler::<
                                                                    String,
                                                                    Message,
                                                                >(
                                                                    Box::new({
                                                                        let route = {
                                                                            let scope = (ctx_0).clone();
                                                                            move |value| Message::EditSettingsPassword(
                                                                                scope.clone(),
                                                                                value,
                                                                            )
                                                                        };
                                                                        move |sent: String| Some(route(sent))
                                                                    }),
                                                                ),
                                                                on_submit: Some(
                                                                    ::ducktape_view_guest::slots::message(
                                                                        Message::SettingsUnlockSubmit(
                                                                            self
                                                                                .settings_screen_states
                                                                                .get(&ctx_0)
                                                                                .map_or_else(
                                                                                    || self.settings_screen_initial.key_pw.clone(),
                                                                                    |state| state.key_pw.clone(),
                                                                                ),
                                                                        ),
                                                                    ),
                                                                ),
                                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                secure: (true),
                                                                style: Box::new(::ducktape_view_guest::wire::InputStyle {
                                                                    utility: ::ducktape_view_guest::wire::InputFace {
                                                                        background: Some(palette.colors[3]),
                                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                                            color: Some(palette.colors[39]),
                                                                            width: Some(1f32),
                                                                            radius: Some([10f32; 4]),
                                                                        }),
                                                                        ..Default::default()
                                                                    },
                                                                    focus_border: Some(palette.colors[42]),
                                                                    focused_hovered: None,
                                                                    active: ::ducktape_view_guest::wire::InputFace {
                                                                        icon: None,
                                                                        background: Some(palette.colors[6]),
                                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                                            color: Some({
                                                                                let mut color = palette.colors[4];
                                                                                color.0[3] = 0.160000;
                                                                                color
                                                                            }),
                                                                            width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                            radius: Some([
                                                                                ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                                ((9.0) as f32).max(0.0).min(f32::MAX),
                                                                            ]),
                                                                        }),
                                                                        value: Some(palette.colors[4]),
                                                                        placeholder: Some(palette.colors[5]),
                                                                        selection: Some({
                                                                            let mut color = palette.colors[4];
                                                                            color.0[3] = 0.180000;
                                                                            color
                                                                        }),
                                                                    },
                                                                    hovered: Some(::ducktape_view_guest::wire::InputFace {
                                                                        icon: None,
                                                                        background: Some(palette.colors[55]),
                                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                                            color: Some({
                                                                                let mut color = palette.colors[4];
                                                                                color.0[3] = 0.210000;
                                                                                color
                                                                            }),
                                                                            width: None,
                                                                            radius: None,
                                                                        }),
                                                                        value: None,
                                                                        placeholder: None,
                                                                        selection: None,
                                                                    }),
                                                                    focused: None,
                                                                    disabled: Some(::ducktape_view_guest::wire::InputFace {
                                                                        icon: None,
                                                                        background: Some({
                                                                            let mut color = palette.colors[6];
                                                                            color.0[3] = 0.540000;
                                                                            color
                                                                        }),
                                                                        border: None,
                                                                        value: Some(palette.colors[5]),
                                                                        placeholder: None,
                                                                        selection: None,
                                                                    }),
                                                                }),
                                                            }
                                                        });
                                                    children
                                                        .push(::ducktape_view_guest::wire::Node::Button {
                                                            checked: None,
                                                            expanded: None,
                                                            description: None,
                                                            key: format!("{}/@button:796", use_scope),
                                                            content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                                                String::from("Unlock"),
                                                            ),
                                                            label: None,
                                                            on_press: if ((self.busy
                                                                || ((*self
                                                                    .settings_screen_states
                                                                    .get(&ctx_0)
                                                                    .map_or(
                                                                        &self.settings_screen_initial.key_pw,
                                                                        |local| &local.key_pw,
                                                                    )))
                                                                    .is_empty()))
                                                            {
                                                                None
                                                            } else {
                                                                Some(
                                                                        ::ducktape_view_guest::slots::message(
                                                                            Message::SettingsUnlockSubmit(
                                                                                self
                                                                                    .settings_screen_states
                                                                                    .get(&ctx_0)
                                                                                    .map_or_else(
                                                                                        || self.settings_screen_initial.key_pw.clone(),
                                                                                        |state| state.key_pw.clone(),
                                                                                    ),
                                                                            ),
                                                                        ),
                                                                    )
                                                            },
                                                            width: None,
                                                            height: None,
                                                            padding: Some(
                                                                ::ducktape_view_guest::wire::Edges::all((6.0) as f32),
                                                            ),
                                                            style: ::ducktape_view_guest::wire::ButtonStyle {
                                                                preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                                recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                                    base: ::ducktape_view_guest::wire::Face {
                                                                        background: Some(palette.colors[12]),
                                                                        text: Some(palette.colors[13]),
                                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                                            color: Some(palette.colors[40]),
                                                                            width: Some(1.0),
                                                                            radius: Some([9.0; 4]),
                                                                        }),
                                                                    },
                                                                    hover_background: Some(palette.colors[14]),
                                                                    pressed_background: Some(palette.colors[6]),
                                                                    disabled_background: None,
                                                                    disabled_text: None,
                                                                    disabled_opacity: Some(0.5f32),
                                                                    focus_ring: Some(palette.colors[42]),
                                                                    text_size: Some(12.5f32),
                                                                    line_height: None,
                                                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                            "Geist".into(),
                                                                        ),
                                                                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                    }),
                                                                }),
                                                                active: ::ducktape_view_guest::wire::Face::default(),
                                                                hovered: None,
                                                                pressed: None,
                                                                disabled: None,
                                                            },
                                                        });
                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                        max_width: None,
                                                        clip: false,
                                                        key: format!("{}/@layout:776", use_scope),
                                                        wrap: None,
                                                        axis: ::ducktape_view_guest::wire::Axis::Row,
                                                        spacing: Some((9.0) as f32),
                                                        padding: None,
                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                        height: None,
                                                        align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                        background: None,
                                                        border: None,
                                                        children: children,
                                                    }
                                                });
                                        }
                                        if self.unlocked {
                                            children
                                                .push({
                                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                    children
                                                        .push(::ducktape_view_guest::wire::Node::Text {
                                                            options: ::ducktape_view_guest::wire::TextOptions {
                                                                height: None,
                                                                align_y: None,
                                                                line_height: None,
                                                                shaping: None,
                                                                wrapping: None,
                                                                tracking: 0.0f32,
                                                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                        "Geist".into(),
                                                                    ),
                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                }),
                                                            },
                                                            key: format!("{}/@text:807", use_scope),
                                                            size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                            color: Some(palette.colors[71]),
                                                            font: ::ducktape_view_guest::wire::Font {
                                                                monospace: false,
                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                            },
                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                            align_x: None,
                                                            content: ("Signing unlocked for this session.".to_owned())
                                                                .to_string(),
                                                        });
                                                    children
                                                        .push(::ducktape_view_guest::wire::Node::Button {
                                                            checked: None,
                                                            expanded: None,
                                                            description: None,
                                                            key: format!("{}/@button:812", use_scope),
                                                            content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                                                String::from("Lock"),
                                                            ),
                                                            label: None,
                                                            on_press: Some(
                                                                ::ducktape_view_guest::slots::message(Message::LockSession),
                                                            ),
                                                            width: None,
                                                            height: None,
                                                            padding: Some(
                                                                ::ducktape_view_guest::wire::Edges::all((6.0) as f32),
                                                            ),
                                                            style: ::ducktape_view_guest::wire::ButtonStyle {
                                                                preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                                recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                                    base: ::ducktape_view_guest::wire::Face {
                                                                        background: Some(palette.colors[12]),
                                                                        text: Some(palette.colors[13]),
                                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                                            color: Some(palette.colors[40]),
                                                                            width: Some(1.0),
                                                                            radius: Some([9.0; 4]),
                                                                        }),
                                                                    },
                                                                    hover_background: Some(palette.colors[14]),
                                                                    pressed_background: Some(palette.colors[6]),
                                                                    disabled_background: None,
                                                                    disabled_text: None,
                                                                    disabled_opacity: Some(0.5f32),
                                                                    focus_ring: Some(palette.colors[42]),
                                                                    text_size: Some(12.5f32),
                                                                    line_height: None,
                                                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                            "Geist".into(),
                                                                        ),
                                                                        weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                    }),
                                                                }),
                                                                active: ::ducktape_view_guest::wire::Face::default(),
                                                                hovered: None,
                                                                pressed: None,
                                                                disabled: None,
                                                            },
                                                        });
                                                    ::ducktape_view_guest::wire::Node::Linear {
                                                        max_width: None,
                                                        clip: false,
                                                        key: format!("{}/@layout:802", use_scope),
                                                        wrap: None,
                                                        axis: ::ducktape_view_guest::wire::Axis::Row,
                                                        spacing: Some((9.0) as f32),
                                                        padding: None,
                                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                        height: None,
                                                        align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                        background: None,
                                                        border: None,
                                                        children: children,
                                                    }
                                                });
                                        }
                                        ::ducktape_view_guest::wire::Node::Linear {
                                            max_width: None,
                                            clip: false,
                                            key: format!("{}/@layout:774", use_scope),
                                            wrap: None,
                                            axis: ::ducktape_view_guest::wire::Axis::Column,
                                            spacing: None,
                                            padding: None,
                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                            height: None,
                                            align: None,
                                            background: None,
                                            border: None,
                                            children: children,
                                        }
                                    }),
                                });
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:755", use_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Column,
                                spacing: None,
                                padding: None,
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                align: None,
                                background: None,
                                border: None,
                                children: children,
                            }
                        });
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: None,
                        clip: false,
                        key: format!("{}/@layout:21", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Column,
                        spacing: None,
                        padding: None,
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        align: None,
                        background: None,
                        border: None,
                        children: children,
                    }
                }),
            }
        }
    }
}
impl SettingsView {
    pub(crate) fn render_settings_screen(
        &self,
        palette: Palette,
        use_scope: String,
    ) -> ::ducktape_view_guest::wire::Node {
        let component_owner = ::ducktape_view_guest::slots::component(
            "SettingsScreen",
            &use_scope,
            false,
        );
        {
            let node_scope = format!("{}/settings-body", use_scope);
            ::ducktape_view_guest::wire::Node::Scroll {
                on_scroll: None,
                virtual_rows: false,
                key: node_scope.clone(),
                direction: ::ducktape_view_guest::wire::ScrollDirection::Vertical,
                width: Some(::ducktape_view_guest::wire::Length::Fill),
                height: Some(::ducktape_view_guest::wire::Length::Fill),
                bar_hidden: false,
                bar_width: None,
                bar_margin: None,
                scroller_width: None,
                bar_spacing: None,
                anchor_x: ::ducktape_view_guest::wire::ScrollAnchor::Start,
                anchor_y: ::ducktape_view_guest::wire::ScrollAnchor::Start,
                auto_scroll: (false),
                background: None,
                border: None,
                content: Box::new({
                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                    children
                        .push({
                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                            children
                                .push(::ducktape_view_guest::wire::Node::Text {
                                    options: ::ducktape_view_guest::wire::TextOptions {
                                        height: None,
                                        align_y: None,
                                        line_height: None,
                                        shaping: None,
                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                        tracking: 0.0f32,
                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                "Geist".into(),
                                            ),
                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                        }),
                                    },
                                    key: format!("{}/@text:62", use_scope),
                                    size: Some(((16.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                    color: Some(palette.colors[7]),
                                    font: ::ducktape_view_guest::wire::Font {
                                        monospace: false,
                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                    },
                                    width: None,
                                    align_x: None,
                                    content: ("Settings".to_owned()).to_string(),
                                });
                            children
                                .push({
                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                    children
                                        .push({
                                            let node_scope = format!(
                                                "{}/settings-general-tab", node_scope
                                            );
                                            ::ducktape_view_guest::wire::Node::Button {
                                                checked: Some(
                                                    ((*self
                                                        .settings_screen_states
                                                        .get(&use_scope)
                                                        .map_or(
                                                            &self.settings_screen_initial.settings_pane,
                                                            |local| &local.settings_pane,
                                                        )) == SettingsPane::General),
                                                ),
                                                expanded: None,
                                                description: None,
                                                key: node_scope.clone(),
                                                content: ::ducktape_view_guest::wire::ButtonContent::Child(
                                                    Box::new(::ducktape_view_guest::wire::Node::Container {
                                                        shadow: ::ducktape_view_guest::wire::Shadow {
                                                            color: None,
                                                            x: None,
                                                            y: None,
                                                            blur: None,
                                                        },
                                                        max_width: None,
                                                        max_height: None,
                                                        clip: false,
                                                        key: format!("{}/@container:80", use_scope),
                                                        width: None,
                                                        height: None,
                                                        padding: Some(::ducktape_view_guest::wire::Edges {
                                                            top: (0.0) as f32,
                                                            right: (15.0) as f32,
                                                            bottom: (0.0) as f32,
                                                            left: (15.0) as f32,
                                                        }),
                                                        align_x: None,
                                                        align_y: None,
                                                        background: (None)
                                                            .map(::ducktape_view_guest::wire::Background::Color),
                                                        border: None,
                                                        snap: None,
                                                        content: Box::new(
                                                            self
                                                                .render_tab_label_0(
                                                                    palette,
                                                                    format!("{}/TabLabel@622", use_scope),
                                                                    use_scope.clone(),
                                                                ),
                                                        ),
                                                    }),
                                                ),
                                                label: Some(String::from("General".to_owned())),
                                                on_press: Some(
                                                    ::ducktape_view_guest::slots::message(
                                                        Message::PickSettingsPane(
                                                            (use_scope).clone(),
                                                            SettingsPane::General,
                                                        ),
                                                    ),
                                                ),
                                                width: Some(::ducktape_view_guest::wire::Length::Shrink),
                                                height: None,
                                                padding: Some(
                                                    ::ducktape_view_guest::wire::Edges::all((0.0) as f32),
                                                ),
                                                style: ::ducktape_view_guest::wire::ButtonStyle {
                                                    preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                    recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                        base: ::ducktape_view_guest::wire::Face {
                                                            background: Some(
                                                                ::ducktape_view_guest::wire::Rgba([
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.000000,
                                                                ]),
                                                            ),
                                                            text: Some(palette.colors[4]),
                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                color: None,
                                                                width: None,
                                                                radius: Some([8.0; 4]),
                                                            }),
                                                        },
                                                        hover_background: Some(palette.colors[14]),
                                                        pressed_background: Some(palette.colors[39]),
                                                        disabled_background: None,
                                                        disabled_text: None,
                                                        disabled_opacity: Some(0.5f32),
                                                        focus_ring: Some(palette.colors[42]),
                                                        text_size: Some(12.5f32),
                                                        line_height: None,
                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                "Geist".into(),
                                                            ),
                                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                        }),
                                                    }),
                                                    active: ::ducktape_view_guest::wire::Face {
                                                        background: Some(
                                                            ::ducktape_view_guest::wire::Rgba([
                                                                0.0 / 255.0,
                                                                0.0 / 255.0,
                                                                0.0 / 255.0,
                                                                0.000000,
                                                            ]),
                                                        ),
                                                        text: Some(palette.colors[5]),
                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                            color: Some(
                                                                ::ducktape_view_guest::wire::Rgba([
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.000000,
                                                                ]),
                                                            ),
                                                            width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                            radius: Some([
                                                                ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                ((8.0) as f32).max(0.0).min(f32::MAX),
                                                            ]),
                                                        }),
                                                    },
                                                    hovered: Some(::ducktape_view_guest::wire::Face {
                                                        background: Some(palette.colors[57]),
                                                        text: Some(palette.colors[4]),
                                                        border: None,
                                                    }),
                                                    pressed: Some(::ducktape_view_guest::wire::Face {
                                                        background: Some(palette.colors[55]),
                                                        text: Some(palette.colors[4]),
                                                        border: None,
                                                    }),
                                                    disabled: None,
                                                },
                                            }
                                        });
                                    children
                                        .push({
                                            let node_scope = format!(
                                                "{}/settings-network-tab", node_scope
                                            );
                                            ::ducktape_view_guest::wire::Node::Button {
                                                checked: Some(
                                                    ((*self
                                                        .settings_screen_states
                                                        .get(&use_scope)
                                                        .map_or(
                                                            &self.settings_screen_initial.settings_pane,
                                                            |local| &local.settings_pane,
                                                        )) == SettingsPane::Network),
                                                ),
                                                expanded: None,
                                                description: None,
                                                key: node_scope.clone(),
                                                content: ::ducktape_view_guest::wire::ButtonContent::Child(
                                                    Box::new(::ducktape_view_guest::wire::Node::Container {
                                                        shadow: ::ducktape_view_guest::wire::Shadow {
                                                            color: None,
                                                            x: None,
                                                            y: None,
                                                            blur: None,
                                                        },
                                                        max_width: None,
                                                        max_height: None,
                                                        clip: false,
                                                        key: format!("{}/@container:96", use_scope),
                                                        width: None,
                                                        height: None,
                                                        padding: Some(::ducktape_view_guest::wire::Edges {
                                                            top: (0.0) as f32,
                                                            right: (15.0) as f32,
                                                            bottom: (0.0) as f32,
                                                            left: (15.0) as f32,
                                                        }),
                                                        align_x: None,
                                                        align_y: None,
                                                        background: (None)
                                                            .map(::ducktape_view_guest::wire::Background::Color),
                                                        border: None,
                                                        snap: None,
                                                        content: Box::new(
                                                            self
                                                                .render_tab_label_1(
                                                                    palette,
                                                                    format!("{}/TabLabel@638", use_scope),
                                                                    use_scope.clone(),
                                                                ),
                                                        ),
                                                    }),
                                                ),
                                                label: Some(String::from("Network".to_owned())),
                                                on_press: Some(
                                                    ::ducktape_view_guest::slots::message(
                                                        Message::PickSettingsPane(
                                                            (use_scope).clone(),
                                                            SettingsPane::Network,
                                                        ),
                                                    ),
                                                ),
                                                width: Some(::ducktape_view_guest::wire::Length::Shrink),
                                                height: None,
                                                padding: Some(
                                                    ::ducktape_view_guest::wire::Edges::all((0.0) as f32),
                                                ),
                                                style: ::ducktape_view_guest::wire::ButtonStyle {
                                                    preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                    recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                        base: ::ducktape_view_guest::wire::Face {
                                                            background: Some(
                                                                ::ducktape_view_guest::wire::Rgba([
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.000000,
                                                                ]),
                                                            ),
                                                            text: Some(palette.colors[4]),
                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                color: None,
                                                                width: None,
                                                                radius: Some([8.0; 4]),
                                                            }),
                                                        },
                                                        hover_background: Some(palette.colors[14]),
                                                        pressed_background: Some(palette.colors[39]),
                                                        disabled_background: None,
                                                        disabled_text: None,
                                                        disabled_opacity: Some(0.5f32),
                                                        focus_ring: Some(palette.colors[42]),
                                                        text_size: Some(12.5f32),
                                                        line_height: None,
                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                "Geist".into(),
                                                            ),
                                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                        }),
                                                    }),
                                                    active: ::ducktape_view_guest::wire::Face {
                                                        background: Some(
                                                            ::ducktape_view_guest::wire::Rgba([
                                                                0.0 / 255.0,
                                                                0.0 / 255.0,
                                                                0.0 / 255.0,
                                                                0.000000,
                                                            ]),
                                                        ),
                                                        text: Some(palette.colors[5]),
                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                            color: Some(
                                                                ::ducktape_view_guest::wire::Rgba([
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.000000,
                                                                ]),
                                                            ),
                                                            width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                            radius: Some([
                                                                ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                ((8.0) as f32).max(0.0).min(f32::MAX),
                                                            ]),
                                                        }),
                                                    },
                                                    hovered: Some(::ducktape_view_guest::wire::Face {
                                                        background: Some(palette.colors[57]),
                                                        text: Some(palette.colors[4]),
                                                        border: None,
                                                    }),
                                                    pressed: Some(::ducktape_view_guest::wire::Face {
                                                        background: Some(palette.colors[55]),
                                                        text: Some(palette.colors[4]),
                                                        border: None,
                                                    }),
                                                    disabled: None,
                                                },
                                            }
                                        });
                                    children
                                        .push({
                                            let node_scope = format!(
                                                "{}/settings-account-tab", node_scope
                                            );
                                            ::ducktape_view_guest::wire::Node::Button {
                                                checked: Some(
                                                    ((*self
                                                        .settings_screen_states
                                                        .get(&use_scope)
                                                        .map_or(
                                                            &self.settings_screen_initial.settings_pane,
                                                            |local| &local.settings_pane,
                                                        )) == SettingsPane::Account),
                                                ),
                                                expanded: None,
                                                description: None,
                                                key: node_scope.clone(),
                                                content: ::ducktape_view_guest::wire::ButtonContent::Child(
                                                    Box::new(::ducktape_view_guest::wire::Node::Container {
                                                        shadow: ::ducktape_view_guest::wire::Shadow {
                                                            color: None,
                                                            x: None,
                                                            y: None,
                                                            blur: None,
                                                        },
                                                        max_width: None,
                                                        max_height: None,
                                                        clip: false,
                                                        key: format!("{}/@container:112", use_scope),
                                                        width: None,
                                                        height: None,
                                                        padding: Some(::ducktape_view_guest::wire::Edges {
                                                            top: (0.0) as f32,
                                                            right: (15.0) as f32,
                                                            bottom: (0.0) as f32,
                                                            left: (15.0) as f32,
                                                        }),
                                                        align_x: None,
                                                        align_y: None,
                                                        background: (None)
                                                            .map(::ducktape_view_guest::wire::Background::Color),
                                                        border: None,
                                                        snap: None,
                                                        content: Box::new(
                                                            self
                                                                .render_tab_label_2(
                                                                    palette,
                                                                    format!("{}/TabLabel@654", use_scope),
                                                                    use_scope.clone(),
                                                                ),
                                                        ),
                                                    }),
                                                ),
                                                label: Some(String::from("Account".to_owned())),
                                                on_press: Some(
                                                    ::ducktape_view_guest::slots::message(
                                                        Message::PickSettingsPane(
                                                            (use_scope).clone(),
                                                            SettingsPane::Account,
                                                        ),
                                                    ),
                                                ),
                                                width: Some(::ducktape_view_guest::wire::Length::Shrink),
                                                height: None,
                                                padding: Some(
                                                    ::ducktape_view_guest::wire::Edges::all((0.0) as f32),
                                                ),
                                                style: ::ducktape_view_guest::wire::ButtonStyle {
                                                    preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                    recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                        base: ::ducktape_view_guest::wire::Face {
                                                            background: Some(
                                                                ::ducktape_view_guest::wire::Rgba([
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.000000,
                                                                ]),
                                                            ),
                                                            text: Some(palette.colors[4]),
                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                color: None,
                                                                width: None,
                                                                radius: Some([8.0; 4]),
                                                            }),
                                                        },
                                                        hover_background: Some(palette.colors[14]),
                                                        pressed_background: Some(palette.colors[39]),
                                                        disabled_background: None,
                                                        disabled_text: None,
                                                        disabled_opacity: Some(0.5f32),
                                                        focus_ring: Some(palette.colors[42]),
                                                        text_size: Some(12.5f32),
                                                        line_height: None,
                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                "Geist".into(),
                                                            ),
                                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                        }),
                                                    }),
                                                    active: ::ducktape_view_guest::wire::Face {
                                                        background: Some(
                                                            ::ducktape_view_guest::wire::Rgba([
                                                                0.0 / 255.0,
                                                                0.0 / 255.0,
                                                                0.0 / 255.0,
                                                                0.000000,
                                                            ]),
                                                        ),
                                                        text: Some(palette.colors[5]),
                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                            color: Some(
                                                                ::ducktape_view_guest::wire::Rgba([
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.000000,
                                                                ]),
                                                            ),
                                                            width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                            radius: Some([
                                                                ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                ((8.0) as f32).max(0.0).min(f32::MAX),
                                                            ]),
                                                        }),
                                                    },
                                                    hovered: Some(::ducktape_view_guest::wire::Face {
                                                        background: Some(palette.colors[57]),
                                                        text: Some(palette.colors[4]),
                                                        border: None,
                                                    }),
                                                    pressed: Some(::ducktape_view_guest::wire::Face {
                                                        background: Some(palette.colors[55]),
                                                        text: Some(palette.colors[4]),
                                                        border: None,
                                                    }),
                                                    disabled: None,
                                                },
                                            }
                                        });
                                    children
                                        .push({
                                            let node_scope = format!(
                                                "{}/settings-security-tab", node_scope
                                            );
                                            ::ducktape_view_guest::wire::Node::Button {
                                                checked: Some(
                                                    ((*self
                                                        .settings_screen_states
                                                        .get(&use_scope)
                                                        .map_or(
                                                            &self.settings_screen_initial.settings_pane,
                                                            |local| &local.settings_pane,
                                                        )) == SettingsPane::Security),
                                                ),
                                                expanded: None,
                                                description: None,
                                                key: node_scope.clone(),
                                                content: ::ducktape_view_guest::wire::ButtonContent::Child(
                                                    Box::new(::ducktape_view_guest::wire::Node::Container {
                                                        shadow: ::ducktape_view_guest::wire::Shadow {
                                                            color: None,
                                                            x: None,
                                                            y: None,
                                                            blur: None,
                                                        },
                                                        max_width: None,
                                                        max_height: None,
                                                        clip: false,
                                                        key: format!("{}/@container:128", use_scope),
                                                        width: None,
                                                        height: None,
                                                        padding: Some(::ducktape_view_guest::wire::Edges {
                                                            top: (0.0) as f32,
                                                            right: (15.0) as f32,
                                                            bottom: (0.0) as f32,
                                                            left: (15.0) as f32,
                                                        }),
                                                        align_x: None,
                                                        align_y: None,
                                                        background: (None)
                                                            .map(::ducktape_view_guest::wire::Background::Color),
                                                        border: None,
                                                        snap: None,
                                                        content: Box::new(
                                                            self
                                                                .render_tab_label_3(
                                                                    palette,
                                                                    format!("{}/TabLabel@670", use_scope),
                                                                    use_scope.clone(),
                                                                ),
                                                        ),
                                                    }),
                                                ),
                                                label: Some(String::from("Security".to_owned())),
                                                on_press: Some(
                                                    ::ducktape_view_guest::slots::message(
                                                        Message::PickSettingsPane(
                                                            (use_scope).clone(),
                                                            SettingsPane::Security,
                                                        ),
                                                    ),
                                                ),
                                                width: Some(::ducktape_view_guest::wire::Length::Shrink),
                                                height: None,
                                                padding: Some(
                                                    ::ducktape_view_guest::wire::Edges::all((0.0) as f32),
                                                ),
                                                style: ::ducktape_view_guest::wire::ButtonStyle {
                                                    preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                    recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                        base: ::ducktape_view_guest::wire::Face {
                                                            background: Some(
                                                                ::ducktape_view_guest::wire::Rgba([
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.000000,
                                                                ]),
                                                            ),
                                                            text: Some(palette.colors[4]),
                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                color: None,
                                                                width: None,
                                                                radius: Some([8.0; 4]),
                                                            }),
                                                        },
                                                        hover_background: Some(palette.colors[14]),
                                                        pressed_background: Some(palette.colors[39]),
                                                        disabled_background: None,
                                                        disabled_text: None,
                                                        disabled_opacity: Some(0.5f32),
                                                        focus_ring: Some(palette.colors[42]),
                                                        text_size: Some(12.5f32),
                                                        line_height: None,
                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                "Geist".into(),
                                                            ),
                                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                        }),
                                                    }),
                                                    active: ::ducktape_view_guest::wire::Face {
                                                        background: Some(
                                                            ::ducktape_view_guest::wire::Rgba([
                                                                0.0 / 255.0,
                                                                0.0 / 255.0,
                                                                0.0 / 255.0,
                                                                0.000000,
                                                            ]),
                                                        ),
                                                        text: Some(palette.colors[5]),
                                                        border: Some(::ducktape_view_guest::wire::Border {
                                                            color: Some(
                                                                ::ducktape_view_guest::wire::Rgba([
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.0 / 255.0,
                                                                    0.000000,
                                                                ]),
                                                            ),
                                                            width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                            radius: Some([
                                                                ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                ((8.0) as f32).max(0.0).min(f32::MAX),
                                                                ((8.0) as f32).max(0.0).min(f32::MAX),
                                                            ]),
                                                        }),
                                                    },
                                                    hovered: Some(::ducktape_view_guest::wire::Face {
                                                        background: Some(palette.colors[57]),
                                                        text: Some(palette.colors[4]),
                                                        border: None,
                                                    }),
                                                    pressed: Some(::ducktape_view_guest::wire::Face {
                                                        background: Some(palette.colors[55]),
                                                        text: Some(palette.colors[4]),
                                                        border: None,
                                                    }),
                                                    disabled: None,
                                                },
                                            }
                                        });
                                    ::ducktape_view_guest::wire::Node::Linear {
                                        max_width: None,
                                        clip: false,
                                        key: format!("{}/@layout:72", use_scope),
                                        wrap: None,
                                        axis: ::ducktape_view_guest::wire::Axis::Row,
                                        spacing: Some((3.0) as f32),
                                        padding: None,
                                        width: None,
                                        height: None,
                                        align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                        background: None,
                                        border: None,
                                        children: children,
                                    }
                                });
                            ::ducktape_view_guest::wire::Node::Linear {
                                max_width: None,
                                clip: false,
                                key: format!("{}/@layout:61", use_scope),
                                wrap: None,
                                axis: ::ducktape_view_guest::wire::Axis::Column,
                                spacing: Some((13.0) as f32),
                                padding: None,
                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                height: None,
                                align: None,
                                background: None,
                                border: None,
                                children: children,
                            }
                        });
                    match &((*self
                        .settings_screen_states
                        .get(&use_scope)
                        .map_or(
                            &self.settings_screen_initial.settings_pane,
                            |local| &local.settings_pane,
                        )))
                    {
                        SettingsPane::General => {
                            children
                                .push({
                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                    children
                                        .push({
                                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                            children
                                                .push(
                                                    self
                                                        .render_group_label_4(
                                                            palette,
                                                            format!("{}/GroupLabel@684", use_scope),
                                                        ),
                                                );
                                            children
                                                .push(::ducktape_view_guest::wire::Node::Container {
                                                    shadow: ::ducktape_view_guest::wire::Shadow {
                                                        color: None,
                                                        x: None,
                                                        y: None,
                                                        blur: None,
                                                    },
                                                    max_width: None,
                                                    max_height: None,
                                                    clip: false,
                                                    key: format!("{}/@container:144", use_scope),
                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                    height: None,
                                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                                        top: (15.0) as f32,
                                                        right: (15.0) as f32,
                                                        bottom: (15.0) as f32,
                                                        left: (15.0) as f32,
                                                    }),
                                                    align_x: None,
                                                    align_y: None,
                                                    background: (Some(palette.colors[3]))
                                                        .map(::ducktape_view_guest::wire::Background::Color),
                                                    border: Some(::ducktape_view_guest::wire::Border {
                                                        color: Some(palette.colors[63]),
                                                        width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                        radius: Some([
                                                            ((11.0) as f32).max(0.0).min(f32::MAX),
                                                            ((11.0) as f32).max(0.0).min(f32::MAX),
                                                            ((11.0) as f32).max(0.0).min(f32::MAX),
                                                            ((11.0) as f32).max(0.0).min(f32::MAX),
                                                        ]),
                                                    }),
                                                    snap: None,
                                                    content: Box::new({
                                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                        children
                                                            .push({
                                                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                children
                                                                    .push(::ducktape_view_guest::wire::Node::Text {
                                                                        options: ::ducktape_view_guest::wire::TextOptions {
                                                                            height: None,
                                                                            align_y: None,
                                                                            line_height: None,
                                                                            shaping: None,
                                                                            wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                                            tracking: 0.0f32,
                                                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                    "Geist".into(),
                                                                                ),
                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                            }),
                                                                        },
                                                                        key: format!("{}/@text:158", use_scope),
                                                                        size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                        color: Some(palette.colors[4]),
                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                            monospace: false,
                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                        },
                                                                        width: None,
                                                                        align_x: None,
                                                                        content: ("Theme".to_owned()).to_string(),
                                                                    });
                                                                if (self.appearance == "system") {
                                                                    children
                                                                        .push(::ducktape_view_guest::wire::Node::Text {
                                                                            options: ::ducktape_view_guest::wire::TextOptions {
                                                                                height: None,
                                                                                align_y: None,
                                                                                line_height: None,
                                                                                shaping: None,
                                                                                wrapping: None,
                                                                                tracking: 0.0f32,
                                                                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                        "Geist".into(),
                                                                                    ),
                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                }),
                                                                            },
                                                                            key: format!("{}/@text:165", use_scope),
                                                                            size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                            color: Some(palette.colors[70]),
                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                monospace: false,
                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                            },
                                                                            width: None,
                                                                            align_x: None,
                                                                            content: ("Following the system appearance.".to_owned())
                                                                                .to_string(),
                                                                        });
                                                                }
                                                                if ((!(self.appearance == "system"))
                                                                    && (self.appearance == "light"))
                                                                {
                                                                    children
                                                                        .push(::ducktape_view_guest::wire::Node::Text {
                                                                            options: ::ducktape_view_guest::wire::TextOptions {
                                                                                height: None,
                                                                                align_y: None,
                                                                                line_height: None,
                                                                                shaping: None,
                                                                                wrapping: None,
                                                                                tracking: 0.0f32,
                                                                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                        "Geist".into(),
                                                                                    ),
                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                }),
                                                                            },
                                                                            key: format!("{}/@text:167", use_scope),
                                                                            size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                            color: Some(palette.colors[70]),
                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                monospace: false,
                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                            },
                                                                            width: None,
                                                                            align_x: None,
                                                                            content: ("Pinned for this device.".to_owned()).to_string(),
                                                                        });
                                                                }
                                                                if ((!((self.appearance == "system")
                                                                    || (self.appearance == "light")))
                                                                    && (self.appearance == "dark"))
                                                                {
                                                                    children
                                                                        .push(::ducktape_view_guest::wire::Node::Text {
                                                                            options: ::ducktape_view_guest::wire::TextOptions {
                                                                                height: None,
                                                                                align_y: None,
                                                                                line_height: None,
                                                                                shaping: None,
                                                                                wrapping: None,
                                                                                tracking: 0.0f32,
                                                                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                        "Geist".into(),
                                                                                    ),
                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                }),
                                                                            },
                                                                            key: format!("{}/@text:169", use_scope),
                                                                            size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                            color: Some(palette.colors[70]),
                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                monospace: false,
                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                            },
                                                                            width: None,
                                                                            align_x: None,
                                                                            content: ("Pinned for this device.".to_owned()).to_string(),
                                                                        });
                                                                }
                                                                ::ducktape_view_guest::wire::Node::Linear {
                                                                    max_width: None,
                                                                    clip: false,
                                                                    key: format!("{}/@layout:157", use_scope),
                                                                    wrap: None,
                                                                    axis: ::ducktape_view_guest::wire::Axis::Column,
                                                                    spacing: Some((3.0) as f32),
                                                                    padding: None,
                                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                    height: None,
                                                                    align: None,
                                                                    background: None,
                                                                    border: None,
                                                                    children: children,
                                                                }
                                                            });
                                                        children
                                                            .push(::ducktape_view_guest::wire::Node::Space {
                                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                height: None,
                                                            });
                                                        if (self.appearance == "light") {
                                                            children
                                                                .push(::ducktape_view_guest::wire::Node::Button {
                                                                    checked: Some(true),
                                                                    expanded: None,
                                                                    description: None,
                                                                    key: format!("{}/@button:172", use_scope),
                                                                    content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                                                        String::from("Light"),
                                                                    ),
                                                                    label: None,
                                                                    on_press: Some(
                                                                        ::ducktape_view_guest::slots::message(
                                                                            Message::SetAppearanceLight,
                                                                        ),
                                                                    ),
                                                                    width: None,
                                                                    height: None,
                                                                    padding: Some(
                                                                        ::ducktape_view_guest::wire::Edges::all((6.0) as f32),
                                                                    ),
                                                                    style: ::ducktape_view_guest::wire::ButtonStyle {
                                                                        preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                                        recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                                            base: ::ducktape_view_guest::wire::Face {
                                                                                background: Some(palette.colors[7]),
                                                                                text: Some(palette.colors[9]),
                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                    color: None,
                                                                                    width: None,
                                                                                    radius: Some([9.0; 4]),
                                                                                }),
                                                                            },
                                                                            hover_background: Some(palette.colors[8]),
                                                                            pressed_background: Some({
                                                                                let mut color = palette.colors[7];
                                                                                color.0[3] = 0.800000;
                                                                                color
                                                                            }),
                                                                            disabled_background: Some(palette.colors[10]),
                                                                            disabled_text: Some(palette.colors[11]),
                                                                            disabled_opacity: None,
                                                                            focus_ring: Some(palette.colors[42]),
                                                                            text_size: Some(12.5f32),
                                                                            line_height: None,
                                                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                    "Geist".into(),
                                                                                ),
                                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                            }),
                                                                        }),
                                                                        active: ::ducktape_view_guest::wire::Face::default(),
                                                                        hovered: None,
                                                                        pressed: None,
                                                                        disabled: None,
                                                                    },
                                                                });
                                                        }
                                                        if (self.appearance != "light") {
                                                            children
                                                                .push(::ducktape_view_guest::wire::Node::Button {
                                                                    checked: Some(false),
                                                                    expanded: None,
                                                                    description: None,
                                                                    key: format!("{}/@button:178", use_scope),
                                                                    content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                                                        String::from("Light"),
                                                                    ),
                                                                    label: None,
                                                                    on_press: Some(
                                                                        ::ducktape_view_guest::slots::message(
                                                                            Message::SetAppearanceLight,
                                                                        ),
                                                                    ),
                                                                    width: None,
                                                                    height: None,
                                                                    padding: Some(
                                                                        ::ducktape_view_guest::wire::Edges::all((6.0) as f32),
                                                                    ),
                                                                    style: ::ducktape_view_guest::wire::ButtonStyle {
                                                                        preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                                        recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                                            base: ::ducktape_view_guest::wire::Face {
                                                                                background: Some(palette.colors[12]),
                                                                                text: Some(palette.colors[13]),
                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                    color: Some(palette.colors[40]),
                                                                                    width: Some(1.0),
                                                                                    radius: Some([9.0; 4]),
                                                                                }),
                                                                            },
                                                                            hover_background: Some(palette.colors[14]),
                                                                            pressed_background: Some(palette.colors[6]),
                                                                            disabled_background: None,
                                                                            disabled_text: None,
                                                                            disabled_opacity: Some(0.5f32),
                                                                            focus_ring: Some(palette.colors[42]),
                                                                            text_size: Some(12.5f32),
                                                                            line_height: None,
                                                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                    "Geist".into(),
                                                                                ),
                                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                            }),
                                                                        }),
                                                                        active: ::ducktape_view_guest::wire::Face::default(),
                                                                        hovered: None,
                                                                        pressed: None,
                                                                        disabled: None,
                                                                    },
                                                                });
                                                        }
                                                        if (self.appearance == "dark") {
                                                            children
                                                                .push(::ducktape_view_guest::wire::Node::Button {
                                                                    checked: Some(true),
                                                                    expanded: None,
                                                                    description: None,
                                                                    key: format!("{}/@button:184", use_scope),
                                                                    content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                                                        String::from("Dark"),
                                                                    ),
                                                                    label: None,
                                                                    on_press: Some(
                                                                        ::ducktape_view_guest::slots::message(
                                                                            Message::SetAppearanceDark,
                                                                        ),
                                                                    ),
                                                                    width: None,
                                                                    height: None,
                                                                    padding: Some(
                                                                        ::ducktape_view_guest::wire::Edges::all((6.0) as f32),
                                                                    ),
                                                                    style: ::ducktape_view_guest::wire::ButtonStyle {
                                                                        preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                                        recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                                            base: ::ducktape_view_guest::wire::Face {
                                                                                background: Some(palette.colors[7]),
                                                                                text: Some(palette.colors[9]),
                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                    color: None,
                                                                                    width: None,
                                                                                    radius: Some([9.0; 4]),
                                                                                }),
                                                                            },
                                                                            hover_background: Some(palette.colors[8]),
                                                                            pressed_background: Some({
                                                                                let mut color = palette.colors[7];
                                                                                color.0[3] = 0.800000;
                                                                                color
                                                                            }),
                                                                            disabled_background: Some(palette.colors[10]),
                                                                            disabled_text: Some(palette.colors[11]),
                                                                            disabled_opacity: None,
                                                                            focus_ring: Some(palette.colors[42]),
                                                                            text_size: Some(12.5f32),
                                                                            line_height: None,
                                                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                    "Geist".into(),
                                                                                ),
                                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                            }),
                                                                        }),
                                                                        active: ::ducktape_view_guest::wire::Face::default(),
                                                                        hovered: None,
                                                                        pressed: None,
                                                                        disabled: None,
                                                                    },
                                                                });
                                                        }
                                                        if (self.appearance != "dark") {
                                                            children
                                                                .push(::ducktape_view_guest::wire::Node::Button {
                                                                    checked: Some(false),
                                                                    expanded: None,
                                                                    description: None,
                                                                    key: format!("{}/@button:190", use_scope),
                                                                    content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                                                        String::from("Dark"),
                                                                    ),
                                                                    label: None,
                                                                    on_press: Some(
                                                                        ::ducktape_view_guest::slots::message(
                                                                            Message::SetAppearanceDark,
                                                                        ),
                                                                    ),
                                                                    width: None,
                                                                    height: None,
                                                                    padding: Some(
                                                                        ::ducktape_view_guest::wire::Edges::all((6.0) as f32),
                                                                    ),
                                                                    style: ::ducktape_view_guest::wire::ButtonStyle {
                                                                        preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                                        recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                                            base: ::ducktape_view_guest::wire::Face {
                                                                                background: Some(palette.colors[12]),
                                                                                text: Some(palette.colors[13]),
                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                    color: Some(palette.colors[40]),
                                                                                    width: Some(1.0),
                                                                                    radius: Some([9.0; 4]),
                                                                                }),
                                                                            },
                                                                            hover_background: Some(palette.colors[14]),
                                                                            pressed_background: Some(palette.colors[6]),
                                                                            disabled_background: None,
                                                                            disabled_text: None,
                                                                            disabled_opacity: Some(0.5f32),
                                                                            focus_ring: Some(palette.colors[42]),
                                                                            text_size: Some(12.5f32),
                                                                            line_height: None,
                                                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                    "Geist".into(),
                                                                                ),
                                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                            }),
                                                                        }),
                                                                        active: ::ducktape_view_guest::wire::Face::default(),
                                                                        hovered: None,
                                                                        pressed: None,
                                                                        disabled: None,
                                                                    },
                                                                });
                                                        }
                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                            max_width: None,
                                                            clip: false,
                                                            key: format!("{}/@layout:152", use_scope),
                                                            wrap: None,
                                                            axis: ::ducktape_view_guest::wire::Axis::Row,
                                                            spacing: Some((9.0) as f32),
                                                            padding: None,
                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                            height: None,
                                                            align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                            background: None,
                                                            border: None,
                                                            children: children,
                                                        }
                                                    }),
                                                });
                                            ::ducktape_view_guest::wire::Node::Linear {
                                                max_width: None,
                                                clip: false,
                                                key: format!("{}/@layout:142", use_scope),
                                                wrap: None,
                                                axis: ::ducktape_view_guest::wire::Axis::Column,
                                                spacing: Some((9.0) as f32),
                                                padding: None,
                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                height: None,
                                                align: None,
                                                background: None,
                                                border: None,
                                                children: children,
                                            }
                                        });
                                    children
                                        .push({
                                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                            children
                                                .push(
                                                    self
                                                        .render_group_label_5(
                                                            palette,
                                                            format!("{}/GroupLabel@737", use_scope),
                                                        ),
                                                );
                                            children
                                                .push(::ducktape_view_guest::wire::Node::Container {
                                                    shadow: ::ducktape_view_guest::wire::Shadow {
                                                        color: None,
                                                        x: None,
                                                        y: None,
                                                        blur: None,
                                                    },
                                                    max_width: None,
                                                    max_height: None,
                                                    clip: false,
                                                    key: format!("{}/@container:197", use_scope),
                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                    height: None,
                                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                                        top: (15.0) as f32,
                                                        right: (15.0) as f32,
                                                        bottom: (15.0) as f32,
                                                        left: (15.0) as f32,
                                                    }),
                                                    align_x: None,
                                                    align_y: None,
                                                    background: (Some(palette.colors[3]))
                                                        .map(::ducktape_view_guest::wire::Background::Color),
                                                    border: Some(::ducktape_view_guest::wire::Border {
                                                        color: Some(palette.colors[63]),
                                                        width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                        radius: Some([
                                                            ((11.0) as f32).max(0.0).min(f32::MAX),
                                                            ((11.0) as f32).max(0.0).min(f32::MAX),
                                                            ((11.0) as f32).max(0.0).min(f32::MAX),
                                                            ((11.0) as f32).max(0.0).min(f32::MAX),
                                                        ]),
                                                    }),
                                                    snap: None,
                                                    content: Box::new({
                                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                        children
                                                            .push({
                                                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                children
                                                                    .push(::ducktape_view_guest::wire::Node::Text {
                                                                        options: ::ducktape_view_guest::wire::TextOptions {
                                                                            height: None,
                                                                            align_y: None,
                                                                            line_height: None,
                                                                            shaping: None,
                                                                            wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                                            tracking: 0.0f32,
                                                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                    "Geist".into(),
                                                                                ),
                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                            }),
                                                                        },
                                                                        key: format!("{}/@text:211", use_scope),
                                                                        size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                        color: Some(palette.colors[4]),
                                                                        font: ::ducktape_view_guest::wire::Font {
                                                                            monospace: false,
                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                        },
                                                                        width: None,
                                                                        align_x: None,
                                                                        content: ("Mentions and direct messages".to_owned())
                                                                            .to_string(),
                                                                    });
                                                                if self.desktop_notifications {
                                                                    children
                                                                        .push(::ducktape_view_guest::wire::Node::Text {
                                                                            options: ::ducktape_view_guest::wire::TextOptions {
                                                                                height: None,
                                                                                align_y: None,
                                                                                line_height: None,
                                                                                shaping: None,
                                                                                wrapping: None,
                                                                                tracking: 0.0f32,
                                                                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                        "Geist".into(),
                                                                                    ),
                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                }),
                                                                            },
                                                                            key: format!("{}/@text:217", use_scope),
                                                                            size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                            color: Some(palette.colors[70]),
                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                monospace: false,
                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                            },
                                                                            width: None,
                                                                            align_x: None,
                                                                            content: ("A desktop banner when you are named, or written to directly."
                                                                                .to_owned())
                                                                                .to_string(),
                                                                        });
                                                                }
                                                                if (!self.desktop_notifications) {
                                                                    children
                                                                        .push(::ducktape_view_guest::wire::Node::Text {
                                                                            options: ::ducktape_view_guest::wire::TextOptions {
                                                                                height: None,
                                                                                align_y: None,
                                                                                line_height: None,
                                                                                shaping: None,
                                                                                wrapping: None,
                                                                                tracking: 0.0f32,
                                                                                font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                    family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                        "Geist".into(),
                                                                                    ),
                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                    stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                    style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                }),
                                                                            },
                                                                            key: format!("{}/@text:222", use_scope),
                                                                            size: Some(((11.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                            color: Some(palette.colors[70]),
                                                                            font: ::ducktape_view_guest::wire::Font {
                                                                                monospace: false,
                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                            },
                                                                            width: None,
                                                                            align_x: None,
                                                                            content: ("Silent — the bell is the only notice."
                                                                                .to_owned())
                                                                                .to_string(),
                                                                        });
                                                                }
                                                                ::ducktape_view_guest::wire::Node::Linear {
                                                                    max_width: None,
                                                                    clip: false,
                                                                    key: format!("{}/@layout:210", use_scope),
                                                                    wrap: None,
                                                                    axis: ::ducktape_view_guest::wire::Axis::Column,
                                                                    spacing: Some((3.0) as f32),
                                                                    padding: None,
                                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                    height: None,
                                                                    align: None,
                                                                    background: None,
                                                                    border: None,
                                                                    children: children,
                                                                }
                                                            });
                                                        children
                                                            .push(::ducktape_view_guest::wire::Node::Space {
                                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                height: None,
                                                            });
                                                        if self.desktop_notifications {
                                                            children
                                                                .push(::ducktape_view_guest::wire::Node::Button {
                                                                    checked: Some(true),
                                                                    expanded: None,
                                                                    description: None,
                                                                    key: format!("{}/@button:225", use_scope),
                                                                    content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                                                        String::from("On"),
                                                                    ),
                                                                    label: None,
                                                                    on_press: Some(
                                                                        ::ducktape_view_guest::slots::message(
                                                                            Message::SetDesktopNotifications(true),
                                                                        ),
                                                                    ),
                                                                    width: None,
                                                                    height: None,
                                                                    padding: Some(
                                                                        ::ducktape_view_guest::wire::Edges::all((6.0) as f32),
                                                                    ),
                                                                    style: ::ducktape_view_guest::wire::ButtonStyle {
                                                                        preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                                        recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                                            base: ::ducktape_view_guest::wire::Face {
                                                                                background: Some(palette.colors[7]),
                                                                                text: Some(palette.colors[9]),
                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                    color: None,
                                                                                    width: None,
                                                                                    radius: Some([9.0; 4]),
                                                                                }),
                                                                            },
                                                                            hover_background: Some(palette.colors[8]),
                                                                            pressed_background: Some({
                                                                                let mut color = palette.colors[7];
                                                                                color.0[3] = 0.800000;
                                                                                color
                                                                            }),
                                                                            disabled_background: Some(palette.colors[10]),
                                                                            disabled_text: Some(palette.colors[11]),
                                                                            disabled_opacity: None,
                                                                            focus_ring: Some(palette.colors[42]),
                                                                            text_size: Some(12.5f32),
                                                                            line_height: None,
                                                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                    "Geist".into(),
                                                                                ),
                                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                            }),
                                                                        }),
                                                                        active: ::ducktape_view_guest::wire::Face::default(),
                                                                        hovered: None,
                                                                        pressed: None,
                                                                        disabled: None,
                                                                    },
                                                                });
                                                        }
                                                        if (!self.desktop_notifications) {
                                                            children
                                                                .push(::ducktape_view_guest::wire::Node::Button {
                                                                    checked: Some(false),
                                                                    expanded: None,
                                                                    description: None,
                                                                    key: format!("{}/@button:231", use_scope),
                                                                    content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                                                        String::from("On"),
                                                                    ),
                                                                    label: None,
                                                                    on_press: Some(
                                                                        ::ducktape_view_guest::slots::message(
                                                                            Message::SetDesktopNotifications(true),
                                                                        ),
                                                                    ),
                                                                    width: None,
                                                                    height: None,
                                                                    padding: Some(
                                                                        ::ducktape_view_guest::wire::Edges::all((6.0) as f32),
                                                                    ),
                                                                    style: ::ducktape_view_guest::wire::ButtonStyle {
                                                                        preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                                        recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                                            base: ::ducktape_view_guest::wire::Face {
                                                                                background: Some(palette.colors[12]),
                                                                                text: Some(palette.colors[13]),
                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                    color: Some(palette.colors[40]),
                                                                                    width: Some(1.0),
                                                                                    radius: Some([9.0; 4]),
                                                                                }),
                                                                            },
                                                                            hover_background: Some(palette.colors[14]),
                                                                            pressed_background: Some(palette.colors[6]),
                                                                            disabled_background: None,
                                                                            disabled_text: None,
                                                                            disabled_opacity: Some(0.5f32),
                                                                            focus_ring: Some(palette.colors[42]),
                                                                            text_size: Some(12.5f32),
                                                                            line_height: None,
                                                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                    "Geist".into(),
                                                                                ),
                                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                            }),
                                                                        }),
                                                                        active: ::ducktape_view_guest::wire::Face::default(),
                                                                        hovered: None,
                                                                        pressed: None,
                                                                        disabled: None,
                                                                    },
                                                                });
                                                        }
                                                        if (!self.desktop_notifications) {
                                                            children
                                                                .push(::ducktape_view_guest::wire::Node::Button {
                                                                    checked: Some(true),
                                                                    expanded: None,
                                                                    description: None,
                                                                    key: format!("{}/@button:237", use_scope),
                                                                    content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                                                        String::from("Off"),
                                                                    ),
                                                                    label: None,
                                                                    on_press: Some(
                                                                        ::ducktape_view_guest::slots::message(
                                                                            Message::SetDesktopNotifications(false),
                                                                        ),
                                                                    ),
                                                                    width: None,
                                                                    height: None,
                                                                    padding: Some(
                                                                        ::ducktape_view_guest::wire::Edges::all((6.0) as f32),
                                                                    ),
                                                                    style: ::ducktape_view_guest::wire::ButtonStyle {
                                                                        preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                                        recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                                            base: ::ducktape_view_guest::wire::Face {
                                                                                background: Some(palette.colors[7]),
                                                                                text: Some(palette.colors[9]),
                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                    color: None,
                                                                                    width: None,
                                                                                    radius: Some([9.0; 4]),
                                                                                }),
                                                                            },
                                                                            hover_background: Some(palette.colors[8]),
                                                                            pressed_background: Some({
                                                                                let mut color = palette.colors[7];
                                                                                color.0[3] = 0.800000;
                                                                                color
                                                                            }),
                                                                            disabled_background: Some(palette.colors[10]),
                                                                            disabled_text: Some(palette.colors[11]),
                                                                            disabled_opacity: None,
                                                                            focus_ring: Some(palette.colors[42]),
                                                                            text_size: Some(12.5f32),
                                                                            line_height: None,
                                                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                    "Geist".into(),
                                                                                ),
                                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                            }),
                                                                        }),
                                                                        active: ::ducktape_view_guest::wire::Face::default(),
                                                                        hovered: None,
                                                                        pressed: None,
                                                                        disabled: None,
                                                                    },
                                                                });
                                                        }
                                                        if self.desktop_notifications {
                                                            children
                                                                .push(::ducktape_view_guest::wire::Node::Button {
                                                                    checked: Some(false),
                                                                    expanded: None,
                                                                    description: None,
                                                                    key: format!("{}/@button:243", use_scope),
                                                                    content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                                                        String::from("Off"),
                                                                    ),
                                                                    label: None,
                                                                    on_press: Some(
                                                                        ::ducktape_view_guest::slots::message(
                                                                            Message::SetDesktopNotifications(false),
                                                                        ),
                                                                    ),
                                                                    width: None,
                                                                    height: None,
                                                                    padding: Some(
                                                                        ::ducktape_view_guest::wire::Edges::all((6.0) as f32),
                                                                    ),
                                                                    style: ::ducktape_view_guest::wire::ButtonStyle {
                                                                        preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                                        recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                                            base: ::ducktape_view_guest::wire::Face {
                                                                                background: Some(palette.colors[12]),
                                                                                text: Some(palette.colors[13]),
                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                    color: Some(palette.colors[40]),
                                                                                    width: Some(1.0),
                                                                                    radius: Some([9.0; 4]),
                                                                                }),
                                                                            },
                                                                            hover_background: Some(palette.colors[14]),
                                                                            pressed_background: Some(palette.colors[6]),
                                                                            disabled_background: None,
                                                                            disabled_text: None,
                                                                            disabled_opacity: Some(0.5f32),
                                                                            focus_ring: Some(palette.colors[42]),
                                                                            text_size: Some(12.5f32),
                                                                            line_height: None,
                                                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                    "Geist".into(),
                                                                                ),
                                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                            }),
                                                                        }),
                                                                        active: ::ducktape_view_guest::wire::Face::default(),
                                                                        hovered: None,
                                                                        pressed: None,
                                                                        disabled: None,
                                                                    },
                                                                });
                                                        }
                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                            max_width: None,
                                                            clip: false,
                                                            key: format!("{}/@layout:205", use_scope),
                                                            wrap: None,
                                                            axis: ::ducktape_view_guest::wire::Axis::Row,
                                                            spacing: Some((9.0) as f32),
                                                            padding: None,
                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                            height: None,
                                                            align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                            background: None,
                                                            border: None,
                                                            children: children,
                                                        }
                                                    }),
                                                });
                                            ::ducktape_view_guest::wire::Node::Linear {
                                                max_width: None,
                                                clip: false,
                                                key: format!("{}/@layout:195", use_scope),
                                                wrap: None,
                                                axis: ::ducktape_view_guest::wire::Axis::Column,
                                                spacing: Some((9.0) as f32),
                                                padding: None,
                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                height: None,
                                                align: None,
                                                background: None,
                                                border: None,
                                                children: children,
                                            }
                                        });
                                    ::ducktape_view_guest::wire::Node::Linear {
                                        max_width: None,
                                        clip: false,
                                        key: format!("{}/@layout:141", use_scope),
                                        wrap: None,
                                        axis: ::ducktape_view_guest::wire::Axis::Column,
                                        spacing: Some((18.0) as f32),
                                        padding: None,
                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                        height: None,
                                        align: None,
                                        background: None,
                                        border: None,
                                        children: children,
                                    }
                                });
                        }
                        SettingsPane::Network => {
                            children
                                .push({
                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                    children
                                        .push({
                                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                            children
                                                .push(
                                                    self
                                                        .render_group_label_6(
                                                            palette,
                                                            format!("{}/GroupLabel@799", use_scope),
                                                        ),
                                                );
                                            children
                                                .push(
                                                    self
                                                        .render_group_card_11(
                                                            palette,
                                                            format!("{}/GroupCard@800", use_scope),
                                                            use_scope.clone(),
                                                        ),
                                                );
                                            ::ducktape_view_guest::wire::Node::Linear {
                                                max_width: None,
                                                clip: false,
                                                key: format!("{}/@layout:257", use_scope),
                                                wrap: None,
                                                axis: ::ducktape_view_guest::wire::Axis::Column,
                                                spacing: Some((9.0) as f32),
                                                padding: None,
                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                height: None,
                                                align: None,
                                                background: None,
                                                border: None,
                                                children: children,
                                            }
                                        });
                                    ::ducktape_view_guest::wire::Node::Linear {
                                        max_width: None,
                                        clip: false,
                                        key: format!("{}/@layout:256", use_scope),
                                        wrap: None,
                                        axis: ::ducktape_view_guest::wire::Axis::Column,
                                        spacing: Some((18.0) as f32),
                                        padding: None,
                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                        height: None,
                                        align: None,
                                        background: None,
                                        border: None,
                                        children: children,
                                    }
                                });
                        }
                        SettingsPane::Account => {
                            children
                                .push({
                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                    children
                                        .push({
                                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                            children
                                                .push(
                                                    self
                                                        .render_group_label_12(
                                                            palette,
                                                            format!("{}/GroupLabel@905", use_scope),
                                                        ),
                                                );
                                            children
                                                .push(::ducktape_view_guest::wire::Node::Container {
                                                    shadow: ::ducktape_view_guest::wire::Shadow {
                                                        color: None,
                                                        x: None,
                                                        y: None,
                                                        blur: None,
                                                    },
                                                    max_width: None,
                                                    max_height: None,
                                                    clip: false,
                                                    key: format!("{}/@container:365", use_scope),
                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                    height: None,
                                                    padding: Some(::ducktape_view_guest::wire::Edges {
                                                        top: (15.0) as f32,
                                                        right: (15.0) as f32,
                                                        bottom: (15.0) as f32,
                                                        left: (15.0) as f32,
                                                    }),
                                                    align_x: None,
                                                    align_y: None,
                                                    background: (Some(palette.colors[3]))
                                                        .map(::ducktape_view_guest::wire::Background::Color),
                                                    border: Some(::ducktape_view_guest::wire::Border {
                                                        color: Some(palette.colors[63]),
                                                        width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                        radius: Some([
                                                            ((11.0) as f32).max(0.0).min(f32::MAX),
                                                            ((11.0) as f32).max(0.0).min(f32::MAX),
                                                            ((11.0) as f32).max(0.0).min(f32::MAX),
                                                            ((11.0) as f32).max(0.0).min(f32::MAX),
                                                        ]),
                                                    }),
                                                    snap: None,
                                                    content: Box::new({
                                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                        children
                                                            .push(
                                                                self
                                                                    .render_person_avatar_13(
                                                                        palette,
                                                                        format!("{}/PersonAvatar@919", use_scope),
                                                                    ),
                                                            );
                                                        children
                                                            .push({
                                                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                children
                                                                    .push({
                                                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                        if (!(self.account_name).is_empty()) {
                                                                            children
                                                                                .push(::ducktape_view_guest::wire::Node::Text {
                                                                                    options: ::ducktape_view_guest::wire::TextOptions {
                                                                                        height: None,
                                                                                        align_y: None,
                                                                                        line_height: None,
                                                                                        shaping: None,
                                                                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                                                        tracking: 0.0f32,
                                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                "Geist".into(),
                                                                                            ),
                                                                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                        }),
                                                                                    },
                                                                                    key: format!("{}/@text:397", use_scope),
                                                                                    size: Some(((13.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                    color: Some(palette.colors[4]),
                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                        monospace: false,
                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                    },
                                                                                    width: None,
                                                                                    align_x: None,
                                                                                    content: (self.account_name.to_owned()).to_string(),
                                                                                });
                                                                        }
                                                                        if (self.account_name).is_empty() {
                                                                            children
                                                                                .push(::ducktape_view_guest::wire::Node::Text {
                                                                                    options: ::ducktape_view_guest::wire::TextOptions {
                                                                                        height: None,
                                                                                        align_y: None,
                                                                                        line_height: None,
                                                                                        shaping: None,
                                                                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                                                        tracking: 0.0f32,
                                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                "Geist".into(),
                                                                                            ),
                                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                        }),
                                                                                    },
                                                                                    key: format!("{}/@text:404", use_scope),
                                                                                    size: Some(((13.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                    color: Some(palette.colors[5]),
                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                        monospace: false,
                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                    },
                                                                                    width: None,
                                                                                    align_x: None,
                                                                                    content: ("(unnamed)".to_owned()).to_string(),
                                                                                });
                                                                        }
                                                                        if self.admin {
                                                                            children
                                                                                .push(
                                                                                    self
                                                                                        .render_badge_secondary_14(
                                                                                            palette,
                                                                                            format!("{}/Badge.Secondary@961", use_scope),
                                                                                        ),
                                                                                );
                                                                        }
                                                                        if ((!self.admin) && (!(self.tier).is_empty())) {
                                                                            children
                                                                                .push(
                                                                                    self
                                                                                        .render_badge_outline_15(
                                                                                            palette,
                                                                                            format!("{}/Badge.Outline@963", use_scope),
                                                                                        ),
                                                                                );
                                                                        }
                                                                        if ((self.tier).is_empty() && self.members_answered) {
                                                                            children
                                                                                .push(
                                                                                    self
                                                                                        .render_badge_outline_16(
                                                                                            palette,
                                                                                            format!("{}/Badge.Outline@965", use_scope),
                                                                                        ),
                                                                                );
                                                                        }
                                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                                            max_width: None,
                                                                            clip: false,
                                                                            key: format!("{}/@layout:391", use_scope),
                                                                            wrap: None,
                                                                            axis: ::ducktape_view_guest::wire::Axis::Row,
                                                                            spacing: Some((7.0) as f32),
                                                                            padding: None,
                                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                            height: None,
                                                                            align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                                            background: None,
                                                                            border: None,
                                                                            children: children,
                                                                        }
                                                                    });
                                                                children
                                                                    .push({
                                                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                        children
                                                                            .push(::ducktape_view_guest::wire::Node::Text {
                                                                                options: ::ducktape_view_guest::wire::TextOptions {
                                                                                    height: None,
                                                                                    align_y: None,
                                                                                    line_height: None,
                                                                                    shaping: None,
                                                                                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                                                    tracking: 0.0f32,
                                                                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                            "Geist Mono".into(),
                                                                                        ),
                                                                                        weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                    }),
                                                                                },
                                                                                key: format!("{}/@text:433", use_scope),
                                                                                size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                color: Some(palette.colors[72]),
                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                    monospace: false,
                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                },
                                                                                width: None,
                                                                                align_x: None,
                                                                                content: (self.account_number.to_owned()).to_string(),
                                                                            });
                                                                        if (!(self.account_number).is_empty()) {
                                                                            children
                                                                                .push(::ducktape_view_guest::wire::Node::Text {
                                                                                    options: ::ducktape_view_guest::wire::TextOptions {
                                                                                        height: None,
                                                                                        align_y: None,
                                                                                        line_height: None,
                                                                                        shaping: None,
                                                                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                                                        tracking: 0.0f32,
                                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                "Geist Mono".into(),
                                                                                            ),
                                                                                            weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                        }),
                                                                                    },
                                                                                    key: format!("{}/@text:448", use_scope),
                                                                                    size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                    color: Some(palette.colors[72]),
                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                        monospace: false,
                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                    },
                                                                                    width: None,
                                                                                    align_x: None,
                                                                                    content: ("·".to_owned()).to_string(),
                                                                                });
                                                                        }
                                                                        if (!(self.tier).is_empty()) {
                                                                            children
                                                                                .push(::ducktape_view_guest::wire::Node::Text {
                                                                                    options: ::ducktape_view_guest::wire::TextOptions {
                                                                                        height: None,
                                                                                        align_y: None,
                                                                                        line_height: None,
                                                                                        shaping: None,
                                                                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                                                        tracking: 0.0f32,
                                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                "Geist Mono".into(),
                                                                                            ),
                                                                                            weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                        }),
                                                                                    },
                                                                                    key: format!("{}/@text:457", use_scope),
                                                                                    size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                    color: Some(palette.colors[72]),
                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                        monospace: false,
                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                    },
                                                                                    width: None,
                                                                                    align_x: None,
                                                                                    content: (self.tier.to_owned()).to_string(),
                                                                                });
                                                                        }
                                                                        children
                                                                            .push(::ducktape_view_guest::wire::Node::Text {
                                                                                options: ::ducktape_view_guest::wire::TextOptions {
                                                                                    height: None,
                                                                                    align_y: None,
                                                                                    line_height: None,
                                                                                    shaping: None,
                                                                                    wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                                                    tracking: 0.0f32,
                                                                                    font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                        family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                            "Geist Mono".into(),
                                                                                        ),
                                                                                        weight: ::ducktape_view_guest::wire::Weight::Medium,
                                                                                        stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                        style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                    }),
                                                                                },
                                                                                key: format!("{}/@text:463", use_scope),
                                                                                size: Some(((10.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                color: Some(palette.colors[72]),
                                                                                font: ::ducktape_view_guest::wire::Font {
                                                                                    monospace: false,
                                                                                    weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                },
                                                                                width: None,
                                                                                align_x: None,
                                                                                content: ("keypair on this device".to_owned()).to_string(),
                                                                            });
                                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                                            max_width: None,
                                                                            clip: false,
                                                                            key: format!("{}/@layout:428", use_scope),
                                                                            wrap: None,
                                                                            axis: ::ducktape_view_guest::wire::Axis::Row,
                                                                            spacing: Some((5.0) as f32),
                                                                            padding: None,
                                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                            height: None,
                                                                            align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                                            background: None,
                                                                            border: None,
                                                                            children: children,
                                                                        }
                                                                    });
                                                                ::ducktape_view_guest::wire::Node::Linear {
                                                                    max_width: None,
                                                                    clip: true,
                                                                    key: format!("{}/@layout:386", use_scope),
                                                                    wrap: None,
                                                                    axis: ::ducktape_view_guest::wire::Axis::Column,
                                                                    spacing: Some((3.0) as f32),
                                                                    padding: None,
                                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                    height: None,
                                                                    align: None,
                                                                    background: None,
                                                                    border: None,
                                                                    children: children,
                                                                }
                                                            });
                                                        children
                                                            .push({
                                                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                children
                                                                    .push({
                                                                        let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                        children
                                                                            .push({
                                                                                let node_scope = format!("{}/account-rename", node_scope);
                                                                                ::ducktape_view_guest::wire::Node::Input {
                                                                                    options: ::ducktape_view_guest::wire::InputOptions {
                                                                                        label: ("New display name".to_owned()).to_string(),
                                                                                        description: None,
                                                                                        disabled: self.account_busy,
                                                                                        padding: Some(
                                                                                            ::ducktape_view_guest::wire::Edges::all((5.0) as f32),
                                                                                        ),
                                                                                        text_size: Some((13.0) as f32),
                                                                                        line_height: Some((1.2) as f32),
                                                                                        align: None,
                                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                "Geist".into(),
                                                                                            ),
                                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                        }),
                                                                                    },
                                                                                    key: node_scope.clone(),
                                                                                    placeholder: String::from("rename account…".to_owned()),
                                                                                    value: (self.account_name_draft).to_string(),
                                                                                    on_input: ::ducktape_view_guest::slots::handler::<
                                                                                        String,
                                                                                        Message,
                                                                                    >(
                                                                                        Box::new({
                                                                                            let route = Message::BindAccountNameDraft
                                                                                                as fn(String) -> Message;
                                                                                            move |sent: String| Some(route(sent))
                                                                                        }),
                                                                                    ),
                                                                                    on_submit: None,
                                                                                    width: Some(
                                                                                        ::ducktape_view_guest::wire::Length::Fixed((150.0) as f32),
                                                                                    ),
                                                                                    secure: (false),
                                                                                    style: Box::new(::ducktape_view_guest::wire::InputStyle {
                                                                                        utility: ::ducktape_view_guest::wire::InputFace {
                                                                                            background: Some(palette.colors[3]),
                                                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                                                color: Some(palette.colors[39]),
                                                                                                width: Some(1f32),
                                                                                                radius: Some([10f32; 4]),
                                                                                            }),
                                                                                            ..Default::default()
                                                                                        },
                                                                                        focus_border: Some(palette.colors[42]),
                                                                                        focused_hovered: None,
                                                                                        active: ::ducktape_view_guest::wire::InputFace {
                                                                                            icon: None,
                                                                                            background: Some(palette.colors[55]),
                                                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                                                color: Some({
                                                                                                    let mut color = palette.colors[4];
                                                                                                    color.0[3] = 0.160000;
                                                                                                    color
                                                                                                }),
                                                                                                width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                                                radius: Some([
                                                                                                    ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                                    ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                                    ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                                    ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                                ]),
                                                                                            }),
                                                                                            value: Some(palette.colors[4]),
                                                                                            placeholder: Some(palette.colors[5]),
                                                                                            selection: Some({
                                                                                                let mut color = palette.colors[4];
                                                                                                color.0[3] = 0.180000;
                                                                                                color
                                                                                            }),
                                                                                        },
                                                                                        hovered: Some(::ducktape_view_guest::wire::InputFace {
                                                                                            icon: None,
                                                                                            background: Some(palette.colors[55]),
                                                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                                                color: Some({
                                                                                                    let mut color = palette.colors[4];
                                                                                                    color.0[3] = 0.210000;
                                                                                                    color
                                                                                                }),
                                                                                                width: None,
                                                                                                radius: None,
                                                                                            }),
                                                                                            value: None,
                                                                                            placeholder: None,
                                                                                            selection: None,
                                                                                        }),
                                                                                        focused: None,
                                                                                        disabled: Some(::ducktape_view_guest::wire::InputFace {
                                                                                            icon: None,
                                                                                            background: Some({
                                                                                                let mut color = palette.colors[6];
                                                                                                color.0[3] = 0.540000;
                                                                                                color
                                                                                            }),
                                                                                            border: None,
                                                                                            value: Some(palette.colors[5]),
                                                                                            placeholder: None,
                                                                                            selection: None,
                                                                                        }),
                                                                                    }),
                                                                                }
                                                                            });
                                                                        children
                                                                            .push(::ducktape_view_guest::wire::Node::Button {
                                                                                checked: None,
                                                                                expanded: None,
                                                                                description: None,
                                                                                key: format!("{}/@button:489", use_scope),
                                                                                content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                                                                    String::from("Rename"),
                                                                                ),
                                                                                label: None,
                                                                                on_press: if ((self.account_busy
                                                                                    || ((self.account_name_draft).trim().to_owned())
                                                                                        .is_empty()))
                                                                                {
                                                                                    None
                                                                                } else {
                                                                                    Some(
                                                                                            ::ducktape_view_guest::slots::message(
                                                                                                Message::AccountRenameSubmit,
                                                                                            ),
                                                                                        )
                                                                                },
                                                                                width: None,
                                                                                height: None,
                                                                                padding: Some(
                                                                                    ::ducktape_view_guest::wire::Edges::all((5.0) as f32),
                                                                                ),
                                                                                style: ::ducktape_view_guest::wire::ButtonStyle {
                                                                                    preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                                                    recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                                                        base: ::ducktape_view_guest::wire::Face {
                                                                                            background: Some(palette.colors[12]),
                                                                                            text: Some(palette.colors[13]),
                                                                                            border: Some(::ducktape_view_guest::wire::Border {
                                                                                                color: Some(palette.colors[40]),
                                                                                                width: Some(1.0),
                                                                                                radius: Some([9.0; 4]),
                                                                                            }),
                                                                                        },
                                                                                        hover_background: Some(palette.colors[14]),
                                                                                        pressed_background: Some(palette.colors[6]),
                                                                                        disabled_background: None,
                                                                                        disabled_text: None,
                                                                                        disabled_opacity: Some(0.5f32),
                                                                                        focus_ring: Some(palette.colors[42]),
                                                                                        text_size: Some(12.5f32),
                                                                                        line_height: None,
                                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                "Geist".into(),
                                                                                            ),
                                                                                            weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                        }),
                                                                                    }),
                                                                                    active: ::ducktape_view_guest::wire::Face::default(),
                                                                                    hovered: None,
                                                                                    pressed: None,
                                                                                    disabled: None,
                                                                                },
                                                                            });
                                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                                            max_width: None,
                                                                            clip: false,
                                                                            key: format!("{}/@layout:470", use_scope),
                                                                            wrap: None,
                                                                            axis: ::ducktape_view_guest::wire::Axis::Row,
                                                                            spacing: Some((5.0) as f32),
                                                                            padding: None,
                                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                            height: Some(
                                                                                ::ducktape_view_guest::wire::Length::Fixed((28.0) as f32),
                                                                            ),
                                                                            align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                                            background: None,
                                                                            border: None,
                                                                            children: children,
                                                                        }
                                                                    });
                                                                if (!self.account_exists) {
                                                                    children
                                                                        .push({
                                                                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                            children
                                                                                .push({
                                                                                    let node_scope = format!("{}/account-create", node_scope);
                                                                                    ::ducktape_view_guest::wire::Node::Input {
                                                                                        options: ::ducktape_view_guest::wire::InputOptions {
                                                                                            label: ("Account name".to_owned()).to_string(),
                                                                                            description: None,
                                                                                            disabled: self.account_busy,
                                                                                            padding: Some(
                                                                                                ::ducktape_view_guest::wire::Edges::all((5.0) as f32),
                                                                                            ),
                                                                                            text_size: Some((13.0) as f32),
                                                                                            line_height: Some((1.2) as f32),
                                                                                            align: None,
                                                                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                    "Geist".into(),
                                                                                                ),
                                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                            }),
                                                                                        },
                                                                                        key: node_scope.clone(),
                                                                                        placeholder: String::from(
                                                                                            "name your account…".to_owned(),
                                                                                        ),
                                                                                        value: (self.account_create_draft).to_string(),
                                                                                        on_input: ::ducktape_view_guest::slots::handler::<
                                                                                            String,
                                                                                            Message,
                                                                                        >(
                                                                                            Box::new({
                                                                                                let route = Message::BindAccountCreateDraft
                                                                                                    as fn(String) -> Message;
                                                                                                move |sent: String| Some(route(sent))
                                                                                            }),
                                                                                        ),
                                                                                        on_submit: None,
                                                                                        width: Some(
                                                                                            ::ducktape_view_guest::wire::Length::Fixed((150.0) as f32),
                                                                                        ),
                                                                                        secure: (false),
                                                                                        style: Box::new(::ducktape_view_guest::wire::InputStyle {
                                                                                            utility: ::ducktape_view_guest::wire::InputFace {
                                                                                                background: Some(palette.colors[3]),
                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                    color: Some(palette.colors[39]),
                                                                                                    width: Some(1f32),
                                                                                                    radius: Some([10f32; 4]),
                                                                                                }),
                                                                                                ..Default::default()
                                                                                            },
                                                                                            focus_border: Some(palette.colors[42]),
                                                                                            focused_hovered: None,
                                                                                            active: ::ducktape_view_guest::wire::InputFace {
                                                                                                icon: None,
                                                                                                background: Some(palette.colors[55]),
                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                    color: Some({
                                                                                                        let mut color = palette.colors[4];
                                                                                                        color.0[3] = 0.160000;
                                                                                                        color
                                                                                                    }),
                                                                                                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                                                    radius: Some([
                                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                                    ]),
                                                                                                }),
                                                                                                value: Some(palette.colors[4]),
                                                                                                placeholder: Some(palette.colors[5]),
                                                                                                selection: Some({
                                                                                                    let mut color = palette.colors[4];
                                                                                                    color.0[3] = 0.180000;
                                                                                                    color
                                                                                                }),
                                                                                            },
                                                                                            hovered: Some(::ducktape_view_guest::wire::InputFace {
                                                                                                icon: None,
                                                                                                background: Some(palette.colors[55]),
                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                    color: Some({
                                                                                                        let mut color = palette.colors[4];
                                                                                                        color.0[3] = 0.210000;
                                                                                                        color
                                                                                                    }),
                                                                                                    width: None,
                                                                                                    radius: None,
                                                                                                }),
                                                                                                value: None,
                                                                                                placeholder: None,
                                                                                                selection: None,
                                                                                            }),
                                                                                            focused: None,
                                                                                            disabled: Some(::ducktape_view_guest::wire::InputFace {
                                                                                                icon: None,
                                                                                                background: Some({
                                                                                                    let mut color = palette.colors[6];
                                                                                                    color.0[3] = 0.540000;
                                                                                                    color
                                                                                                }),
                                                                                                border: None,
                                                                                                value: Some(palette.colors[5]),
                                                                                                placeholder: None,
                                                                                                selection: None,
                                                                                            }),
                                                                                        }),
                                                                                    }
                                                                                });
                                                                            children
                                                                                .push(::ducktape_view_guest::wire::Node::Button {
                                                                                    checked: None,
                                                                                    expanded: None,
                                                                                    description: None,
                                                                                    key: format!("{}/@button:517", use_scope),
                                                                                    content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                                                                        String::from("Create account"),
                                                                                    ),
                                                                                    label: None,
                                                                                    on_press: if (((self.account_busy || (!self.unlocked))
                                                                                        || ((self.account_create_draft).trim().to_owned())
                                                                                            .is_empty()))
                                                                                    {
                                                                                        None
                                                                                    } else {
                                                                                        Some(
                                                                                                ::ducktape_view_guest::slots::message(
                                                                                                    Message::AccountCreateSubmit,
                                                                                                ),
                                                                                            )
                                                                                    },
                                                                                    width: None,
                                                                                    height: None,
                                                                                    padding: Some(
                                                                                        ::ducktape_view_guest::wire::Edges::all((5.0) as f32),
                                                                                    ),
                                                                                    style: ::ducktape_view_guest::wire::ButtonStyle {
                                                                                        preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                                                        recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                                                            base: ::ducktape_view_guest::wire::Face {
                                                                                                background: Some(palette.colors[12]),
                                                                                                text: Some(palette.colors[13]),
                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                    color: Some(palette.colors[40]),
                                                                                                    width: Some(1.0),
                                                                                                    radius: Some([9.0; 4]),
                                                                                                }),
                                                                                            },
                                                                                            hover_background: Some(palette.colors[14]),
                                                                                            pressed_background: Some(palette.colors[6]),
                                                                                            disabled_background: None,
                                                                                            disabled_text: None,
                                                                                            disabled_opacity: Some(0.5f32),
                                                                                            focus_ring: Some(palette.colors[42]),
                                                                                            text_size: Some(12.5f32),
                                                                                            line_height: None,
                                                                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                    "Geist".into(),
                                                                                                ),
                                                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                            }),
                                                                                        }),
                                                                                        active: ::ducktape_view_guest::wire::Face::default(),
                                                                                        hovered: None,
                                                                                        pressed: None,
                                                                                        disabled: None,
                                                                                    },
                                                                                });
                                                                            ::ducktape_view_guest::wire::Node::Linear {
                                                                                max_width: None,
                                                                                clip: false,
                                                                                key: format!("{}/@layout:498", use_scope),
                                                                                wrap: None,
                                                                                axis: ::ducktape_view_guest::wire::Axis::Row,
                                                                                spacing: Some((5.0) as f32),
                                                                                padding: None,
                                                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                height: Some(
                                                                                    ::ducktape_view_guest::wire::Length::Fixed((28.0) as f32),
                                                                                ),
                                                                                align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                                                background: None,
                                                                                border: None,
                                                                                children: children,
                                                                            }
                                                                        });
                                                                    children
                                                                        .push({
                                                                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                            children
                                                                                .push({
                                                                                    let node_scope = format!("{}/account-join", node_scope);
                                                                                    ::ducktape_view_guest::wire::Node::Input {
                                                                                        options: ::ducktape_view_guest::wire::InputOptions {
                                                                                            label: ("Add-key ticket".to_owned()).to_string(),
                                                                                            description: None,
                                                                                            disabled: self.account_busy,
                                                                                            padding: Some(
                                                                                                ::ducktape_view_guest::wire::Edges::all((5.0) as f32),
                                                                                            ),
                                                                                            text_size: Some((13.0) as f32),
                                                                                            line_height: Some((1.2) as f32),
                                                                                            align: None,
                                                                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                    "Geist".into(),
                                                                                                ),
                                                                                                weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                            }),
                                                                                        },
                                                                                        key: node_scope.clone(),
                                                                                        placeholder: String::from(
                                                                                            "paste a ticket from a member device…".to_owned(),
                                                                                        ),
                                                                                        value: (self.account_join_draft).to_string(),
                                                                                        on_input: ::ducktape_view_guest::slots::handler::<
                                                                                            String,
                                                                                            Message,
                                                                                        >(
                                                                                            Box::new({
                                                                                                let route = Message::BindAccountJoinDraft
                                                                                                    as fn(String) -> Message;
                                                                                                move |sent: String| Some(route(sent))
                                                                                            }),
                                                                                        ),
                                                                                        on_submit: None,
                                                                                        width: Some(
                                                                                            ::ducktape_view_guest::wire::Length::Fixed((150.0) as f32),
                                                                                        ),
                                                                                        secure: (false),
                                                                                        style: Box::new(::ducktape_view_guest::wire::InputStyle {
                                                                                            utility: ::ducktape_view_guest::wire::InputFace {
                                                                                                background: Some(palette.colors[3]),
                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                    color: Some(palette.colors[39]),
                                                                                                    width: Some(1f32),
                                                                                                    radius: Some([10f32; 4]),
                                                                                                }),
                                                                                                ..Default::default()
                                                                                            },
                                                                                            focus_border: Some(palette.colors[42]),
                                                                                            focused_hovered: None,
                                                                                            active: ::ducktape_view_guest::wire::InputFace {
                                                                                                icon: None,
                                                                                                background: Some(palette.colors[55]),
                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                    color: Some({
                                                                                                        let mut color = palette.colors[4];
                                                                                                        color.0[3] = 0.160000;
                                                                                                        color
                                                                                                    }),
                                                                                                    width: Some(((1.0) as f32).max(0.0).min(f32::MAX)),
                                                                                                    radius: Some([
                                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                                        ((7.0) as f32).max(0.0).min(f32::MAX),
                                                                                                    ]),
                                                                                                }),
                                                                                                value: Some(palette.colors[4]),
                                                                                                placeholder: Some(palette.colors[5]),
                                                                                                selection: Some({
                                                                                                    let mut color = palette.colors[4];
                                                                                                    color.0[3] = 0.180000;
                                                                                                    color
                                                                                                }),
                                                                                            },
                                                                                            hovered: Some(::ducktape_view_guest::wire::InputFace {
                                                                                                icon: None,
                                                                                                background: Some(palette.colors[55]),
                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                    color: Some({
                                                                                                        let mut color = palette.colors[4];
                                                                                                        color.0[3] = 0.210000;
                                                                                                        color
                                                                                                    }),
                                                                                                    width: None,
                                                                                                    radius: None,
                                                                                                }),
                                                                                                value: None,
                                                                                                placeholder: None,
                                                                                                selection: None,
                                                                                            }),
                                                                                            focused: None,
                                                                                            disabled: Some(::ducktape_view_guest::wire::InputFace {
                                                                                                icon: None,
                                                                                                background: Some({
                                                                                                    let mut color = palette.colors[6];
                                                                                                    color.0[3] = 0.540000;
                                                                                                    color
                                                                                                }),
                                                                                                border: None,
                                                                                                value: Some(palette.colors[5]),
                                                                                                placeholder: None,
                                                                                                selection: None,
                                                                                            }),
                                                                                        }),
                                                                                    }
                                                                                });
                                                                            children
                                                                                .push(::ducktape_view_guest::wire::Node::Button {
                                                                                    checked: None,
                                                                                    expanded: None,
                                                                                    description: None,
                                                                                    key: format!("{}/@button:541", use_scope),
                                                                                    content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                                                                        String::from("Join"),
                                                                                    ),
                                                                                    label: None,
                                                                                    on_press: if (((self.account_busy || (!self.unlocked))
                                                                                        || ((self.account_join_draft).trim().to_owned())
                                                                                            .is_empty()))
                                                                                    {
                                                                                        None
                                                                                    } else {
                                                                                        Some(
                                                                                                ::ducktape_view_guest::slots::message(
                                                                                                    Message::AccountKeyJoinSubmit,
                                                                                                ),
                                                                                            )
                                                                                    },
                                                                                    width: None,
                                                                                    height: None,
                                                                                    padding: Some(
                                                                                        ::ducktape_view_guest::wire::Edges::all((5.0) as f32),
                                                                                    ),
                                                                                    style: ::ducktape_view_guest::wire::ButtonStyle {
                                                                                        preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                                                        recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                                                            base: ::ducktape_view_guest::wire::Face {
                                                                                                background: Some(palette.colors[12]),
                                                                                                text: Some(palette.colors[13]),
                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                    color: Some(palette.colors[40]),
                                                                                                    width: Some(1.0),
                                                                                                    radius: Some([9.0; 4]),
                                                                                                }),
                                                                                            },
                                                                                            hover_background: Some(palette.colors[14]),
                                                                                            pressed_background: Some(palette.colors[6]),
                                                                                            disabled_background: None,
                                                                                            disabled_text: None,
                                                                                            disabled_opacity: Some(0.5f32),
                                                                                            focus_ring: Some(palette.colors[42]),
                                                                                            text_size: Some(12.5f32),
                                                                                            line_height: None,
                                                                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                    "Geist".into(),
                                                                                                ),
                                                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                            }),
                                                                                        }),
                                                                                        active: ::ducktape_view_guest::wire::Face::default(),
                                                                                        hovered: None,
                                                                                        pressed: None,
                                                                                        disabled: None,
                                                                                    },
                                                                                });
                                                                            ::ducktape_view_guest::wire::Node::Linear {
                                                                                max_width: None,
                                                                                clip: false,
                                                                                key: format!("{}/@layout:522", use_scope),
                                                                                wrap: None,
                                                                                axis: ::ducktape_view_guest::wire::Axis::Row,
                                                                                spacing: Some((5.0) as f32),
                                                                                padding: None,
                                                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                height: Some(
                                                                                    ::ducktape_view_guest::wire::Length::Fixed((28.0) as f32),
                                                                                ),
                                                                                align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                                                background: None,
                                                                                border: None,
                                                                                children: children,
                                                                            }
                                                                        });
                                                                    children
                                                                        .push({
                                                                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                            children
                                                                                .push(::ducktape_view_guest::wire::Node::Text {
                                                                                    options: ::ducktape_view_guest::wire::TextOptions {
                                                                                        height: None,
                                                                                        align_y: None,
                                                                                        line_height: None,
                                                                                        shaping: None,
                                                                                        wrapping: None,
                                                                                        tracking: 0.0f32,
                                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                "Geist".into(),
                                                                                            ),
                                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                        }),
                                                                                    },
                                                                                    key: format!("{}/@text:554", use_scope),
                                                                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                    color: Some(palette.colors[71]),
                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                        monospace: false,
                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                    },
                                                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                    align_x: None,
                                                                                    content: ("…or let a passkey from another device admit this one."
                                                                                        .to_owned())
                                                                                        .to_string(),
                                                                                });
                                                                            children
                                                                                .push(::ducktape_view_guest::wire::Node::Button {
                                                                                    checked: None,
                                                                                    expanded: None,
                                                                                    description: None,
                                                                                    key: format!("{}/@button:559", use_scope),
                                                                                    content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                                                                        String::from("Log in with a passkey"),
                                                                                    ),
                                                                                    label: None,
                                                                                    on_press: if ((self.account_busy || (!self.unlocked))) {
                                                                                        None
                                                                                    } else {
                                                                                        Some(
                                                                                                ::ducktape_view_guest::slots::message(
                                                                                                    Message::AccountLoginSubmit,
                                                                                                ),
                                                                                            )
                                                                                    },
                                                                                    width: None,
                                                                                    height: None,
                                                                                    padding: Some(
                                                                                        ::ducktape_view_guest::wire::Edges::all((5.0) as f32),
                                                                                    ),
                                                                                    style: ::ducktape_view_guest::wire::ButtonStyle {
                                                                                        preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                                                        recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                                                            base: ::ducktape_view_guest::wire::Face {
                                                                                                background: Some(palette.colors[12]),
                                                                                                text: Some(palette.colors[13]),
                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                    color: Some(palette.colors[40]),
                                                                                                    width: Some(1.0),
                                                                                                    radius: Some([9.0; 4]),
                                                                                                }),
                                                                                            },
                                                                                            hover_background: Some(palette.colors[14]),
                                                                                            pressed_background: Some(palette.colors[6]),
                                                                                            disabled_background: None,
                                                                                            disabled_text: None,
                                                                                            disabled_opacity: Some(0.5f32),
                                                                                            focus_ring: Some(palette.colors[42]),
                                                                                            text_size: Some(12.5f32),
                                                                                            line_height: None,
                                                                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                    "Geist".into(),
                                                                                                ),
                                                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                            }),
                                                                                        }),
                                                                                        active: ::ducktape_view_guest::wire::Face::default(),
                                                                                        hovered: None,
                                                                                        pressed: None,
                                                                                        disabled: None,
                                                                                    },
                                                                                });
                                                                            ::ducktape_view_guest::wire::Node::Linear {
                                                                                max_width: None,
                                                                                clip: false,
                                                                                key: format!("{}/@layout:548", use_scope),
                                                                                wrap: None,
                                                                                axis: ::ducktape_view_guest::wire::Axis::Row,
                                                                                spacing: Some((5.0) as f32),
                                                                                padding: None,
                                                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                height: Some(
                                                                                    ::ducktape_view_guest::wire::Length::Fixed((28.0) as f32),
                                                                                ),
                                                                                align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                                                background: None,
                                                                                border: None,
                                                                                children: children,
                                                                            }
                                                                        });
                                                                    children
                                                                        .push({
                                                                            let node_scope = format!(
                                                                                "{}/account-login-ceremony", node_scope
                                                                            );
                                                                            self.render_settings_loading(palette, node_scope.clone())
                                                                        });
                                                                }
                                                                if self.account_exists {
                                                                    children
                                                                        .push({
                                                                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                                            children
                                                                                .push(::ducktape_view_guest::wire::Node::Text {
                                                                                    options: ::ducktape_view_guest::wire::TextOptions {
                                                                                        height: None,
                                                                                        align_y: None,
                                                                                        line_height: None,
                                                                                        shaping: None,
                                                                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                                                        tracking: 0.0f32,
                                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                "Geist Mono".into(),
                                                                                            ),
                                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                        }),
                                                                                    },
                                                                                    key: format!("{}/@text:582", use_scope),
                                                                                    size: Some(((12.0) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                    color: Some(palette.colors[71]),
                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                        monospace: false,
                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                    },
                                                                                    width: None,
                                                                                    align_x: None,
                                                                                    content: ((*self.derived_account_keys())).to_string(),
                                                                                });
                                                                            children
                                                                                .push(::ducktape_view_guest::wire::Node::Text {
                                                                                    options: ::ducktape_view_guest::wire::TextOptions {
                                                                                        height: None,
                                                                                        align_y: None,
                                                                                        line_height: None,
                                                                                        shaping: None,
                                                                                        wrapping: Some(::ducktape_view_guest::wire::Wrapping::None),
                                                                                        tracking: 0.0f32,
                                                                                        font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                            family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                "Geist".into(),
                                                                                            ),
                                                                                            weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                            stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                            style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                        }),
                                                                                    },
                                                                                    key: format!("{}/@text:588", use_scope),
                                                                                    size: Some(((12.5) as f32).max(f32::EPSILON).min(f32::MAX)),
                                                                                    color: Some(palette.colors[71]),
                                                                                    font: ::ducktape_view_guest::wire::Font {
                                                                                        monospace: false,
                                                                                        weight: ::ducktape_view_guest::wire::Weight::Normal,
                                                                                    },
                                                                                    width: None,
                                                                                    align_x: None,
                                                                                    content: ("keys".to_owned()).to_string(),
                                                                                });
                                                                            children
                                                                                .push(::ducktape_view_guest::wire::Node::Space {
                                                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                                                    height: None,
                                                                                });
                                                                            children
                                                                                .push(::ducktape_view_guest::wire::Node::Button {
                                                                                    checked: None,
                                                                                    expanded: None,
                                                                                    description: None,
                                                                                    key: format!("{}/@button:594", use_scope),
                                                                                    content: ::ducktape_view_guest::wire::ButtonContent::Label(
                                                                                        String::from("Copy number"),
                                                                                    ),
                                                                                    label: Some(String::from("Copy number".to_owned())),
                                                                                    on_press: if ((self.account_number).is_empty()) {
                                                                                        None
                                                                                    } else {
                                                                                        Some(
                                                                                                ::ducktape_view_guest::slots::message(
                                                                                                    Message::CopyToClipboard(
                                                                                                        self.account_number.to_owned(),
                                                                                                        "Number copied".to_owned(),
                                                                                                    ),
                                                                                                ),
                                                                                            )
                                                                                    },
                                                                                    width: None,
                                                                                    height: None,
                                                                                    padding: Some(
                                                                                        ::ducktape_view_guest::wire::Edges::all((7.0) as f32),
                                                                                    ),
                                                                                    style: ::ducktape_view_guest::wire::ButtonStyle {
                                                                                        preset: ::ducktape_view_guest::wire::ButtonPreset::Primary,
                                                                                        recipe: Some(::ducktape_view_guest::wire::ButtonRecipe {
                                                                                            base: ::ducktape_view_guest::wire::Face {
                                                                                                background: Some(palette.colors[12]),
                                                                                                text: Some(palette.colors[13]),
                                                                                                border: Some(::ducktape_view_guest::wire::Border {
                                                                                                    color: Some(palette.colors[40]),
                                                                                                    width: Some(1.0),
                                                                                                    radius: Some([9.0; 4]),
                                                                                                }),
                                                                                            },
                                                                                            hover_background: Some(palette.colors[14]),
                                                                                            pressed_background: Some(palette.colors[6]),
                                                                                            disabled_background: None,
                                                                                            disabled_text: None,
                                                                                            disabled_opacity: Some(0.5f32),
                                                                                            focus_ring: Some(palette.colors[42]),
                                                                                            text_size: Some(12.5f32),
                                                                                            line_height: None,
                                                                                            font: Some(::ducktape_view_guest::wire::NamedFont {
                                                                                                family: ::ducktape_view_guest::wire::FontFamily::Named(
                                                                                                    "Geist".into(),
                                                                                                ),
                                                                                                weight: ::ducktape_view_guest::wire::Weight::Semibold,
                                                                                                stretch: ::ducktape_view_guest::wire::FontStretch::Normal,
                                                                                                style: ::ducktape_view_guest::wire::FontStyle::Normal,
                                                                                            }),
                                                                                        }),
                                                                                        active: ::ducktape_view_guest::wire::Face::default(),
                                                                                        hovered: None,
                                                                                        pressed: None,
                                                                                        disabled: None,
                                                                                    },
                                                                                });
                                                                            ::ducktape_view_guest::wire::Node::Linear {
                                                                                max_width: None,
                                                                                clip: false,
                                                                                key: format!("{}/@layout:581", use_scope),
                                                                                wrap: None,
                                                                                axis: ::ducktape_view_guest::wire::Axis::Row,
                                                                                spacing: Some((8.0) as f32),
                                                                                padding: None,
                                                                                width: None,
                                                                                height: None,
                                                                                align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                                                background: None,
                                                                                border: None,
                                                                                children: children,
                                                                            }
                                                                        });
                                                                }
                                                                ::ducktape_view_guest::wire::Node::Linear {
                                                                    max_width: None,
                                                                    clip: false,
                                                                    key: format!("{}/@layout:469", use_scope),
                                                                    wrap: None,
                                                                    axis: ::ducktape_view_guest::wire::Axis::Column,
                                                                    spacing: Some((5.0) as f32),
                                                                    padding: None,
                                                                    width: None,
                                                                    height: None,
                                                                    align: None,
                                                                    background: None,
                                                                    border: None,
                                                                    children: children,
                                                                }
                                                            });
                                                        ::ducktape_view_guest::wire::Node::Linear {
                                                            max_width: None,
                                                            clip: false,
                                                            key: format!("{}/@layout:373", use_scope),
                                                            wrap: None,
                                                            axis: ::ducktape_view_guest::wire::Axis::Row,
                                                            spacing: Some((13.0) as f32),
                                                            padding: None,
                                                            width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                            height: None,
                                                            align: Some(::ducktape_view_guest::wire::AlignX::Center),
                                                            background: None,
                                                            border: None,
                                                            children: children,
                                                        }
                                                    }),
                                                });
                                            ::ducktape_view_guest::wire::Node::Linear {
                                                max_width: None,
                                                clip: false,
                                                key: format!("{}/@layout:363", use_scope),
                                                wrap: None,
                                                axis: ::ducktape_view_guest::wire::Axis::Column,
                                                spacing: Some((9.0) as f32),
                                                padding: None,
                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                height: None,
                                                align: None,
                                                background: None,
                                                border: None,
                                                children: children,
                                            }
                                        });
                                    if self.account_exists {
                                        children
                                            .push({
                                                let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                                children
                                                    .push(
                                                        self
                                                            .render_group_label_18(
                                                                palette,
                                                                format!("{}/GroupLabel@1147", use_scope),
                                                            ),
                                                    );
                                                children
                                                    .push(
                                                        self
                                                            .render_group_card_20(
                                                                palette,
                                                                format!("{}/GroupCard@1148", use_scope),
                                                                use_scope.clone(),
                                                            ),
                                                    );
                                                ::ducktape_view_guest::wire::Node::Linear {
                                                    max_width: None,
                                                    clip: false,
                                                    key: format!("{}/@layout:605", use_scope),
                                                    wrap: None,
                                                    axis: ::ducktape_view_guest::wire::Axis::Column,
                                                    spacing: Some((9.0) as f32),
                                                    padding: None,
                                                    width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                    height: None,
                                                    align: None,
                                                    background: None,
                                                    border: None,
                                                    children: children,
                                                }
                                            });
                                    }
                                    ::ducktape_view_guest::wire::Node::Linear {
                                        max_width: None,
                                        clip: false,
                                        key: format!("{}/@layout:362", use_scope),
                                        wrap: None,
                                        axis: ::ducktape_view_guest::wire::Axis::Column,
                                        spacing: Some((18.0) as f32),
                                        padding: None,
                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                        height: None,
                                        align: None,
                                        background: None,
                                        border: None,
                                        children: children,
                                    }
                                });
                        }
                        SettingsPane::Security => {
                            children
                                .push({
                                    let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                    children
                                        .push({
                                            let mut children: Vec<::ducktape_view_guest::wire::Node> = Vec::new();
                                            children
                                                .push(
                                                    self
                                                        .render_group_label_21(
                                                            palette,
                                                            format!("{}/GroupLabel@1294", use_scope),
                                                        ),
                                                );
                                            children
                                                .push(
                                                    self
                                                        .render_group_card_24(
                                                            palette,
                                                            format!("{}/GroupCard@1295", use_scope),
                                                            use_scope.clone(),
                                                        ),
                                                );
                                            ::ducktape_view_guest::wire::Node::Linear {
                                                max_width: None,
                                                clip: false,
                                                key: format!("{}/@layout:752", use_scope),
                                                wrap: None,
                                                axis: ::ducktape_view_guest::wire::Axis::Column,
                                                spacing: Some((9.0) as f32),
                                                padding: None,
                                                width: Some(::ducktape_view_guest::wire::Length::Fill),
                                                height: None,
                                                align: None,
                                                background: None,
                                                border: None,
                                                children: children,
                                            }
                                        });
                                    ::ducktape_view_guest::wire::Node::Linear {
                                        max_width: None,
                                        clip: false,
                                        key: format!("{}/@layout:751", use_scope),
                                        wrap: None,
                                        axis: ::ducktape_view_guest::wire::Axis::Column,
                                        spacing: Some((18.0) as f32),
                                        padding: None,
                                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                                        height: None,
                                        align: None,
                                        background: None,
                                        border: None,
                                        children: children,
                                    }
                                });
                        }
                    }
                    ::ducktape_view_guest::wire::Node::Linear {
                        max_width: Some(((860.0) as f32).max(0.0).min(f32::MAX)),
                        clip: false,
                        key: format!("{}/@layout:51", use_scope),
                        wrap: None,
                        axis: ::ducktape_view_guest::wire::Axis::Column,
                        spacing: Some((18.0) as f32),
                        padding: Some(::ducktape_view_guest::wire::Edges {
                            top: (22.0) as f32,
                            right: (22.0) as f32,
                            bottom: (22.0) as f32,
                            left: (22.0) as f32,
                        }),
                        width: Some(::ducktape_view_guest::wire::Length::Fill),
                        height: None,
                        align: None,
                        background: None,
                        border: None,
                        children: children,
                    }
                }),
            }
        }
    }
}
ducktape_view_guest::export_app!(
    SettingsView, "Settings",
    "This device's preferences, the account this key speaks for, and the workspace's lifecycle.",
    ["settings"]
);
