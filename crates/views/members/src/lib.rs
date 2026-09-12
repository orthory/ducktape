//! The Members roster as a module-owned view: who may act on this network.
//!
//! The kernel pushes session facts only (`members.props`: connected, admin,
//! dark). The view reads the roster itself through the kernel's
//! `rpc.status` / `rpc.peers` / `rpc.query`, re-reads it on every `rpc.live`
//! hit for the valset plane, and a row opens its record. Pausing an agent
//! and opening a membership ballot leave as `op.submit` — the module
//! message the kernel signs with the seated key; copying a key stays an
//! intent, because the clipboard is an OS door the kernel has not opened.
//! The endpoint, the key and the password never cross: a guest that sees no
//! key cannot leak one.

pub mod host;

use serde::{Deserialize, Serialize};
use ui_lang_guest::{slots, wire};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MembersFilter { #[default] All, Humans, Agents, Validators }

#[derive(Serialize, Deserialize)]
pub struct MembersView {
    rows: Vec<host::MemberRow>,
    admin: bool,
    connected: bool,
    dark: bool,
    connection_serial: i64,
    answered: bool,
    host_error: String,
    filter: MembersFilter,
    selected: String,
    height: i64,
    acting: String,
    viewport_width: f64,
    member_width: f64,
}

#[derive(Clone)]
pub enum Message {
    Session(host::SessionItem),
    Roster(host::RosterItem),
    Act(host::ActItem),
    Filter(MembersFilter),
    Open(String),
    Copy(String, String),
    Resize(f64),
    Viewport(f32),
    Agent(String, bool),
    Ballot(String, String),
}

type __IceMessage = Message;

impl MembersView {
    const __PREFERRED_WINDOW_SIZE: &'static str = "";

    fn __boot() -> (Self, iced::Task<Message>) {
        (Self { rows: Vec::new(), admin: false, connected: false, dark: false,
            connection_serial: 0, answered: false, host_error: String::new(),
            filter: MembersFilter::All, selected: String::new(), height: 0,
            acting: String::new(), viewport_width: 1280.0, member_width: 312.0 }, iced::Task::none())
    }

    fn __snapshot(&self) -> Result<Vec<u8>, String> { serde_json::to_vec(self).map_err(|error| error.to_string()) }
    fn __restore(bytes: &[u8]) -> Result<Self, String> { serde_json::from_slice(bytes).map_err(|error| error.to_string()) }

    fn __subscription(&self) -> iced::Subscription<Message> {
        let mut subscriptions = vec![host::session().map(Message::Session), host::acts().map(Message::Act)];
        if self.connected { subscriptions.push(host::roster(self.connection_serial).map(Message::Roster)); }
        iced::Subscription::batch(subscriptions)
    }

    fn __update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::Session(item) => self.session(item),
            Message::Roster(item) => self.roster(item),
            Message::Act(item) => self.act(item),
            Message::Filter(filter) => self.filter(filter),
            Message::Open(key) => self.open(key),
            Message::Copy(key, label) => self.copy(key, label),
            Message::Resize(delta) => self.resize(delta),
            Message::Viewport(width) => self.viewport(width),
            Message::Agent(key, paused) => self.agent(key, paused),
            Message::Ballot(action, key) => self.ballot(action, key),
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

    fn roster(&mut self, item: host::RosterItem) -> iced::Task<Message> {
        self.host_error = item.error;
        self.answered = true;
        if !self.host_error.is_empty() { return iced::Task::none(); }
        self.rows = item.rows;
        self.height = item.height;
        iced::Task::none()
    }

    fn act(&mut self, item: host::ActItem) -> iced::Task<Message> {
        self.acting.clear();
        self.host_error = item.error;
        iced::Task::none()
    }
    fn filter(&mut self, filter: MembersFilter) -> iced::Task<Message> { self.filter = filter; iced::Task::none() }
    fn open(&mut self, key: String) -> iced::Task<Message> { self.selected = key; iced::Task::none() }
    fn copy(&mut self, key: String, label: String) -> iced::Task<Message> { host::copy(&key, &label); iced::Task::none() }
    fn resize(&mut self, delta: f64) -> iced::Task<Message> {
        self.member_width = host::member_width_after_delta(self.member_width, -delta, self.viewport_width);
        iced::Task::none()
    }
    fn viewport(&mut self, width: f32) -> iced::Task<Message> {
        self.viewport_width = f64::from(width);
        self.member_width = host::member_width_after_delta(self.member_width, 0.0, self.viewport_width);
        iced::Task::none()
    }
    fn agent(&mut self, key: String, paused: bool) -> iced::Task<Message> {
        let available = self.connected && self.acting.is_empty();
        if !available { return iced::Task::none(); }
        self.acting = key.clone();
        host::agent_status(&key, paused);
        iced::Task::none()
    }
    fn ballot(&mut self, action: String, key: String) -> iced::Task<Message> {
        let available = self.connected && self.admin && self.acting.is_empty();
        if !available { return iced::Task::none(); }
        self.acting = key.clone();
        host::propose(&action, &key, self.height);
        iced::Task::none()
    }

    fn __view(&self) -> wire::Node {
        let mut content = vec![text("title", "Members", 16.0), text("meta", host::members_summary(self.connected, &self.rows), 12.0)];
        if !self.host_error.is_empty() { content.push(text("host-error", &self.host_error, 12.0)); }
        if !self.connected {
            content.push(text("offline", "Not connected", 16.0));
            content.push(text("offline-hint", "Click the network name in the titlebar to pick or reconnect a network.", 12.5));
        }
        if self.connected {
            let filters = [(MembersFilter::All, "All", "Show every member"), (MembersFilter::Humans, "Humans", "Show people only"),
                (MembersFilter::Agents, "Agents", "Show agents only"), (MembersFilter::Validators, "Validators", "Show validators only")];
            let buttons = filters.into_iter().map(|(filter, title, label)| {
                let count = host::filter_members(&self.rows, filter).len();
                let mut button = button(&format!("filter/{title}"), label, Message::Filter(filter), true);
                if let wire::Node::Button { content, checked, .. } = &mut button {
                    *content = wire::ButtonContent::Label(format!("{title} {count}"));
                    *checked = Some(self.filter == filter);
                }
                button
            }).collect();
            content.push(linear("filters", wire::Axis::Row, buttons));
            let rows = host::filter_members(&self.rows, self.filter);
            let empty = rows.is_empty() && self.answered;
            if empty { content.push(text("empty", "No members here yet — validators, residents and registered agents appear as they join.", 13.0)); }
            let cards = rows.iter().map(|member| {
                let key = format!("row/{}", member.key);
                let mut card = button(&key, &member.label, Message::Open(member.key.clone()), true);
                let mut detail = vec![text(&format!("{key}/name"), &member.label, 13.5),
                    text(&format!("{key}/key"), &member.key, 10.5), text(&format!("{key}/role"), &member.role, 9.0)];
                if member.is_this_node { detail.push(text(&format!("{key}/self"), "this node", 9.5)); }
                if let wire::Node::Button { content, checked, .. } = &mut card {
                    *content = wire::ButtonContent::Child(Box::new(linear(&format!("{key}/details"), wire::Axis::Column, detail)));
                    *checked = Some(self.selected == member.key);
                }
                card
            }).collect();
            content.push(scroll("members-body", linear("roster", wire::Axis::Column, cards)));
        }
        let mut columns = vec![linear("list", wire::Axis::Column, content)];
        let selected = self.rows.iter().find(|member| member.key == self.selected).filter(|_| self.connected);
        if let Some(member) = selected {
            columns.push(wire::Node::ResizeHandle { key: "member-resize".into(), on_press: None, on_release: None,
                on_drag: Some(slots::handler(Box::new(|(dx, _): (f64, f64)| Some(Message::Resize(dx))))),
                cursor: None, content: Box::new(wire::Node::Space { width: Some(wire::Length::Fixed(10.0)), height: Some(wire::Length::Fill) }) });
            columns.push(self.member(member));
        }
        let size = slots::handler(Box::new(|(width, _): (f32, f32)| Some(Message::Viewport(width))));
        wire::Node::Sensor { key: "root".into(), reset: None, on_show: Some(size), on_resize: Some(size),
            on_hide: None, anticipate: None, delay: None, child: Box::new(linear("columns", wire::Axis::Row, columns)) }
    }

    fn member(&self, member: &host::MemberRow) -> wire::Node {
        let status = match (member.is_agent, member.live) { (true, true) => "active", (true, false) => "paused", (false, true) => "live", (false, false) => "offline" };
        let key_label = match member.is_agent { true => "agent id", false => "public key" };
        let mut content = vec![button("member/close", "Close member", Message::Open(String::new()), true),
            text("member/name", &member.label, 16.0), text("member/status", status, 12.0),
            text("member/role", &member.role, 9.0), text("member/key", format!("{key_label}: {}", member.key), 11.0)];
        if !member.model.is_empty() { content.push(text("member/capability", format!("capability: {}", member.model), 11.0)); }
        if member.is_this_node { content.push(button("member/copy", "Copy this node's key", Message::Copy(member.key.clone(), "Node key copied".into()), true)); }
        let available = self.acting.is_empty();
        if member.is_agent {
            let label = match member.live { true => "Pause agent", false => "Resume agent" };
            content.push(button("member/agent", label, Message::Agent(member.key.clone(), member.live), available));
            content.push(text("member/owner-gate", "Pause and resume are owner-gated writes. The model accepts changes from its program account or current controller.", 12.0));
        }
        let promote = self.admin && !member.is_agent && member.role == "resident";
        if promote { content.push(button("member/promote", "Promote to validator", Message::Ballot("add_validator".into(), member.key.clone()), available)); }
        let remove = self.admin && !member.is_agent && member.role == "validator" && !member.is_this_node;
        if remove { content.push(button("member/remove", "Remove from the validator set", Message::Ballot("remove_validator".into(), member.key.clone()), available)); }
        let ballot = promote || remove;
        if ballot { content.push(text("member/quorum", "needs quorum", 9.0)); }
        let refused = !self.admin && !member.is_this_node && !member.is_agent;
        if refused { content.push(text("member/gate", "Only a validator node may open a membership proposal. This node holds no quorum seat, so the network refuses the write.", 12.0)); }
        let mut panel = linear("member", wire::Axis::Column, vec![scroll("member-body", linear("member-content", wire::Axis::Column, content))]);
        if let wire::Node::Linear { width, .. } = &mut panel { *width = Some(wire::Length::Fixed(self.member_width as f32)); }
        panel
    }
}

fn text(key: &str, content: impl Into<String>, size: f32) -> wire::Node {
    wire::Node::Text { key: key.into(), content: content.into(), size: Some(size), color: None,
        font: wire::Font::default(), width: None, align_x: None, options: Default::default() }
}
fn linear(key: &str, axis: wire::Axis, children: Vec<wire::Node>) -> wire::Node {
    wire::Node::Linear { key: key.into(), axis, children, max_width: None, clip: false, wrap: None,
        spacing: Some(12.0), padding: Some(wire::Edges::all(16.0)), width: Some(wire::Length::Fill),
        height: Some(wire::Length::Fill), align: None, background: None, border: None }
}
fn button(key: &str, label: &str, message: Message, enabled: bool) -> wire::Node {
    wire::Node::Button { key: key.into(), content: wire::ButtonContent::Label(label.into()), label: Some(label.into()),
        checked: None, expanded: None, description: None, on_press: enabled.then(|| slots::message(message)),
        width: None, height: None, padding: Some(wire::Edges::all(8.0)), style: Default::default() }
}
fn scroll(key: &str, content: wire::Node) -> wire::Node {
    wire::Node::Scroll { key: key.into(), on_scroll: None, virtual_rows: false,
        direction: wire::ScrollDirection::Vertical, width: Some(wire::Length::Fill), height: Some(wire::Length::Fill),
        bar_hidden: false, bar_width: None, bar_margin: None, scroller_width: None, bar_spacing: None,
        anchor_x: wire::ScrollAnchor::Start, anchor_y: wire::ScrollAnchor::Start, auto_scroll: false,
        background: None, border: None, content: Box::new(content) }
}

ui_lang_guest::export_app!(
    MembersView,
    "Members",
    "Who may act on this network: validators, residents and registered agents.",
    ["members"]
);
