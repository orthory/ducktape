//! The Approvals screen as a module-owned view: every decision the network
//! is being asked to make, and the ones it has settled, rendered from a
//! wasm component the desktop app loads from a file.
//!
//! The kernel pushes session facts only (`governance.props`: connected,
//! admin, dark). The view reads its own register through the kernel's
//! `rpc.query` / `rpc.blocks`, re-reads it on every `rpc.live` hit for the
//! governance plane, and a vote or a settle leaves as `op.submit` — the
//! governance message the kernel signs with the seated key. Signing secrets
//! and passwords stay in the host; public proposal data belongs to the guest.
pub mod host;
#[derive(serde::Serialize, serde::Deserialize)]
pub struct GovernanceView {
    pub(crate) rows: Vec<crate::host::ProposalRow>,
    pub(crate) voting: String,
    pub(crate) admin: bool,
    pub(crate) connected: bool,
    pub(crate) connection_serial: i64,
    pub(crate) answered: bool,
    pub(crate) host_error: String,
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
impl GovernanceView {
    fn state() -> Self {
        Self {
            rows: Vec::new(),
            voting: "".to_owned(),
            admin: false,
            connected: false,
            connection_serial: 0,
            answered: false,
            host_error: "".to_owned(),
        }
    }
    pub(crate) fn boot() -> (Self, ::ducktape_view_guest::Task<Message>) {
        (Self::state(), ::ducktape_view_guest::Task::none())
    }
    pub(crate) const PREFERRED_WINDOW_SIZE: &'static str = "none";
    pub(crate) const SNAPSHOT_SCHEMA: &'static str =
        "7c12db27b05b027805b40f4d493f95bcbf83f7b71fb9a350d90ef241043cbc72";
}
impl GovernanceView {
    pub(crate) fn snapshot(&self) -> Result<Vec<u8>, String> {
        use ducktape_view_guest::wire;
        wire::Snapshot {
            schema: Self::SNAPSHOT_SCHEMA.into(),
            state: wire::SnapshotValue::Bytes(wire::encode(self)),
        }
        .encode()
    }
    pub(crate) fn restore(bytes: &[u8]) -> Result<Self, String> {
        use ducktape_view_guest::wire;
        let snapshot = wire::Snapshot::decode(bytes)?;
        if snapshot.schema != Self::SNAPSHOT_SCHEMA {
            return Err("invalid Governance snapshot schema".into());
        }
        let wire::SnapshotValue::Bytes(state) = snapshot.state else {
            return Err("invalid Governance snapshot".into());
        };
        wire::decode(&state)
    }
    fn subscription(&self) -> ::ducktape_view_guest::Subscription<Message> {
        ::ducktape_view_guest::Subscription::batch([
            crate::host::session().map(Message::SessionArrived),
            if self.connected {
                ::ducktape_view_guest::Subscription::batch([crate::host::register(
                    self.connection_serial,
                )
                .map(Message::RegisterArrived)])
            } else {
                ::ducktape_view_guest::Subscription::none()
            },
            crate::host::acts().map(Message::ActDone),
        ])
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disconnected_view_hides_retained_proposals_and_actions() {
        let (mut view, _) = GovernanceView::boot();
        view.rows.push(host::ProposalRow {
            id: "stale-proposal".into(),
            open: true,
            ..Default::default()
        });
        let mut tree = view.view();
        tree.for_each_mut(&mut |node| {
            assert!(!node.key().is_some_and(|key| key.contains("stale-proposal")));
            assert!(!matches!(
                node,
                ducktape_view_guest::wire::Node::Button {
                    on_press: Some(_),
                    ..
                }
            ));
        });
        assert_eq!(
            view.rows.len(),
            1,
            "disconnect does not discard snapshot state"
        );
    }
    #[test]
    fn snapshot_preserves_proposals_without_theme_bookkeeping() {
        let (mut view, _) = GovernanceView::boot();
        view.rows.push(host::ProposalRow {
            id: "proposal".into(),
            approvals: 2,
            required_yes: 3,
            open: true,
            ..Default::default()
        });
        view.connected = true;
        let bytes = view.snapshot().unwrap();
        let restored = GovernanceView::restore(&bytes).unwrap();
        assert_eq!(restored.rows, view.rows);
        assert_eq!(restored.snapshot().unwrap(), bytes);
    }
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
            if !(item.error).is_empty() {
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
            if !(item.error).is_empty() {
                return ::ducktape_view_guest::Task::none();
            }
            self.rows = item.rows.clone();
            crate::host::badge(crate::host::open_proposals(&self.rows));
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
            if (!self.connected) || (!(self.voting).is_empty()) {
                return ::ducktape_view_guest::Task::none();
            }
            self.voting = proposal_id.to_owned();
            let _sent = crate::host::vote(proposal_id.to_owned(), approve);
            ::ducktape_view_guest::Task::none()
        }
    }
    fn on_gov_execute(&mut self, proposal_id: String) -> ::ducktape_view_guest::Task<Message> {
        {
            if (!self.connected) || (!(self.voting).is_empty()) {
                return ::ducktape_view_guest::Task::none();
            }
            self.voting = proposal_id.to_owned();
            let _sent = crate::host::execute(proposal_id.to_owned());
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
            content
                .push(
                    kit::text(
                        "governance/standing",
                        "Approval votes are cast by this network's validators, and this node does not hold validator standing.",
                    ),
                );
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
