//! Settings as a module-owned view: this device's preferences and the account
//! this key speaks for, from the facts the desktop app pushes. Every act — a
//! theme, a rename, a minted ticket, the signing seat — leaves as an intent
//! the app signs.
pub mod host;
use ducktape_view_guest::{Subscription, Task, wire};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SettingsPane {
    General,
    Network,
    Account,
    Security,
}
#[derive(serde::Serialize, serde::Deserialize)]
pub struct SettingsView {
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
    key_password: String,
    settings_pane: SettingsPane,
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
    PickSettingsPane(SettingsPane),
    EditSettingsPassword(String),
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
impl SettingsView {
    fn state() -> Self {
        Self {
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
            key_password: String::new(),
            settings_pane: SettingsPane::General,
        }
    }
    pub(crate) fn boot() -> (Self, Task<Message>) {
        (Self::state(), Task::none())
    }
    pub(crate) const PREFERRED_WINDOW_SIZE: &'static str = "none";
    pub(crate) const SNAPSHOT_SCHEMA: &'static str =
        "f7208e9a48ef29a29f3cb594ed4aec98d6e8173f657a57fe97c64a8ff33cbe21";
    pub(crate) fn snapshot(&self) -> Result<Vec<u8>, String> {
        wire::Snapshot {
            schema: Self::SNAPSHOT_SCHEMA.into(),
            state: wire::SnapshotValue::Bytes(wire::encode(self)),
        }
        .encode()
    }
    pub(crate) fn restore(bytes: &[u8]) -> Result<Self, String> {
        let snapshot = wire::Snapshot::decode(bytes)?;
        if snapshot.schema != Self::SNAPSHOT_SCHEMA {
            return Err("snapshot schema mismatch".into());
        }
        let wire::SnapshotValue::Bytes(state) = snapshot.state else {
            return Err("snapshot state mismatch".into());
        };
        wire::decode(&state)
    }
}
impl SettingsView {
    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            crate::host::session().map(move |value| Message::SessionArrived(value)),
            if self.connected {
                Subscription::batch([crate::host::standing(self.connection_serial)
                    .map(move |value| Message::StandingArrived(value))])
            } else {
                Subscription::none()
            },
            if (self.connected && (!(self.seat_key).is_empty())) {
                Subscription::batch([crate::host::account_keys(
                    self.connection_serial,
                    self.seat_key.to_owned(),
                )
                .map(move |value| Message::KeysArrived(value))])
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
    fn snapshot_preserves_account_drafts_and_selected_settings_pane() {
        let (mut view, _) = SettingsView::boot();
        view.account_name_draft = "새 이름".into();
        view.account_key_draft = "aabb".into();
        view.account_key_label_draft = "휴대폰".into();
        view.account_create_draft = "new account".into();
        view.account_join_draft = "ticket".into();
        view.account_key_rows = vec![host::AccountKeyRow {
            scheme: "ed25519".into(),
            pubkey: "public-key".into(),
            label: "Laptop".into(),
        }];
        view.account_ceremony_phase = "show_qr".into();
        view.account_ceremony_qr = "ceremony payload".into();
        view.connection_serial = 17;
        view.settings_pane = SettingsPane::Account;
        view.key_password = "retained draft".into();
        let bytes = view.snapshot().unwrap();
        let restored = SettingsView::restore(&bytes).unwrap();
        assert_eq!(restored.account_name_draft, "새 이름");
        assert_eq!(restored.account_key_draft, "aabb");
        assert_eq!(restored.settings_pane, SettingsPane::Account);
        assert_eq!(restored.key_password, "retained draft");
        assert_eq!(restored.snapshot().unwrap(), bytes);
    }

    #[test]
    fn malformed_snapshot_is_rejected_without_partial_state() {
        let (view, _) = SettingsView::boot();
        let bytes = view.snapshot().unwrap();
        assert!(SettingsView::restore(&bytes[..bytes.len() / 2]).is_err());
        let mut snapshot = wire::Snapshot::decode(&bytes).unwrap();
        snapshot.state = wire::SnapshotValue::Bytes(vec![0xff]);
        assert!(SettingsView::restore(&snapshot.encode().unwrap()).is_err());
    }

    #[test]
    fn pane_changes_keep_the_same_password_draft() {
        let (mut view, _) = SettingsView::boot();
        drop(view.update(Message::EditSettingsPassword("draft".into())));
        for pane in [
            SettingsPane::Security,
            SettingsPane::General,
            SettingsPane::Security,
        ] {
            drop(view.update(Message::PickSettingsPane(pane)));
            assert_eq!(view.settings_pane, pane);
            assert_eq!(view.key_password, "draft");
        }
    }
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
    pub(crate) fn update(&mut self, message: Message) -> Task<Message> {
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
            Message::PickSettingsPane(picked) => self.on_pick_settings_pane(picked),
            Message::EditSettingsPassword(value) => self.on_edit_settings_password(value),
            Message::BindAccountNameDraft(value) => self.on_bind_account_name_draft(value),
            Message::BindAccountCreateDraft(value) => self.on_bind_account_create_draft(value),
            Message::BindAccountJoinDraft(value) => self.on_bind_account_join_draft(value),
            Message::BindAccountKeyDraft(value) => self.on_bind_account_key_draft(value),
            Message::BindAccountKeyLabelDraft(value) => self.on_bind_account_key_label_draft(value),
        }
    }
    fn on_session_arrived(&mut self, item: crate::host::SessionItem) -> Task<Message> {
        self.host_error = item.error.to_owned();
        if (!(item.error).is_empty()) {
            return Task::none();
        }
        let next = item.next.clone();
        self.connection_serial = crate::host::connection_serial_after(
            self.connected,
            next.connected,
            self.connection_serial,
        );
        self.connected = next.connected;
        self.loading = next.loading;
        self.status = next.status.to_owned();
        self.busy = next.busy;
        self.recovering = next.recovering;
        self.appearance = next.appearance.to_owned();
        self.desktop_notifications = next.desktop_notifications;
        self.unlocked = next.unlocked;
        self.seat_key = next.seat_key.to_owned();
        self.account_name = next.account_name.to_owned();
        self.network_name = next.network_name.to_owned();
        self.connected_rpc = next.connected_rpc.to_owned();
        self.account_ceremony_phase = next.account_ceremony_phase.to_owned();
        self.account_ceremony_qr = next.account_ceremony_qr.to_owned();
        self.account_ceremony_detail = next.account_ceremony_detail.to_owned();
        self.account_ceremony_left = next.account_ceremony_left.to_owned();
        self.settings_key_state = next.settings_key_state.to_owned();
        self.settings_key_path = next.settings_key_path.to_owned();
        self.account_busy = next.account_busy;
        self.account_ticket = next.account_ticket.to_owned();
        let renamed = crate::host::renamed_to(&(next.account_name), &(self.renaming_to));
        self.renaming_to = crate::host::keep_draft(renamed, &(self.renaming_to));
        self.account_name_draft = crate::host::keep_draft(renamed, &(self.account_name_draft));
        let founded = (next.account_exists && (!self.account_exists));
        self.account_exists = next.account_exists;
        self.account_number = next.account_number.to_owned();
        self.account_create_draft = crate::host::keep_draft(founded, &(self.account_create_draft));
        self.account_join_draft = crate::host::keep_draft(founded, &(self.account_join_draft));
        let minted = (!(next.account_ticket).is_empty());
        self.account_key_draft = crate::host::keep_draft(minted, &(self.account_key_draft));
        self.account_key_label_draft =
            crate::host::keep_draft(minted, &(self.account_key_label_draft));
        Task::none()
    }
    fn on_standing_arrived(&mut self, item: crate::host::StandingItem) -> Task<Message> {
        self.host_error = item.error.to_owned();
        self.members_answered = item.answered;
        if (!(item.error).is_empty()) {
            return Task::none();
        }
        self.tier = item.next.tier.to_owned();
        self.admin = item.next.admin;
        self.members_line = item.next.members_line.to_owned();
        Task::none()
    }
    fn on_keys_arrived(&mut self, item: crate::host::KeysItem) -> Task<Message> {
        self.host_error = item.error.to_owned();
        if (!(item.error).is_empty()) {
            return Task::none();
        }
        self.account_key_rows = item.rows.clone();
        Task::none()
    }
    fn on_show_tab(&mut self, tab: String) -> Task<Message> {
        crate::host::open_tab(&(tab));
        Task::none()
    }
    fn on_reconnect(&mut self) -> Task<Message> {
        crate::host::reconnect_network();
        Task::none()
    }
    fn on_switch_network(&mut self) -> Task<Message> {
        crate::host::switch_workspace();
        Task::none()
    }
    fn on_settings_unlock_submit(&mut self, pw: String) -> Task<Message> {
        if (self.busy || (pw).is_empty()) {
            return Task::none();
        }
        crate::host::unlock(&(pw));
        Task::none()
    }
    fn on_lock_session(&mut self) -> Task<Message> {
        crate::host::lock();
        Task::none()
    }
    fn on_account_rename_submit(&mut self) -> Task<Message> {
        if !self.connected {
            return Task::none();
        }
        if (self.account_busy || ((self.account_name_draft).trim().to_owned()).is_empty()) {
            return Task::none();
        }
        self.renaming_to = (self.account_name_draft).trim().to_owned();
        crate::host::rename_account(&((self.account_name_draft).trim().to_owned()));
        Task::none()
    }
    fn on_account_create_submit(&mut self) -> Task<Message> {
        if !self.connected {
            return Task::none();
        }
        if ((self.account_busy || (!self.unlocked))
            || ((self.account_create_draft).trim().to_owned()).is_empty())
        {
            return Task::none();
        }
        crate::host::create_account(&((self.account_create_draft).trim().to_owned()));
        Task::none()
    }
    fn on_account_key_add_submit(&mut self) -> Task<Message> {
        if !self.connected {
            return Task::none();
        }
        if ((self.account_busy || (!self.unlocked))
            || ((self.account_key_draft).trim().to_owned()).is_empty())
        {
            return Task::none();
        }
        crate::host::mint_ticket(
            &((self.account_key_draft).trim().to_owned()),
            &((self.account_key_label_draft).trim().to_owned()),
        );
        Task::none()
    }
    fn on_account_key_join_submit(&mut self) -> Task<Message> {
        if !self.connected {
            return Task::none();
        }
        if ((self.account_busy || (!self.unlocked))
            || ((self.account_join_draft).trim().to_owned()).is_empty())
        {
            return Task::none();
        }
        crate::host::join_account(&((self.account_join_draft).trim().to_owned()));
        Task::none()
    }
    fn on_account_key_remove(&mut self, pubkey: String) -> Task<Message> {
        if !self.connected {
            return Task::none();
        }
        if self.account_busy || !self.unlocked || self.account_key_rows.len() <= 1 {
            return Task::none();
        }
        crate::host::remove_key(&(pubkey));
        Task::none()
    }
    fn on_account_passkey_submit(&mut self) -> Task<Message> {
        if !self.connected {
            return Task::none();
        }
        crate::host::add_passkey(&((self.account_key_label_draft).trim().to_owned()));
        Task::none()
    }
    fn on_account_passkey_desktop(&mut self) -> Task<Message> {
        if !self.connected {
            return Task::none();
        }
        crate::host::add_passkey_here(&((self.account_key_label_draft).trim().to_owned()));
        Task::none()
    }
    fn on_account_ceremony_cancel(&mut self) -> Task<Message> {
        crate::host::cancel_ceremony();
        Task::none()
    }
    fn on_account_wallet_submit(&mut self) -> Task<Message> {
        if !self.connected {
            return Task::none();
        }
        crate::host::link_wallet(&((self.account_key_label_draft).trim().to_owned()));
        Task::none()
    }
    fn on_account_login_submit(&mut self) -> Task<Message> {
        if !self.connected {
            return Task::none();
        }
        crate::host::login();
        Task::none()
    }
    fn on_copy_to_clipboard(&mut self, text: String, label: String) -> Task<Message> {
        crate::host::copy(&(text), &(label));
        Task::none()
    }
    fn on_set_appearance_light(&mut self) -> Task<Message> {
        crate::host::set_light();
        Task::none()
    }
    fn on_set_appearance_dark(&mut self) -> Task<Message> {
        crate::host::set_dark();
        Task::none()
    }
    fn on_set_desktop_notifications(&mut self, enabled: bool) -> Task<Message> {
        crate::host::set_notifications(enabled);
        Task::none()
    }
    fn on_pick_settings_pane(&mut self, picked: SettingsPane) -> Task<Message> {
        self.settings_pane = picked;
        Task::none()
    }
    fn on_edit_settings_password(&mut self, value: String) -> Task<Message> {
        self.key_password = value;
        Task::none()
    }
    fn on_bind_account_name_draft(&mut self, value: String) -> Task<Message> {
        self.account_name_draft = value;
        Task::none()
    }
    fn on_bind_account_create_draft(&mut self, value: String) -> Task<Message> {
        self.account_create_draft = value;
        Task::none()
    }
    fn on_bind_account_join_draft(&mut self, value: String) -> Task<Message> {
        self.account_join_draft = value;
        Task::none()
    }
    fn on_bind_account_key_draft(&mut self, value: String) -> Task<Message> {
        self.account_key_draft = value;
        Task::none()
    }
    fn on_bind_account_key_label_draft(&mut self, value: String) -> Task<Message> {
        self.account_key_label_draft = value;
        Task::none()
    }
}
fn settings_action(
    key: impl Into<String>,
    label: &str,
    message: Message,
    enabled: bool,
) -> wire::Node {
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
) -> wire::Node {
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
    pub(crate) fn view(&self) -> wire::Node {
        use ducktape_view_guest::{kit, wire};
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
                        Message::PickSettingsPane(pane),
                        true,
                    );
                    if let wire::Node::Button { checked, .. } = &mut tab {
                        *checked = Some(self.settings_pane == pane);
                    }
                    tab
                }),
            ),
        ];
        if !self.host_error.is_empty() {
            content.push(kit::text("settings/error", &self.host_error));
        }
        content.push(match self.settings_pane {
            SettingsPane::General => self.general_settings(),
            SettingsPane::Network => self.network_settings(),
            SettingsPane::Account => self.account_settings(),
            SettingsPane::Security => self.security_settings(),
        });
        kit::scroll(
            "settings",
            kit::padded(
                kit::column("settings/content", content),
                wire::Edges::all(22.),
            ),
        )
    }
    fn general_settings(&self) -> wire::Node {
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
    fn network_settings(&self) -> wire::Node {
        use ducktape_view_guest::kit;
        if !self.connected {
            return kit::column(
                "settings/disconnected-network",
                [
                    kit::heading("settings/disconnected", "Not connected"),
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
            );
        }
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
    fn account_settings(&self) -> wire::Node {
        use ducktape_view_guest::{kit, wire};
        if !self.connected {
            return kit::column(
                "settings/disconnected-account",
                [
                    kit::heading("settings/account-offline", "Not connected"),
                    kit::text(
                        "settings/account-connect-help",
                        "Reconnect to read or change your account on this network.",
                    ),
                ],
            );
        }
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
                !self.account_busy && !self.account_name_draft.trim().is_empty(),
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
                Message::CopyToClipboard(self.account_number.clone(), "Number copied".into()),
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
    fn security_settings(&self) -> wire::Node {
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
                &self.key_password,
                slots::handler::<String, Message>(Box::new(|text| {
                    Some(Message::EditSettingsPassword(text))
                })),
                Some(slots::message(Message::SettingsUnlockSubmit(
                    self.key_password.clone(),
                ))),
            );
            if let wire::Node::Input { secure, .. } = &mut password {
                *secure = true;
            }
            content.push(password);
            content.push(settings_action(
                "settings/unlock",
                "Unlock",
                Message::SettingsUnlockSubmit(self.key_password.clone()),
                !self.busy && !self.key_password.is_empty(),
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
