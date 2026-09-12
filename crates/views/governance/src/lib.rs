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

use ui_lang_guest::{slots, wire};
use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
pub struct GovernanceView {
    rows: Vec<host::ProposalRow>,
    voting: String,
    admin: bool,
    connected: bool,
    dark: bool,
    connection_serial: i64,
    answered: bool,
    host_error: String,
}

#[derive(Clone)]
pub enum Message {
    Session(host::SessionItem),
    Register(host::RegisterItem),
    Act(host::ActItem),
    Vote(String, bool),
    Execute(String),
}

type __IceMessage = Message;

impl GovernanceView {
    const __PREFERRED_WINDOW_SIZE: &'static str = "";

    fn __boot() -> (Self, iced::Task<Message>) { (Self::default(), iced::Task::none()) }
    fn __snapshot(&self) -> Result<Vec<u8>, String> { serde_json::to_vec(self).map_err(|error| error.to_string()) }
    fn __restore(bytes: &[u8]) -> Result<Self, String> { serde_json::from_slice(bytes).map_err(|error| error.to_string()) }

    fn __subscription(&self) -> iced::Subscription<Message> {
        let mut subscriptions = vec![host::session().map(Message::Session), host::acts().map(Message::Act)];
        if self.connected { subscriptions.push(host::register(self.connection_serial).map(Message::Register)); }
        iced::Subscription::batch(subscriptions)
    }

    fn __update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::Session(item) => self.session(item),
            Message::Register(item) => self.register(item),
            Message::Act(item) => self.act(item),
            Message::Vote(id, approve) => self.vote(id, approve),
            Message::Execute(id) => self.execute(id),
        }
    }

    fn session(&mut self, item: host::SessionItem) -> iced::Task<Message> {
        self.host_error = item.error;
        if !self.host_error.is_empty() { return iced::Task::none(); }
        self.connection_serial = host::connection_serial_after(self.connected, item.next.connected, self.connection_serial);
        self.connected = item.next.connected;
        self.admin = item.next.admin;
        self.dark = item.next.dark;
        iced::Task::none()
    }

    fn register(&mut self, item: host::RegisterItem) -> iced::Task<Message> {
        self.host_error = item.error;
        self.answered = true;
        if !self.host_error.is_empty() { return iced::Task::none(); }
        self.rows = item.rows;
        host::badge(host::open_proposals(&self.rows));
        iced::Task::none()
    }

    fn act(&mut self, item: host::ActItem) -> iced::Task<Message> {
        self.voting.clear();
        self.host_error = item.error;
        iced::Task::none()
    }

    fn vote(&mut self, id: String, approve: bool) -> iced::Task<Message> {
        let available = self.connected && self.voting.is_empty();
        if !available { return iced::Task::none(); }
        self.voting = id.clone();
        host::vote(id, approve);
        iced::Task::none()
    }

    fn execute(&mut self, id: String) -> iced::Task<Message> {
        let available = self.connected && self.voting.is_empty();
        if !available { return iced::Task::none(); }
        self.voting = id.clone();
        host::execute(id);
        iced::Task::none()
    }

    fn __view(&self) -> wire::Node {
        let mut heading = vec![text("title", "Approvals", 16.0), text("meta", host::proposals_summary(self.connected, &self.rows), 12.0)];
        let pending = self.connected && host::open_proposals(&self.rows) > 0;
        if pending { heading.push(text("pending", host::pending_label(&self.rows), 11.0)); }
        let mut body = Vec::new();
        if !self.host_error.is_empty() { body.push(text("host-error", &self.host_error, 12.0)); }
        let reader = self.connected && !self.admin;
        if reader {
            body.push(text("gate", "Approval votes are cast by this network's validators, and this node does not hold validator standing. You can still read every proposal and follow its tally while it runs.", 12.0));
        }
        if !self.connected {
            body.push(text("offline", "Not connected", 16.0));
            body.push(text("offline-hint", "Click the network name in the titlebar to pick or reconnect a network.", 12.5));
        }
        let empty = self.connected && self.rows.is_empty() && self.answered;
        if empty { body.push(text("empty", "No proposals yet — a membership or configuration change opens the first one.", 13.0)); }
        let settled = self.connected && !self.rows.is_empty() && !pending && self.answered;
        if settled { body.push(text("all-settled", "No proposals waiting — every decision on this network is finalized.", 13.0)); }
        if self.connected {
            for proposal in self.rows.iter().filter(|proposal| proposal.open) {
                body.push(self.proposal(proposal));
            }
            let finalized = host::settled_proposals(&self.rows);
            if !finalized.is_empty() {
                body.push(text("settled-heading", "RECENTLY FINALIZED", 9.0));
                for proposal in finalized {
                    let mut label = format!("✓ {} · {} · {}", proposal.id, proposal.status, host::tally_label(proposal.approvals, proposal.required_yes));
                    if proposal.settled_height > 0 { label.push_str(&format!(" · {}", host::height_label_short(proposal.settled_height))); }
                    body.push(text(&format!("settled/{}", proposal.id), label, 13.0));
                }
            }
        }
        linear("root", wire::Axis::Column, vec![linear("header", wire::Axis::Row, heading),
            wire::Node::Scroll {
                key: "approvals-body".into(), on_scroll: None, virtual_rows: false,
                direction: wire::ScrollDirection::Vertical, width: Some(wire::Length::Fill), height: Some(wire::Length::Fill),
                bar_hidden: false, bar_width: None, bar_margin: None, scroller_width: None, bar_spacing: None,
                anchor_x: wire::ScrollAnchor::Start, anchor_y: wire::ScrollAnchor::Start, auto_scroll: false,
                background: None, border: None, content: Box::new(linear("body", wire::Axis::Column, body)),
            }])
    }

    fn proposal(&self, proposal: &host::ProposalRow) -> wire::Node {
        let key = format!("proposal/{}", proposal.id);
        let mut children = vec![text(&format!("{key}/kind"), &proposal.action, 9.0),
            text(&format!("{key}/id"), &proposal.id, 14.0),
            text(&format!("{key}/detail"), format!("proposed by @{} · expires at h {} · {}", proposal.proposer, proposal.deadline, proposal.detail), 12.0),
            text(&format!("{key}/tally"), format!("{} · {}", host::tally_label(proposal.approvals, proposal.required_yes), host::tally_note(proposal.approvals, proposal.required_yes)), 12.0)];
        if proposal.rejections > 0 { children.push(text(&format!("{key}/against"), format!("{} against", proposal.rejections), 12.0)); }
        let available = self.voting.is_empty();
        let mut actions = vec![button(&format!("{key}/reject"), "Reject", Message::Vote(proposal.id.clone(), false), available)];
        let quorum = proposal.approvals >= proposal.required_yes;
        actions.push(match quorum {
            true => button(&format!("{key}/settle"), "Settle", Message::Execute(proposal.id.clone()), available),
            false => button(&format!("{key}/approve"), "Approve", Message::Vote(proposal.id.clone(), true), available),
        });
        children.push(linear(&format!("{key}/actions"), wire::Axis::Row, actions));
        linear(&key, wire::Axis::Column, children)
    }
}

fn text(key: &str, content: impl Into<String>, size: f32) -> wire::Node {
    wire::Node::Text { key: key.into(), content: content.into(), size: Some(size), color: None,
        font: wire::Font::default(), width: None, align_x: None, options: Default::default() }
}

fn linear(key: &str, axis: wire::Axis, children: Vec<wire::Node>) -> wire::Node {
    wire::Node::Linear { key: key.into(), axis, children, max_width: None, clip: false, wrap: None,
        spacing: Some(12.0), padding: Some(wire::Edges::all(16.0)), width: Some(wire::Length::Fill),
        height: None, align: None, background: None, border: None }
}

fn button(key: &str, label: &str, message: Message, enabled: bool) -> wire::Node {
    wire::Node::Button { key: key.into(), content: wire::ButtonContent::Label(label.into()), label: Some(label.into()),
        checked: None, expanded: None, description: None, on_press: enabled.then(|| slots::message(message)),
        width: None, height: None, padding: Some(wire::Edges::all(8.0)), style: Default::default() }
}

ui_lang_guest::export_app!(
    GovernanceView,
    "Approvals",
    "Every decision this network is being asked to make, and the ones it has settled.",
    ["governance"]
);
