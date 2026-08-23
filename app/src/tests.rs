// The app's update-loop and source-sweep suite. `super` is the crate
// root, so the generated `Ducktape` app and both native modules resolve
// exactly as they did when this mod lived inline in main.rs.
use super::*;

mod connection;
mod design;
mod forge;
mod huddle_live;
mod messages;
mod page_autosave_gate;
mod pages;
mod rooms;
mod sends;
mod shell;
mod stream;
mod threads;

/// EVERY SCREEN BODY, as one string. These are the slot bodies that used to
/// sit inline in `view.ice`; the sweeps below read the console's authored
/// markup, so they must read where that markup now lives. `view.ice` keeps
/// only the mounts, and asserting a widget shape against it now would pass
/// vacuously — the worst kind of green.
static SCREENS: std::sync::LazyLock<String> =
    std::sync::LazyLock::new(|| inlined(&ice_sources_in("screens")));

/// Fold `with` blocks back onto their node line, so the source sweeps keep
/// pinning a node and its props as ONE readable line no matter how
/// `cargo ice fmt` wrapped it — and so `!contains` sweeps stay falsifiable
/// instead of passing vacuously against wrapped text. Props keep source
/// order; a trailing `-> route` stays last.
fn inlined(source: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut lines = source.lines().peekable();
    while let Some(line) = lines.next() {
        let indent = line.len() - line.trim_start().len();
        if line.trim() == "with" && !out.is_empty() {
            let mut props = Vec::new();
            while let Some(next) = lines.peek() {
                let deeper = next.len() - next.trim_start().len() > indent;
                if next.trim().is_empty() || !deeper {
                    break;
                }
                props.push(next.trim().to_owned());
                lines.next();
            }
            let node = out.pop().expect("with follows its node line");
            let props = props.join(" ");
            out.push(match node.split_once(" -> ") {
                Some((head, route)) => format!("{head} {props} -> {route}"),
                None => format!("{node} {props}"),
            });
            continue;
        }
        out.push(line.to_owned());
    }
    out.join("\n")
}

/// Every authored `.ice` file, walked rather than listed — a hardcoded list is
/// a rule with its own escape hatch, since the next screen added is the one the
/// sweep never sees.
fn ice_sources() -> Vec<(String, String)> {
    fn walk(dir: &std::path::Path, out: &mut Vec<(String, String)>) {
        let entries = std::fs::read_dir(dir).expect("the ui tree is readable");
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|kind| kind == "ice") {
                let source = std::fs::read_to_string(&path).expect("an .ice file reads");
                out.push((path.display().to_string(), source));
            }
        }
    }
    let mut out = Vec::new();
    walk(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui"),
        &mut out,
    );
    out
}

/// Every `on <name>` handler in one `.ice` source, as (name, body). A handler
/// body runs from its header to the next line at or above its own indent, so a
/// slice taken this way cannot absorb the handler after it — which is what
/// keeps a `contains`/`!contains` assertion over one falsifiable. App handlers
/// sit at column 0 and a component's at column 2; both are found the same way.
fn ice_handlers(source: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut current: Option<(String, usize, Vec<&str>)> = None;
    for line in source.lines() {
        let indent = line.len() - line.trim_start().len();
        if let Some((_, header_indent, body)) = &mut current
            && (line.trim().is_empty() || indent > *header_indent)
        {
            body.push(line);
            continue;
        }
        if let Some((name, _, body)) = current.take() {
            out.push((name, body.join("\n")));
        }
        let Some(rest) = line.trim_start().strip_prefix("on ") else {
            continue;
        };
        let name = rest.split(['(', ' ']).next().unwrap_or_default().to_owned();
        current = Some((name, indent, Vec::new()));
    }
    if let Some((name, _, body)) = current {
        out.push((name, body.join("\n")));
    }
    out
}

fn ice_handler_body(source: &str, handler: &str) -> String {
    ice_handlers(source)
        .into_iter()
        .find(|(name, _)| name == handler)
        .unwrap_or_else(|| panic!("`on {handler}` is a handler in this source"))
        .1
}

fn ice_sources_in(directory: &str) -> String {
    let suffix = std::path::Path::new("src/ui").join(directory);
    ice_sources()
        .into_iter()
        .filter(|(path, _)| {
            std::path::Path::new(path)
                .parent()
                .is_some_and(|parent| parent.ends_with(&suffix))
        })
        .map(|(_, source)| source)
        .collect::<Vec<_>>()
        .join("\n")
}

fn message(seq: i64, body: &str, deleted: bool) -> backend::ChatMessage {
    backend::ChatMessage {
        id: format!("message-{seq}"),
        view_key: seq,
        seq,
        author: "user".into(),
        meta: format!("#{seq}"),
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

/// THE COMPOSERS ARE COMPONENT INSTANCES NOW (ducktape-ui#697), so a harness
/// reaches them the way any harness does: render once to materialize the
/// instances, then read and drive them through the generated test seam
/// (ducktape-ui#696 layer 1). The scope is the rendered instance path; the
/// two composers are told apart by the id segment their mount declares.
fn materialize_composers(app: &mut Ducktape) {
    let window = app.console_win.unwrap_or_else(iced::window::Id::unique);
    app.console_win = Some(window);
    app.shell_tab = ShellTab::Chat;
    let _ = app.__view(window);
    let boots: Vec<__DucktapeMessage> = app.__ice_boot_queue.borrow_mut().drain(..).collect();
    for message in boots {
        let _ = app.__update(message);
    }
}

/// The instance whose scope names BOTH the mount and this key. Retained
/// storage keeps every instance the app has ever rendered, which is the whole
/// promise — so a scope lookup has to say which room it means, exactly as the
/// mount does.
fn composer_scope_named(app: &Ducktape, mount: &str, key: &str) -> Option<String> {
    // THIS APP'S OWN WINDOW, and no other's. The sighting side-channel a
    // freshly rendered instance is found through is a THREAD-local, so a
    // sibling test that rendered the same room on the same test thread has a
    // scope with the same mount and the same key — differing only in the
    // window the render was for. Reading that one back finds no state and the
    // assertion fails in a full run while passing alone.
    let window = format!("/{:?}/", app.console_win?);
    app.__ice_test_scopes_chat_composer()
        .into_iter()
        .find(|scope| scope.contains(&window) && scope.contains(mount) && scope.contains(key))
}

/// The stream composer of the room the app is in, materializing it if needed.
fn composer_scope(app: &mut Ducktape) -> String {
    materialize_composers(app);
    let key = backend::composer_scope(app.connected_rpc.clone(), app.active_channel.clone());
    composer_scope_named(app, "/composer(", &key)
        .unwrap_or_else(|| panic!("the composer for `{}` materialized", app.active_channel))
}

/// The rail composer of the thread the app is in, materializing it if needed.
fn reply_composer_scope(app: &mut Ducktape) -> String {
    materialize_composers(app);
    let key = backend::thread_scope(
        app.connected_rpc.clone(),
        app.active_channel.clone(),
        app.active_thread_seq,
    );
    composer_scope_named(app, "/reply_composer(", &key).unwrap_or_else(|| {
        panic!(
            "the reply composer for thread {} materialized",
            app.active_thread_seq
        )
    })
}

/// Types `text` into one composer instance, one character at a time — the
/// same route a real keystroke takes through the rich composer.
fn type_into(app: &mut Ducktape, scope: &str, kind: ComposerKind, text: &str) {
    for character in text.chars() {
        let message = Ducktape::__ice_test_message_chat_composer_composer_event(
            scope.to_owned(),
            editor::ComposerEvent::Apply(editor::RichAction::Edit(
                iced::widget::text_editor::Action::Edit(iced::widget::text_editor::Edit::Insert(
                    character,
                )),
            )),
            false,
            kind,
        );
        let task = app.__update(message);
        pump(app, task);
    }
}

/// Replaces one composer instance's whole content.
fn seed_composer(app: &mut Ducktape, scope: &str, kind: ComposerKind, text: &str) {
    let clear = Ducktape::__ice_test_message_chat_composer_composer_event(
        scope.to_owned(),
        editor::ComposerEvent::Apply(editor::RichAction::Edit(
            iced::widget::text_editor::Action::SelectAll,
        )),
        false,
        kind,
    );
    let _ = app.__update(clear);
    let cut = Ducktape::__ice_test_message_chat_composer_composer_event(
        scope.to_owned(),
        editor::ComposerEvent::Apply(editor::RichAction::Edit(
            iced::widget::text_editor::Action::Edit(iced::widget::text_editor::Edit::Delete),
        )),
        false,
        kind,
    );
    let _ = app.__update(cut);
    type_into(app, scope, kind, text);
}

/// Submits one composer instance, the way plain Enter and the Send button do.
fn submit_composer(app: &mut Ducktape, scope: &str, kind: ComposerKind, blocked: bool) {
    let message = Ducktape::__ice_test_message_chat_composer_composer_event(
        scope.to_owned(),
        editor::composer_submit_event(),
        blocked,
        kind,
    );
    let task = app.__update(message);
    pump(app, task);
}

/// DELIVERS WHAT A HANDLER'S TASK PUBLISHES back into the loop. A component
/// handler's `emit` rides `Task::done` (ducktape-ui#712) — the event is the
/// NEXT update-loop message, so the emitting handler's writes land first —
/// and a test that drops the returned task never sees the app half of the
/// round trip.
fn pump(app: &mut Ducktape, task: iced::Task<__DucktapeMessage>) {
    use iced_test::futures::futures::StreamExt as _;
    let Some(stream) = iced_test::runtime::task::into_stream(task) else {
        return;
    };
    let published: Vec<__DucktapeMessage> = iced_test::futures::futures::executor::block_on(
        stream
            .filter_map(|action| async move {
                match action {
                    iced_test::runtime::Action::Output(message) => Some(message),
                    _ => None,
                }
            })
            .collect(),
    );
    // ONE HOP, AND NO FURTHER. An `emit` is `Task::done`, so this task is a
    // ready message and nothing else — but what the RECEIVING handler
    // launches is a real request, and running that here would answer with the
    // failure a unit test's absent node returns and roll the optimistic row
    // back under the assertions. A test that wants the answer drives it, as
    // every other test in this file already does.
    for message in published {
        let _ = app.__update(message);
    }
}

/// THE SUBMIT AS THE APP SEES IT. A composer instance clears itself and then
/// hands up `(kind, body, operation_id)`; a test that is about what the app
/// does with a submitted body says exactly that, without driving keystrokes
/// through an instance it never asserts on. Returns the operation id, which
/// the send lane's receipts and failures are keyed by.
fn submit(app: &mut Ducktape, kind: ComposerKind, body: &str) -> String {
    let id = backend::fresh_operation_id(backend::composer_op_prefix(kind));
    let _ = app.__update(__DucktapeMessage::ComposerSubmitted(
        kind,
        body.to_owned(),
        id.clone(),
    ));
    id
}

/// THE SUBMIT THE GATE TURNS BACK. A refusal is not a discard: the refusing
/// arm hands the body to that room's own composer with a slice, and a slice is
/// a published message — so this one pumps, where `submit` above deliberately
/// does not. Only the refused arm is safe to pump: the admitted arm's task IS
/// the send request, and running it here would answer with the failure a unit
/// test's absent node returns.
fn submit_refused(app: &mut Ducktape, scope: &str, kind: ComposerKind, body: &str) {
    // The instance has to EXIST for the refusal to reach it — a composer that
    // has never been typed into holds no state yet, and a slice delivers to
    // instances that do. Typing and clearing is what a real submit did.
    seed_composer(app, scope, kind, body);
    seed_composer(app, scope, kind, "");
    let id = backend::fresh_operation_id(backend::composer_op_prefix(kind));
    let task = app.__update(__DucktapeMessage::ComposerSubmitted(
        kind,
        body.to_owned(),
        id,
    ));
    pump(app, task);
}

/// One composer instance's draft, as the reader sees it.
fn composer_text(app: &Ducktape, scope: &str) -> String {
    app.__ice_test_state_chat_composer(scope)
        .map(|state| state.body.trim().to_owned())
        .unwrap_or_default()
}

/// THE WORDS ONE COMPOSER INSTANCE'S PLATE IS HOLDING. The failed-send stash
/// used to be two app fields, so the plate a refused send raised followed the
/// reader into whatever room she moved to; it is the instance's own state now
/// (ducktape-ui#698), which is why reading it takes a scope.
fn composer_stash(app: &Ducktape, scope: &str) -> String {
    app.__ice_test_state_chat_composer(scope)
        .map(|state| state.failed)
        .unwrap_or_default()
}

/// Clicks one composer instance's Restore, the way the plate's button does.
/// The instance writes its own body and clears its own stash under its own
/// guards, so `blocked` is the verdict the frame drew, nothing more.
fn restore_composer(app: &mut Ducktape, scope: &str, blocked: bool) {
    let task = app.__update(Ducktape::__ice_test_message_chat_composer_restore(
        scope.to_owned(),
        blocked,
    ));
    pump(app, task);
}

fn compose(text: &str) -> iced::widget::text_editor::Content {
    iced::widget::text_editor::Content::with_text(text)
}

/// The page document's text, the way the save tick reads it.
fn page_document_text(app: &Ducktape) -> String {
    app.page_editor.text()
}

fn default_ice_color(name: &str) -> iced::Color {
    // 2.0 allows ONE theme contract and one palette, so the kit's theme moved
    // out of the vendored copy into the app's own file.
    let source = inlined(include_str!("ui/theme.ice"));
    let value = source
        .lines()
        .find_map(|line| {
            let mut parts = line.split_ascii_whitespace();
            (parts.next() == Some(name)).then(|| parts.next()).flatten()
        })
        .unwrap_or_else(|| panic!("theme.ice palette is missing `{name}`"));
    let hex = value
        .strip_prefix('#')
        .expect("default Ice colors use hexadecimal literals");
    let value =
        u32::from_str_radix(hex, 16).expect("default Ice colors are valid hexadecimal literals");
    match hex.len() {
        6 => iced::Color::from_rgb8(
            ((value >> 16) & 0xff) as u8,
            ((value >> 8) & 0xff) as u8,
            (value & 0xff) as u8,
        ),
        8 => iced::Color::from_rgba8(
            ((value >> 24) & 0xff) as u8,
            ((value >> 16) & 0xff) as u8,
            ((value >> 8) & 0xff) as u8,
            (value & 0xff) as f32 / 255.0,
        ),
        _ => panic!("default Ice colors use #RRGGBB or #RRGGBBAA"),
    }
}

fn live_refresh(
    generation: i64,
    active_channel: &str,
    messages: Vec<backend::ChatMessage>,
    active_page: &str,
    blocks: Vec<backend::PageBlock>,
) -> backend::LiveRefresh {
    backend::LiveRefresh {
        generation,
        fold_serial: 0,
        chat_loaded: true,
        channels: Vec::new(),
        messages,
        has_older_history: false,
        active_channel: active_channel.into(),
        active_channel_name: active_channel.into(),
        active_channel_archived: false,
        active_channel_members_only: false,
        huddle_roster: Vec::new(),
        channel_members: Vec::new(),
        pages_loaded: true,
        pages: Vec::new(),
        blocks,
        active_page: active_page.into(),
        active_page_title: active_page.into(),
        comment_thread_total: 0,
        commented_block_hits: Vec::new(),
        active_page_parent: String::new(),
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

fn chat_data(active_channel: &str, messages: Vec<backend::ChatMessage>) -> backend::ChatData {
    backend::ChatData {
        generation: 0,
        channels: Vec::new(),
        messages,
        has_older_history: false,
        active_channel: active_channel.into(),
        active_channel_name: active_channel.into(),
        active_channel_archived: false,
        active_channel_members_only: false,
        huddle_roster: Vec::new(),
        channel_members: Vec::new(),
        selected_message_seq: 0,
        selected_message_rev: 0,
        selected_message_body: String::new(),
        active_thread_seq: 0,
        thread_target_seq: 0,
        thread_messages: Vec::new(),
        thread_has_more: false,
    }
}

/// A hit left over from an already-answered search — what a navigation reset
/// must sweep away. Content is irrelevant; identity says "stale".
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
        messages: Vec::new(),
        has_older_history: false,
        active_channel: active_channel.into(),
        active_channel_name: active_channel.into(),
        active_channel_archived: false,
        active_channel_members_only: false,
        huddle_roster: Vec::new(),
        channel_members: Vec::new(),
        pages: Vec::new(),
        blocks: Vec::new(),
        active_page: String::new(),
        active_page_title: String::new(),
        active_page_parent: String::new(),
        comment_thread_total: 0,
        commented_block_hits: Vec::new(),
    }
}

/// The app has NO polling loop: every live surface rides the delta stream.
/// The only recurring subscriptions are wall clocks that nothing else can
/// supply — the huddle call timer and the toast's own dismissal — and this
/// pins that set exactly, so a reintroduced poll fails the build.
fn assert_no_polling(lifecycle: &str) {
    let recurring: Vec<_> = lifecycle
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("every "))
        .collect();
    assert_eq!(
        recurring,
        [
            // NO video clock here, on purpose: the tile strip is a
            // self-redrawing widget that repaints only its own window at
            // the capture cadence. A reintroduced video tick would rebuild
            // EVERY window's view tree per beat — fail the build instead.
            "every 1s when huddle_joined -> tick",
            // One shared wall reading makes every relative-time renderer pure.
            // The new runtime's logical clock owns this tick in tests.
            "every 1s when console_win != none -> wall_tick",
            // the toast's dismissal clock: fine ticks against a per-toast
            // age, so a toast raised late in the old shared 2800ms window
            // no longer flashes and vanishes. Still gated on a visible
            // toast — it costs nothing at rest.
            "every 300ms when !empty(toast) -> toast_tick",
            // the block editor's autosave clock: the stock editor's edits
            // never pass through a handler, so a dirty buffer is the only
            // signal there is — and the gate IS the dirty test, so the tick
            // exists solely while unsaved text needs the node. It costs
            // nothing at rest and dies the moment the save lands.
            // the page document's write gate: dirty IS the condition, so the
            // tick exists only while the buffer has drifted from the node's
            // text — not a poll, an edit-driven flush.
            "every 900ms when (connected && !empty(active_page) && editor_text(page_editor) != page_saved_text) -> page_autosave_tick",
        ]
    );
}

fn page_item(id: &str, title: &str) -> backend::PageItem {
    backend::PageItem {
        id: id.into(),
        title: title.into(),
        parent: String::new(),
        prefix: String::new(),
        child_count: 0,
    }
}

fn page_block(id: &str, page: &str, text: &str) -> backend::PageBlock {
    backend::PageBlock {
        key: 0,
        id: id.into(),
        parent: page.into(),
        kind: "Text".into(),
        text: text.into(),
        pending: false,
        checked: false,
        prefix: String::new(),
        child_count: 0,
    }
}

fn page_load(id: &str, title: &str, body: &str) -> backend::PagesData {
    backend::PagesData {
        pages: vec![page_item("alpha", "Alpha"), page_item("beta", "Beta")],
        blocks: vec![page_block(&format!("{id}-1"), id, body)],
        active_page: id.into(),
        active_page_title: title.into(),
        active_page_parent: String::new(),
        comment_thread_total: 0,
        commented_block_hits: Vec::new(),
    }
}

/// The app on Alpha, its document loaded and its buffer clean.
fn reading_alpha() -> Ducktape {
    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.pages = vec![page_item("alpha", "Alpha"), page_item("beta", "Beta")];
    app.doc_tabs = vec!["alpha".into(), "beta".into()];
    app.active_page = "alpha".into();
    app.active_page_title = "Alpha".into();
    app.active_page_parent = "Root".into();
    app.blocks = vec![page_block("alpha-1", "alpha", "alpha body")];
    app.page_editor = compose("Alpha\nalpha body");
    app.page_saved_text = "Alpha\nalpha body".into();
    app.buffer_page = "alpha".into();
    app
}

fn command_chord(code: iced::keyboard::key::Code) -> __IceKeyPress {
    __IceKeyPress {
        key: iced::keyboard::Key::Unidentified,
        modified_key: iced::keyboard::Key::Unidentified,
        physical_key: iced::keyboard::key::Physical::Code(code),
        location: iced::keyboard::Location::Standard,
        modifiers: iced::keyboard::Modifiers::COMMAND,
        text: None,
        repeat: false,
    }
}

/// The press the escape ladder answers, as the subscription delivers it.
fn escape_press() -> __IceKeyPress {
    __IceKeyPress {
        key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
        modified_key: iced::keyboard::Key::Unidentified,
        physical_key: iced::keyboard::key::Physical::Code(iced::keyboard::key::Code::Escape),
        location: iced::keyboard::Location::Standard,
        modifiers: iced::keyboard::Modifiers::empty(),
        text: None,
        repeat: false,
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

/// THE "Not connected" WORDING, ONCE. Every data screen swaps its empty-state
/// claim for this exact plate, so the console reads as one app rather than eight
/// dialects of "I don't know".
const NOT_CONNECTED_PLATE: &str = concat!(
    "EmptyState title=\"Not connected\" ",
    "description=\"Click the network name in the titlebar to pick or reconnect a network.\""
);

/// Every `.ice` file that is a VIEW: the mounts, the screens, the components.
/// Handlers and extern declarations are not views (a handler runs once per
/// event, a view expression runs once per frame), and the `.ice` tests mount
/// their own fixtures.
fn view_sources() -> Vec<(String, String)> {
    ice_sources()
        .into_iter()
        .filter(|(path, _)| {
            let path = path.replace('\\', "/");
            !path.contains("/handlers/") && !path.contains("/extern/") && !path.contains("/tests/")
        })
        .collect()
}

fn indent_of(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// The code half of a line, with any trailing comment cut off.
fn code_of(line: &str) -> &str {
    line.split("//").next().unwrap_or_default()
}

/// A component mount is a node whose name is Capitalized — `MessageCard`,
/// `Badge.Secondary`. Every other node in the language is lowercase.
fn mounts_a_component(code: &str) -> bool {
    let node = code.trim();
    let Some(first) = node.chars().next() else {
        return false;
    };
    first.is_ascii_uppercase()
        && node.split_whitespace().next().is_some_and(|name| {
            name.chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '.')
        })
}

/// `name(` at an identifier boundary, so `post_gate` does not match
/// `no_post_gate`.
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
