//! The Approvals screen as a module-owned view: every decision the network
//! is being asked to make, and the ones it has settled, rendered from a
//! wasm component the desktop app loads from a file.
//!
//! The kernel pushes session facts only (`governance.props`: connected,
//! admin, dark). The view reads its own register through the kernel's
//! `rpc.query` / `rpc.blocks`, re-reads it on every `rpc.live` hit for the
//! governance plane, and a vote or a settle leaves as `op.submit` — the
//! governance message the kernel signs with the seated key. The endpoint,
//! the key and the password never cross: a guest that sees no key cannot
//! leak one.
pub mod host;
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum AppTheme {
    App,
    AppDark,
}

#[allow(dead_code)]
pub struct GovernanceView {
    pub(crate) active_palette: AppTheme,
    pub(crate) rows: Vec<crate::host::ProposalRow>,
    pub(crate) voting: String,
    pub(crate) admin: bool,
    pub(crate) connected: bool,
    pub(crate) connection_serial: i64,
    pub(crate) answered: bool,
    pub(crate) host_error: String,
    pub(crate) badge_sent: bool,
}
impl ::std::fmt::Debug for GovernanceView {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.write_str("GovernanceView")
    }
}
#[derive(Clone)]
pub enum Message {
    SessionArrived(crate::host::SessionItem),
    RegisterArrived(crate::host::RegisterItem),
    ActDone(crate::host::ActItem),
    GovVote(String, bool),
    GovExecute(String),
}
impl ::std::fmt::Debug for Message {
    fn fmt(&self, formatter: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        formatter.write_str("Message")
    }
}

#[allow(unused_parens)]
impl GovernanceView {
    fn state() -> Self {
        Self {
            active_palette: AppTheme::App,
            rows: Vec::new(),
            voting: "".to_owned(),
            admin: false,
            connected: false,
            connection_serial: 0,
            answered: false,
            host_error: "".to_owned(),
            badge_sent: false,
        }
    }
    pub(crate) fn boot() -> (Self, ::ducktape_view_guest::Task<Message>) {
        (Self::state(), ::ducktape_view_guest::Task::none())
    }
    pub(crate) const PREFERRED_WINDOW_SIZE: &'static str = "none";
    #[allow(clippy::too_many_arguments)]
    fn restore_state(
        active_palette: AppTheme,
        rows: Vec<crate::host::ProposalRow>,
        voting: String,
        admin: bool,
        connected: bool,
        connection_serial: i64,
        answered: bool,
        host_error: String,
        badge_sent: bool,
    ) -> Self {
        Self {
            active_palette: active_palette,
            rows: rows,
            voting: voting,
            admin: admin,
            connected: connected,
            connection_serial: connection_serial,
            answered: answered,
            host_error: host_error,
            badge_sent: badge_sent,
        }
    }
    pub(crate) const SNAPSHOT_SCHEMA: &'static str =
        "7c12db27b05b027805b40f4d493f95bcbf83f7b71fb9a350d90ef241043cbc72";
    pub(crate) fn snapshot(&self) -> Result<Vec<u8>, String> {
        ::ducktape_view_guest::wire::Snapshot {
            schema: String::from(Self::SNAPSHOT_SCHEMA),
            state: ::ducktape_view_guest::wire::SnapshotValue::Record {
                name: String::from("GovernanceView"),
                fields: vec![
                    (
                        String::from("active_palette"),
                        match &self.active_palette {
                            AppTheme::App => ::ducktape_view_guest::wire::SnapshotValue::Record {
                                name: String::from("AppTheme"),
                                fields: vec![(
                                    String::from("app"),
                                    ::ducktape_view_guest::wire::SnapshotValue::Unit,
                                )],
                            },
                            AppTheme::AppDark => {
                                ::ducktape_view_guest::wire::SnapshotValue::Record {
                                    name: String::from("AppTheme"),
                                    fields: vec![(
                                        String::from("app_dark"),
                                        ::ducktape_view_guest::wire::SnapshotValue::Unit,
                                    )],
                                }
                            }
                        },
                    ),
                    (
                        String::from("rows"),
                        ::ducktape_view_guest::wire::SnapshotValue::List(
                            (&self.rows)
                                .iter()
                                .map(|item| ::ducktape_view_guest::wire::SnapshotValue::Record {
                                    name: String::from("ProposalRow"),
                                    fields: ::std::vec![
                                        (
                                            String::from("id"),
                                            ::ducktape_view_guest::wire::SnapshotValue::Str(
                                                ::std::string::ToString::to_string(&(item).id)
                                            )
                                        ),
                                        (
                                            String::from("action"),
                                            ::ducktape_view_guest::wire::SnapshotValue::Str(
                                                ::std::string::ToString::to_string(&(item).action)
                                            )
                                        ),
                                        (
                                            String::from("detail"),
                                            ::ducktape_view_guest::wire::SnapshotValue::Str(
                                                ::std::string::ToString::to_string(&(item).detail)
                                            )
                                        ),
                                        (
                                            String::from("proposer"),
                                            ::ducktape_view_guest::wire::SnapshotValue::Str(
                                                ::std::string::ToString::to_string(
                                                    &(item).proposer
                                                )
                                            )
                                        ),
                                        (
                                            String::from("status"),
                                            ::ducktape_view_guest::wire::SnapshotValue::Str(
                                                ::std::string::ToString::to_string(&(item).status)
                                            )
                                        ),
                                        (
                                            String::from("deadline"),
                                            ::ducktape_view_guest::wire::SnapshotValue::I64(
                                                *(&(item).deadline)
                                            )
                                        ),
                                        (
                                            String::from("approvals"),
                                            ::ducktape_view_guest::wire::SnapshotValue::I64(
                                                *(&(item).approvals)
                                            )
                                        ),
                                        (
                                            String::from("rejections"),
                                            ::ducktape_view_guest::wire::SnapshotValue::I64(
                                                *(&(item).rejections)
                                            )
                                        ),
                                        (
                                            String::from("rule"),
                                            ::ducktape_view_guest::wire::SnapshotValue::Str(
                                                ::std::string::ToString::to_string(&(item).rule)
                                            )
                                        ),
                                        (
                                            String::from("required_yes"),
                                            ::ducktape_view_guest::wire::SnapshotValue::I64(
                                                *(&(item).required_yes)
                                            )
                                        ),
                                        (
                                            String::from("electorate"),
                                            ::ducktape_view_guest::wire::SnapshotValue::I64(
                                                *(&(item).electorate)
                                            )
                                        ),
                                        (
                                            String::from("open"),
                                            ::ducktape_view_guest::wire::SnapshotValue::Bool(
                                                *(&(item).open)
                                            )
                                        ),
                                        (
                                            String::from("settled_height"),
                                            ::ducktape_view_guest::wire::SnapshotValue::I64(
                                                *(&(item).settled_height)
                                            )
                                        )
                                    ],
                                })
                                .collect(),
                        ),
                    ),
                    (
                        String::from("voting"),
                        ::ducktape_view_guest::wire::SnapshotValue::Str(
                            ::std::string::ToString::to_string(&self.voting),
                        ),
                    ),
                    (
                        String::from("admin"),
                        ::ducktape_view_guest::wire::SnapshotValue::Bool(*(&self.admin)),
                    ),
                    (
                        String::from("connected"),
                        ::ducktape_view_guest::wire::SnapshotValue::Bool(*(&self.connected)),
                    ),
                    (
                        String::from("connection_serial"),
                        ::ducktape_view_guest::wire::SnapshotValue::I64(*(&self.connection_serial)),
                    ),
                    (
                        String::from("answered"),
                        ::ducktape_view_guest::wire::SnapshotValue::Bool(*(&self.answered)),
                    ),
                    (
                        String::from("host_error"),
                        ::ducktape_view_guest::wire::SnapshotValue::Str(
                            ::std::string::ToString::to_string(&self.host_error),
                        ),
                    ),
                    (
                        String::from("badge_sent"),
                        ::ducktape_view_guest::wire::SnapshotValue::Bool(*(&self.badge_sent)),
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
            if name != "GovernanceView" || fields.len() != 9 {
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
            if name != "rows" {
                return None;
            }
            let rows: Vec<crate::host::ProposalRow> = (match value {
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
                            if name != "ProposalRow" || fields.len() != 13 {
                                return None;
                            }
                            let mut fields = fields.into_iter();
                            let (name, field_0) = fields.next()?;
                            if name != "id" {
                                return None;
                            }
                            let (name, field_1) = fields.next()?;
                            if name != "action" {
                                return None;
                            }
                            let (name, field_2) = fields.next()?;
                            if name != "detail" {
                                return None;
                            }
                            let (name, field_3) = fields.next()?;
                            if name != "proposer" {
                                return None;
                            }
                            let (name, field_4) = fields.next()?;
                            if name != "status" {
                                return None;
                            }
                            let (name, field_5) = fields.next()?;
                            if name != "deadline" {
                                return None;
                            }
                            let (name, field_6) = fields.next()?;
                            if name != "approvals" {
                                return None;
                            }
                            let (name, field_7) = fields.next()?;
                            if name != "rejections" {
                                return None;
                            }
                            let (name, field_8) = fields.next()?;
                            if name != "rule" {
                                return None;
                            }
                            let (name, field_9) = fields.next()?;
                            if name != "required_yes" {
                                return None;
                            }
                            let (name, field_10) = fields.next()?;
                            if name != "electorate" {
                                return None;
                            }
                            let (name, field_11) = fields.next()?;
                            if name != "open" {
                                return None;
                            }
                            let (name, field_12) = fields.next()?;
                            if name != "settled_height" {
                                return None;
                            }
                            Some(crate::host::ProposalRow {
                                id: (match field_0 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                action: (match field_1 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                detail: (match field_2 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                proposer: (match field_3 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                status: (match field_4 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                deadline: (match field_5 {
                                    ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                approvals: (match field_6 {
                                    ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                rejections: (match field_7 {
                                    ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                rule: (match field_8 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Str(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                required_yes: (match field_9 {
                                    ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                electorate: (match field_10 {
                                    ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                open: (match field_11 {
                                    ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => {
                                        Some(item)
                                    }
                                    _ => None,
                                })?,
                                settled_height: (match field_12 {
                                    ::ducktape_view_guest::wire::SnapshotValue::I64(item) => {
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
            if name != "voting" {
                return None;
            }
            let voting: String = (match value {
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
            if name != "connected" {
                return None;
            }
            let connected: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
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
            if name != "answered" {
                return None;
            }
            let answered: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
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
            if name != "badge_sent" {
                return None;
            }
            let badge_sent: bool = (match value {
                ::ducktape_view_guest::wire::SnapshotValue::Bool(item) => Some(item),
                _ => None,
            })?;
            Some(Self::restore_state(
                active_palette,
                rows,
                voting,
                admin,
                connected,
                connection_serial,
                answered,
                host_error,
                badge_sent,
            ))
        })())
        .ok_or_else(|| String::from("snapshot state mismatch"))
    }
}
#[allow(unused_parens)]
impl GovernanceView {
    fn subscription(&self) -> ::ducktape_view_guest::Subscription<Message> {
        ::ducktape_view_guest::Subscription::batch([
            crate::host::session().map(move |value| Message::SessionArrived(value)),
            if self.connected {
                ::ducktape_view_guest::Subscription::batch([crate::host::register(
                    self.connection_serial,
                )
                .map(move |value| Message::RegisterArrived(value))])
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
                let (app, _) = GovernanceView::boot();
                let _ = app.view();
            })
            .unwrap()
            .join()
            .unwrap();
    }
}

impl GovernanceView {
    #[allow(clippy::assign_op_pattern)]
    pub(crate) fn update(&mut self, message: Message) -> ::ducktape_view_guest::Task<Message> {
        match message {
            Message::SessionArrived(item) => self.on_session_arrived(item),
            Message::RegisterArrived(item) => self.on_register_arrived(item),
            Message::ActDone(item) => self.on_act_done(item),
            Message::GovVote(proposal_id, approve) => self.on_gov_vote(proposal_id, approve),
            Message::GovExecute(proposal_id) => self.on_gov_execute(proposal_id),
        }
    }
    fn on_session_arrived(
        &mut self,
        item: crate::host::SessionItem,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            self.host_error = item.error.to_owned();
            if (!(item.error).is_empty()) {
                return ::ducktape_view_guest::Task::none();
            }
            let next = item.next.clone();
            self.connection_serial = crate::host::connection_serial_after(
                self.connected,
                next.connected,
                self.connection_serial,
            );
            self.admin = next.admin;
            self.connected = next.connected;
            self.active_palette = AppTheme::App;
            if (!next.dark) {
                return ::ducktape_view_guest::Task::none();
            }
            self.active_palette = AppTheme::AppDark;
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_register_arrived(
        &mut self,
        item: crate::host::RegisterItem,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            self.host_error = item.error.to_owned();
            self.answered = true;
            if (!(item.error).is_empty()) {
                return ::ducktape_view_guest::Task::none();
            }
            self.rows = item.rows.clone();
            self.badge_sent = (crate::host::badge(crate::host::open_proposals(
                ::std::convert::AsRef::as_ref(&(self.rows)),
            )));
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_act_done(&mut self, item: crate::host::ActItem) -> ::ducktape_view_guest::Task<Message> {
        {
            self.voting = "".to_owned();
            self.host_error = item.error.to_owned();
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_gov_vote(
        &mut self,
        proposal_id: String,
        approve: bool,
    ) -> ::ducktape_view_guest::Task<Message> {
        {
            if ((!self.connected) || (!(self.voting).is_empty())) {
                return ::ducktape_view_guest::Task::none();
            }
            self.voting = proposal_id.to_owned();
            let _sent = (crate::host::vote(proposal_id.to_owned(), approve));
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_gov_execute(&mut self, proposal_id: String) -> ::ducktape_view_guest::Task<Message> {
        {
            if ((!self.connected) || (!(self.voting).is_empty())) {
                return ::ducktape_view_guest::Task::none();
            }
            self.voting = proposal_id.to_owned();
            let _sent = (crate::host::execute(proposal_id.to_owned()));
            ::ducktape_view_guest::Task::none()
        }
    }
}

impl GovernanceView {
    pub(crate) fn view(&self) -> ducktape_view_guest::wire::Node {
        use ducktape_view_guest::{kit, wire};
        let mut content = vec![
            kit::heading("governance/title", "Approvals"),
            kit::text(
                "governance/summary",
                host::proposals_summary(self.connected, &self.rows),
            ),
        ];
        if !self.host_error.is_empty() {
            content.push(kit::text("governance/error", &self.host_error));
        }
        if !self.connected {
            content.push(kit::text("governance/disconnected", "Not connected"));
            content.push(kit::text(
                "governance/connect-help",
                "Click the network name in the titlebar to pick or reconnect a network.",
            ));
            return kit::scroll(
                "governance",
                kit::padded(
                    kit::column("governance/content", content),
                    wire::Edges::all(22.),
                ),
            );
        }
        if !self.admin {
            content.push(kit::text("governance/standing", "Approval votes are cast by this network's validators, and this node does not hold validator standing."));
            content.push(kit::text(
                "governance/read-help",
                "You can still read every proposal and follow its tally while it runs.",
            ));
        }
        let open = host::open_proposals(&self.rows);
        if open > 0 {
            content.push(kit::text(
                "governance/pending",
                host::pending_label(&self.rows),
            ));
        } else if self.answered {
            let message = if self.rows.is_empty() {
                "No proposals yet — a membership or configuration change opens the first one."
            } else {
                "No proposals waiting — every decision on this network is finalized."
            };
            content.push(kit::text("governance/empty", message));
        }
        for proposal in self.rows.iter().filter(|proposal| proposal.open) {
            content.push(self.proposal(proposal));
        }
        let settled = host::settled_proposals(&self.rows);
        if !settled.is_empty() {
            content.push(kit::heading("governance/finalized", "RECENTLY FINALIZED"));
            for proposal in settled {
                let key = format!("governance/settled/{}", proposal.id);
                let mut details = vec![
                    kit::text(format!("{key}/id"), &proposal.id),
                    kit::text(format!("{key}/action"), &proposal.action),
                    kit::text(format!("{key}/status"), &proposal.status),
                ];
                if proposal.settled_height > 0 {
                    details.push(kit::text(
                        format!("{key}/height"),
                        host::height_label_short(proposal.settled_height),
                    ));
                }
                content.push(kit::row(key, details));
            }
        }
        kit::scroll(
            "governance",
            kit::padded(
                kit::column("governance/content", content),
                wire::Edges::all(22.),
            ),
        )
    }

    fn proposal(&self, proposal: &host::ProposalRow) -> ducktape_view_guest::wire::Node {
        use ducktape_view_guest::{kit, slots, wire};
        let key = format!("governance/proposal/{}", proposal.id);
        let available = self.voting.is_empty();
        let mut content = vec![
            kit::heading(format!("{key}/id"), &proposal.id),
            kit::text(format!("{key}/action"), &proposal.action),
            kit::text(
                format!("{key}/proposer"),
                format!(
                    "proposed by @{} · expires at h{}",
                    proposal.proposer, proposal.deadline
                ),
            ),
        ];
        if !proposal.detail.is_empty() {
            content.push(kit::text(format!("{key}/detail"), &proposal.detail));
        }
        content.push(kit::text(
            format!("{key}/tally"),
            host::tally_label(proposal.approvals, proposal.required_yes),
        ));
        content.push(kit::text(
            format!("{key}/quorum"),
            host::tally_note(proposal.approvals, proposal.required_yes),
        ));
        if proposal.rejections > 0 {
            content.push(kit::text(
                format!("{key}/rejections"),
                format!("{} against", proposal.rejections),
            ));
        }
        let reject = kit::button(
            format!("{key}/reject"),
            "Reject",
            available.then(|| slots::message(Message::GovVote(proposal.id.clone(), false))),
            wire::ButtonPreset::Danger,
        );
        let mut approval = if proposal.approvals < proposal.required_yes {
            kit::button(
                format!("{key}/approve"),
                host::approve_label(proposal.approvals, proposal.required_yes),
                available.then(|| slots::message(Message::GovVote(proposal.id.clone(), true))),
                wire::ButtonPreset::Primary,
            )
        } else {
            kit::button(
                format!("{key}/settle"),
                "Settle →",
                available.then(|| slots::message(Message::GovExecute(proposal.id.clone()))),
                wire::ButtonPreset::Primary,
            )
        };
        if let wire::Node::Button { label, .. } = &mut approval {
            *label = Some(
                if proposal.approvals < proposal.required_yes {
                    "Approve"
                } else {
                    "Settle"
                }
                .into(),
            );
        }
        content.push(kit::row(format!("{key}/actions"), [reject, approval]));
        kit::padded(kit::column(key, content), wire::Edges::all(12.))
    }
}

ducktape_view_guest::export_app!(
    GovernanceView,
    "Approvals",
    "Every decision this network is being asked to make, and the ones it has settled.",
    ["governance"]
);
