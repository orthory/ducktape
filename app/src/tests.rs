//! App state regressions and native UI/kernel contracts.
use super::*;
mod bell;
mod canary;
mod connection;
mod design;
mod font_fallback;
mod huddle_live;
mod messages;
mod rooms;
mod sends;
mod settings;
mod shell;
mod stream;
mod window_lifecycle;
use crate::composer_surface::testing as composer;
const RAIL_THREAD_SEQ: i64 = 7;
fn message(seq: i64, body: &str, deleted: bool) -> backend::ChatMessage {
    backend::ChatMessage {
        id: format!("message-{seq}"),
        view_key: seq,
        seq,
        author: "user".into(),
        meta: format!("#{seq}"),
        edit_body: body.into(),
        body: body.into(),
        blocks: backend::paragraph_blocks(body),
        pending: false,
        rev: 2,
        edited: false,
        deleted,
        reply_count: 0,
        thread_seq: 0,
        show_author: true,
        initial: "U".into(),
        avatar_kind: "human".into(),
        height: 0,
        time: 0,
        reactions: Vec::new(),
        render_rev: 0,
    }
}

fn composer_scope(app: &Ducktape) -> String {
    backend::composer_scope(&app.connected_rpc, &app.active_channel)
}

fn reply_composer_scope(app: &Ducktape, thread_seq: i64) -> String {
    backend::thread_scope(&app.connected_rpc, &app.active_channel, thread_seq)
}

fn submit(app: &mut Ducktape, kind: ComposerKind, body: &str) -> String {
    let id = backend::fresh_operation_id(backend::composer_op_prefix(kind));
    let scope = match kind {
        ComposerKind::Message => composer_scope(app),
        ComposerKind::Reply => reply_composer_scope(app, RAIL_THREAD_SEQ),
        ComposerKind::Edit | ComposerKind::ThreadEdit => {
            backend::edit_scope(&app.connected_rpc, &app.active_channel, app.chat_edit_seq)
        }
    };
    let _ = app.__update(__DucktapeMessage::ComposerSubmitted(
        kind,
        body.to_owned(),
        id.clone(),
        scope,
    ));
    id
}

fn composer_intent(scope: &str, kind: &str, body: &str) -> module_view::ModuleViewEvent {
    module_view::ModuleViewEvent {
        kind: "composer".into(),
        detail: serde_json::json!({
            "scope": scope,
            "kind": kind,
            "body": body,
            "id": backend::fresh_operation_id(kind.to_owned()),
        })
        .to_string(),
    }
}

fn composer_text(scope: &str) -> String {
    composer::text(scope).trim().to_owned()
}

fn composer_stash(scope: &str) -> String {
    composer::failed(scope)
}

fn live_refresh(generation: i64, active_channel: &str) -> backend::LiveRefresh {
    backend::LiveRefresh {
        generation,
        chat_loaded: true,
        channels: Vec::new(),
        active_channel: active_channel.into(),
        active_channel_name: active_channel.into(),
        active_channel_archived: false,
        active_channel_members_only: false,
        huddle_roster: Vec::new(),
        channel_members: Vec::new(),
    }
}

fn posted_delta(channel: &str, row: backend::ChatMessage) -> backend::LiveUpdate {
    backend::LiveUpdate {
        kind: LiveKind::Chat,
        status: "Live".into(),
        height: row.seq.max(1),
        chat: vec![backend::ChatDelta::Posted {
            channel_id: channel.into(),
            seq: row.seq,
            message: row,
        }],
        ..backend::LiveUpdate::default()
    }
}

fn chat_data(active_channel: &str) -> backend::ChatData {
    backend::ChatData {
        generation: 0,
        channels: Vec::new(),
        active_channel: active_channel.into(),
        active_channel_name: active_channel.into(),
        active_channel_archived: false,
        active_channel_members_only: false,
        huddle_roster: Vec::new(),
        channel_members: Vec::new(),
    }
}

fn stale_chat_hit() -> backend::ChatSearchHit {
    backend::ChatSearchHit {
        channel_id: "old".into(),
        seq: 1,
        root_seq: 1,
        author: "user".into(),
        text: "stale".into(),
        meta: "#1".into(),
    }
}

fn stale_page_hit() -> backend::PageSearchHit {
    backend::PageSearchHit {
        page_id: "old".into(),
        page_title: "Old".into(),
        block_id: "old-block".into(),
        kind: "Text".into(),
        text: "stale".into(),
    }
}

fn workspace(active_channel: &str) -> backend::WorkspaceData {
    backend::WorkspaceData {
        generation: 0,
        rpc: "http://node".into(),
        status: "current".into(),
        height: 1,
        channels: Vec::new(),
        active_channel: active_channel.into(),
        active_channel_name: active_channel.into(),
        active_channel_archived: false,
        active_channel_members_only: false,
        huddle_roster: Vec::new(),
        channel_members: Vec::new(),
    }
}

fn room(id: &str, head: i64) -> backend::ChatChannel {
    backend::ChatChannel {
        id: id.into(),
        name: id.into(),
        archived: false,
        members_only: false,
        huddle_count: 0,
        head_seq: head,
    }
}

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

fn code_of(line: &str) -> &str {
    line.split("//").next().unwrap_or_default()
}

fn calls(code: &str, name: &str) -> bool {
    let mut rest = code;
    while let Some(at) = rest.find(name) {
        let before = rest[..at].chars().next_back();
        let after = rest[at + name.len()..].chars().next();
        let bounded = !before.is_some_and(|c| c.is_alphanumeric() || c == '_');
        if bounded && after == Some('(') {
            return true;
        }
        rest = &rest[at + name.len()..];
    }
    false
}
fn type_into(scope: &str, text: &str) {
    composer::append(scope, text);
}
fn seed_composer(scope: &str, text: &str) {
    composer::replace(scope, text);
}
fn restore_composer(scope: &str, blocked: bool) {
    composer::restore(scope, blocked);
}
fn submit_composer(app: &mut Ducktape, scope: &str, kind: ComposerKind, blocked: bool) {
    let Some(value) = composer::submit(scope, &backend::composer_op_prefix(kind), blocked) else {
        return;
    };
    let event = composer_surface::intent(&value).expect("native composer submit intent");
    let task = app.__update(__DucktapeMessage::ChatViewEvent(event));
    pump(app, task);
}
/// Drain only this task's messages. Follow-up I/O belongs to the caller's fixture.
fn pump(app: &mut Ducktape, task: ducktape_view_guest::Task<__DucktapeMessage>) {
    use futures::StreamExt;
    let messages: Vec<_> = futures::executor::block_on(task.into_stream().collect());
    for message in messages {
        let _ = app.__update(message);
    }
}
fn command_chord(key: &str) -> crate::shell::KeyPress {
    crate::shell::KeyPress {
        key: key.into(),
        modifiers: gpui_kit::Modifiers {
            platform: cfg!(target_os = "macos"),
            control: !cfg!(target_os = "macos"),
            ..Default::default()
        },
    }
}

/// Parse Rust tokens so formatting and comments cannot satisfy a source rule.
pub(crate) fn rust_tokens(source: &str) -> String {
    use quote::ToTokens;
    syn::parse_file(source)
        .expect("valid Rust source")
        .to_token_stream()
        .to_string()
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}
pub(crate) fn handler_bodies() -> Vec<(String, String)> {
    use quote::ToTokens;
    use syn::visit::Visit;
    struct Handlers(Vec<(String, String)>);
    impl<'ast> Visit<'ast> for Handlers {
        fn visit_arm(&mut self, arm: &'ast syn::Arm) {
            let path = match &arm.pat {
                syn::Pat::Path(path) => Some(&path.path),
                syn::Pat::TupleStruct(tuple) => Some(&tuple.path),
                _ => None,
            };
            if let Some(path) = path {
                let parts: Vec<_> = path
                    .segments
                    .iter()
                    .map(|part| part.ident.to_string())
                    .collect();
                if parts.len() == 2 && parts[0] == "__DucktapeMessage" {
                    self.0.push((
                        parts[1].clone(),
                        arm.body
                            .to_token_stream()
                            .to_string()
                            .chars()
                            .filter(|character| !character.is_whitespace())
                            .collect(),
                    ));
                }
            }
            syn::visit::visit_arm(self, arm);
        }
    }
    let source = syn::parse_file(include_str!("ui/app_update.rs")).expect("native update Rust");
    let mut found = Handlers(Vec::new());
    found.visit_file(&source);
    assert!(!found.0.is_empty(), "real native handlers are present");
    found.0
}
pub(crate) fn handler_body(variant: &str) -> String {
    let mut found = handler_bodies()
        .into_iter()
        .filter(|(name, _)| name == variant);
    let (_, body) = found
        .next()
        .unwrap_or_else(|| panic!("missing native handler {variant}"));
    assert!(found.next().is_none(), "one handler per message variant");
    body
}
fn inlined(source: &str) -> String {
    source.split_whitespace().collect::<Vec<_>>().join(" ")
}
