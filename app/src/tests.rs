// The app's update-loop and source-sweep suite. `super` is the crate
// root, so the generated `Ducktape` app and both native modules resolve
// exactly as they did when this mod lived inline in main.rs.
use super::*;

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

/// A SEAT THAT PRINTS A CHECKPOINT MUST OWN THE HEAD IT IS COMPARED AGAINST.
///
/// A checkpoint height is meaningless on its own: it says how far the durable
/// snapshot trails the head, and nothing else. Print it beside a head from a
/// DIFFERENT read and the pair can render an order no node is ever in — which
/// is what the titlebar's status card did, live `block_height` at the top and
/// the facts document's `node_checkpoint` four rows down, and what Settings'
/// node tile did with three reads on one screen.
///
/// So: the live register and a checkpoint may never be props of the same mount
/// or the same component. `node_height` — the checkpoint's own document's head
/// — is what a seat pairs a checkpoint with. The rule runs over every mount in
/// every `.ice` file, folded by [`inlined`] so a wrapped `with` block cannot
/// hide the pairing, because the seat this catches next is one nobody has
/// written yet.
#[test]
fn no_seat_prints_a_checkpoint_beside_the_live_head() {
    let sources = ice_sources();
    assert!(
        sources.len() > 10,
        "the walk found the .ice tree, not an empty directory"
    );
    let mut mounts = 0;
    for (path, source) in sources {
        for line in inlined(&source).lines() {
            // Prose is not a mount. Comments here NAME both registers on
            // purpose — explaining why they are apart is not putting them
            // together — and a sweep that flags its own documentation is the
            // whole-file negative that shipped green in this PR's first round.
            let code = line.split("//").next().unwrap_or_default();
            let live_head = code.contains("block_height");
            let a_checkpoint = code.contains("checkpoint");
            mounts += usize::from(live_head || a_checkpoint);
            assert!(
                !(live_head && a_checkpoint),
                "{path}: `{}` pairs the live head register with a checkpoint. \
                 They are two reads of a chain that moves several blocks a \
                 second, so the pair can print the checkpoint ABOVE the head. \
                 A checkpoint goes beside `node_height`, its own document's head.",
                code.trim()
            );
        }
    }
    assert!(
        mounts > 4,
        "the sweep found the registers it is supposed to be watching"
    );
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

fn compose(text: &str) -> iced::widget::text_editor::Content {
    iced::widget::text_editor::Content::with_text(text)
}

fn composer(app: &Ducktape) -> String {
    app.message_editor.text().trim().to_string()
}

fn reply_composer(app: &Ducktape) -> String {
    app.reply_editor.text().trim().to_string()
}

/// TWO STREAMS, TWO SURFACES, NEITHER BLANKING THE OTHER.
///
/// The two topics now ride separate sockets with separate gates, so a status
/// push must not touch the peers table and a peers push must not touch the
/// consensus facts. Under the old merged stream that was enforced by
/// `answered` flags; now it is enforced by there being nothing to confuse.
#[test]
fn a_pushed_status_moves_the_facts_and_leaves_the_table() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.node_peers = vec![backend::PeerRow {
        key: "aa".into(),
        role: "validator".into(),
        live: true,
    }];

    let _ = app.__update(__DucktapeMessage::NodeStatusPushed(backend::NodeFacts {
        public_key: "node-key".into(),
        version: "0.2.0".into(),
        root_hash: "hash-new".into(),
        checkpoint_height: 512,
        last_finalized_at: 999,
        height: 888,
        view: Some(9),
        quorum: Some(5),
        reachable_validators: Some(6),
        phase: "syncing".into(),
        phase_since: 1_700_000_000,
        sync_target: 900,
        sync_applied: 412,
        sync_retries: 2,
        sync_failures: 1,
        sync_last_error: "peer hung up".into(),
    }));

    // ALL SEVENTEEN, because a field the handler forgot stays frozen at its
    // connect-time value for as long as the console is open.
    assert_eq!(app.node_key, "node-key");
    assert_eq!(app.node_version, "0.2.0");
    assert_eq!(app.node_root_hash, "hash-new");
    assert_eq!(app.node_checkpoint, 512);
    assert_eq!(app.node_last_finalized, 999);
    assert_eq!(app.node_height, 888);
    assert_eq!(app.node_view_label, "9");
    assert_eq!(app.node_quorum_label, "5");
    assert_eq!(app.node_reachable_label, "6");
    assert_eq!(app.node_phase, "syncing");
    assert_eq!(app.node_phase_since, 1_700_000_000);
    assert_eq!(app.node_sync_target, 900);
    assert_eq!(app.node_sync_applied, 412);
    assert_eq!(app.node_sync_retries, 2);
    assert_eq!(app.node_sync_failures, 1);
    assert_eq!(app.node_sync_last_error, "peer hung up");
    assert_eq!(
        app.node_peers.len(),
        1,
        "a status push must not empty the peers table"
    );

    let _ = app.__update(__DucktapeMessage::NodePeersPushed(backend::PeersData {
        generation: -1,
        peers: vec![
            backend::PeerRow {
                key: "aa".into(),
                role: "validator".into(),
                live: true,
            },
            backend::PeerRow {
                key: "bb".into(),
                role: "resident".into(),
                live: false,
            },
        ],
    }));
    assert_eq!(app.node_peers.len(), 2, "the peers push landed");
    assert_eq!(
        app.node_phase, "syncing",
        "a peers push must not blank the node's phase"
    );
    assert_eq!(app.node_sync_applied, 412);
}

/// THE EXPLORER DRAWS THE LIVE REGISTER, NOT ITS OWN NEWEST ROW.
///
/// Its list is op-carrying blocks only, so the top row lags the chain by
/// however many idle blocks have passed — on a quiet chain, forever. The head
/// a reader watches moves on the ws heartbeat, every block, nop fillers
/// included, and the screen was not even handed it: it had a hundred-block
/// snapshot and a refresh button.
#[test]
fn the_explorer_is_handed_the_live_head_and_the_phase() {
    let view = inlined(include_str!("ui/view.ice"));
    let explorer = view
        .split_once("ExplorerScreen")
        .expect("the explorer mounts here")
        .1;
    let explorer = explorer.split_once("events").expect("props end").0;
    assert!(
        explorer.contains("head=block_height"),
        "the explorer must draw the live register, not the newest row of its own window"
    );
    assert!(
        explorer.contains("sync_line=sync_label(node_phase, node_sync_applied, node_sync_target)"),
        "a head that is not advancing and a node still catching up are different \
         facts, and the second one needs saying"
    );
}

/// A TYPED CHARACTER COSTS ONE VIEW REBUILD, NOT TWO.
///
/// iced 0.14 rebuilds the whole UI once per message batch and has no dirty
/// check. A `keyboard press` with no `status=` fires for keys a focused widget
/// already CONSUMED, and the message it publishes cannot join the batch that
/// widget's own message is in — it leaves through the event-loop proxy and
/// comes back a turn later. So an unfiltered global key subscription charged
/// every character typed into a composer a SECOND full ChatScreen build+layout,
/// which `frame_probe`'s keystroke gate could not see: that gate drives the
/// widget's message alone.
///
/// The arbitration is mechanical, so it is pinned rather than commented. Every
/// `keyboard press` names a `status=`, and the one that takes the CAPTURED half
/// is gated on the escape ladder's OWN reading of whether a transient layer is
/// up — iced's single-line input consumes Escape, and that is the only reason
/// the captured half exists. With no layer open a captured key has nothing to
/// dismiss, which is exactly the state a reader typing into a composer is in.
///
/// Pinned as a SET, for the reason the node streams below are: a `contains` is
/// equally satisfied by a second, unfiltered subscription sitting beside the
/// right one.
#[test]
fn no_keyboard_subscription_charges_a_captured_key_to_a_bare_composer() {
    let lifecycle = inlined(include_str!("ui/handlers/lifecycle.ice"));
    let presses: Vec<_> = lifecycle
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("keyboard press"))
        .collect();
    assert_eq!(
        presses,
        [
            "keyboard press status=ignored when (connected || palette_open) -> global_key_pressed _",
            "keyboard press key=escape status=captured when !empty(topmost_overlay(palette_open, \
             bell_open, channel_create_open, thread_message_action, message_action, \
             channel_settings_open, forge_repo_menu)) -> global_key_pressed _",
            "keyboard press status=ignored -> content_scroll_key _",
        ],
        "a `keyboard press` without `status=` bills every captured key a whole \
         extra view rebuild; the captured half is Escape-only (ducktape-ui#602) \
         and stays gated on an open layer"
    );
}

/// STATUS EVERYWHERE, PEERS ONLY WHERE IT IS DRAWN — pinned as sets, because a
/// `contains` is satisfied by a commented-out line and equally by a SECOND,
/// wrongly-gated subscription sitting beside the right one.
#[test]
fn the_node_streams_carry_the_gates_their_costs_require() {
    let lifecycle = inlined(include_str!("ui/handlers/lifecycle.ice"));
    let status: Vec<_> = lifecycle
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("run node_status_live("))
        .collect();
    assert_eq!(
        status,
        ["run node_status_live(connected_rpc) when connected -> node_status_pushed _"],
        "status is a cell read and a fact about the node, so it rides every tab"
    );

    let peers: Vec<_> = lifecycle
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("run node_peers_live("))
        .collect();
    assert_eq!(
        peers,
        [
            "run node_peers_live(connected_rpc) when (connected && shell_tab == ShellTab.node && node_tab == NodeTab.overview) -> node_peers_pushed _"
        ],
        "every peers sample encodes the whole metrics registry; this gate is the budget"
    );

    assert!(lifecycle.contains(
        "from done load_request(shell_tab == ShellTab.node && node_tab == NodeTab.overview, connected_rpc, \"\", node_peers_generation)\n      try request -> done request\n      done -> peers_load_selected _"
    ));
    assert!(lifecycle.contains(
        "on peers_load_selected(request)\n  let obsolete_request = request.rpc != connected_rpc || request.generation != node_peers_generation\n  let unmounted = shell_tab != ShellTab.node || node_tab != NodeTab.overview\n  return if obsolete_request || unmounted\n  run replace lane=peers_load load_peers(request.rpc, request.generation)"
    ));
    assert_no_polling(&lifecycle);
}

/// The page document's text, the way the save tick reads it.
fn page_document_text(app: &Ducktape) -> String {
    crate::pages::page_text(app.page_editor.clone())
}

#[test]
fn full_view_fits_the_default_test_stack() {
    std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(|| {
            let (mut app, _) = Ducktape::__boot();
            let console = iced::window::Id::unique();
            app.console_win = Some(console);
            let _ = app.__view(console);
            let onboarding = iced::window::Id::unique();
            app.onboarding_win = Some(onboarding);
            app.hub_step = HubStep::Networks;
            let _ = app.__view(onboarding);
            let huddle = iced::window::Id::unique();
            app.huddle_win = Some(huddle);
            let _ = app.__view(huddle);
        })
        .unwrap()
        .join()
        .unwrap();
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
        kind: "chat".into(),
        status: "Live".into(),
        height: row.seq.max(1),
        chat: backend::ChatDelta {
            kind: "posted".into(),
            channel_id: channel.into(),
            seq: row.seq,
            message: row,
            ..backend::ChatDelta::default()
        },
        ..backend::LiveUpdate::default()
    }
}

fn chat_data(active_channel: &str, messages: Vec<backend::ChatMessage>) -> backend::ChatData {
    backend::ChatData {
        generation: 0,
        channels: Vec::new(),
        messages,
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
        thread_next_reply_offset: 0,
        thread_has_more: false,
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

#[test]
fn shell_tab_is_app_state_and_palette_hits_switch_panes() {
    let (mut app, _) = Ducktape::__boot();
    assert_eq!(app.shell_tab, ShellTab::Chat);
    let _ = app.__update(__DucktapeMessage::SelectShellTab(ShellTab::Pages));
    assert_eq!(app.shell_tab, ShellTab::Pages);

    // a palette chat hit closes the palette and lands on the chat pane
    app.loading = false;
    app.mutation_phase = MutationPhase::Idle;
    app.connected_rpc = "http://node".into();
    app.palette_open = true;
    let _ = app.__update(__DucktapeMessage::OpenChatSearchHit("general".into(), 7, 7));
    assert!(!app.palette_open);
    assert_eq!(app.shell_tab, ShellTab::Chat);
}

/// Node operations are an operator surface, not a tail appended to device
/// preferences. Pin all three routing seams so a visual reshuffle cannot bury
/// the screen in Settings again while leaving its handlers intact.
#[test]
fn node_operations_are_a_first_class_screen() {
    let settings = include_str!("ui/screens/settings.ice");
    let node = include_str!("ui/screens/node.ice");
    assert!(settings.contains("component SettingsScreen("));
    assert!(node.contains("component NodeScreen("));
    assert!(settings.contains("emit(select_shell_tab, ShellTab.node)"));
    for node_detail in [
        "node-overview-tab",
        "node-permissions-tab",
        "node-activity-tab",
        "node-modules-tab",
    ] {
        assert!(
            !settings.contains(node_detail),
            "Settings must link to Node, not embed {node_detail}"
        );
        assert!(node.contains(node_detail), "Node owns {node_detail}");
    }

    let shell = include_str!("ui/components/shell.ice");
    assert!(shell.contains("ShellTab.node\n                  slot node"));
    let view = include_str!("ui/view.ice");
    assert!(view.contains("node:\n          NodeScreen"));
    assert!(view.contains("settings:\n          SettingsScreen"));
}

/// A hydration error belongs to the pane that raised it.
///
/// The banner has no self-retiring path — it is dismissed by hand or it
/// stays — so leaving it up across a navigation tells the user the pane they
/// just opened is broken. `select_shell_tab` clears it ABOVE both of its
/// early returns, which is the part worth pinning: the `!connected` return
/// and the chat/pages return each skip the generation bumps, and a clear
/// placed below either one would silently cover only some tabs.
#[test]
fn switching_panes_retires_a_stale_error_banner_on_every_tab() {
    // the disconnected path returns first, and must still clear.
    let (mut app, _) = Ducktape::__boot();
    app.error = "could not reach the node".into();
    let _ = app.__update(__DucktapeMessage::SelectShellTab(ShellTab::Files));
    assert_eq!(
        app.error, "",
        "the !connected early return must still clear"
    );

    // the chat/pages path returns second, and must still clear.
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.error = "files: path not found".into();
    let _ = app.__update(__DucktapeMessage::SelectShellTab(ShellTab::Pages));
    assert_eq!(
        app.error, "",
        "the chat/pages early return must still clear"
    );

    // and the full path, which falls through to the generation bumps.
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.error = "explorer hydration failed".into();
    let _ = app.__update(__DucktapeMessage::SelectShellTab(ShellTab::Members));
    assert_eq!(app.error, "");
    assert_eq!(app.shell_tab, ShellTab::Members);
}

/// EVERY READER OF `/v1/peers` USES THE NAMES `PeerView` SERIALIZES.
///
/// `bin/noded/src/peers.rs` serves `peer` / `connected` / `role`; it has never
/// served `key`, `live`, or a per-peer `height`. Reading the wrong ones does
/// not fail — `as_str()` answers `None` and the row renders blank, zero and
/// offline for a peer that is connected.
///
/// This has already happened twice in two different readers: `roster.rs`
/// carries the scar in a comment, and Settings' PEERS table shipped with all
/// three wrong names. The app cannot depend on `noded` to pin the contract with
/// a type, so it is pinned here instead — one rule over every reader at once.
#[test]
fn peer_readers_use_the_names_the_node_serves() {
    const READERS: [(&str, &str); 2] = [
        ("backend/node.rs", include_str!("backend/node.rs")),
        ("backend/roster.rs", include_str!("backend/roster.rs")),
    ];
    for (name, source) in READERS {
        for wrong in ["peer[\"key\"]", "peer[\"live\"]", "peer[\"height\"]"] {
            assert!(
                !source.contains(wrong),
                "{name} reads {wrong}, which `/v1/peers` does not serve — \
                 see bin/noded/src/peers.rs for the names it does"
            );
        }
        assert!(
            source.contains("peer[\"peer\"]") || source.contains("peer[\"connected\"]"),
            "{name} was expected to read the peers view; if it no longer does, \
             drop it from this lint rather than leaving the guard vacuous"
        );
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
            "every 900ms when (connected && !empty(active_page) && page_text(page_editor) != page_saved_text) -> page_autosave_tick",
        ]
    );
}

/// THE TAB GATE IS WIRING, NOT A PREDICATE. The optional request must be the
/// task-flow input: an unselected `try` becomes Task::none and cannot supersede
/// an already-running replace lane.
///
/// It pins the OTHER half too: the `plane` arm is the chips' only off-tab
/// writer, so its governance/agents runs must NOT carry a second
/// `shell_tab ==` gate. Gating both leaves the approvals badge dark until you
/// open Approvals — which is the one thing the badge exists to spare you.
#[test]
fn a_gated_plane_is_gated_at_the_call_site_and_still_lands_off_tab() {
    let lifecycle = inlined(include_str!("ui/handlers/lifecycle.ice"));

    for (loader, lane, selected, plane, generation) in [
        (
            "load_members",
            "members_load",
            "members_load_selected",
            "members",
            "members_generation",
        ),
        (
            "load_governance",
            "governance_load",
            "governance_load_selected",
            "governance",
            "gov_generation",
        ),
        (
            "load_agents",
            "agents_load",
            "agents_load_selected",
            "agents",
            "agents_generation",
        ),
        (
            "load_account",
            "account_load",
            "account_load_selected",
            "account",
            "account_generation",
        ),
    ] {
        let wired = format!(
            "from done load_request(tab_reads_plane(shell_tab, \"{plane}\"), connected_rpc, \"\", {generation})\n      try request -> done request\n      done -> {selected} _"
        );
        assert!(
            lifecycle.contains(&wired),
            "the tab switch must gate {loader}: {wired}"
        );
        let launch = format!(
            "on {selected}(request)\n  let obsolete_request = request.rpc != connected_rpc || request.generation != {generation}\n  return if obsolete_request\n  run replace lane={lane} {loader}(request.rpc, request.generation)"
        );
        assert!(
            lifecycle.contains(&launch),
            "selected load lost its compiler lane: {launch}"
        );
    }

    for (loader, selected, module, generation) in [
        (
            "load_members",
            "members_load_selected",
            "valset",
            "members_generation",
        ),
        (
            "load_governance",
            "governance_load_selected",
            "governance",
            "gov_generation",
        ),
        (
            "load_account",
            "account_load_selected",
            "identity",
            "account_generation",
        ),
        (
            "load_dm_peers",
            "dm_peers_load_selected",
            "identity",
            "dm_peers_generation",
        ),
        (
            "load_agents",
            "agents_load_selected",
            "agent",
            "agents_generation",
        ),
    ] {
        let live = format!(
            "from done load_request(plane_live_hit(next.kind, next.module, \"{module}\"), connected_rpc, \"\", {generation})\n      try request -> done request\n      done -> {selected} _"
        );
        assert!(
            lifecycle.contains(&live),
            "a {module} commit must refresh {loader} on any tab: {live}"
        );
    }

    // THE SETTINGS FACTS ARE THE INLINE HALF OF THE SAME GATE. No module
    // commits a key file or the local prefs, so they get no
    // `tab_reads_plane` row and no live arm — just the tab that draws them.
    // The EXACT SET is the pin, both lines: the CONNECT load must stay
    // ungated (it is what fills chat's `user_key`, which chat cannot refetch
    // for itself — a move into chat returns above the tab block), and the tab
    // move must carry the gate.
    assert!(lifecycle.contains(
        "run replace lane=settings_load load_settings_facts(connected_rpc, settings_generation) -> settings_loaded _ | settings_failed _"
    ));
    assert!(lifecycle.contains(
        "from done load_request(shell_tab == ShellTab.settings, connected_rpc, \"\", settings_generation)\n      try request -> done request\n      done -> settings_load_selected _"
    ));
    assert!(lifecycle.contains(
        "on settings_load_selected(request)\n  let obsolete_request = request.rpc != connected_rpc || request.generation != settings_generation\n  let unmounted = shell_tab != ShellTab.settings\n  return if obsolete_request || unmounted\n  run replace lane=settings_load load_settings_facts(request.rpc, request.generation)"
    ));

    // A selector is a queued message. If an older one lands after a newer
    // intent, it must not start and replace the newer lane.
    for (selected, generation) in [
        ("explorer_load_selected", "explorer_generation"),
        ("files_list_selected", "fs_generation"),
        ("files_history_selected", "fs_generation"),
        ("members_load_selected", "members_generation"),
        ("governance_load_selected", "gov_generation"),
        ("settings_load_selected", "settings_generation"),
        ("peers_load_selected", "node_peers_generation"),
        ("agents_load_selected", "agents_generation"),
        ("account_load_selected", "account_generation"),
        ("dm_peers_load_selected", "dm_peers_generation"),
        ("forge_load_selected", "forge_generation"),
        (
            "shell_credentials_load_selected",
            "shell_credentials_generation",
        ),
    ] {
        let guarded = format!(
            "on {selected}(request)\n  let obsolete_request = request.rpc != connected_rpc || request.generation != {generation}"
        );
        assert!(
            lifecycle.contains(&guarded),
            "queued selector lacks an endpoint-and-generation launch guard: {guarded}"
        );
    }

    for (selected, unmounted) in [
        ("explorer_load_selected", "shell_tab != ShellTab.explorer"),
        ("files_list_selected", "shell_tab != ShellTab.files"),
        ("files_history_selected", "shell_tab != ShellTab.files"),
        ("settings_load_selected", "shell_tab != ShellTab.settings"),
        (
            "peers_load_selected",
            "shell_tab != ShellTab.node || node_tab != NodeTab.overview",
        ),
        ("forge_load_selected", "shell_tab != ShellTab.forge"),
        (
            "shell_credentials_load_selected",
            "shell_tab != ShellTab.shell",
        ),
    ] {
        let handler = lifecycle
            .split_once(&format!("on {selected}(request)"))
            .unwrap_or_else(|| panic!("missing selected handler {selected}"))
            .1
            .split("\non ")
            .next()
            .expect("selected handler body");
        assert!(
            handler.contains(&format!("let unmounted = {unmounted}")),
            "{selected}: tab-owned selector lacks its current-mount guard: {unmounted}"
        );
        assert!(handler.contains("return if obsolete_request || unmounted"));
    }
}

/// THE GATE'S OTHER HALF IS THE BUMP. `settings_loaded` is dropped when its
/// generation is stale, so a tab move that bumps unconditionally revokes the
/// connect load still in flight. Every other generation on that
/// block may bump freely — their loaders draw one tab, so opening it re-earns
/// the read. The settings facts are the exception: `settings_user_key` is
/// chat's `me`, chat returns above the tab block, and nothing chat does ever
/// re-issues the load. Lose it and `me` stays "" for the session — which
/// `chat_sidebar_rooms` reads as "show every DM under CHANNELS" and `post_gate` as
/// "not seated", refusing the composer on every DM.
#[test]
fn a_move_to_a_pane_that_does_not_draw_the_settings_facts_keeps_the_connect_load() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    let in_flight = app.settings_generation;

    let _ = app.__update(__DucktapeMessage::SelectShellTab(ShellTab::Members));
    let _ = app.__update(__DucktapeMessage::SettingsLoaded(
        crate::backend::SettingsFacts {
            generation: in_flight,
            key_path: "/w/user.key".into(),
            key_state: "encrypted".into(),
            data_dir: "/w".into(),
            open_tabs: 0,
            user_key: "abcd".into(),
        },
    ));
    assert_eq!(
        app.settings_user_key, "abcd",
        "the move off-tab must not revoke the connect load's facts"
    );

    // and the tab that DOES draw them still re-reads on entry.
    let _ = app.__update(__DucktapeMessage::SelectShellTab(ShellTab::Settings));
    assert_ne!(
        app.settings_generation, in_flight,
        "entering Settings must issue a fresh read"
    );
}

#[test]
fn forge_depth_rides_the_established_seams() {
    // the forge handlers moved out of lifecycle.ice into their own file;
    // the seams they guard did not, so the guard reads both.
    let lifecycle = inlined(concat!(
        include_str!("ui/handlers/lifecycle.ice"),
        include_str!("ui/handlers/forge.ice"),
    ));
    let forge = inlined(include_str!("ui/components/forge.ice"));
    let backend = inlined(include_str!("ui/extern/backend.ice"));
    let forge_state = inlined(include_str!("ui/state/forge.ice"));
    let onboarding = inlined(include_str!("ui/handlers/onboarding.ice"));

    // the item discussion IS a chat surface: hydrated through the chat
    // lanes and spliced by the SAME fold the chat pane uses, scoped to
    // the item's hidden channel — never a forge-private message path.
    assert!(lifecycle.contains(
        "forge_discussion = apply_chat_messages(forge_discussion, next.chat, forge_item_channel)"
    ));
    assert!(lifecycle.contains(
        "run every send_message(connected_rpc, password, forge_item_channel, forge_discussion_pending"
    ));

    // Replace lanes own request freshness. Their payloads keep only the
    // semantic scope needed when the selected repo/item moves without a new
    // request: channel for discussion, and repo/revision/path for code.
    assert!(lifecycle.contains(
        "run replace lane=forge_discussion load_forge_discussion(connected_rpc, forge_item_channel)"
    ));
    assert!(lifecycle.contains("return if next.channel_id != forge_item_channel"));
    assert!(lifecycle.contains("let same_repo = next.repo == forge_repo"));
    assert!(
        lifecycle.contains("let same_rev = empty(forge_tree_rev) || next.rev == forge_tree_rev")
    );
    assert!(lifecycle.contains("let same_path = next.path == forge_file_path"));
    assert!(backend.contains(
        "load_forge_discussion(rpc:str, channel_id:str) -> ForgeDiscussionData ! AppError"
    ));
    assert!(
        backend.contains(
            "forge_tree(rpc:str, repo:str, rev:str, path:str) -> ForgeTreeData ! AppError"
        )
    );
    assert!(
        backend.contains("forge_blob(rpc:str, repo:str, rev:str, path:str) -> BlobView ! AppError")
    );
    for deleted in [
        concat!("forge_", "discussion_generation"),
        concat!("forge_", "code_generation"),
    ] {
        for (path, source) in [
            ("state/forge.ice", forge_state.as_str()),
            ("handlers/forge.ice", lifecycle.as_str()),
            ("handlers/onboarding.ice", onboarding.as_str()),
            ("extern/backend.ice", backend.as_str()),
        ] {
            assert!(
                !source.contains(deleted),
                "{path} still carries deleted request token `{deleted}`"
            );
        }
    }

    // a review pins the source head the reviewer saw; the merge CASes
    // BOTH heads (recompute on a moved branch, never a blind retry).
    //
    // the line comments ride INSIDE the review's own transaction — there is
    // no standalone comment op, so a comment cannot land without the
    // verdict it was written under, and it cannot outlive the diff it
    // anchors to (`keep_staged_comments` drops them when the head moves).
    assert!(backend.contains(
        "submit_forge_review(rpc:str, password:str, repo:str, number:i64, verdict:ForgeReviewVerdict, body:str, commit_oid:str, comments:[ForgeDraftComment])"
    ));
    assert!(backend.contains(
        "merge_forge_pr(rpc:str, password:str, repo:str, number:i64, source_branch:str, expected_source_oid:str, prev_target_oid:str)"
    ));

    // committed forge ops refresh scoped slices through the handler's one
    // terminal parallel — no polling, no per-op full reloads. The repo LIST
    // is the one slice with no open-scope of its own, so it carries the forge
    // surface's own gate: a chain op must not query a list that is not on
    // screen.
    assert!(lifecycle.contains(
        "run replace lane=forge_live forge_live_refresh(connected_rpc, forge_repo, forge_item_number, next.kind, next.module, next.forge, (shell_tab == ShellTab.forge), forge_generation)"
    ));
    assert_no_polling(&lifecycle);

    // approvals stay advisory in the merge box — `MergeAdvisory` is the
    // ONLY thing said above the merge button, and it recommends, never
    // refuses. The merged state renders the CAS'd commit.
    let forge_screen = inlined(include_str!("ui/screens/forge.ice"));
    assert!(forge_screen.contains("MergeAdvisory change_requests=forge_item_change_requests"));
    assert_eq!(forge.matches("merge not recommended").count(), 2);
    // MergeAdvisory owns the count: no OTHER predicate may branch on it.
    // The one sibling read is the disclaimer's `<= 0`, which is the
    // no-advisory half and cannot contradict it.
    assert!(!forge_screen.contains("forge_item_change_requests > 0"));
    assert_eq!(
        forge_screen
            .matches("forge_item_change_requests <= 0")
            .count(),
        1
    );
    assert!(forge_screen.contains("forge_merge_note(forge_item_merge_oid, forge_item_branches)"));
}

/// THE COMMITTED LIST IS THE WHOLE CARD ANSWER. A former follow-up lane launched
/// from `forge_loaded`, fetched every repo mirror and walked README/tree facts.
/// That made the tab wait on work no card needs. Keep the landing handler pure
/// state installation and keep mirror work behind the explicit merge act.
#[test]
fn forge_repo_list_never_launches_mirror_details_work() {
    let backend = inlined(include_str!("backend/forge.rs"));
    let list_loader = backend
        .split_once("pub async fn load_forge(")
        .expect("forge list loader")
        .1
        .split_once("pub async fn load_forge_repo(")
        .expect("repo loader boundary")
        .0;
    assert!(list_loader.contains("list_forge_repos"));
    for mirror_work in [
        "load_forge_details",
        "repo_card_facts",
        "sync_forge_mirror",
        "spawn_blocking",
    ] {
        assert!(
            !list_loader.contains(mirror_work),
            "the repo list must not start {mirror_work}"
        );
    }
    assert!(!backend.contains("pub async fn load_forge_details("));
    assert!(!backend.contains("fn repo_card_facts("));
    assert!(
        backend.contains("fn sync_forge_mirror("),
        "merge preflight still owns its client-computed commit mirror"
    );

    let handlers = inlined(include_str!("ui/handlers/forge.ice"));
    let loaded = handlers
        .split_once("on forge_loaded(next)")
        .expect("forge loaded handler")
        .1
        .split_once("\non ")
        .expect("forge loaded arm")
        .0;
    assert!(loaded.contains("forge_repos = next.repos"));
    assert!(!loaded.contains("run "));
    assert!(!handlers.contains("forge_details"));

    let externs = inlined(include_str!("ui/extern/backend.ice"));
    assert!(externs.contains("ForgeRepo(name:str, head:str)"));
    assert!(!externs.contains("load_forge_details"));

    let components = inlined(include_str!("ui/components/forge.ice"));
    let header = components
        .split_once("component ForgeOrgHeader(")
        .expect("forge org header")
        .1
        .split_once("\ncomponent ")
        .expect("forge org header boundary")
        .0;
    assert!(header.contains("answered:bool"));
    assert!(header.contains("if answered"));
    assert!(!header.contains("if connected"));

    let card = components
        .split_once("component RepoCard(")
        .expect("repo card")
        .1
        .split_once("\ncomponent ")
        .expect("repo card boundary")
        .0;
    assert!(card.contains("repo.name"));
    assert!(card.contains("repo.head"));
    for removed in [
        "repo.about",
        "repo.language",
        "repo.updated_at",
        "relative_time",
    ] {
        assert!(!card.contains(removed), "repo card must not read {removed}");
    }
}

/// CODE BROWSING IS AN API READ, NOT A COLD CLONE. The root tree query resolves
/// an empty revision to one exact commit; every directory and blob click then
/// sends that commit back. Neither loader may touch the merge-only mirror or a
/// blocking git task.
#[test]
fn forge_code_loaders_query_only_the_requested_tree_or_blob() {
    let backend = include_str!("backend/forge.rs");
    let tree = backend
        .split_once("pub async fn forge_tree(")
        .expect("tree loader")
        .1
        .split_once("pub async fn forge_blob(")
        .expect("blob loader boundary")
        .0;
    let blob = backend
        .split_once("pub async fn forge_blob(")
        .expect("blob loader")
        .1
        .split_once("pub fn forge_live_hit(")
        .expect("blob loader boundary")
        .0;
    for (loader, query) in [(tree, "\"tree\""), (blob, "\"blob\"")] {
        assert!(loader.contains(query));
        for field in ["\"repo\": &repo", "\"rev\": &rev", "\"path\": &path"] {
            assert!(loader.contains(field));
        }
        assert!(loader.contains("client.query(\"forge\", &query).await?"));
        for full_repo_work in [
            "sync_forge_mirror",
            "mirror_holding_revision",
            "spawn_blocking",
        ] {
            assert!(
                !loader.contains(full_repo_work),
                "Code loader must not start {full_repo_work}"
            );
        }
    }

    let handlers = include_str!("ui/handlers/forge.ice");
    assert!(
        handlers.contains(
            "run replace lane=forge_code forge_tree(connected_rpc, forge_repo, \"\", \"\")"
        )
    );
    assert!(handlers.contains(
        "run replace lane=forge_code forge_tree(connected_rpc, forge_repo, forge_tree_rev, path)"
    ));
    assert!(handlers.contains(
        "run replace lane=forge_code forge_blob(connected_rpc, forge_repo, forge_tree_rev, path)"
    ));
    assert!(!handlers.contains("forge_code_generation"));
}

/// Forge's repo chrome used to stack three independent rows — crumb, every
/// branch, then tabs — before a reader reached any code or tracker content.
/// Keep branch context in the tab row and keep detail navigation in the
/// persistent repo bar, so neither can quietly grow another empty band.
#[test]
fn forge_layout_keeps_repo_navigation_compact() {
    let screen = inlined(include_str!("ui/screens/forge.ice"));

    let repo_body = screen
        .split_once("if forge_item_number <= 0")
        .expect("repo body")
        .1
        .split_once("match tab")
        .expect("repo navigation boundary")
        .0;
    let tabs_end = repo_body
        .find("emit(select_forge_tab, ForgeTab.issues)")
        .expect("issues tab");
    let branches = repo_body
        .find("for branch in branches")
        .expect("branch strip");
    assert!(
        tabs_end < branches,
        "branch context follows the tabs in their shared navigation row"
    );
    assert_eq!(repo_body.matches("for branch in branches").count(), 1);

    let item_body = screen
        .split_once("if forge_item_number > 0 && item_phase == ForgePhase.ready")
        .expect("detail back control")
        .1;
    assert!(item_body.starts_with("\n                BackToList"));
    assert_eq!(screen.matches("BackToList kind=forge_item_kind").count(), 1);
}

/// Source and patch rows are one code-reading surface. The semantic diff
/// plates may vary, but their metrics, neutral code ink and numbered gutter do
/// not: that keeps a changed line from switching type weight or losing its
/// numbers when the palette flips.
#[test]
fn forge_source_and_diff_rows_share_a_compact_code_style() {
    let components = inlined(include_str!("ui/components/forge.ice"));
    let source = components
        .split_once("component ForgeCodeLine(")
        .expect("source row")
        .1
        .split_once("\ncomponent ")
        .expect("source row boundary")
        .0;
    assert_eq!(source.matches("h=20.0").count(), 2);
    assert_eq!(source.matches("size=11.5").count(), 2);
    assert!(source.contains("font=code @text-forge_gutter_ink"));
    assert!(source.contains("font=code @text-strong_ink"));
    assert!(!source.contains("@text-icon_idle"));

    let diff = components
        .split_once("component DiffRow(")
        .expect("diff row")
        .1
        .split_once("\ncomponent ")
        .expect("diff row boundary")
        .0;
    assert!(diff.contains("font=code_semibold @text-diff_add_fg"));
    assert!(diff.contains("font=code_semibold @text-diff_del_fg"));
    assert!(!diff.contains("text=gutter_ink"));
    assert_eq!(diff.matches("text=forge_gutter_ink").count(), 3);
    assert_eq!(
        diff.matches("font=code @text-strong_ink").count(),
        3,
        "added, deleted and context code use the same neutral ink"
    );
    assert!(diff.contains("font=code_semibold @text-merged"));
}

#[test]
fn background_refresh_preserves_editing_state() {
    let root = inlined(include_str!("ui/app.ice"));
    let view = inlined(include_str!("ui/view.ice"));
    let lifecycle = inlined(include_str!("ui/handlers/lifecycle.ice"));
    assert!(!view.contains("sync_phase"));
    assert!(root.contains("use \"view.ice\""));
    assert!(!lifecycle.contains("on refresh_now"));
    // live surfaces (chat/pages) never need a manual refresh — the delta
    // stream keeps them current. The explorer's recent-window reload is
    // the one legitimate refresh affordance.
    let before_explorer = view
        .split_once("    explorer:")
        .map_or(view.as_str(), |(head, _)| head);
    assert!(!before_explorer.contains("button \"Refresh\""));

    let refresh = lifecycle
        .split_once("on live_resynced(next)\n")
        .unwrap()
        .1
        .split_once("\non ")
        .unwrap()
        .0;
    let editable = [
        "rpc",
        "password",
        "channel_draft",
        "chat_search_draft",
        "page_draft",
        "block_draft",
        "page_search_draft",
    ];
    let overwrites_editable = refresh.lines().any(|line| {
        editable
            .iter()
            .any(|name| line.trim_start().starts_with(&format!("{name} =")))
    });
    assert!(!overwrites_editable);
    for scoped in ["channel_name_draft", "member_key_draft", "message_draft"] {
        assert!(refresh.contains(&format!(
            "{scoped} = retain_for_endpoint({scoped}, active_channel, \
keep_str(next.chat_loaded, next.active_channel, active_channel))"
        )));
    }
    assert!(refresh.contains("selected_message_seq = refreshed_required_message_seq("));
    assert!(refresh.contains("failed_message_draft = remember_failed_draft("));
    assert!(lifecycle.contains("run live_events(connected_rpc) when connected"));
    assert_no_polling(&lifecycle);
    assert!(lifecycle.contains("run replace lane=live_resync live_resync_load(connected_rpc"));
    assert!(lifecycle.contains("run replace lane=live_thread refresh_live_thread(connected_rpc"));
    assert!(lifecycle.contains("parallel\n    run replace lane=live_thread refresh_live_thread("));
    // Page-scoped state waits for a reply that answers for the page in hand —
    // a resync issued before a mutation moved the selection speaks for a
    // document nobody is on. And the fold-owned fields (#1041) additionally
    // wait for a reply no text fold outran: the title and the row titles keep
    // the fold's value when the serial moved, while the structural half still
    // lands from the reply.
    assert!(lifecycle.contains(
        "active_page_title = keep_str(pages_answer_is_current && !pages_fold_outran_reply, next.active_page_title, active_page_title)"
    ));
    assert!(lifecycle.contains(
        "let pages_answer_is_current = next.pages_loaded && pages_reply_answers_current(next.pages, next.active_page, active_page)"
    ));
    assert!(
        lifecycle.contains("let pages_fold_outran_reply = next.fold_serial != pages_fold_serial")
    );
    // The fold site is the ONE writer of the serial: a text fold bumps it, and
    // every resync request snapshots it, or the token guards nothing.
    assert!(lifecycle.contains(
        "pages_fold_serial = keep_i64(pages_delta_folds(next.pages), pages_fold_serial + 1, pages_fold_serial)"
    ));
    // The page LIST's structure is never stale — it is the whole index either
    // way — but shared rows keep their folded titles.
    assert!(lifecycle.contains(
        "pages = keep_pages(next.pages_loaded, keep_folded_page_titles(pages_fold_outran_reply, next.pages, pages), pages)"
    ));
    assert!(lifecycle.contains(
        "blocks = keep_blocks(pages_answer_is_current, merge_pending_blocks(keep_folded_block_texts(pages_fold_outran_reply, next.blocks, blocks), blocks, buffer_page, next.active_page, \"\"), blocks)"
    ));
    // A live resync must never install remote text over a buffer the user is
    // still typing in; the buffer and its dirty baseline move on ONE decision.
    assert!(lifecycle.contains("page_editor = refreshed_page_editor("));
    assert!(lifecycle.contains("page_saved_text = resynced_saved"));
    // the comment rail is scoped to the PAGE it hangs off, so its draft
    // survives moving the cursor between blocks and dies with the page.
    assert!(lifecycle.contains(
        "block_comment_draft = retain_selected_string(block_comment_draft, block_comments_target)"
    ));
    // the live comment-list callback settles state and stops — re-entering
    // the resync from inside it would loop the rail against the page.
    let pages_handlers = inlined(include_str!("ui/handlers/pages.ice"));
    let comment_callbacks = pages_handlers
        .split_once("on block_threads_loaded(next)\n")
        .unwrap()
        .1
        .split_once("\non load_more_block_threads")
        .unwrap()
        .0;
    assert!(!comment_callbacks.contains("run "));
}

#[test]
fn context_destroying_page_handlers_recover_drafts() {
    let pages = inlined(include_str!("ui/handlers/pages.ice"));
    // The page BODY is no longer among the drafts to rescue: it is one buffer
    // that flushes to the node on its own tick and is reinstalled from the
    // node's text on the next load. A half-typed COMMENT still has nowhere
    // else to live, so every context-destroying handler still guards it.
    for name in [
        "open_page_search_hit(page_id, _block_id)",
        "choose_page(id)",
        "toggle_block_comments",
        "pages_mutated(next)",
    ] {
        let rest = pages.split_once(&format!("on {name}")).unwrap().1;
        let body = rest.split_once("\non ").map_or(rest, |(body, _)| body);
        assert!(body.contains("remember_orphaned_comment_drafts("), "{name}");
    }
    let close_comments = pages
        .split_once("on close_block_comments\n")
        .unwrap()
        .1
        .split_once("\non ")
        .unwrap()
        .0;
    assert!(close_comments.contains("remember_orphaned_comment_drafts("));
}

#[test]
fn stale_resyncs_are_ignored_and_deltas_fold_without_reloads() {
    let (mut app, _) = Ducktape::__boot();
    app.status = "current".into();
    app.hydration_generation = 3;
    app.loading = false;

    // a channel switch invalidates any in-flight resync
    let _ = app.__update(__DucktapeMessage::ChooseChannel("next".into()));
    assert_eq!(app.hydration_generation, 4);

    // a chat delta folds straight into state — no reload cycle
    app.loading = false;
    app.active_channel = "next".into();
    let _ = app.__update(__DucktapeMessage::LiveUpdated(posted_delta(
        "next",
        message(1, "hello from the feed", false),
    )));
    assert_eq!(app.messages.len(), 1);
    assert_eq!(app.messages[0].body, "hello from the feed");

    // a resync from a superseded generation is dropped whole
    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
        3,
        "stale",
        vec![message(9, "stale", false)],
        "stale-page",
        Vec::new(),
    )));
    assert_eq!(app.active_channel, "next");
    assert_eq!(app.messages[0].body, "hello from the feed");

    // the current generation applies
    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
        4,
        "next",
        vec![message(1, "hello from the feed", false)],
        "page",
        Vec::new(),
    )));
    assert_eq!(app.active_page_title, "page");
}

#[test]
fn consecutive_deltas_fold_in_place_and_keep_the_freshest_status() {
    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.hydration_generation = 10;
    app.active_channel = "general".into();

    let _ = app.__update(__DucktapeMessage::LiveUpdated(posted_delta(
        "general",
        message(1, "first", false),
    )));
    let _ = app.__update(__DucktapeMessage::LiveUpdated(posted_delta(
        "general",
        message(2, "second", false),
    )));
    assert_eq!(app.messages.len(), 2);
    assert_eq!(app.messages[1].body, "second");
    assert_eq!(
        app.hydration_generation, 10,
        "chat deltas never start a reload cycle"
    );
}

#[test]
fn resyncs_cannot_retarget_drafts_to_fallback_contexts() {
    let (mut app, _) = Ducktape::__boot();
    app.connected_rpc = "http://node".into();
    app.loading = false;
    app.hydration_generation = 7;
    app.active_channel = "deleted-channel".into();
    app.message_draft = "channel draft".into();
    app.selected_message_seq = 7;
    app.selected_message_rev = 2;
    app.message_action = MessageAction::Editing;
    app.message_edit_draft = "message edit".into();
    app.channel_settings_open = true;
    app.channel_name_draft = "channel rename".into();
    app.member_key_draft = "member".into();
    app.active_thread_seq = 7;
    app.thread_generation = 4;
    app.thread_target_seq = 9;
    app.thread_messages = vec![message(7, "old thread", false)];
    app.thread_next_reply_offset = 4;
    app.thread_has_more = true;
    app.thread_loading = true;
    app.reply_editor = compose("thread reply");
    app.active_page = "deleted-page".into();

    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
        7,
        "fallback-channel",
        vec![message(7, "same sequence, other channel", false)],
        "fallback-page",
        Vec::new(),
    )));

    assert_eq!(app.active_channel, "fallback-channel");
    assert_eq!(app.selected_message_seq, 0);
    assert_eq!(app.selected_message_rev, 0);
    assert_eq!(app.message_action, MessageAction::Toolbar);
    assert!(app.message_edit_draft.is_empty());
    assert!(!app.channel_settings_open);
    assert!(app.channel_name_draft.is_empty());
    assert!(app.member_key_draft.is_empty());
    assert_eq!(app.active_thread_seq, 0);
    assert_eq!(app.thread_generation, 5);
    assert_eq!(app.thread_target_seq, 0);
    assert!(app.thread_messages.is_empty());
    assert_eq!(app.thread_next_reply_offset, 0);
    assert!(!app.thread_has_more);
    assert!(!app.thread_loading);
    assert_eq!(
        backend::parked_reply_draft(app.reply_drafts.clone(), "deleted-channel".into(), 7,),
        "thread reply"
    );
    assert!(app.message_draft.is_empty());
    assert_eq!(app.active_page, "fallback-page");
}

#[test]
fn mutation_acks_preserve_open_editors_and_thread_state() {
    let (mut app, _) = Ducktape::__boot();
    app.active_channel = "general".into();
    app.selected_message_seq = 7;
    app.selected_message_rev = 2;
    app.message_action = MessageAction::Editing;
    app.message_edit_draft = "edit in progress".into();
    app.active_thread_seq = 9;
    app.thread_target_seq = 10;
    app.thread_messages = vec![message(9, "thread root", false)];
    app.thread_next_reply_offset = 3;
    app.thread_has_more = true;
    app.reply_editor = compose("reply in progress");
    app.message_editor = compose("next message");
    app.mutation_phase = MutationPhase::Channel;

    // an unrelated mutation's ack carries no snapshot — nothing to stomp
    // (reactions no longer route through ChatAcked at all; a channel op is
    // the surviving non-message phase)
    let _ = app.__update(__DucktapeMessage::ChatAcked(true));

    assert_eq!(app.selected_message_seq, 7);
    assert_eq!(app.message_action, MessageAction::Editing);
    assert_eq!(app.message_edit_draft, "edit in progress");
    assert_eq!(app.active_thread_seq, 9);
    assert_eq!(app.thread_target_seq, 10);
    assert_eq!(app.thread_messages.len(), 1);
    assert_eq!(app.thread_next_reply_offset, 3);
    assert!(app.thread_has_more);
    assert_eq!(reply_composer(&app), "reply in progress");
    assert_eq!(composer(&app), "next message");
    assert_eq!(app.mutation_phase, MutationPhase::Idle);
}

/// The picker-dismissal bug in one contract: an in-flight reaction must not
/// take the global mutation lock. A locked picker's disabled cells capture
/// no press, so the SECOND click of a picking session fell through to the
/// backdrop and dismissed the overlay; the hover bar's one-tap reactions
/// silently no-op'd through the same window.
#[test]
fn reactions_run_outside_the_mutation_lock() {
    let (mut app, _) = Ducktape::__boot();
    app.connected_rpc = "http://node".into();
    app.loading = false;
    app.active_channel = "general".into();
    app.messages = vec![message(7, "root", false)];
    app.selected_message_seq = 7;
    app.selected_message_rev = 1;
    app.message_action = MessageAction::Reactions;

    let _ = app.__update(__DucktapeMessage::AddReactionSubmit("👍".into()));

    assert_eq!(
        app.mutation_phase,
        MutationPhase::Idle,
        "reactions never take the lock"
    );
    assert_eq!(app.selected_message_seq, 7);
    assert_eq!(
        app.message_action,
        MessageAction::Reactions,
        "the picker stays open"
    );

    // the ack leaves the picker exactly where it was — multi-pick works
    let _ = app.__update(__DucktapeMessage::ReactionAcked(true));
    assert_eq!(app.selected_message_seq, 7);
    assert_eq!(app.message_action, MessageAction::Reactions);
    assert_eq!(app.mutation_phase, MutationPhase::Idle);
}

/// THE CANONICAL REFETCH *IS* THE REVERT. A reaction fold is not invertible
/// under concurrent deltas, so a refusal carries no rollback token — and
/// nothing else can heal it: a chat delta folds a reactor SET, it never
/// replaces one, so a chip the chain refused survives every later message until
/// the room is switched. The resync `reaction_failed` launches is the only
/// thing that takes it back, on both copies of the row.
#[test]
fn a_refused_reaction_is_reverted_by_the_resync_it_launches() {
    let mut tapped = message(7, "root", false);
    tapped.reactions = vec![backend::ChatReaction {
        emoji: "👍".into(),
        count: 1,
        reacted_by_me: true,
        reactors: vec!["user:aa11".into()],
    }];

    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.loading = false;
    app.active_channel = "general".into();
    app.channels = vec![room("general", 7)];
    app.messages = vec![tapped.clone()];
    app.thread_messages = vec![tapped];
    app.active_thread_seq = 7;
    let resync_before = app.hydration_generation;

    let _ = app.__update(__DucktapeMessage::ReactionFailed(backend::AppError {
        message: "the chain refused it".into(),
        committed: false,
    }));
    assert_eq!(app.error, "the chain refused it");
    assert_ne!(
        app.hydration_generation, resync_before,
        "a fresh resync is issued to fetch what is actually there"
    );
    assert_eq!(
        app.mutation_phase,
        MutationPhase::Idle,
        "and it never took the mutation lock to do it"
    );

    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
        app.hydration_generation,
        "general",
        vec![message(7, "root", false)],
        "",
        Vec::new(),
    )));
    assert!(
        app.messages[0].reactions.is_empty(),
        "the canonical page takes the chip back off the timeline"
    );
    assert_eq!(
        app.active_thread_seq, 7,
        "the open rail remains its own scope"
    );

    // AND THE CEILING IT REVERTS UNDER, pinned so a later change to the merge
    // cannot widen it by accident. The refetch answers with the tail; a tap on a
    // row the reader had paged BACK to is outside that page, so no canonical row
    // wins on `rev` and the phantom chip rides along until she re-enters the
    // room. Replacing the whole window here instead would take it back — and
    // throw away every "Load older" page, which is the trade this seam refuses.
    let mut paged_back = message(3, "months ago", false);
    paged_back.reactions = vec![backend::ChatReaction {
        emoji: "👍".into(),
        count: 1,
        reacted_by_me: true,
        reactors: vec!["user:aa11".into()],
    }];
    app.messages = vec![paged_back, message(45, "still on screen", false)];
    let _ = app.__update(__DucktapeMessage::ReactionFailed(backend::AppError {
        message: "the chain refused it".into(),
        committed: false,
    }));
    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
        app.hydration_generation,
        "general",
        vec![
            message(45, "still on screen", false),
            message(50, "the tail", false),
        ],
        "",
        Vec::new(),
    )));
    assert_eq!(
        app.messages.iter().map(|row| row.seq).collect::<Vec<_>>(),
        vec![3, 45, 50],
        "her scrollback survives the revert, which is the point of the fold"
    );
    assert!(
        !app.messages[0].reactions.is_empty(),
        "and the chip on the row the page does not cover is the known residue"
    );
}

/// BOTH COPIES OF THE ROW TAKE THE TAP. A message on screen can be the root (or
/// a reply) of the thread rail open beside it, so a tap that folded into one
/// list left the two disagreeing about the count until the room was switched.
/// The un-react direction is the same fold with `added: false` and carries the
/// same obligation.
#[test]
fn every_reaction_tap_folds_into_the_timeline_and_the_thread_rail() {
    let chat = inlined(include_str!("ui/handlers/chat.ice"));
    for handler in [
        "add_reaction_submit(emoji)",
        "add_reaction_at(seq, emoji)",
        "remove_reaction_at(seq, emoji)",
    ] {
        let body = chat
            .split_once(&format!("on {handler}\n"))
            .unwrap_or_else(|| panic!("{handler} is a handler"))
            .1
            .split_once("\non ")
            .expect("a handler ends at the next one")
            .0;
        assert!(
            body.contains("messages = reaction_applied(messages,"),
            "{handler} folds the timeline"
        );
        assert!(
            body.contains("thread_messages = reaction_applied(thread_messages,"),
            "{handler} folds the rail with it"
        );
    }

    // and the un-react path is wired and gated like the others
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.loading = false;
    app.active_channel = "general".into();
    app.error = "something older".into();

    let before = app.hydration_generation;
    let _ = app.__update(__DucktapeMessage::RemoveReactionAt(7, "👍".into()));
    assert!(app.error.is_empty());
    assert_ne!(app.hydration_generation, before);
    assert_eq!(app.mutation_phase, MutationPhase::Idle);

    let armed = app.hydration_generation;
    let _ = app.__update(__DucktapeMessage::RemoveReactionAt(0, "👍".into()));
    assert_eq!(
        app.hydration_generation, armed,
        "there is no row 0 to un-react"
    );
    app.active_channel_archived = true;
    let _ = app.__update(__DucktapeMessage::RemoveReactionAt(7, "👍".into()));
    assert_eq!(
        app.hydration_generation, armed,
        "and an archived room takes no reaction either way"
    );
    assert!(
        app.error.contains("archived"),
        "and it says so — see the refusal test below"
    );
}

/// AN ARCHIVED CHANNEL REFUSES A REACTION OUT LOUD. The module refuses it
/// (`check_post_policy`, reached through `reaction_target`), and the handlers
/// have always refused it first — silently: no error, no state change, nothing
/// to tell a dropped press from a landed one. The surface cannot carry that
/// refusal instead, because the quiet message rows are `lazy` on ONE dependency
/// and `active_channel_archived` never reaches a chip or a one-tap bar. So the
/// banner is the affordance, and ♡ must not open a picker whose 32 cells are
/// all disabled.
#[test]
fn an_archived_channel_says_why_it_dropped_the_reaction() {
    let archived_routes: Vec<(&str, __DucktapeMessage)> = vec![
        ("one-tap", __DucktapeMessage::AddReactionAt(7, "👍".into())),
        (
            "picker cell",
            __DucktapeMessage::AddReactionSubmit("👍".into()),
        ),
        (
            "chip removal",
            __DucktapeMessage::RemoveReactionAt(7, "👍".into()),
        ),
    ];
    for (route, press) in archived_routes {
        let (mut app, _) = Ducktape::__boot();
        app.connected_rpc = "http://node".into();
        app.loading = false;
        app.active_channel = "general".into();
        app.active_channel_archived = true;
        app.messages = vec![message(7, "root", false)];
        app.selected_message_seq = 7;
        app.selected_message_rev = 1;

        let _ = app.__update(press);

        assert!(
            app.error.contains("archived"),
            "the {route} refusal must name the archive"
        );
        // No optimistic fold: the refusal is not a half-applied reaction.
        assert!(app.messages[0].reactions.is_empty(), "{route}");
    }

    // ♡ OPENS NOTHING ON AN ARCHIVED CHANNEL — in the stream and in the RAIL
    // alike. The picker it opened was a dead-end overlay of 32 disabled cells
    // whose only exit was Esc, and the rail mounts the very same one; the ⋯
    // menu's "Manage reactions" row routes here too, live precisely so its
    // press reaches this refusal instead of dying on a disabled button.
    // A ♡ route: what the press is, and which action slot its picker lands in.
    type PickerRoute = (
        &'static str,
        fn() -> __DucktapeMessage,
        fn(&Ducktape) -> MessageAction,
    );
    let picker_routes: [PickerRoute; 2] = [
        (
            "stream ♡",
            || __DucktapeMessage::OpenMessageReactions(7, "root".into(), 1),
            |app| app.message_action,
        ),
        (
            "rail ♡",
            || __DucktapeMessage::OpenThreadMessageReactions(7, "root".into(), 1),
            |app| app.thread_message_action,
        ),
    ];
    for (route, press, opened_action) in picker_routes {
        let (mut app, _) = Ducktape::__boot();
        app.connected_rpc = "http://node".into();
        app.loading = false;
        app.active_channel = "general".into();
        app.active_channel_archived = true;
        app.messages = vec![message(7, "root", false)];

        let _ = app.__update(press());

        assert_eq!(
            opened_action(&app),
            MessageAction::Toolbar,
            "{route}: the picker never opened"
        );
        assert!(app.error.contains("archived"), "{route}: and it said why");

        // AND ON A LIVE CHANNEL THE REFUSAL LINE WRITES NOTHING. Opening ♡ is a
        // READ — it must hand the standing banner back untouched, or reaching
        // for a reaction becomes the gesture that wipes the failure you had not
        // read yet. Only the three mutations clear it, on their own line, where
        // the clear has always been.
        app.active_channel_archived = false;
        app.error = "Send failed — the node refused the message.".into();
        let _ = app.__update(press());
        assert_eq!(
            app.error, "Send failed — the node refused the message.",
            "{route}: opening the picker is a read and must not clear the banner"
        );
        assert_eq!(
            opened_action(&app),
            MessageAction::Reactions,
            "{route}: the picker opened"
        );

        // …and the mutation that follows still clears the banner on its own
        // line. Only the banner is read here: the fold itself needs the
        // process-wide `cached_user_key`, which is what
        // `every_reaction_tap_folds_into_the_timeline_and_the_thread_rail`
        // covers — asserting it from here would make this test depend on
        // whichever sibling happened to seed that global first.
        app.selected_message_seq = 7;
        let _ = app.__update(__DucktapeMessage::AddReactionAt(7, "👍".into()));
        assert_eq!(
            app.error, "",
            "{route}: the mutation clears the banner it replaces"
        );
    }

    // AND THE COMMENT OVER `add_reaction_submit` CLAIMS ALL FIVE. A hardcoded
    // list of the five cannot keep that claim honest — it stays green over a
    // sixth route it does not name, which is the only failure it exists to
    // catch. So the ROUTES SELECT THEMSELVES: walk every handler and conscript
    // the ones that reach a reaction op (`run every add_reaction(` /
    // `remove_reaction(`) or open the picker (`_action = MessageAction.reactions`). Those
    // discriminants are the ACTS, so the landings that merely fold one —
    // `reaction_acked`, `reaction_failed` — are not swept in, and prose naming
    // an op does not conscript a handler that never calls it.
    let chat = inlined(include_str!("ui/handlers/chat.ice"));
    let mut reaction_routes: Vec<&str> = Vec::new();
    for block in chat.split("\non ").skip(1) {
        let handler = block.split('(').next().unwrap_or(block).trim();
        let handler = handler.lines().next().unwrap_or(handler).trim();
        let reaches_reaction = block.lines().any(|line| {
            let statement = line.trim_start();
            let is_comment = statement.starts_with("//");
            let taps = statement.contains("run every add_reaction(")
                || statement.contains("run every remove_reaction(");
            let opens_picker = statement.contains("_action = MessageAction.reactions");
            !is_comment && (taps || opens_picker)
        });
        if !reaches_reaction {
            continue;
        }
        reaction_routes.push(handler);
        assert!(
            block.contains("error = reaction_refusal(active_channel_archived, error)")
                && block.contains("return if active_channel_archived"),
            "`on {handler}` reaches a reaction op and must answer an archived \
             channel with the banner"
        );
    }
    assert_eq!(
        reaction_routes,
        [
            "open_thread_message_reactions",
            "open_message_reactions",
            "add_reaction_submit",
            "add_reaction_at",
            "remove_reaction_at",
        ],
        "a route started or stopped reaching a reaction op: it owes the refusal"
    );
}

#[test]
fn a_tombstoned_thread_root_renders_deleted_in_place() {
    let (mut app, _) = Ducktape::__boot();
    app.connected_rpc = "http://node".into();
    app.loading = false;
    app.active_channel = "general".into();
    app.messages = vec![message(9, "thread root", false)];
    app.active_thread_seq = 9;
    app.thread_target_seq = 10;
    app.thread_messages = vec![message(9, "thread root", false)];
    app.reply_editor = compose("unsent reply");

    // the root's delete arrives as a delta: both lists tombstone the row
    // in place; the open thread stays open showing the tombstone (the
    // module allows replying to a tombstoned root).
    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: "chat".into(),
        status: "Live".into(),
        height: 5,
        chat: backend::ChatDelta {
            kind: "deleted".into(),
            channel_id: "general".into(),
            seq: 9,
            ..backend::ChatDelta::default()
        },
        ..backend::LiveUpdate::default()
    }));

    assert!(app.messages[0].deleted);
    assert!(app.thread_messages[0].deleted);
    assert_eq!(app.thread_messages[0].body, "Message deleted");
    assert_eq!(app.active_thread_seq, 9, "the panel stays open");
    assert_eq!(reply_composer(&app), "unsent reply");
}

#[test]
fn unrelated_resyncs_keep_an_initial_thread_load_alive() {
    let (mut refresh, _) = Ducktape::__boot();
    refresh.connected_rpc = "http://node".into();
    refresh.active_channel = "general".into();
    refresh.loading = false;
    refresh.mutation_phase = MutationPhase::Idle;
    refresh.thread_generation = 6;
    let _ = refresh.__update(__DucktapeMessage::OpenThreadFor(7));
    assert_eq!(refresh.active_thread_seq, 7);
    assert_eq!(refresh.thread_generation, 7);
    assert!(refresh.thread_loading);
    refresh.hydration_generation = 5;

    // an unrelated resync leaves the in-flight thread load untouched
    let _ = refresh.__update(__DucktapeMessage::LiveResynced(live_refresh(
        5,
        "general",
        vec![message(7, "root", false)],
        "",
        Vec::new(),
    )));
    assert_eq!(refresh.active_thread_seq, 7);
    assert_eq!(refresh.thread_generation, 7);
    assert!(refresh.thread_loading);
    let _ = refresh.__update(__DucktapeMessage::ThreadLoaded(backend::ThreadLoadData {
        generation: 7,
        root_seq: 7,
        target_seq: 7,
        messages: vec![message(7, "root", false)],
        next_reply_offset: 1,
        has_more: true,
    }));
    assert_eq!(refresh.active_thread_seq, 7);
    assert_eq!(refresh.thread_messages.len(), 1);
    assert!(!refresh.thread_loading);
}

/// A HISTORY PAGE BELONGS TO THE CHANNEL THAT ASKED FOR IT. The compiler drops
/// a superseded `history` run, while `HistoryPageData` carries the channel for
/// room movement that starts no replacement history request.
///
/// The flag is released ABOVE that check: a page dropped for landing in the
/// wrong room must still free "Load older", which `load_more_history` refuses
/// while `history_loading` stands.
#[test]
fn a_history_page_prepends_only_into_the_channel_that_asked_for_it() {
    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.mutation_phase = MutationPhase::Idle;
    app.active_channel = "a".into();
    app.messages = vec![message(10, "a-ten", false)];
    let _ = app.__update(__DucktapeMessage::LoadMoreHistory);
    assert!(app.history_loading);

    // The reader is on #b by the time the page lands. `active_channel` moves
    // under an open request on the resync, the search-hit and the create routes
    // too, without necessarily starting another history request.
    app.active_channel = "b".into();
    app.messages = vec![message(10, "b-ten", false)];
    let _ = app.__update(__DucktapeMessage::HistoryLoaded(backend::HistoryPageData {
        channel_id: "a".into(),
        messages: vec![message(1, "a-one", false)],
    }));
    assert_eq!(
        app.messages.len(),
        1,
        "a page for #a must not prepend into #b's timeline"
    );
    assert_eq!(app.messages[0].body, "b-ten");
    assert!(
        !app.history_loading,
        "the dropped page still frees `Load older` in the room she is in"
    );

    // The same page stamped for #b IS #b's history, and prepends.
    let _ = app.__update(__DucktapeMessage::LoadMoreHistory);
    let _ = app.__update(__DucktapeMessage::HistoryLoaded(backend::HistoryPageData {
        channel_id: "b".into(),
        messages: vec![message(1, "b-one", false)],
    }));
    assert_eq!(app.messages.len(), 2);
    assert_eq!(app.messages[0].body, "b-one");
    assert!(!app.history_loading);

    // AND THE FLAG DOES NOT SURVIVE ANY ROUTE THAT ABANDONS ITS REQUEST.
    // `load_more_history` returns early on it, so until the abandoned page lands
    // — forever if it hangs — "Load older" is dead in the room she lands in.
    // Every LAUNCH that starts a room transition is here, not just the two
    // channel pickers: the search hit and the create both land in a different
    // room, and the reconnect and the console open drop the socket the page was
    // requested on, so those two may never answer at all.
    for abandoning in [
        __DucktapeMessage::ChooseChannel("b".into()),
        __DucktapeMessage::ChooseDm("peer".into()),
        __DucktapeMessage::OpenChatSearchHit("b".into(), 7, 7),
        __DucktapeMessage::CreateChannelSubmit,
        __DucktapeMessage::Reconnect,
        __DucktapeMessage::ConsoleOpened(iced::window::Id::unique()),
    ] {
        let (mut app, _) = Ducktape::__boot();
        app.loading = false;
        app.mutation_phase = MutationPhase::Idle;
        app.active_channel = "a".into();
        app.messages = vec![message(10, "a-ten", false)];
        // `create_channel_submit` refuses an empty draft; the rest ignore it.
        app.channel_draft = "new-room".into();
        let _ = app.__update(__DucktapeMessage::LoadMoreHistory);
        assert!(
            app.history_loading,
            "the route must start with a live request"
        );
        let route = format!("{abandoning:?}");
        let _ = app.__update(abandoning);
        assert!(
            !app.history_loading,
            "{route} abandons the request, so it must release the flag"
        );
    }
}

/// A RESYNC IS THE ONE DROPPER THAT MUST ASK. Every other route that abandons a
/// history request is a launch the reader drove, so it clears the flag flatly.
/// `live_resynced` is server-driven and moves `active_channel` on its own, so a
/// flat clear would strand a page that is still legitimately coming: the reducer
/// refuses any page arriving with the flag already down (`|| !history_loading`),
/// which would drop it silently and leave the timeline short.
#[test]
fn a_resync_releases_load_older_only_when_it_moves_the_room() {
    for (landing, expected) in [("a", true), ("b", false)] {
        let (mut app, _) = Ducktape::__boot();
        app.loading = false;
        app.mutation_phase = MutationPhase::Idle;
        app.active_channel = "a".into();
        app.messages = vec![message(10, "a-ten", false)];
        let _ = app.__update(__DucktapeMessage::LoadMoreHistory);
        assert!(app.history_loading);

        let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
            app.hydration_generation,
            landing,
            vec![message(10, "ten", false)],
            "",
            Vec::new(),
        )));
        assert_eq!(
            app.history_loading,
            expected,
            "a resync landing on #{landing} from #a must {} the flag",
            if expected { "keep" } else { "release" }
        );
    }
}

/// THE ROUTE LIST IS THE INVARIANT, SO THE ROUTE LIST IS PINNED. A ninth handler
/// that moves the reader between rooms has to decide whether it abandons a
/// history request, and nothing about writing one would prompt that thought —
/// which is exactly how the five uncovered routes above got written. This fails
/// the build on a new mover so the decision is forced, rather than trusting the
/// next author to remember an invariant spread across three files.
///
/// LAUNCHES clear the flag themselves. LANDINGS do not, and must not be added
/// here without checking that every launch reaching them already cleared it:
/// `chat_updated` answers the two pickers, `chat_hit_loaded` answers the search
/// hit, `channel_created` answers the create, `workspace_connected` answers the
/// reconnect. `live_resynced` is a landing with NO launch behind it, which is
/// why it is the one that asks.
#[test]
fn every_handler_that_moves_the_reader_between_rooms_is_accounted_for() {
    const HANDLERS: &str = concat!(
        include_str!("ui/handlers/chat.ice"),
        include_str!("ui/handlers/lifecycle.ice"),
        include_str!("ui/handlers/onboarding.ice"),
    );

    let mut handler = "";
    let mut movers: Vec<&str> = Vec::new();
    for line in HANDLERS.lines() {
        if let Some(rest) = line.strip_prefix("on ") {
            handler = rest.split('(').next().unwrap_or(rest).trim();
        }
        if line.trim_start().starts_with("active_channel = ") {
            movers.push(handler);
        }
    }
    movers.sort_unstable();
    movers.dedup();

    assert_eq!(
        movers,
        [
            "channel_created",
            "chat_hit_loaded",
            "chat_updated",
            "choose_channel",
            "choose_dm",
            "console_opened",
            "live_resynced",
            "open_chat_search_hit",
            "reconnect",
            "workspace_connected",
        ],
        "a handler started or stopped moving `active_channel`: decide whether it \
         abandons an in-flight history page, then update this list"
    );

    // And the launches genuinely carry the clear — a mover list alone would pass
    // with every clear deleted.
    //
    // THE SECOND TERM RIDES THE SAME LIST. `chat_window_loading` is the "a chat
    // load is in flight" reading the history routes gained when a cache hit
    // stopped raising `loading`, and it splits this list in two: the three
    // routes that LAUNCH a window load raise it, and the three that abandon one
    // without starting another lower it — a launch that raised it with no
    // landing left to lower it refuses "Load older" for the rest of the session.
    for (launch, window_term) in [
        ("choose_channel", "chat_window_loading = true"),
        ("choose_dm", "chat_window_loading = true"),
        ("open_chat_search_hit", "chat_window_loading = true"),
        ("create_channel_submit", "chat_window_loading = false"),
        ("reconnect", "chat_window_loading = false"),
        ("console_opened", "chat_window_loading = false"),
    ] {
        let body = HANDLERS
            .split(&format!("\non {launch}"))
            .nth(1)
            .unwrap_or_else(|| panic!("{launch} is a handler"))
            .split("\non ")
            .next()
            .expect("handler body");
        assert!(
            body.contains("history_loading = false"),
            "{launch} abandons a history request and must release the flag"
        );
        assert!(
            body.contains(window_term),
            "{launch} must write `{window_term}`"
        );
    }

    // The landings lower it, and each is generation-guarded: a superseded reply
    // must not open the history routes against rows the winning load is still
    // about to replace.
    for landing in ["chat_updated", "chat_hit_loaded", "chat_load_failed"] {
        let body = HANDLERS
            .split(&format!("\non {landing}"))
            .nth(1)
            .unwrap_or_else(|| panic!("{landing} is a handler"))
            .split("\non ")
            .next()
            .expect("handler body");
        let guard = body
            .lines()
            .position(|line| line.trim_start().starts_with("return if "))
            .expect("a generation guard");
        let release = body
            .lines()
            .position(|line| line.trim() == "chat_window_loading = false")
            .expect("the window term is released");
        assert!(
            guard < release,
            "{landing} must release `chat_window_loading` BELOW its generation guard"
        );
    }

    // THE COMPOSER IS PER-ROOM, AND ITS TWO LINES ARE ORDER-DEPENDENT. The park
    // must read `active_channel` while it still names the room being LEFT and
    // the restore must read it once it names the room being ENTERED, so the
    // rule is not "both lines are present" but "park, move, restore" — a
    // restore above the move hands the old room its own draft back and the new
    // one whatever it was already holding.
    //
    // `channel_created` IS ON THIS LIST, and was the route that proved it has
    // to be: creating a channel lands the reader IN it (`create_channel_submit`
    // abandons the old room's load for exactly that reason), so a composer left
    // alone there followed her into the new room armed to send — and the next
    // switch parked those words under the NEW room's id, silently reattributing
    // them. The three landings that also write `active_channel`
    // (`chat_updated`, `chat_hit_loaded`, `live_resynced`) are NOT switches:
    // they re-affirm or correct the id of the room already on screen, so a park
    // there would file the live composer under a room she never left.
    //
    // AND ONE SWITCH IS SPREAD OVER TWO HANDLERS, which is how it escaped: the
    // pair is `reconnect` (blanks the room, carrying the live composer across)
    // and `workspace_connected` (lands on `landing_channel(channels)` — the
    // first room with traffic, which is rarely the room she left). Both halves
    // are named here so the ordering rule reaches across the round trip.
    let body_of = |handler: &str| {
        HANDLERS
            .split(&format!("\non {handler}"))
            .nth(1)
            .unwrap_or_else(|| panic!("{handler} is a handler"))
            .split("\non ")
            .next()
            .expect("handler body")
    };
    let at = |handler: &str, body: &str, token: &str| {
        body.lines()
            .position(|line| line.trim_start().starts_with(token))
            .unwrap_or_else(|| {
                panic!(
                    "{handler} moves the reader between contexts and must carry \
                     `{token}` — a composer that follows her is armed to send \
                     what she wrote next door into the room she clicked"
                )
            })
    };
    for (leaves, lands) in [
        ("choose_channel", "choose_channel"),
        ("choose_dm", "choose_dm"),
        ("open_chat_search_hit", "open_chat_search_hit"),
        ("channel_created", "channel_created"),
        ("reconnect", "workspace_connected"),
    ] {
        let out = body_of(leaves);
        let landing = body_of(lands);
        let park = at(leaves, out, "message_drafts = park_message_draft(");
        let rail_park = at(leaves, out, "reply_drafts = park_reply_draft(");
        let left = at(leaves, out, "active_channel = ");
        let arrived = at(lands, landing, "active_channel = ");
        let restore = at(
            lands,
            landing,
            "message_editor = editor(parked_message_draft(",
        );
        assert!(
            park < left && rail_park < left && arrived < restore,
            "{leaves} must park BOTH composers BEFORE it moves `active_channel` \
             and {lands} must restore AFTER (park {park}, rail park {rail_park}, \
             move {left}, landing move {arrived}, restore {restore})"
        );
    }

    // AND THE RAIL'S OWN SWITCH OBEYS THE SAME RULE ONE LEVEL DOWN, on
    // `active_thread_seq` instead of `active_channel`. `open_thread_for` is
    // what every "N replies" row in the timeline emits, so this is the ordinary
    // click that used to destroy a half-typed reply.
    //
    // THE PARK MUST SIT ABOVE THE WRITE, not merely inside the handler:
    // `park_reply_draft` refuses `thread_seq <= 0` outright, so a park read
    // below a line that can zero the seq — `live_resynced`'s deleted-root and
    // channel-move arms — is a guaranteed no-op that files nothing.
    for parker in ["open_thread_for", "live_resynced"] {
        let body = body_of(parker);
        let park = at(parker, body, "reply_drafts = park_reply_draft(");
        let moved = at(parker, body, "active_thread_seq = ");
        assert!(
            park < moved,
            "{parker} must park the reply BEFORE it moves `active_thread_seq` — \
             a park below the move reads a seq that no longer names the thread \
             (park {park}, move {moved})"
        );
    }

    // AND EVERY LANDING THAT SEATS A THREAD RESTORES BESIDE THE WRITE. Arriving
    // in a thread by any other route left an empty box over parked words, and
    // the first character typed into it parked OVER them under the same key —
    // silent overwrite, not just loss of a live buffer. `chat_hit_loaded` is the
    // reachable one (`load_chat_hit` answers with `root.seq` for a reply hit);
    // `chat_updated` and `channel_created` answer 0 today and ride the same
    // rule so a payload that starts seating a thread cannot forget it.
    //
    // THE SEATERS ARE DERIVED, NOT LISTED, exactly as the movers above are. A
    // literal `active_thread_seq = 0` RETIRES the rail rather than seating one,
    // so the zero writers stay off this list; every other writer lands on it,
    // and a new one fails the pin below until its restore decision is made.
    let mut handler = "";
    let mut seaters: Vec<&str> = Vec::new();
    for line in HANDLERS.lines() {
        if let Some(rest) = line.strip_prefix("on ") {
            handler = rest.split('(').next().unwrap_or(rest).trim();
        }
        let writes = line.trim_start().starts_with("active_thread_seq = ");
        let retires = line.trim() == "active_thread_seq = 0";
        if writes && !retires {
            seaters.push(handler);
        }
    }
    seaters.sort_unstable();
    seaters.dedup();

    assert_eq!(
        seaters,
        [
            "channel_created",
            "chat_hit_loaded",
            "chat_updated",
            "live_resynced",
            "open_thread_for",
            "thread_loaded",
        ],
        "a handler started or stopped seating `active_thread_seq`: decide \
         whether it restores the parked reply beside the write, then update \
         this list"
    );

    for landing in &seaters {
        // Two seaters land under a rail whose LIVE buffer is the truth, so
        // they must NOT restore — each is pinned to that refusal below.
        let live_buffer_is_the_truth = matches!(*landing, "live_resynced" | "thread_loaded");
        if live_buffer_is_the_truth {
            continue;
        }
        let body = body_of(landing);
        let moved = at(landing, body, "active_thread_seq = ");
        let restore = at(landing, body, "reply_editor = editor(parked_reply_draft(");
        assert!(
            moved < restore,
            "{landing} seats `active_thread_seq` and must restore the parked \
             reply AFTER it (move {moved}, restore {restore})"
        );
    }
    assert!(
        !body_of("thread_loaded").contains("parked_reply_draft("),
        "thread_loaded lands under a rail that is already open and typeable — a \
         restore there overwrites the keystrokes the round trip collected"
    );
    assert!(
        !body_of("live_resynced").contains("parked_reply_draft("),
        "live_resynced either leaves the rail on the thread it was already on — \
         where the live buffer is the truth — or closes it, and a closed rail \
         has no composer to fill"
    );

    // TWO READINGS OF THE ROOM RIDE WITH IT. `active_dm_peer` decides whether
    // the header names a peer instead of the channel (suppressing the `#` and
    // the channel name with it), and `history_view` decides whether the amber
    // banner claims these rows are old scrollback. Both used to be written by
    // one handler each — the DM picker and the search hit — so every OTHER
    // route that moved the room left them describing a pane that was gone.
    // A mover answers both or the build fails here.
    for mover in movers {
        let body = HANDLERS
            .split(&format!("\non {mover}"))
            .nth(1)
            .unwrap_or_else(|| panic!("{mover} is a handler"))
            .split("\non ")
            .next()
            .expect("handler body");
        // An ASSIGNMENT, not a mention: the token opens a statement line, so
        // prose naming the field where the write used to be fails here.
        for reading in ["active_dm_peer = ", "history_view = "] {
            assert!(
                body.lines()
                    .any(|line| line.trim_start().starts_with(reading)),
                "{mover} moves the reader between rooms and must answer \
                 `{reading}` — a reading of the room cannot outlive it"
            );
        }
    }
}

/// THE STREAM'S TAIL IS ASSERTED BY THE SWITCH, NOT INHERITED FROM THE LAST ROOM.
///
/// The scroll reset used to be a side effect of the timeline going empty: the
/// `if connected && !empty(messages)` gate around the stack dropped, the
/// scrollable unmounted, and its `scrollable::State` — the offset with it —
/// died. The window cache ended that. A handler that paints PARKED rows takes
/// `messages` non-empty → non-empty in one reducer pass, the gate never renders
/// false, and iced's positional `Tree::diff` hands the surviving offset to the
/// room being entered — a distance from the BOTTOM under `anchor-y=end`, so an
/// 800px scroll-up in #general opens #design 800px above its own tail.
///
/// So the rule is mechanical: a handler that restores a window from the cache
/// asserts the tail explicitly. Absolute `0.0` IS the tail under this anchor
/// (`snap-end`'s relative 1.0 would be the TOP of scrollback). The lint keys on
/// `cached_window(` rather than a hardcoded handler list, so the next picker
/// that gains a cache restore fails here until it carries the operation too.
#[test]
fn a_room_restored_from_the_cache_asserts_the_streams_tail() {
    const HANDLERS: &str = include_str!("ui/handlers/chat.ice");
    const TAIL: &str = "task widget scroll-to #workspace-tabs/content/chat/message-stream 0.0 0.0";

    let mut restorers: Vec<&str> = Vec::new();
    for block in HANDLERS.split("\non ").skip(1) {
        let handler = block.split('(').next().unwrap_or(block).trim();
        let handler = handler.lines().next().unwrap_or(handler).trim();
        // ANY CALL, NOT JUST A `let`-BOUND ONE. Keying on the `let ` prefix let
        // a restore written as `messages = cached_window(…).messages` slip the
        // fence entirely — the operation is what matters, not how its answer is
        // spelled. Comment lines are skipped so naming the function in prose
        // does not conscript a handler that never calls it.
        let restores = block
            .lines()
            .any(|line| !line.trim_start().starts_with("//") && line.contains("cached_window("));
        if !restores {
            continue;
        }
        restorers.push(handler);
        assert!(
            block.lines().any(|line| line.trim() == TAIL),
            "`on {handler}` paints parked rows into a scrollable that survives \
             the switch, so it must end with `{TAIL}` — the unmount is no longer \
             the reset"
        );
    }
    assert_eq!(
        restorers,
        ["choose_channel", "choose_dm"],
        "a handler started or stopped restoring a parked window: it owes the \
         stream its tail"
    );

    // And the screen no longer claims the gate is the reset — that comment sent
    // the last reader looking for a reset that #1059 had already deleted.
    const SCREEN: &str = include_str!("ui/screens/chat.ice");
    assert!(
        !SCREEN.contains("THIS GATE IS THE SCROLL RESET"),
        "the stream's gate stopped being the scroll reset when the cache landed"
    );
}

/// A MIRRORED VIEW READING IS ONLY AS GOOD AS ITS WRITERS, SO THE WRITERS ARE
/// PINNED. These fields exist purely so the view stops paying for them —
/// sidebar rows, page-comment anchors, huddle tile mute readings,
/// `post_refusal`, and `active_dm` — because a
/// `sync` extern takes every list BY VALUE and a call in a view expression is
/// therefore a deep clone per frame (the room projection also ran a SHA-256 per DM
/// peer, twice a frame). The trade is real: a mirror that a writer forgets is a
/// sidebar listing DMs under CHANNELS, an unread dot that never lights, a
/// composer refused in a room she may post in, or a stranger's face over the
/// header — none of which any type checker can see.
///
/// So the rule is mechanical and checked here: a handler that assigns any of a
/// mirror's SOURCES assigns the mirror too. That is what makes mirroring
/// cheaper than the per-frame call instead of six chances to drift, and it is
/// the same shape as the caret-retire and room-mover lints above.
#[test]
fn every_writer_of_a_mirrored_view_reading_refreshes_its_mirror() {
    // (mirror, the sources whose movement invalidates it). `settings_user_key`
    // is in two of them because THIS DEVICE'S KEY decides both which channels
    // are its own DMs and whether it is seated in a members-only room.
    const MIRRORS: [(&str, &[&str]); 8] = [
        (
            "rooms",
            &["channels", "dm_peers", "settings_user_key", "channel_reads"],
        ),
        ("dm_rows", &["channels", "dm_peers", "channel_reads"]),
        (
            "block_comment_rows",
            &["blocks", "block_comment_threads", "active_page"],
        ),
        (
            "active_thread_anchor",
            &["blocks", "active_thread_target", "active_page"],
        ),
        (
            "huddle_rows",
            &["huddle_roster", "call_peers", "call_muted"],
        ),
        ("fs_preview_entry", &["fs_entries", "fs_preview_path"]),
        (
            "post_refusal",
            &[
                "channel_members",
                "active_channel_archived",
                "active_channel_members_only",
                "settings_user_key",
            ],
        ),
        ("active_dm", &["active_dm_peer", "dm_peers"]),
    ];

    // Every handler file, because a mirror's source can move in any of them.
    macro_rules! handler_sources {
        ($($path:literal),* $(,)?) => { [$(($path, include_str!($path))),*] };
    }
    let files = handler_sources![
        "ui/handlers/chat.ice",
        "ui/handlers/files.ice",
        "ui/handlers/forge.ice",
        "ui/handlers/huddle.ice",
        "ui/handlers/lifecycle.ice",
        "ui/handlers/node.ice",
        "ui/handlers/onboarding.ice",
        "ui/handlers/overlays.ice",
        "ui/handlers/pages.ice",
        "ui/handlers/roster.ice",
    ];

    // An ASSIGNMENT opens a statement line — prose naming a field, and a call
    // that merely READS one, are not writes.
    let assigns = |body: &str, field: &str| {
        let statement = format!("{field} = ");
        body.lines()
            .any(|line| line.trim_start().starts_with(&statement))
    };

    let mut checked = 0usize;
    for (path, source) in files {
        for block in source
            .split(
                "
on ",
            )
            .skip(1)
        {
            let handler = block.split('(').next().unwrap_or(block).trim();
            let handler = handler.lines().next().unwrap_or(handler).trim();
            for (mirror, sources) in MIRRORS {
                let Some(moved) = sources.iter().find(|field| assigns(block, field)) else {
                    continue;
                };
                checked += 1;
                assert!(
                    assigns(block, mirror),
                    "{path}: `on {handler}` assigns `{moved}`, so it must also                      assign `{mirror}` — the view reads the mirror and never                      recomputes it (see state/chat.ice)"
                );
            }
        }
    }
    // The sweep must actually have found writers: a rename that silently
    // stopped matching would otherwise pass with nothing checked at all.
    assert!(
        checked >= 20,
        "the mirror sweep matched only {checked} writers — it has stopped seeing them"
    );
}

#[test]
fn a_failed_huddle_leave_keeps_the_retained_roster_visible() {
    let handler = inlined(include_str!("ui/handlers/huddle.ice"));
    let leave = handler
        .split_once("on leave_huddle_here")
        .expect("the leave handler exists")
        .1;

    assert!(leave.contains("call_peers = []"));
    assert!(
        leave.contains("huddle_rows = huddle_tile_rows(huddle_roster, call_peers, call_muted)")
    );
    assert!(
        !leave.contains("huddle_rows = []"),
        "an uncommitted leave failure retains the roster, so blanking its mirror is permanent"
    );
}

/// CLOSING RETIRES THE LOAD THAT WOULD REOPEN WHAT YOU JUST LEFT. Named request
/// lanes abort the work immediately, and the generation bump rejects a
/// completion already queued for delivery: `forge_repo_loaded` re-assigns
/// `forge_repo` and `forge_item_loaded` re-assigns `forge_item_number`, dropping
/// the user straight back into the repo or item they had just backed out of.
///
/// Review and merge launches snapshot repo + number into their completion
/// routes. That identity follows the request without requiring the backend to
/// echo UI routing state, while the busy flag still comes down before a stale
/// completion is rejected.
#[test]
fn closing_a_repo_or_an_item_retires_the_load_that_would_reopen_it() {
    let handlers = inlined(include_str!("ui/handlers/forge.ice"));
    let close_repo = handlers
        .split_once("on forge_close_repo")
        .expect("repo close handler")
        .1
        .split_once("\non ")
        .expect("repo close arm")
        .0;
    for lane in ["forge_repo", "forge_item", "forge_discussion", "forge_code"] {
        assert!(close_repo.contains(&format!("invalidate lane={lane}")));
    }
    let close_item = handlers
        .split_once("on forge_close_item")
        .expect("item close handler")
        .1
        .split_once("\non ")
        .expect("item close arm")
        .0;
    for lane in ["forge_item", "forge_discussion"] {
        assert!(close_item.contains(&format!("invalidate lane={lane}")));
    }

    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();

    let _ = app.__update(__DucktapeMessage::ForgeOpenRepo("core".into()));
    let in_flight = app.forge_generation;
    let _ = app.__update(__DucktapeMessage::ForgeCloseRepo);
    assert!(app.forge_repo.is_empty());
    let _ = app.__update(__DucktapeMessage::ForgeRepoLoaded(backend::ForgeRepoData {
        generation: in_flight,
        repo: "core".into(),
        branches: vec!["main".into()],
        items: Vec::new(),
    }));
    assert!(
        app.forge_repo.is_empty(),
        "a closed repo must not be reopened by the load it left in flight"
    );
    assert!(app.forge_branches.is_empty());

    // The same retirement one level in.
    let _ = app.__update(__DucktapeMessage::ForgeOpenRepo("core".into()));
    let _ = app.__update(__DucktapeMessage::ForgeOpenItem(7));
    let in_flight = app.forge_generation;
    let _ = app.__update(__DucktapeMessage::ForgeCloseItem);
    assert_eq!(app.forge_item_number, 0);
    let _ = app.__update(__DucktapeMessage::ForgeItemLoaded(backend::ForgeItemData {
        generation: in_flight,
        repo: "core".into(),
        number: 7,
        title: "a pull request".into(),
        ..backend::ForgeItemData::default()
    }));
    assert_eq!(
        app.forge_item_number, 0,
        "a closed item must not be reopened by the load it left in flight"
    );
    assert!(app.forge_item_title.is_empty());

    // AND THE MERGE FLAG COMES DOWN EVEN THOUGH ITS ITEM IS GONE.
    let (mut merging, _) = Ducktape::__boot();
    merging.connected = true;
    merging.connected_rpc = "http://node".into();
    merging.forge_repo = "core".into();
    merging.forge_item_number = 7;
    let _ = merging.__update(__DucktapeMessage::ForgeMergeSubmit);
    assert!(merging.forge_merge_busy);
    let _ = merging.__update(__DucktapeMessage::ForgeCloseItem);
    let _ = merging.__update(__DucktapeMessage::ForgeMerged(
        "http://node".into(),
        "core".into(),
        7,
        backend::ForgeMergeOutcome {
            merged: false,
            merge_oid: String::new(),
            conflicts: vec!["app/src/main.rs".into()],
        },
    ));
    assert!(
        !merging.forge_merge_busy,
        "closing an item mid-merge must not disable Merge for the rest of the session"
    );
    // The identity check still guards the BODY: that outcome describes an item
    // nobody has open, so nothing of it is rendered.
    assert!(merging.forge_merge_conflicts.is_empty());
}

#[test]
fn forge_code_reads_are_compiler_replaced_without_ui_generations() {
    let handlers = inlined(include_str!("ui/handlers/forge.ice"));
    for launch in [
        "forge_tree(connected_rpc, forge_repo, \"\", \"\")",
        "forge_tree(connected_rpc, forge_repo, forge_tree_rev, path)",
        "forge_blob(connected_rpc, forge_repo, forge_tree_rev, path)",
    ] {
        assert!(
            handlers.contains(&format!("run replace lane=forge_code {launch}")),
            "{launch} must supersede the previous code read"
        );
    }
    assert!(!handlers.contains("forge_code_generation"));

    let backend = include_str!("backend/forge.rs");
    assert!(backend.contains("item: item_slice.unwrap_or(noop.item)"));
}

#[test]
fn forge_scoped_reads_do_not_call_loading_or_failure_empty() {
    let (mut failed_list, _) = Ducktape::__boot();
    failed_list.forge_generation = 3;
    let _ = failed_list.__update(__DucktapeMessage::ForgeListFailed(
        backend::HydrationError {
            generation: 3,
            message: "forge unavailable".into(),
        },
    ));
    assert_eq!(failed_list.forge_list_phase, ForgePhase::Failed);
    assert_eq!(failed_list.error, "forge unavailable");

    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();

    let _ = app.__update(__DucktapeMessage::ForgeOpenRepo("core".into()));
    assert_eq!(app.forge_repo_phase, ForgePhase::Loading);
    assert_eq!(app.forge_code_phase, ForgeCodePhase::TreeLoading);
    let generation = app.forge_generation;

    let _ = app.__update(__DucktapeMessage::ForgeRepoLoaded(backend::ForgeRepoData {
        generation,
        repo: "core".into(),
        branches: Vec::new(),
        items: Vec::new(),
    }));
    assert_eq!(app.forge_repo_phase, ForgePhase::Ready);

    let _ = app.__update(__DucktapeMessage::ForgeTreeLoaded(backend::ForgeTreeData {
        repo: "other".into(),
        rev: "2222222222222222222222222222222222222222".into(),
        path: String::new(),
        born: true,
        entries: Vec::new(),
        truncated: false,
    }));
    assert_eq!(app.forge_code_phase, ForgeCodePhase::TreeLoading);
    assert!(!app.forge_tree_born, "another repo's tree must not paint");

    let _ = app.__update(__DucktapeMessage::ForgeTreeLoaded(backend::ForgeTreeData {
        repo: "core".into(),
        rev: "1111111111111111111111111111111111111111".into(),
        path: String::new(),
        born: true,
        entries: Vec::new(),
        truncated: true,
    }));
    assert_eq!(app.forge_code_phase, ForgeCodePhase::Ready);
    assert!(app.forge_tree_born);
    assert!(app.forge_tree_truncated);
    assert_eq!(
        app.forge_tree_rev, "1111111111111111111111111111111111111111",
        "nested tree and file reads stay pinned to the tree's commit"
    );

    let _ = app.__update(__DucktapeMessage::ForgeOpenDir("src".into()));
    let _ = app.__update(__DucktapeMessage::ForgeTreeLoaded(backend::ForgeTreeData {
        repo: "core".into(),
        rev: "2222222222222222222222222222222222222222".into(),
        path: "src".into(),
        born: true,
        entries: Vec::new(),
        truncated: false,
    }));
    assert_eq!(app.forge_code_phase, ForgeCodePhase::TreeLoading);
    assert!(
        app.forge_tree_entries.is_empty(),
        "a tree from another revision must not paint"
    );

    let _ = app.__update(__DucktapeMessage::ForgeTreeLoaded(backend::ForgeTreeData {
        repo: "core".into(),
        rev: "1111111111111111111111111111111111111111".into(),
        path: "src".into(),
        born: true,
        entries: Vec::new(),
        truncated: false,
    }));
    assert_eq!(app.forge_code_phase, ForgeCodePhase::Ready);

    let _ = app.__update(__DucktapeMessage::ForgeOpenFile("src/lib.rs".into()));
    let _ = app.__update(__DucktapeMessage::ForgeBlobLoaded(backend::BlobView {
        repo: "core".into(),
        rev: "1111111111111111111111111111111111111111".into(),
        path: "src/other.rs".into(),
        text: "wrong file".into(),
        truncated: false,
        binary: false,
        lines: 1,
    }));
    assert_eq!(app.forge_code_phase, ForgeCodePhase::FileLoading);
    assert!(app.forge_file_text.is_empty());

    let _ = app.__update(__DucktapeMessage::ForgeBlobLoaded(backend::BlobView {
        repo: "core".into(),
        rev: "1111111111111111111111111111111111111111".into(),
        path: "src/lib.rs".into(),
        text: "pub fn main() {}".into(),
        truncated: false,
        binary: false,
        lines: 1,
    }));
    assert_eq!(app.forge_code_phase, ForgeCodePhase::Ready);
    assert_eq!(app.forge_file_text, "pub fn main() {}");

    app.forge_item_channel = "forge:core:7".into();
    let _ = app.__update(__DucktapeMessage::ForgeDiscussionLoaded(
        backend::ForgeDiscussionData {
            channel_id: "forge:core:8".into(),
            messages: vec![message(1, "wrong item", false)],
            members: Vec::new(),
        },
    ));
    assert!(
        app.forge_discussion.is_empty(),
        "another item's discussion must not paint"
    );
    let _ = app.__update(__DucktapeMessage::ForgeDiscussionLoaded(
        backend::ForgeDiscussionData {
            channel_id: "forge:core:7".into(),
            messages: vec![message(1, "right item", false)],
            members: Vec::new(),
        },
    ));
    assert_eq!(app.forge_discussion[0].body, "right item");

    let _ = app.__update(__DucktapeMessage::ForgeOpenItem(7));
    assert_eq!(app.forge_item_phase, ForgePhase::Loading);
    let generation = app.forge_generation;
    let _ = app.__update(__DucktapeMessage::ForgeItemFailed(
        backend::HydrationError {
            generation,
            message: "tracker unavailable".into(),
        },
    ));
    assert_eq!(app.forge_item_phase, ForgePhase::Failed);
    assert_eq!(app.error, "tracker unavailable");
}

#[test]
fn forge_directory_navigation_clears_the_previous_file_preview() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.forge_repo = "core".into();
    app.forge_tree_rev = "1111111111111111111111111111111111111111".into();
    app.forge_file_path = "old.rs".into();
    app.forge_file_text = "stale".into();
    app.forge_file_binary = true;
    app.forge_file_truncated = true;

    let _ = app.__update(__DucktapeMessage::ForgeOpenDir("src".into()));

    assert_eq!(app.forge_tree_path, "src");
    assert_eq!(app.forge_code_phase, ForgeCodePhase::TreeLoading);
    assert!(app.forge_file_path.is_empty());
    assert!(app.forge_file_text.is_empty());
    assert!(!app.forge_file_binary);
    assert!(!app.forge_file_truncated);
}

#[test]
fn forge_review_completion_cannot_clear_a_new_items_draft() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.forge_repo = "core".into();
    app.forge_item_number = 7;
    app.forge_review_draft = "review seven".into();

    let _ = app.__update(__DucktapeMessage::ForgeReviewSubmit);
    assert!(app.forge_review_busy);

    app.forge_item_number = 8;
    app.forge_review_draft = "review eight".into();
    let _ = app.__update(__DucktapeMessage::ForgeReviewSubmitted(
        "http://node".into(),
        "core".into(),
        7,
        true,
    ));

    assert!(!app.forge_review_busy);
    assert_eq!(app.forge_review_draft, "review eight");
}

#[test]
fn onboarding_capabilities_are_secret_buffers_cleared_on_navigation() {
    let (mut app, _) = Ducktape::__boot();
    let recovery = "duck ".repeat(24);
    let invite = "duck-capability".to_string();

    let _ = app.__update(__DucktapeMessage::__SecretTyped(
        "restore_words".into(),
        recovery.clone(),
    ));
    let _ = app.__update(__DucktapeMessage::__SecretTyped(
        "join_invite".into(),
        invite.clone(),
    ));
    assert_eq!(app.__ice_secrets.text("restore_words"), recovery);
    assert_eq!(app.__ice_secrets.text("join_invite"), invite);
    let snapshot = format!("{app:?}");
    assert!(!snapshot.contains("duck-capability"));
    assert!(!snapshot.contains("duck duck"));

    let _ = app.__update(__DucktapeMessage::GoNetworks);
    assert!(app.__ice_secrets.text("restore_words").is_empty());
    assert!(app.__ice_secrets.text("join_invite").is_empty());
}

#[test]
fn ready_events_rehydrate_without_rewinding_the_tip() {
    let (mut live, _) = Ducktape::__boot();
    live.loading = false;
    live.block_height = 41;
    live.hydration_generation = 2;
    let _ = live.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: "ready".into(),
        status: "Live".into(),
        height: -1,
        load_chat: true,
        load_pages: true,
        ..backend::LiveUpdate::default()
    }));
    assert_eq!(
        live.hydration_generation, 3,
        "ready starts the subscribe-then-hydrate catch-up resync"
    );
    assert_eq!(live.block_height, 41, "a heightless event keeps the tip");
}

#[test]
fn optimistic_sends_are_independent_and_never_erase_the_next_draft() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_channel = "general".into();
    // The first draft arrives as typed composer events — the same route a
    // real keystroke takes through the rich composer — so this also pins
    // the apply half of `chat_composer_event`, not just the submit half.
    for character in "first".chars() {
        let _ = app.__update(__DucktapeMessage::ChatComposerEvent(
            editor::ComposerEvent::Apply(editor::RichAction::Edit(
                iced::widget::text_editor::Action::Edit(iced::widget::text_editor::Edit::Insert(
                    character,
                )),
            )),
        ));
    }
    assert_eq!(composer(&app), "first");

    let _ = app.__update(__DucktapeMessage::ChatComposerEvent(
        editor::composer_submit_event(),
    ));
    let first_id = app.messages[0].id.clone();
    let first_view_key = app.messages[0].view_key;
    assert_eq!(app.mutation_phase, MutationPhase::Idle);
    assert!(app.message_draft.is_empty());
    assert!(composer(&app).is_empty());
    assert_eq!(app.messages.len(), 1);
    assert!(app.messages[0].pending);

    app.message_editor = compose("second");
    let _ = app.__update(__DucktapeMessage::ChatComposerEvent(
        editor::composer_submit_event(),
    ));
    let second_id = app.messages[1].id.clone();
    let second_view_key = app.messages[1].view_key;
    assert_ne!(first_id, second_id);
    assert_eq!(app.messages.len(), 2);
    assert!(app.messages.iter().all(|message| message.pending));

    app.message_editor = compose("third");
    // the submit receipt itself never touches the list…
    let _ = app.__update(__DucktapeMessage::MessageSent(backend::SendReceipt {
        operation_id: first_id.clone(),
        channel_id: "general".into(),
    }));
    assert_eq!(app.messages.len(), 2);
    assert!(app.messages.iter().all(|message| message.pending));

    // The SECOND send commits first. Root confirmation must sort by canonical
    // seq just like the thread rail while keeping that row's virtual identity.
    let mut second = message(1, "second", false);
    second.id = second_id.clone();
    let _ = app.__update(__DucktapeMessage::LiveUpdated(posted_delta(
        "general", second,
    )));
    assert_eq!(composer(&app), "third");
    assert_eq!(app.mutation_phase, MutationPhase::Idle);
    assert!(!app.messages[0].pending);
    assert_eq!(app.messages[0].seq, 1);
    assert_eq!(app.messages[0].id, second_id);
    assert_eq!(app.messages[0].view_key, second_view_key);
    assert_eq!(app.messages[1].id, first_id);
    assert_eq!(app.messages[1].view_key, first_view_key);
    assert!(app.messages[1].pending);

    // Then the first send lands at seq 2. Committed rows stay ordered, and
    // neither virtual key is replaced.
    let mut first = message(2, "first", false);
    first.id = first_id.clone();
    let _ = app.__update(__DucktapeMessage::LiveUpdated(posted_delta(
        "general", first,
    )));
    assert!(app.messages.iter().all(|message| !message.pending));
    assert_eq!(
        app.messages
            .iter()
            .map(|message| (message.id.as_str(), message.seq, message.view_key))
            .collect::<Vec<_>>(),
        [
            (second_id.as_str(), 1, second_view_key),
            (first_id.as_str(), 2, first_view_key),
        ]
    );

    // A canonical reload is allowed to replace rendered content, but not the
    // client-only identity of the same IDs.
    let reloaded = backend::merge_pending_messages(
        vec![message(1, "second", false), message(2, "first", false)]
            .into_iter()
            .zip([second_id.clone(), first_id.clone()])
            .map(|(mut message, id)| {
                message.id = id;
                message
            })
            .collect(),
        app.messages.clone(),
        "general".into(),
        "general".into(),
    );
    assert_eq!(
        reloaded
            .iter()
            .map(|message| message.view_key)
            .collect::<Vec<_>>(),
        [second_view_key, first_view_key]
    );

    let chat = inlined(include_str!("ui/screens/chat.ice"));
    assert!(chat.contains("keyed message in messages by=message.view_key"));
    assert!(!chat.contains("keyed message in messages by=message.seq"));
    assert!(chat.contains("stack #message(message.id) w=fill"));
    assert!(!chat.contains("#message(message.seq)"));
}

#[test]
fn history_windows_offer_a_jump_back_to_latest() {
    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.active_channel = "general".into();

    // landing on a search hit enters history mode…
    let _ = app.__update(__DucktapeMessage::ChatHitLoaded(chat_data(
        "general",
        vec![message(7, "an old message", false)],
    )));
    assert!(app.history_view);

    // …and a plain channel load (the Jump-to-latest path) leaves it
    let _ = app.__update(__DucktapeMessage::ChatUpdated(chat_data(
        "general",
        vec![message(50, "the latest", false)],
    )));
    assert!(!app.history_view);

    let chat = inlined(include_str!("ui/screens/chat.ice"));
    assert!(chat.contains("button \"Jump to latest\""));
    assert!(chat.contains("-> emit(choose_channel, active_channel)"));
}

/// THE BANNER DESCRIBES THE ROWS IN HAND, SO EVERY WRITER OF THEM ANSWERS IT.
///
/// `history_view` was raised by the search hit and lowered by a channel load,
/// and by nothing else — so a resync (a `files` write in another window, a
/// teammate joining a huddle, any plane op at all) replaced the window with
/// `load_chat_data`'s LATEST page and left the amber "Viewing history" banner
/// up over the live tail, with a "Jump to latest" that reloads the channel the
/// reader is already at the end of. Same after a create.
#[test]
fn a_resync_that_lands_the_live_tail_lowers_the_history_banner() {
    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.active_channel = "general".into();
    let _ = app.__update(__DucktapeMessage::ChatHitLoaded(chat_data(
        "general",
        vec![message(7, "six months ago", false)],
    )));
    assert!(app.history_view);

    // a resync carrying no chat news leaves the window — and its banner — alone
    let _ = app.__update(__DucktapeMessage::LiveResynced(backend::LiveRefresh {
        chat_loaded: false,
        ..live_refresh(
            app.hydration_generation,
            "general",
            Vec::new(),
            "",
            Vec::new(),
        )
    }));
    assert!(
        app.history_view,
        "a pages-only resync did not touch the timeline, so the window stands"
    );

    // one that carries chat replaced it with the latest page
    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
        app.hydration_generation,
        "general",
        vec![message(50, "the latest", false)],
        "",
        Vec::new(),
    )));
    assert!(
        !app.history_view,
        "the rows on screen are the tail now — the banner is a lie about them"
    );

    // and a create lands you in a brand-new room, which has no history at all
    let _ = app.__update(__DucktapeMessage::ChatHitLoaded(chat_data(
        "general",
        vec![message(7, "six months ago", false)],
    )));
    assert!(app.history_view);
    let _ = app.__update(__DucktapeMessage::ChannelCreated(chat_data(
        "brand-new",
        Vec::new(),
    )));
    assert!(!app.history_view);
}

/// A RESYNC ANSWERS WITH THE TAIL, SO IT FOLDS ONTO THE WINDOW — IT DOES NOT
/// REPLACE IT.
///
/// `load_chat_data` walks a bounded number of roots back from HEAD however far
/// the reader has paged, and the triggers are ordinary: a huddle join or leave
/// in the room on screen, a websocket reconnect, any chat op the delta path
/// cannot fold, the three chat failure resyncs. Assigning that page back threw
/// away every "Load older" page she had loaded — and, the scrollable staying
/// mounted at `anchor-y=end`, clamped her offset onto the top of the suddenly
/// short window, hundreds of rows forward from where she was reading, with no
/// banner and nothing to click to get back.
#[test]
fn a_chat_resync_keeps_the_pages_the_reader_paged_in() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.loading = false;
    app.active_channel = "general".into();
    app.channels = vec![room("general", 51)];

    // the live tail, then two "Load older" pages behind it
    let _ = app.__update(__DucktapeMessage::ChatUpdated(chat_data(
        "general",
        vec![
            message(50, "the tail", false),
            message(51, "and its next", false),
        ],
    )));
    let _ = app.__update(__DucktapeMessage::LoadMoreHistory);
    let _ = app.__update(__DucktapeMessage::HistoryLoaded(backend::HistoryPageData {
        channel_id: "general".into(),
        messages: vec![message(20, "older", false)],
    }));
    let _ = app.__update(__DucktapeMessage::LoadMoreHistory);
    let _ = app.__update(__DucktapeMessage::HistoryLoaded(backend::HistoryPageData {
        channel_id: "general".into(),
        messages: vec![message(2, "older still", false)],
    }));
    assert_eq!(backend::oldest_message_seq(app.messages.clone()), 2);
    assert!(!app.history_view, "back-paging is not a history window");

    // someone joins the huddle in this room, and the resync it forces answers
    // with the latest page alone
    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
        app.hydration_generation,
        "general",
        vec![
            message(51, "and its next", false),
            message(52, "arrived meanwhile", false),
        ],
        "",
        Vec::new(),
    )));

    assert_eq!(
        app.messages.iter().map(|row| row.seq).collect::<Vec<_>>(),
        vec![2, 20, 50, 51, 52],
        "the pages she loaded survive, with the fresh tail spliced onto them"
    );
    assert!(
        app.has_older_history,
        "and 'Load older' still points past the oldest row she holds"
    );

    // A HISTORY WINDOW IS STILL REPLACED: it is not contiguous with the tail,
    // so merging the two would leave a hole in the middle that nothing pages in.
    let _ = app.__update(__DucktapeMessage::ChatHitLoaded(chat_data(
        "general",
        vec![message(7, "six months ago", false)],
    )));
    assert!(app.history_view);
    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
        app.hydration_generation,
        "general",
        vec![message(52, "arrived meanwhile", false)],
        "",
        Vec::new(),
    )));
    assert_eq!(
        app.messages.iter().map(|row| row.seq).collect::<Vec<_>>(),
        vec![52],
        "the window is dropped whole, and the banner goes with it"
    );
    assert!(!app.history_view);
}

/// AND A SPLICE THAT DOES NOT TOUCH IS NOT A SPLICE — it is a HOLE, and the
/// hole is permanent.
///
/// `ModuleEvent::Lagged` says the client fell so far behind that the missed ops
/// are gone; the resync it forces answers with the last N roots, which can start
/// PAST the newest row on screen. Merging those two windows draws today's
/// messages directly under a stretch that is simply missing, and nothing can
/// ever fill it: "Load older" pages back from `oldest_message_seq`, now the
/// far-back end, so every click walks further AWAY from the gap. `history_view`
/// is not the only non-contiguous landing, so the test is the rows themselves.
#[test]
fn a_resync_the_window_cannot_reach_replaces_it_rather_than_leaving_a_hole() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.loading = false;
    app.active_channel = "general".into();
    app.channels = vec![room("general", 900)];
    app.messages = vec![
        message(2, "she paged back this far", false),
        message(20, "and read up to here", false),
    ];
    // an in-flight send of her own is still on screen, seq -1 and no page
    app.messages.push(backend::ChatMessage {
        view_key: -1,
        seq: -1,
        pending: true,
        ..message(0, "mine, still sending", false)
    });

    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
        app.hydration_generation,
        "general",
        vec![
            message(880, "the tail after the lag", false),
            message(900, "and today", false),
        ],
        "",
        Vec::new(),
    )));

    assert_eq!(
        app.messages
            .iter()
            .filter(|row| !row.pending)
            .map(|row| row.seq)
            .collect::<Vec<_>>(),
        vec![880, 900],
        "the unreachable window is dropped whole — no 20-then-880 seam"
    );
    assert_eq!(
        backend::oldest_message_seq(app.messages.clone()),
        880,
        "so 'Load older' walks back from the tail, into the gap and not past it"
    );
    assert!(
        app.messages.iter().any(|row| row.pending),
        "and her own in-flight send is not collateral"
    );
}

/// A PLANE OP IS NOT "JUMP TO LATEST".
///
/// The resync that lands on every files write, valset change, identity, agent
/// or governance op carries no chat — the search window and its amber banner are
/// still exactly what is on screen — and it used to mark the room read to a head
/// the reader has demonstrably not reached — and `mark_channel_read` only moves
/// forward, so the badge `chat_sidebar_rooms` paints off that cursor never comes
/// back. `chat_hit_loaded` refuses that write; this is the handler that was
/// undoing it one save later.
#[test]
fn a_plane_resync_leaves_a_search_window_and_its_badge_alone() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_channel = "general".into();
    // she read up to 10 when she connected; thirty messages have landed since
    app.channels = vec![room("general", 10)];
    app.channel_reads = backend::initial_channel_reads(app.channels.clone(), Vec::new());
    app.channels = vec![room("general", 40)];

    let _ = app.__update(__DucktapeMessage::ChatHitLoaded(chat_data(
        "general",
        vec![message(7, "six months ago", false)],
    )));
    assert!(app.history_view);
    assert!(
        app.rooms
            .iter()
            .any(|row| row.channel.id == "general" && row.unread)
    );

    let plane_only = backend::LiveRefresh {
        chat_loaded: false,
        pages_loaded: false,
        ..live_refresh(
            app.hydration_generation,
            "general",
            Vec::new(),
            "",
            Vec::new(),
        )
    };
    let _ = app.__update(__DucktapeMessage::LiveResynced(plane_only));

    assert!(
        app.history_view,
        "the banner is still the only way back to the tail"
    );
    assert_eq!(
        app.messages.iter().map(|row| row.seq).collect::<Vec<_>>(),
        vec![7],
        "and the window around the hit is still what she is reading"
    );
    assert!(
        app.rooms
            .iter()
            .any(|row| row.channel.id == "general" && row.unread),
        "so the room is no more read than it was before she saved a file"
    );
}

/// A HISTORY WINDOW IS A SNAPSHOT, NOT A LIVE TAIL.
///
/// The rows in hand are a window around one old message, so a post from today
/// has a seq past every one of them and `insert_committed_root` appends it to
/// the END of the window — today's message drawn directly under one from six
/// months ago, and (authors matching) folded into the same run, with no gap
/// marker anywhere. Marking the channel read off that fold is the same lie in
/// the sidebar: the reader is not caught up on a room she is reading backwards.
#[test]
fn a_live_post_does_not_splice_itself_into_a_history_window() {
    // the room as the sidebar knows it. re-seated by hand between steps: a
    // landing FOLDS its refreshed row into this list rather than installing
    // one (`upsert_channel_rows`), so the fixture has to put the row back.
    let room = || {
        vec![backend::ChatChannel {
            id: "general".into(),
            name: "general".into(),
            archived: false,
            members_only: false,
            huddle_count: 0,
            head_seq: 7,
        }]
    };
    let cursor = |app: &Ducktape| {
        app.channel_reads
            .iter()
            .find(|read| read.channel == "general")
            .map(|read| read.seq)
    };

    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.connected = true;
    app.active_channel = "general".into();
    // THE SIDEBAR ALREADY KNOWS THE ROOM'S HEAD when the hit lands, which is the
    // whole point: search is workspace-wide, so the hit routinely opens a room
    // with unread waiting, and `MessageWindow::Around` is not the tail.
    app.channels = room();
    let _ = app.__update(__DucktapeMessage::ChatHitLoaded(chat_data(
        "general",
        vec![message(7, "six months ago", false)],
    )));
    assert!(app.history_view);
    assert_eq!(
        cursor(&app),
        None,
        "opening a search hit is not catching up on the room it landed in"
    );
    assert!(
        app.rooms
            .iter()
            .any(|row| row.channel.id == "general" && row.unread),
        "so the badge she has not cleared is still lit"
    );
    app.channels = room();

    let _ = app.__update(__DucktapeMessage::LiveUpdated(posted_delta(
        "general",
        message(500, "posted just now", false),
    )));
    assert_eq!(
        app.messages.len(),
        1,
        "a message 493 seqs newer is not the next row of this window"
    );
    assert_eq!(
        cursor(&app),
        None,
        "and reading old scrollback is not being caught up on today's post"
    );

    // HER OWN SEND IS THE EXCEPTION. The composer posts from a window too and
    // splices the optimistic row in unconditionally, so a refused settle would
    // strand it `pending` forever.
    app.message_editor = compose("mine, from the window");
    let _ = app.__update(__DucktapeMessage::ChatComposerEvent(
        editor::composer_submit_event(),
    ));
    assert!(app.messages[1].pending);
    let mut settled = message(501, "mine, from the window", false);
    settled.id = app.messages[1].id.clone();
    app.channels = room();
    let _ = app.__update(__DucktapeMessage::LiveUpdated(posted_delta(
        "general", settled,
    )));
    assert_eq!(app.messages.len(), 2, "her row settled in place, not twice");
    assert!(!app.messages[1].pending, "and the row becomes canonical");
    assert_eq!(
        cursor(&app),
        None,
        "settling her own send is still not catching up on the room"
    );

    // Jump to latest, and the tail is live again.
    let _ = app.__update(__DucktapeMessage::ChatUpdated(chat_data(
        "general",
        vec![message(499, "the latest", false)],
    )));
    app.channels = room();
    // 502, not 500: her own send settled at 501 two steps up, so a post that
    // arrives after the jump is newer than that.
    let _ = app.__update(__DucktapeMessage::LiveUpdated(posted_delta(
        "general",
        message(502, "posted just now", false),
    )));
    assert_eq!(app.messages.len(), 2);
    assert_eq!(app.messages[1].body, "posted just now");
    assert_eq!(
        cursor(&app),
        Some(502),
        "the tail marks the room read as it arrives"
    );
}

/// THE DM HEADER NAMES A PEER, AND THE ROOM IT NAMES HIM FOR IS `active_channel`.
///
/// A non-empty `active_dm_peer` draws `DmHeader` AND suppresses both the `#`
/// glyph and `active_channel_name`, so a peer that outlived the room he named
/// put Alice's face over #general's timeline with the room the composer posts
/// into never named — and left two sidebar rows reading as selected. It is a
/// derivation of the room now, so no landing can disagree with the pane.
#[test]
fn a_landing_in_another_room_retires_the_dm_header() {
    let me = "aa";
    let peer = "bb";
    let dm = backend::dm_channel_id(me.into(), peer.into());

    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.settings_user_key = me.into();
    app.active_dm_peer = peer.into();
    app.active_channel = dm.clone();

    // a search hit jumps to an ordinary room…
    let _ = app.__update(__DucktapeMessage::ChatHitLoaded(chat_data(
        "general",
        vec![message(7, "an old message", false)],
    )));
    assert!(
        app.active_dm_peer.is_empty(),
        "the peer does not follow the reader into #general"
    );

    // …and a landing inside the DM itself keeps him
    app.active_dm_peer = peer.into();
    let _ = app.__update(__DucktapeMessage::ChatUpdated(chat_data(
        &dm,
        vec![message(1, "hey", false)],
    )));
    assert_eq!(app.active_dm_peer, peer, "this room IS his DM");

    // the resync is the landing with no launch behind it — it moves the room
    // on its own, which is how the peer used to survive every other route
    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
        app.hydration_generation,
        "general",
        vec![message(9, "in the room she was moved to", false)],
        "",
        Vec::new(),
    )));
    assert!(app.active_dm_peer.is_empty());

    // BUT A RESYNC THAT MOVED NO ROOM DERIVES NOTHING. `choose_dm` names the
    // peer optimistically and leaves `active_channel` on the room being left
    // for the several blocks `open_dm` takes to answer; a pages-only resync
    // landing in that window would otherwise derive the peer against the OLD
    // room and blank him, and `chat_updated` then derives "" from "" — the DM
    // opens under a `#` for good.
    app.active_dm_peer = peer.into();
    app.active_channel = "general".into();
    let _ = app.__update(__DucktapeMessage::LiveResynced(backend::LiveRefresh {
        chat_loaded: false,
        ..live_refresh(
            app.hydration_generation,
            "general",
            Vec::new(),
            "",
            Vec::new(),
        )
    }));
    assert_eq!(
        app.active_dm_peer, peer,
        "the room did not move, so nothing about it was re-read"
    );

    // NOR DOES A CHAT-CARRYING ONE INSIDE THAT SAME WINDOW. `live_resync_load`
    // is launched with today's `active_channel`, so a `ready`/`Lagged{chat}`
    // resync lands `chat_loaded` on the room being LEFT — deriving against it
    // blanks the peer just as permanently as the pages-only case above.
    app.active_dm_peer = peer.into();
    app.active_channel = "general".into();
    app.loading = true;
    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
        app.hydration_generation,
        "general",
        Vec::new(),
        "",
        Vec::new(),
    )));
    assert_eq!(
        app.active_dm_peer, peer,
        "a landing is in flight — it answers for the peer, this resync does not"
    );
    app.loading = false;

    // a device with no user key derives no DM id, so it holds no DM — the same
    // answer `chat_sidebar_rooms` gives when `me` is empty
    app.settings_user_key = String::new();
    app.active_dm_peer = peer.into();
    let _ = app.__update(__DucktapeMessage::ChatUpdated(chat_data(
        &dm,
        vec![message(1, "hey", false)],
    )));
    assert!(app.active_dm_peer.is_empty());
}

/// THE DM HEADER IS THE ROW'S FLEXIBLE CHILD, exactly as the channel title is.
///
/// `align=center` is CROSS-axis only and iced's Row has no main-axis
/// justification, so the header's right-hand cluster — the huddle control and
/// the ⋯ that is the only mouse route to Channel details — sits at the right
/// edge only while some child takes the row's slack. The channel arm has a
/// `box w=fill clip=true` around its title for exactly this; the DM arm mounted
/// `DmHeader` bare, so ⋯ packed against the peer's name and moved with its
/// length, and a long name pushed the huddle control and ⋯ past the pane's clip.
///
/// It also branches on the resolved NAME, not the key: `dm_peer_named` answers
/// a roster miss with the blank peer while the key stays set, so branching on
/// the key drew an empty plate with no name — never the fall-through to the
/// derived two-party title that three comments promise. ONE discriminant for
/// the whole surface: the thread rail draws the same room's breadcrumb, and a
/// rail still reading the KEY would print that room without its `#` while the
/// header above it printed one — two readings of one room, on screen together.
#[test]
fn the_dm_header_takes_the_slack_the_channel_title_would() {
    let screen = inlined(include_str!("ui/screens/chat.ice"));
    assert!(screen.contains(
        "if !empty(active_dm.name)\n                    box w=fill clip=true\n                      DmHeader peer=active_dm"
    ));
    // The header's two fall-through arms (`#` glyph, channel title) and the
    // thread rail's breadcrumb, all reading the one derivation.
    assert_eq!(screen.matches("if empty(active_dm.name)").count(), 3);
    // No arm anywhere on this screen decides a title from the KEY, which
    // survives the roster miss the resolved row does not.
    assert!(!screen.contains("if empty(active_dm_peer)"));
    assert!(!screen.contains("if !empty(active_dm_peer)"));
}

/// ONE COMMITTED REPLY, ONE ROW, ONE STEP OF THE CURSOR.
///
/// `thread_reply_send_failed` bumped the reply cursor for a reply that had
/// committed, and then the SAME reply's delta arrived on the live stream and
/// bumped it again — two observers of one row. The cursor then pointed one
/// reply past the loaded run, so the next "Load more replies" started late and
/// the skipped reply was never rendered at all.
#[test]
fn a_committed_reply_moves_the_thread_cursor_exactly_once() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_channel = "general".into();
    app.active_thread_seq = 1;
    app.thread_messages = vec![message(1, "the root", false), message(2, "a reply", false)];
    app.thread_next_reply_offset = 1;
    app.thread_has_more = false;
    app.reply_editor = compose("mine");

    let _ = app.__update(__DucktapeMessage::ReplyComposerEvent(
        editor::composer_submit_event(),
    ));
    let operation_id = app.thread_messages[2].id.clone();

    // the write committed, but the read after it failed
    let _ = app.__update(__DucktapeMessage::ThreadReplySendFailed(
        backend::OptimisticMutationError {
            message: "read failed after commit".into(),
            committed: true,
            operation_id: operation_id.clone(),
            scope_id: "general".into(),
            body: "mine".into(),
        },
    ));
    assert_eq!(
        app.thread_next_reply_offset, 1,
        "the failure counts nothing — the reply's own delta is the observer"
    );

    // …and here is that delta, settling the pending row in place
    let mut settled = message(3, "mine", false);
    settled.id = operation_id;
    settled.thread_seq = 1;
    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: "chat".into(),
        status: "Live".into(),
        height: 3,
        chat: backend::ChatDelta {
            kind: "reply".into(),
            channel_id: "general".into(),
            seq: 3,
            root_seq: 1,
            message: settled,
            ..backend::ChatDelta::default()
        },
        ..backend::LiveUpdate::default()
    }));
    assert_eq!(app.thread_messages.len(), 3, "root plus two replies");
    assert!(app.thread_messages.iter().all(|reply| !reply.pending));
    assert_eq!(
        app.thread_next_reply_offset,
        app.thread_messages.len() as i64 - 1,
        "the cursor is where the loaded run ends, not one past it"
    );
}

/// A LOAD FAILED; THE CONNECTION SAID NOTHING.
///
/// The generic failed arm wrote `status = "Offline"` for one slow load, over a
/// live socket — and `connected` stays true, so nothing reconnects and nothing
/// corrects it: the sidebar dot goes red and the pill reads Offline until the
/// next block's `live_updated` overwrites the status, up to 3s on a quiet chain.
#[test]
fn a_single_failed_load_does_not_report_the_connection_offline() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = true;
    app.status = "Live".into();

    let _ = app.__update(__DucktapeMessage::Failed(backend::AppError {
        message: "the channel did not load".into(),
        committed: false,
    }));

    assert_eq!(
        app.status, "Live",
        "the connection's word belongs to the connection's own handlers"
    );
    assert_eq!(app.error, "the channel did not load");
    assert!(!app.loading, "and the pane is released either way");
}

/// THE ONE VIRTUAL LIST IN THIS APP THAT PREPENDS MUST BE KEYED.
///
/// `chat_scrolled` fires the older page automatically inside the last tenth of
/// the scrollback and `prepend_history` merges up to 256 rows AHEAD of the
/// timeline. An unkeyed virtual column diffs its children by index, so every
/// one of those rows hands its measured height to its neighbour: the rows below
/// the viewport are re-estimated at the 44px placeholder, the content height
/// moves, and an `anchor-y=end` offset — a fixed distance from the BOTTOM —
/// lands on entirely different messages. The reader gets thrown backwards
/// mid-sentence, once per page, for as long as she keeps reading upwards.
#[test]
fn the_message_timeline_virtualizes_under_an_end_anchored_scroll() {
    let chat = inlined(include_str!("ui/screens/chat.ice"));
    // Only the rows the viewport can see are laid out, which is what lets the
    // timeline hold a whole channel without paying a text layout per row — and
    // `by=message.view_key` is what makes per-row state and per-row MEASUREMENT
    // follow the message through prepends AND optimistic confirmation instead
    // of following the slot it happened to occupy.
    let above = chat
        .split_once("keyed message in messages by=message.view_key w=fill gap=3.0 virtual-row=44.0")
        .expect("the message timeline is a KEYED virtual-row column")
        .0;
    // That is only correct under an end-anchored scroll: measuring a row ABOVE
    // the viewport moves everything below it, and a bottom-anchored offset is
    // what carries the visible rows along with it. The two travel together —
    // the thread rail's own scroll sits further down the file, past the split.
    // `h=shrink` is the composer-anchored height: the virtual column reports a
    // whole-list estimate, so a long timeline still hits the box's cap.
    assert!(
        above
            .contains("scroll #message-stream dir=vertical w=fill h=shrink anchor-y=end auto=true")
    );
    // The page controls stay OUTSIDE the keyed column. A keyed column repeats
    // one template over one list; a button folded into that list is a row whose
    // arrival and departure shift every index below it — the same defect one
    // level up, and `has_older_history` flips on every page.
    assert!(above.contains("col w=fill gap=3.0 pr=6.0"));
    assert!(above.contains("button \"Load older messages\""));
    // A key is only an identity if it is unique. The allocator gives every
    // concurrent pending row its own widget state and measurement.
    let mut pending = Vec::new();
    for id in ["a", "b", "c"] {
        pending = backend::optimistic_message(pending, id.into(), id.into());
    }
    let keys = pending
        .iter()
        .map(|message| message.view_key)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(keys.len(), pending.len(), "every pending row keys apart");
}

/// A LINK IS A DESTINATION, NOT A PERSON — AND IT PRESSES.
///
/// The chat module sets `highlight` for a `Link` mark and a `Mention` mark
/// alike (`highlight = link.is_some() || mention`), so ONE arm painted both:
/// every URL anyone posted wore the mention's plate, in the mention's ink, and
/// was dead text — no cursor change, no press, no menu — while `span.link`
/// carried the destination all the way to the view and no `.ice` file read it.
/// Sharing a URL in this app meant the reader selected it by hand.
#[test]
fn a_posted_url_presses_and_does_not_wear_the_mention_plate() {
    let components = inlined(include_str!("ui/components/chat.ice"));
    // The plate is the mention's token, and ONLY the mention's.
    assert!(components.contains("if span.highlight && empty(span.link)"));
    assert!(
        !components.contains("if span.highlight\n"),
        "an unqualified highlight arm is the mention plate back on every link"
    );
    // A link is its own arm, and it is a press.
    assert!(components.contains("if !empty(span.link)"));
    assert!(
        components.contains("button label=span.text p=0.0 -> emit(open_message_link, span.link)")
    );
    // AND IT DRAWS THE UNDERLINE — the one link convention every reader
    // already knows (ducktape-ui#604 grew `underline` on plain `text`). The
    // rule marks a destination: of RichLine's per-token arms, the link's text
    // ALONE wears it, and the exact-line equality also proves the ruled text
    // carries no `tracking=`/`shape=` — the E174 pair a one-span paragraph
    // cannot express.
    let rich_line = components
        .split_once("component RichLine")
        .expect("the per-token flex")
        .1;
    let underlined: Vec<&str> = rich_line
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("text span.text") && line.contains(" underline"))
        .collect();
    assert_eq!(
        underlined,
        ["text span.text wrap=word-or-glyph size=13.5 line-h=1.55 font=medium underline"],
        "the link token alone draws the rule"
    );
    // It hands off through the SAME external-URL route the page renderer's
    // link press takes — one mechanism for one act, not a second one here.
    let handlers = include_str!("ui/handlers/chat.ice");
    assert!(handlers.contains("on open_message_link(url)"));
    assert!(handlers.contains(
        "run every open_external_url(url) -> external_url_opened _ | external_url_failed _"
    ));
}

/// `· edited` ANNOTATES A MESSAGE, SO IT RIDES THE MESSAGE.
///
/// It lived inside the `show_author` run header, so in a run of five messages
/// only the first could ever say it had been edited — and runs are most of a
/// busy channel. A message's text changing under its readers with no mark
/// anywhere on the row silently spends the one integrity signal this product
/// has. The thread root drew a header and still never carried it at all.
#[test]
fn the_edited_marker_reaches_every_row_it_annotates() {
    let components = inlined(include_str!("ui/components/chat.ice"));
    let marker = "text \"· edited\" size=11.0 wrap=none font=code_medium @text-muted";
    assert_eq!(
        components.matches(marker).count(),
        3,
        "the run header, the continuation row, and the thread root each carry it"
    );
    assert!(
        components.contains(&format!(
            "if message.edited && !message.show_author\n          {marker}"
        )),
        "a continuation row trails its own marker under the body"
    );
    let parent = components
        .split_once("component ThreadParentBlock")
        .expect("the thread root block")
        .1;
    assert!(parent.contains(&format!("if message.edited\n            {marker}")));
}

/// THE RAIL IS THE SAME LIST WITH THE SAME BILL. A thread pages in at the same
/// 256 replies a channel does, and a plain column culls only `draw` — `update`,
/// `mouse_interaction`, `overlay` and `layout` walk every reply ever loaded, on
/// every event and every frame. Virtualization culls those four; `lazy` stops
/// the rows that ARE visible from rebuilding ~60 nodes of scope strings and
/// a11y keys apiece. The two are not alternatives, and the stream carries both.
#[test]
fn the_thread_rail_virtualizes_and_caches_its_quiet_replies() {
    let chat = inlined(include_str!("ui/screens/chat.ice"));
    let above = chat
        .split_once("col w=fill gap=3.0 pl=16.0 pr=16.0 pt=12.0 pb=8.0 virtual-row=44.0")
        .expect("the thread rail is a virtual-row column")
        .0;
    assert!(above.contains("scroll dir=vertical w=fill h=fill anchor-y=end auto=true"));
    // A `lazy` subtree reads nothing but its dependency, so the quiet arm can
    // only exist because the rows that read SCREEN state — the search target
    // and the open action menu — were split off into live arms. Confirmation
    // is row state and moves `render_rev`, so it belongs inside the lazy arm.
    // The KEYED form is pinned too:
    // dropping `by (seq, render_rev)` silently reverts every visible reply to
    // a full row clone + hash per frame — the #1058 residue this collects.
    assert!(chat.contains(
        "lazy thread_message by thread_message.seq, thread_message.render_rev as cached_reply"
    ));
    for live in [
        "thread_message.seq == thread_target_seq",
        "thread_message.seq == thread_selected_seq",
    ] {
        assert!(chat.contains(live), "the live arm on {live} is gone");
    }
}

#[test]
fn message_actions_require_explicit_intent() {
    let (mut app, _) = Ducktape::__boot();
    app.mutation_phase = MutationPhase::Idle;

    let _ = app.__update(__DucktapeMessage::OpenMessageActions(7, "hello".into(), 2));
    assert_eq!(app.selected_message_seq, 7);
    assert_eq!(app.message_action, MessageAction::More);
    let _ = app.__update(__DucktapeMessage::BeginMessageEdit(7, "hello".into(), 2));
    assert_eq!(app.message_action, MessageAction::Editing);
    // Every cancel affordance in the view routes `clear_message_selection`
    // (view.ice:441, :467, :511, :523, :538), so that is the transition
    // under test — it drops to the toolbar AND drops the selection.
    let _ = app.__update(__DucktapeMessage::ClearMessageSelection);
    assert_eq!(app.message_action, MessageAction::Toolbar);
    assert_eq!(app.selected_message_seq, 0);
    let _ = app.__update(__DucktapeMessage::OpenMessageReactions(
        7,
        "hello".into(),
        2,
    ));
    assert_eq!(app.message_action, MessageAction::Reactions);
    let _ = app.__update(__DucktapeMessage::ClearMessageSelection);
    let _ = app.__update(__DucktapeMessage::ArmMessageDelete(7, "hello".into(), 2));
    assert_eq!(app.message_action, MessageAction::Delete);
}

#[test]
fn message_action_toolbar_stays_compact_and_accessible() {
    let components = inlined(include_str!("ui/components/chat.ice"));
    let toolbar = components
        .split_once("component MessageCard")
        .unwrap()
        .1
        .split_once("component ThreadMessageCard")
        .unwrap()
        .0;
    // Hover is DRAW-TIME: the `hover` widget reveals the toolbar under the
    // cursor with no enter/exit routes and no hovered state — a cached lazy
    // row keeps native-latency hover. `open=menu_open` is the one exception,
    // and it is the ANCHOR CONTRACT: while the ♡/⋯ card this row opened is
    // up, the toolbar it hangs off stays, however far the pointer went.
    assert!(toolbar.contains("hover tint=row_hover r=9.0 open=menu_open"));
    let stream = inlined(include_str!("ui/screens/chat.ice"));
    assert!(stream.contains("MessageCard message selected=true menu_open=true"));
    assert!(toolbar.contains("if !message.deleted && !message.pending"));
    assert!(!toolbar.contains("&& hovered"));
    assert!(!toolbar.contains("mouse enter="));
    // the artifact's hover bar is five 27×25 cells: three one-tap reactions,
    // the reaction picker and the overflow menu (Console:244).
    assert_eq!(toolbar.matches("w=27.0 h=25.0").count(), 5);
    // the one svg cell takes the icon as a direct child; a `h=fill` wrapper
    // inside a fixed-size button collapses an SVG to a hairline. The other
    // four cells are the artifact's own typographic glyphs, not icons.
    assert_eq!(toolbar.matches("p=5.0 @icon_action").count(), 1);
    // The name shares BODY size with the message it heads — Slack/Discord's
    // own convention — and separates on weight alone.
    assert!(components.contains(
        "text message.author size=13.5 wrap=none font=display @text-fg\n            if message.avatar_kind == \"agent\""
    ));
    // the stamp beside the author is the block the message was finalized
    // in — a chain fact the app can prove, never a wall-clock time. `muted`
    // clears the AA contrast floor; `hint` (2.10:1) did not.
    assert!(components.contains(
        "if message.height > 0\n              text height_label_short(message.height) size=11.0 wrap=none font=code_medium @text-muted"
    ));
    // Slack-style grouping: the shared avatar + author header only renders
    // for a run's first message; continuations keep the body aligned via a
    // gutter that matches the avatar's width.
    assert!(components.contains(
        "if message.show_author\n        MessageAvatar initials=message.initial kind=message.avatar_kind"
    ));
    assert!(components.contains("if !message.show_author\n        space w=30.0"));
    assert!(components.contains("\"human\"\n        PersonAvatar initials plate=30.0 ink=11.0"));
    assert!(components.contains("\"agent\"\n        AgentAvatar initials plate=30.0 ink=11.0"));
    assert!(!components.contains("avatar_style"));
    // Rich bodies render structured blocks, not one flattened string.
    assert!(components.contains("for block in message.blocks"));
    assert!(components.contains("if block.kind == \"code\""));
    assert!(components.contains("flex w=fill wrap=wrap"));
    // The hover toolbar uses the shared popover depth role instead of
    // carrying another inline shadow variant. The artifact's own plate is
    // `border-radius:9px; box-shadow:0 3px 12px rgba(40,38,34,.13);
    // padding:2px` (Console:243).
    assert!(toolbar.contains(
        "box p=2.0 bg=surface border=border border-w=1.0 r=9.0 shadow=shadow_popover shadow-y=3.0 shadow-blur=12.0"
    ));
    for label in ["Open thread", "Manage reactions", "More message actions"] {
        assert!(toolbar.contains(&format!("label=\"{label}\"")));
    }
    assert!(components.contains(
        "button label=\"Open thread\" disabled=disabled p=5.0 @icon_action -> emit(open_thread_for, message.seq)"
    ));

    let chat = inlined(include_str!("ui/screens/chat.ice"));
    assert!(chat.contains(
        "overlay when=(selected_message_seq > 0 && message_action != MessageAction.toolbar)"
    ));
    assert!(chat.contains("dismiss=emit(clear_message_selection) backdrop=transparent"));
    assert!(chat.contains("mouse press-at=chat_pointer_pressed"));
    // per-press, never per-move: a move= stream here rebuilds the view per pixel
    assert!(!chat.contains("mouse move="));
    assert!(chat.contains(
        "box w=fill h=fill pt=block_action_menu_y(chat_pointer_y, chat_height) align-x=end align-y=start"
    ));
    // the pointer sensor is the MESSAGE-LIST stack's first child, so it
    // measures the message list itself and not whatever an overlay happens
    // to cover. The anchor names that stack by its exact indentation: the
    // outer content stack (which floats the search-results card) sits
    // shallower and must not satisfy this pin.
    let sensor = chat
        .split_once("                stack w=fill h=fill\n")
        .unwrap()
        .1;
    assert!(
        sensor
            .trim_start()
            .starts_with("sensor show=chat_resized resize=chat_resized")
    );
    let overlay_content = chat
        .split_once("                  content\n")
        .unwrap()
        .1
        .split_once("                  layer\n")
        .unwrap()
        .0;
    assert!(overlay_content.contains("space w=fill h=fill"));
    assert!(!overlay_content.contains("message_action =="));
    let more = chat
        .split_once("message_action == MessageAction.more")
        .unwrap()
        .1
        .split_once("message_action == MessageAction.reactions")
        .unwrap()
        .0;
    // Icon + sentence rows on one raised plate; Esc and the backdrop dismiss,
    // so the menu lists no Close row of its own.
    for row in [
        "label=\"Manage reactions\"",
        "label=\"Reply in thread\"",
        "label=\"Edit message\"",
        "label=\"Delete message\"",
    ] {
        assert!(more.contains(row), "{row}");
    }
    for icon in ["\"emoji\"", "\"nav-chat\"", "\"pencil\"", "\"trash\""] {
        assert!(more.contains(&format!("Icon name={icon}")), "{icon}");
    }
    assert!(!more.contains("button \"Close\""));
    // The reactions arm is the shared ADD grid — removal rides the message's
    // own reaction chips, which already toggle off for `reacted_by_me`.
    let picker = chat
        .split_once("message_action == MessageAction.reactions")
        .unwrap()
        .1
        .split_once("message_action == MessageAction.editing")
        .unwrap()
        .0;
    assert!(picker.contains("for emoji in reaction_palette()"));
    assert!(picker.contains("-> emit(add_reaction_submit, emoji)"));
    assert!(!picker.contains("remove_reaction_submit"));
    // Cells must stay pressable while a reaction is in flight: a disabled
    // button captures no press, and an uncaptured press inside the overlay
    // dismisses it (see `reactions_run_outside_the_mutation_lock`).
    assert!(!picker.contains("mutation_phase"));

    let handlers = inlined(include_str!("ui/handlers/chat.ice"));
    for focus in [
        "#workspace-tabs/content/chat/message-action-focus",
        "#workspace-tabs/content/chat/message-reaction-focus",
        "#workspace-tabs/content/chat/message-delete-focus",
    ] {
        assert!(handlers.contains(focus));
    }
    for focus in [
        "#message-action-focus",
        "#message-reaction-focus",
        "#message-delete-focus",
    ] {
        assert!(chat.contains(&format!("input \"\" {focus}")));
    }
    assert_eq!(handlers.matches("task widget focus-next").count(), 6);
    assert!(!inlined(include_str!("ui/extern/backend.ice")).contains("task focus_next()"));
    let activate = handlers
        .split_once("on begin_message_edit(seq, body, rev)\n")
        .unwrap()
        .1
        .split_once("\non ")
        .unwrap()
        .0;
    assert!(activate.contains("task widget focus #workspace-tabs/content/chat/message-edit"));
}

#[test]
fn thread_messages_mirror_the_main_action_system() {
    let components = inlined(include_str!("ui/components/chat.ice"));
    let card = components
        .split_once("component ThreadMessageCard")
        .unwrap()
        .1;
    // `open=menu_open` is the toolbar's anchor contract: the reveal outlives
    // the pointer for exactly as long as the card it opened is up.
    assert!(card.contains("hover tint=row_hover r=9.0 open=menu_open"));
    assert!(
        card.contains(
            "-> emit(open_thread_message_actions, message.seq, message.body, message.rev)"
        )
    );
    assert!(
        card.contains(
            "-> emit(open_thread_message_actions, message.seq, message.body, message.rev)"
        )
    );
    assert!(card.contains(
        "-> emit(open_thread_message_reactions, message.seq, message.body, message.rev)"
    ));
    // A reply is the SAME message block as a timeline row — the rail mounts
    // the shared contents rather than a second spelling of them, so the
    // message redesign lands in both lanes at once.
    assert!(card.contains("MessageContents message=message"));
    // Confirmation is the pending dot disappearing, so the card needs no
    // timer or animation prop. (`card` starts right after the component name,
    // so the signature is its head.)
    assert!(
        card.starts_with("(message:ChatMessage, selected:bool, menu_open:bool, disabled:bool)")
    );
    // `menu_open` cannot be `selected` here: in the rail `selected` marks the
    // deep-link TARGET reply, not the row whose action card is open.
    let chat_screen_rail = inlined(include_str!("ui/screens/chat.ice"));
    assert!(chat_screen_rail.contains("menu_open=(thread_message.seq == thread_selected_seq)"));
    // No open-thread action from inside a thread you are already reading. The
    // shared contents still declare the event (their reply pill emits it) so
    // the card forwards it, but the rail's toolbar has no seat for it — and a
    // reply carries no replies, so the pill never renders here.
    assert!(!card.contains("label=\"Open thread\""));

    let chat_screen = inlined(include_str!("ui/screens/chat.ice"));
    let thread = chat_screen
        .split_once("if active_thread_seq > 0 && !channel_settings_open")
        .unwrap()
        .1;
    // A SECOND overlay, keyed on thread-scoped state, independent of the main one.
    assert!(thread.contains(
        "overlay when=(thread_selected_seq > 0 && thread_message_action != MessageAction.toolbar)"
    ));
    assert!(thread.contains("dismiss=emit(clear_thread_message_selection) backdrop=transparent"));
    assert!(thread.contains(
        "box w=fill h=fill pt=block_action_menu_y(thread_pointer_y, thread_height) align-x=end align-y=start"
    ));
    assert!(thread.contains("mouse press-at=thread_pointer_pressed"));
    // same seat as the message list — the rail measures itself
    assert!(thread.contains("sensor show=thread_resized resize=thread_resized"));
    // The picker is the shared ADD grid targeting the thread selection;
    // removal rides the reply's own reaction chips.
    assert!(thread.contains("for emoji in reaction_palette()"));
    assert!(thread.contains("-> emit(add_reaction_at, thread_selected_seq, emoji)"));
    // Same pressable-while-in-flight contract as the stream picker.
    let thread_picker = thread
        .split_once("thread_message_action == MessageAction.reactions")
        .unwrap()
        .1
        .split_once("thread_message_action == MessageAction.editing")
        .unwrap()
        .0;
    assert!(!thread_picker.contains("mutation_phase"));
    // More-menu omits Reply in thread (already inside the thread) and Close.
    let more = thread
        .split_once("thread_message_action == MessageAction.more")
        .unwrap()
        .1
        .split_once("thread_message_action == MessageAction.reactions")
        .unwrap()
        .0;
    for label in [
        "label=\"Manage reactions\"",
        "label=\"Edit message\"",
        "label=\"Delete message\"",
    ] {
        assert!(more.contains(label), "{label}");
    }
    assert!(!more.contains("Reply in thread"));
    assert!(!more.contains("button \"Close\""));

    let handlers = inlined(include_str!("ui/handlers/chat.ice"));
    for name in [
        "on open_thread_message_actions(seq, body, rev)",
        "on open_thread_message_reactions(seq, body, rev)",
        "on begin_thread_message_edit(seq, body, rev)",
        "on arm_thread_message_delete(seq, body, rev)",
        "on clear_thread_message_selection",
        "on edit_thread_message_submit",
        "on delete_thread_message_submit",
    ] {
        assert!(handlers.contains(name), "{name}");
    }
    // Thread edit/delete target the thread selection, never the main one.
    let edit = handlers
        .split_once("on edit_thread_message_submit\n")
        .unwrap()
        .1
        .split_once("\non ")
        .unwrap()
        .0;
    assert!(edit.contains(
        "edit_message(connected_rpc, password, active_channel, thread_selected_seq, thread_selected_rev, trim(thread_edit_draft), channel_members)"
    ));
    let delete = handlers
        .split_once("on delete_thread_message_submit\n")
        .unwrap()
        .1
        .split_once("\non ")
        .unwrap()
        .0;
    assert!(
        delete.contains(
            "delete_message(connected_rpc, password, active_channel, thread_selected_seq)"
        )
    );
}

#[test]
fn thread_action_state_is_independent_of_the_main_message_menu() {
    let (mut app, _) = Ducktape::__boot();
    app.mutation_phase = MutationPhase::Idle;
    app.active_channel = "general".into();
    app.active_thread_seq = 1;

    // Opening a thread action must not touch the main message menu.
    let _ = app.__update(__DucktapeMessage::OpenThreadMessageActions(
        2,
        "reply".into(),
        3,
    ));
    assert_eq!(app.thread_selected_seq, 2);
    assert_eq!(app.thread_message_action, MessageAction::More);
    assert_eq!(app.selected_message_seq, 0);
    assert_eq!(app.message_action, MessageAction::Toolbar);

    // And a main message action must not touch the thread menu.
    let _ = app.__update(__DucktapeMessage::OpenMessageActions(5, "root".into(), 1));
    assert_eq!(app.selected_message_seq, 5);
    assert_eq!(app.message_action, MessageAction::More);
    assert_eq!(app.thread_selected_seq, 2);
    assert_eq!(app.thread_message_action, MessageAction::More);

    let _ = app.__update(__DucktapeMessage::ClearThreadMessageSelection);
    assert_eq!(app.thread_selected_seq, 0);
    assert_eq!(app.thread_message_action, MessageAction::Toolbar);
    assert_eq!(app.selected_message_seq, 5);
    assert_eq!(app.message_action, MessageAction::More);
}

#[test]
fn opening_another_thread_invalidates_the_pending_thread() {
    let (mut app, _) = Ducktape::__boot();
    app.mutation_phase = MutationPhase::Idle;
    app.active_channel = "general".into();
    app.selected_message_seq = 1;
    app.thread_generation = 4;
    app.thread_loading = true;
    app.active_thread_seq = 1;
    app.thread_messages =
        backend::optimistic_message(Vec::new(), "old thread".into(), "pending-old".into());
    app.reply_editor = compose("old reply");

    let _ = app.__update(__DucktapeMessage::OpenThreadFor(2));
    assert_eq!(app.thread_generation, 5);
    assert!(app.thread_loading);
    assert_eq!(app.active_thread_seq, 2);
    assert!(app.thread_messages.is_empty());
    assert!(reply_composer(&app).is_empty());
    assert_eq!(
        backend::parked_reply_draft(app.reply_drafts.clone(), "general".into(), 1),
        "old reply",
        "the box is emptied, the words are not thrown away"
    );

    let _ = app.__update(__DucktapeMessage::ThreadLoaded(backend::ThreadLoadData {
        generation: 4,
        root_seq: 1,
        target_seq: 0,
        messages: Vec::new(),
        next_reply_offset: 0,
        has_more: false,
    }));
    assert_eq!(app.active_thread_seq, 2);
}

/// CLICKING ANOTHER THREAD IS NOT A REQUEST TO THROW THE REPLY AWAY.
///
/// The rail sits beside a timeline that stays mounted, and every "N replies"
/// row in it emits `open_thread_for` — which blanks `reply_editor`, the LIVE
/// buffer every keystroke lands in. So three sentences into a reply, a click
/// meant to check something next door destroyed them: no banner, no Restore,
/// nothing.
///
/// The park is keyed by room AND root rather than harvested into
/// `failed_reply_draft`, which is only channel-scoped: that plate would have
/// offered thread A's words over every later thread of the room, and its
/// Restore would have armed them to post in B — the same cross-context
/// re-targeting the stream composer's own park exists to end.
///
/// `close_thread` stays the one route that discards, because that one is a
/// request to — the drawer got the same treatment in
/// `the_channel_drawer_does_not_eat_a_reply_you_are_typing`.
#[test]
fn opening_another_thread_parks_the_reply_in_the_thread_it_belongs_to() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_channel = "general".into();
    app.active_thread_seq = 1;
    app.reply_editor = compose("three sentences in and");

    let _ = app.__update(__DucktapeMessage::OpenThreadFor(2));
    assert_eq!(
        app.active_thread_seq, 2,
        "the rail she clicked is the one open"
    );
    assert!(
        reply_composer(&app).is_empty(),
        "a rail that just opened has an untouched composer"
    );
    assert!(
        app.failed_reply_draft.is_empty(),
        "and NOT through the channel-scoped plate, which would offer thread 1's \
         words to every other thread in #general"
    );

    // Back to the thread they belong to, and they are waiting there.
    app.reply_editor = compose("");
    let _ = app.__update(__DucktapeMessage::OpenThreadFor(1));
    assert_eq!(reply_composer(&app), "three sentences in and");

    // A ROOM SWITCH PARKS THE RAIL TOO, and hands it back on the way in. The
    // text is CHANGED first, so this arm reads the picker's own park rather
    // than the entry `open_thread_for` filed above.
    app.reply_editor = compose("and then the pager went off");
    app.channels = vec![room("general", 10), room("random", 20)];
    app.mutation_phase = MutationPhase::Idle;
    let _ = app.__update(__DucktapeMessage::ChooseChannel("random".into()));
    assert_eq!(app.active_thread_seq, 0, "the rail closes with the room");
    assert!(reply_composer(&app).is_empty());
    let _ = app.__update(__DucktapeMessage::ChooseChannel("general".into()));
    let _ = app.__update(__DucktapeMessage::OpenThreadFor(1));
    assert_eq!(
        reply_composer(&app),
        "and then the pager went off",
        "the reply belongs to #general's thread 1 and is still there"
    );

    // AND CLOSE IS A DISCARD. That click asks for the reply to go away, so the
    // park must not hand it back on the next open.
    let _ = app.__update(__DucktapeMessage::CloseThread);
    let _ = app.__update(__DucktapeMessage::OpenThreadFor(1));
    assert!(
        reply_composer(&app).is_empty(),
        "Close is a request to discard, and the park heard it"
    );

    // AN ORDINARY RAIL OPEN PARKS NOTHING — an empty editor is not an unsent
    // reply, so no entry is filed for every thread she merely looks at.
    let (mut quiet, _) = Ducktape::__boot();
    quiet.active_channel = "general".into();
    let _ = quiet.__update(__DucktapeMessage::OpenThreadFor(4));
    assert!(
        quiet.reply_drafts.is_empty(),
        "nothing was typed, so there is nothing to park"
    );
}

/// A PEER DELETING THE ROOT IS NOT A REQUEST TO THROW THE REPLY AWAY EITHER.
///
/// `live_resynced` closes the rail on its own whenever `refreshed_known_message_seq`
/// finds the root deleted or the room moved. The park has to sit ABOVE that
/// line: `park_reply_draft` refuses `thread_seq <= 0` outright, so a park read
/// below it is a guaranteed no-op — the rail vanishes, nothing is filed, and the
/// next thread she opens hands her an empty box with her words nowhere.
#[test]
fn a_resync_that_closes_the_rail_parks_the_reply_it_closes_over() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_channel = "general".into();
    app.hydration_generation = 4;
    app.active_thread_seq = 7;
    app.reply_editor = compose("three sentences in and");

    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
        4,
        "general",
        vec![message(7, "the root", true)],
        "",
        Vec::new(),
    )));
    assert_eq!(
        app.active_thread_seq, 0,
        "a deleted root closes the rail under the caret"
    );
    assert_eq!(
        backend::parked_reply_draft(app.reply_drafts.clone(), "general".into(), 7),
        "three sentences in and",
        "and the words are filed under the thread they were written in"
    );

    // Which is the only place they can be posted, so that is where they come
    // back — through the ordinary rail open, no banner and no Restore needed.
    app.reply_editor = compose("");
    let _ = app.__update(__DucktapeMessage::OpenThreadFor(7));
    assert_eq!(reply_composer(&app), "three sentences in and");
}

/// AND ARRIVING IN A THREAD BY THE SEARCH ROUTE RESTORES THE SAME PARK.
///
/// `load_chat_hit` answers with `root.seq` when the hit is a reply, so a
/// chat-search jump SEATS a thread — and `chat_hit_loaded` wrote
/// `active_thread_seq` with no restore beside it. The rail opened on an empty
/// box over her parked reply, and the first character typed into it parked OVER
/// those words under the same `general#7` key: a silent overwrite, not just the
/// loss of a live buffer.
#[test]
fn a_search_hit_that_seats_a_thread_opens_on_that_threads_parked_reply() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_channel = "general".into();
    app.active_thread_seq = 7;
    app.reply_editor = compose("half an answer");

    // Clicking another thread parks it — the route the park was built for.
    let _ = app.__update(__DucktapeMessage::OpenThreadFor(9));
    assert!(reply_composer(&app).is_empty());

    let mut hit = chat_data("general", vec![message(7, "the root", false)]);
    hit.generation = app.chat_generation;
    hit.active_thread_seq = 7;
    let _ = app.__update(__DucktapeMessage::ChatHitLoaded(hit));

    assert_eq!(app.active_thread_seq, 7, "the hit seated its thread");
    assert_eq!(
        reply_composer(&app),
        "half an answer",
        "and the rail it opened is the rail she left words in"
    );
}

/// THE STREAM'S LOAD FLAG IS NOT THE RAIL'S, AND THE RAIL'S SEND SAID SO.
///
/// `reply_composer_event` refused on `loading` — a term neither the reply
/// editor, its marks row nor its Send button wears — so in the one state that
/// can raise it under an open rail the reader saw a fully lit Send, pressed it,
/// and got nothing: no post, no error, no banner. Every chat-plane writer of
/// `loading = true` zeroes `active_thread_seq` in the same handler, so the term
/// never fired for a chat load at all; the state it caught was a PAGES load
/// still in flight behind a cross-tab bounce.
#[test]
fn a_pages_load_in_flight_does_not_deaden_the_lit_reply_send() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.active_channel = "general".into();
    app.active_thread_seq = 7;
    // What `open_page_search_hit` leaves behind: the stream's flag up, the rail
    // untouched, and `select_shell_tab` back to Chat clears neither.
    app.loading = true;
    app.reply_editor = compose("on it");

    let _ = app.__update(__DucktapeMessage::ReplyComposerEvent(
        editor::composer_submit_event(),
    ));
    assert_eq!(
        app.thread_messages.len(),
        1,
        "a Send the surface draws as live must actually send"
    );
    assert!(app.thread_messages[0].pending);
    assert!(reply_composer(&app).is_empty());
}

/// A TERM IN THE GUARD THAT THE BUTTON DOES NOT WEAR IS A DEAD CONTROL.
///
/// The affordance is decided at render time and the guard runs at apply time,
/// so the guard may re-read a term — but it may not carry a term the button
/// never showed, or the click lands in a silent `return`. The two the rail's
/// MOUNT already answers are the exception: the whole plate is drawn under
/// `if active_thread_seq > 0`, and `open_thread_for` refuses an empty channel.
#[test]
fn the_reply_send_refuses_only_on_what_its_button_shows() {
    const HANDLERS: &str = include_str!("ui/handlers/chat.ice");
    const SCREEN: &str = include_str!("ui/screens/chat.ice");
    const ANSWERED_BY_THE_MOUNT: [&str; 2] = ["active_thread_seq <= 0", "empty(active_channel)"];

    let guard = HANDLERS
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("return if ") && line.contains("editor_text(reply_editor)"))
        .and_then(|line| line.strip_prefix("return if "))
        .expect("the reply submit guard");
    let send = SCREEN
        .lines()
        .map(str::trim)
        .find(|line| {
            line.starts_with("disabled=(thread_loading")
                && line.contains("editor_text(reply_editor)")
        })
        .and_then(|line| line.strip_prefix("disabled=("))
        .and_then(|line| line.strip_suffix(')'))
        .expect("the reply Send's disabled expression");

    let terms = |expression: &str| -> Vec<String> {
        expression
            .split("||")
            .map(|term| term.trim().to_owned())
            .collect()
    };
    let shown = terms(send);
    for term in terms(guard) {
        let on_the_button = shown.contains(&term);
        let structural = ANSWERED_BY_THE_MOUNT.contains(&term.as_str());
        assert!(
            on_the_button || structural,
            "`reply_composer_event` refuses on `{term}`, which the rail's Send does \
             not wear — put it on the button or take it out of the guard"
        );
    }
}

#[test]
fn thread_pagination_preserves_multiple_pending_replies() {
    let message = |seq: i64, thread_seq: i64, body: &str| backend::ChatMessage {
        id: format!("message-{seq}"),
        view_key: seq,
        seq,
        author: "user".into(),
        meta: format!("#{seq}"),
        body: body.into(),
        blocks: backend::paragraph_blocks(body),
        pending: false,
        rev: 0,
        edited: false,
        deleted: false,
        reply_count: 0,
        thread_seq,
        show_author: true,
        initial: "U".into(),
        avatar_kind: "human".into(),
        height: 0,
        time: 0,
        reactions: Vec::new(),
        render_rev: 0,
    };
    let (mut app, _) = Ducktape::__boot();
    app.active_thread_seq = 1;
    app.thread_generation = 7;
    app.thread_loading = true;
    app.thread_messages = backend::optimistic_message(
        backend::optimistic_message(
            vec![message(1, 0, "root"), message(2, 1, "first")],
            "pending first".into(),
            "pending-first".into(),
        ),
        "pending second".into(),
        "pending-second".into(),
    );

    let _ = app.__update(__DucktapeMessage::ThreadPageLoaded(
        backend::ThreadPageData {
            generation: 7,
            messages: vec![message(3, 1, "second")],
            next_reply_offset: 2,
            has_more: false,
        },
    ));
    assert_eq!(app.thread_messages.len(), 5);
    assert_eq!(app.thread_messages[1].body, "first");
    assert_eq!(app.thread_next_reply_offset, 2);
    assert!(
        app.thread_messages
            .iter()
            .any(|message| { message.pending && message.id == "pending-first" })
    );
    assert!(
        app.thread_messages
            .iter()
            .any(|message| { message.pending && message.id == "pending-second" })
    );
}

// Opening a (possibly different) network through the console handoff clears
// every reading and draft of the previous one — and the in-flight huddle —
// while the KEY password survives: it unlocks this device's user.key, not an
// endpoint.
#[test]
fn opening_a_network_clears_the_previous_networks_state() {
    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.connected_rpc = "http://node-a".into();
    app.rpc = "http://node-b".into();
    app.password = "device-key-password".into();
    app.selected_message_seq = 1;
    app.message_action = MessageAction::Editing;
    app.message_edit_draft = "node a edit".into();
    app.active_thread_seq = 1;
    app.reply_editor = compose("node a reply");
    app.page_editor = compose("node a page body");
    app.page_saved_text = "node a page body".into();
    app.block_comments_open = true;
    app.block_comments_target = "same-id".into();
    app.block_comment_draft = "node a comment".into();
    app.message_editor = compose("node a message");
    app.page_search_draft = "node a search".into();
    app.forge_list_phase = ForgePhase::Ready;
    app.forge_repo = "same-repo".into();
    app.forge_repo_phase = ForgePhase::Ready;
    app.forge_item_number = 1;
    app.forge_item_phase = ForgePhase::Ready;
    app.forge_review_draft = "node a review".into();
    app.forge_tree_repo = "same-repo".into();
    app.forge_tree_born = true;
    app.forge_tree_truncated = true;
    app.forge_code_phase = ForgeCodePhase::Ready;
    app.huddle_joined = true;
    app.huddle_channel = "chan-a".into();
    // AND THE PARKS, which the by-name clears around them would otherwise miss.
    // A channel id is a user-chosen string, so both networks can hold a
    // `#general` and a park keyed on it would hand node A's sentence to node B.
    app.message_drafts =
        backend::park_message_draft(Vec::new(), "general".into(), "node a draft".into());
    app.reply_drafts =
        backend::park_reply_draft(Vec::new(), "general".into(), 1, "node a reply draft".into());

    let _ = app.__update(__DucktapeMessage::ConsoleOpened(iced::window::Id::unique()));

    assert_eq!(app.connected_rpc, "http://node-b");
    assert_eq!(app.password, "device-key-password");
    assert_eq!(app.selected_message_seq, 0);
    assert_eq!(app.message_action, MessageAction::Toolbar);
    assert!(app.message_edit_draft.is_empty());
    assert_eq!(app.active_thread_seq, 0);
    assert!(reply_composer(&app).is_empty());
    assert!(page_document_text(&app).is_empty());
    assert!(app.page_saved_text.is_empty());
    assert!(!app.block_comments_open);
    assert!(app.block_comments_target.is_empty());
    assert!(app.block_comment_draft.is_empty());
    assert!(app.message_draft.is_empty());
    assert!(composer(&app).is_empty());
    assert!(
        app.message_drafts.is_empty() && app.reply_drafts.is_empty(),
        "a draft parked on node A is not node B's to hand back"
    );
    assert!(app.page_search_draft.is_empty());
    assert_eq!(app.forge_list_phase, ForgePhase::Idle);
    assert!(app.forge_repo.is_empty());
    assert_eq!(app.forge_repo_phase, ForgePhase::Idle);
    assert_eq!(app.forge_item_number, 0);
    assert_eq!(app.forge_item_phase, ForgePhase::Idle);
    assert!(app.forge_review_draft.is_empty());
    assert!(app.forge_tree_repo.is_empty());
    assert!(!app.forge_tree_born);
    assert!(!app.forge_tree_truncated);
    assert_eq!(app.forge_code_phase, ForgeCodePhase::Idle);
    assert!(!app.huddle_joined);
    assert!(app.huddle_channel.is_empty());

    let _ = app.__update(__DucktapeMessage::Failed(backend::AppError {
        message: "offline".into(),
        committed: false,
    }));
    assert_eq!(app.connected_rpc, "http://node-b");
}

// The dirty gate makes the tick FIRE; these two guards make it WAIT. An
// in-flight op chain must finish before the next starts (the awaited loop is
// the ordering rule), and an open ``` must be closed before the buffer is
// parsed — otherwise everything under it reads as one code block and the plan
// removes the "vanished" lines.
#[test]
fn the_save_tick_waits_for_inflight_saves_and_open_fences() {
    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.connected = true;
    app.active_page = "page".into();
    // The buffer is this page's — the tick refuses one that is not.
    app.buffer_page = "page".into();
    app.page_editor = compose("Title\nfresh body");
    app.page_saved_text = "Title\nstale".into();
    app.block_autosave_status = AutosaveStatus::Saving;

    let _ = app.__update(__DucktapeMessage::PageAutosaveTick);
    assert_eq!(
        app.block_autosave_status,
        AutosaveStatus::Saving,
        "inflight guard"
    );

    app.block_autosave_status = AutosaveStatus::Idle;
    app.page_editor = compose("Title\n```\nstill typing");
    let _ = app.__update(__DucktapeMessage::PageAutosaveTick);
    assert_eq!(
        app.block_autosave_status,
        AutosaveStatus::Idle,
        "fence guard"
    );

    app.page_editor = compose("Title\n```\ndone\n```");
    let _ = app.__update(__DucktapeMessage::PageAutosaveTick);
    assert_eq!(app.block_autosave_status, AutosaveStatus::Saving);
}

#[test]
fn page_autosave_freshness_is_compiler_owned_without_aborting_writes() {
    let pages = inlined(include_str!("ui/handlers/pages.ice"));
    assert!(pages.contains(
        "run latest lane=page_autosave save_page_document(connected_rpc, password, active_page, text, page_saved_text) -> page_document_saved _ | page_document_save_failed _"
    ));
    assert!(!pages.contains("run replace lane=page_autosave"));
    assert_eq!(pages.matches("invalidate lane=page_autosave").count(), 5);

    let lifecycle = inlined(include_str!("ui/handlers/lifecycle.ice"));
    assert_eq!(
        lifecycle.matches("invalidate lane=page_autosave").count(),
        2
    );
    let onboarding = inlined(include_str!("ui/handlers/onboarding.ice"));
    assert_eq!(
        onboarding.matches("invalidate lane=page_autosave").count(),
        2
    );
}

/// TICK TWO MUST NOT REVERT WHAT TICK ONE CORRECTLY LEFT ALONE.
///
/// The predicate alone survives exactly one tick. A save that writes body ops
/// comes back with the node's canonical text — carrying a rename someone else
/// made — and the handler adopts it as the baseline while deliberately leaving
/// the dirty buffer's stale line 0 in place. That manufactures authorship out
/// of nothing, and the NEXT tick writes the old name back on chain.
///
/// Driven through the handler, two ticks, on the fixture #1032 uses for the
/// same collision: a reader mid-sentence whose page is renamed under her.
#[test]
fn a_save_that_lands_body_ops_does_not_manufacture_a_rename_next_tick() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.active_page = "page".into();
    app.buffer_page = "page".into();
    // she is mid-sentence; her line 0 is the OLD name and she never touched it.
    app.page_editor = compose("Old Name\nbody mid-sentence");
    app.page_saved_text = "Old Name\nbody".into();
    // WHAT THE TICK ACTUALLY SUBMITTED. The correction reads this, not the live
    // buffer, so leaving it at its default empty string would hand the baseline
    // an empty title and prove nothing about the case under test.
    app.page_inflight_text = "Old Name\nbody mid-sentence".into();

    // the save landed her body edit. The node's canonical text carries the
    // other person's rename, which her buffer has never shown.
    let _ = app.__update(__DucktapeMessage::PageDocumentSaved(
        backend::DocumentSaveResult {
            written: true,
            refusal: String::new(),
            document: "New Name\nbody mid-sentence".into(),
            data: backend::PagesData {
                pages: vec![page_item("page", "New Name")],
                blocks: vec![page_block("b1", "page", "body mid-sentence")],
                active_page: "page".into(),
                active_page_title: "New Name".into(),
                active_page_parent: String::new(),
                comment_thread_total: 0,
                commented_block_hits: Vec::new(),
            },
        },
    ));

    // the label follows the chain — that half is right and stays right.
    assert_eq!(app.active_page_title, "New Name");
    // THE BASELINE KEEPS HER LINE 0. Adopting "New Name" here is what made the
    // next tick believe she had retitled the page.
    assert_eq!(
        app.page_saved_text, "Old Name\nbody mid-sentence",
        "the baseline may not claim a title the buffer never showed"
    );
    // and with buffer and baseline agreeing at line 0, the document is clean:
    // the tick does not even fire, so no rename can be planned from it.
    assert_eq!(
        page_document_text(&app),
        app.page_saved_text,
        "no manufactured dirt at line 0"
    );
    let _ = app.__update(__DucktapeMessage::PageAutosaveTick);
    assert_eq!(
        app.block_autosave_status,
        AutosaveStatus::Saved,
        "tick two must plan nothing — there is nothing of hers left unsaved"
    );
}

/// A RENAME TYPED DURING THE ROUND TRIP MUST STILL REACH THE CHAIN.
///
/// The correction has to use the text the tick actually reconciled against the
/// node, never the live buffer — she keeps typing while the save is in flight.
/// Feeding the live buffer adopts characters she has not saved into the
/// baseline, which makes the document read CLEAN, retires the tick that owed
/// the node her rename, and lets the next live fold rebuild the buffer and
/// erase what she typed. Worse than the bug this file exists to fix.
#[test]
fn a_title_typed_during_the_round_trip_is_not_swallowed_by_the_baseline() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.active_page = "page".into();
    app.buffer_page = "page".into();
    app.page_editor = compose("Notes\nhello");
    app.page_saved_text = "Notes\nhello".into();

    // the tick submits what it can see.
    app.page_inflight_text = "Notes \nhello".into();

    // SHE FINISHES THE WORD while the save is in flight.
    app.page_editor = compose("Notes A\nhello");

    // the save was a no-op — the trimmed title still matched the node.
    let _ = app.__update(__DucktapeMessage::PageDocumentSaved(
        backend::DocumentSaveResult {
            written: false,
            refusal: String::new(),
            document: "Notes\nhello".into(),
            data: backend::PagesData {
                pages: vec![page_item("page", "Notes")],
                blocks: vec![page_block("b1", "page", "hello")],
                active_page: "page".into(),
                active_page_title: "Notes".into(),
                active_page_parent: String::new(),
                comment_thread_total: 0,
                commented_block_hits: Vec::new(),
            },
        },
    ));

    assert_ne!(
        page_document_text(&app),
        app.page_saved_text,
        "her unsaved rename must leave the document DIRTY — a clean one retires \
         the tick that owes the node that rename, and it is never written"
    );
    let _ = app.__update(__DucktapeMessage::PageAutosaveTick);
    assert_eq!(
        app.block_autosave_status,
        AutosaveStatus::Saving,
        "the next tick must plan her rename"
    );
}

/// The refusal path takes the same correction, and nothing pinned it: deleting
/// that call site left every test in this change green while a refused write
/// plus a remote rename reverted the rename on the next tick.
#[test]
fn a_refused_write_does_not_hand_the_baseline_someone_elses_title() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.active_page = "page".into();
    app.buffer_page = "page".into();
    app.page_editor = compose("Old Name\nbody typed on");
    app.page_saved_text = "Old Name\nbody".into();
    app.page_inflight_text = "Old Name\nbody typed".into();

    // the node refused the body op, and its text carries someone else's rename.
    let _ = app.__update(__DucktapeMessage::PageDocumentSaved(
        backend::DocumentSaveResult {
            written: false,
            refusal: "that edit would destroy comments".into(),
            document: "New Name\nbody".into(),
            data: backend::PagesData {
                pages: vec![page_item("page", "New Name")],
                blocks: vec![page_block("b1", "page", "body")],
                active_page: "page".into(),
                active_page_title: "New Name".into(),
                active_page_parent: String::new(),
                comment_thread_total: 0,
                commented_block_hits: Vec::new(),
            },
        },
    ));

    assert_eq!(
        app.page_saved_text, "Old Name\nbody",
        "the baseline keeps the title she submitted — adopting the node's makes \
         the next tick believe she renamed the page and revert the other rename"
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

/// AN ERROR MUST NOT ASSERT A DIAGNOSIS IT HAS NOT MADE. `connect` discarded
/// the real cause with `map_err(|_| …)` and said "Could not connect. Check the
/// endpoint and node." — the one thing the reader can act on, and wrong
/// whenever the node is answering fine and the failure is a timeout, an
/// unreadable reply or a broken signer. Measured while debugging this screen:
/// the node served `/v1/status` in under a millisecond and the app still said
/// to go check it.
///
/// Pinned as a source shape because the failure is an async RPC round trip with
/// no seam to fake here; `user_error` itself is covered by its own tests.
#[test]
fn connect_reports_the_cause_instead_of_guessing_at_it() {
    const LIVE: &str = include_str!("backend/live.rs");
    let connect = LIVE
        .split("pub async fn connect(")
        .nth(1)
        .expect("connect is declared")
        .split("\npub ")
        .next()
        .expect("connect body");

    assert!(
        connect.contains("user_error(cause.to_string())"),
        "connect must route its cause through the translator the rest of the app uses"
    );
    assert!(
        !connect.contains("map_err(|_|"),
        "throwing the cause away is what made this error a guess"
    );
    // NOT asserted: that the old sentence is absent from the function. The
    // comment above the fix quotes it to explain what was wrong, and a sweep
    // over source text cannot tell a message from the prose about it — the
    // check would fail on its own documentation.
}

/// A CHAT-ONLY RESYNC MUST NOT CLAIM THE PAGE IT CARRIES NO NEWS ABOUT. The
/// click blanks the pane and moves `active_page`; a resync that arrives with
/// `pages_loaded == false` keeps the empty `blocks` and canonicalises
/// `title + []` into a document the node never sent. Stamping `buffer_page`
/// for that fabrication hands `page_autosave_tick` a blank document it is
/// willing to write over the real page.
#[test]
fn a_chat_only_resync_does_not_claim_the_page_it_never_loaded() {
    let mut app = reading_alpha();
    let _ = app.__update(__DucktapeMessage::ChoosePage("beta".into()));
    assert!(app.buffer_page.is_empty(), "the click released the buffer");

    let mut chat_only = live_refresh(app.hydration_generation, "", Vec::new(), "", Vec::new());
    chat_only.pages_loaded = false;
    chat_only.active_page = String::new();
    let _ = app.__update(__DucktapeMessage::LiveResynced(chat_only));

    assert!(
        app.buffer_page.is_empty(),
        "a resync carrying no page news must not claim the page as the buffer's"
    );

    // And the tick still refuses, which is the consequence that matters.
    let _ = app.__update(__DucktapeMessage::Failed(backend::AppError {
        message: "node blip".into(),
        committed: false,
    }));
    app.page_editor = compose("h");
    let _ = app.__update(__DucktapeMessage::PageAutosaveTick);
    assert_eq!(
        app.block_autosave_status,
        AutosaveStatus::Idle,
        "a fabricated buffer must never be saved into a real page"
    );
}

/// A FAILED LOAD MUST NOT LET THE BLANK PANE EAT THE PAGE IT NEVER OPENED.
/// The optimistic switch moves `active_page` and blanks the buffer before the
/// round trip. If the load then FAILS, `on failed` clears `loading` without
/// clearing `connected` or putting `active_page` back — so the reader is left
/// looking at an empty, fully typable document under the new page's title.
///
/// One keystroke there used to reach the 900ms save tick, which wrote
/// `page_text(page_editor)` into `active_page`. Saving an empty document
/// against a real page is a `RemoveBlock` for every line it had: the page would
/// be destroyed by the act of failing to open it, and the reader would never
/// have seen a line of it.
#[test]
fn a_failed_page_load_cannot_save_the_blank_pane_over_the_page() {
    let mut app = reading_alpha();

    let _ = app.__update(__DucktapeMessage::ChoosePage("beta".into()));
    let _ = app.__update(__DucktapeMessage::Failed(backend::AppError {
        message: "node blip".into(),
        committed: false,
    }));

    // The pane is live and typable: this is the state the guard must survive,
    // not one it can assume away.
    assert!(!app.loading, "the failure released the load");
    assert!(app.connected, "the failure did not disconnect");
    assert_eq!(app.active_page, "beta");
    assert!(
        app.buffer_page.is_empty(),
        "no load landed, so the buffer belongs to no page"
    );

    app.page_editor = compose("h");
    let _ = app.__update(__DucktapeMessage::PageAutosaveTick);

    // The tick must refuse: the buffer is not Beta's.
    assert_eq!(
        app.block_autosave_status,
        AutosaveStatus::Idle,
        "a buffer that belongs to no page must never be saved into one"
    );
    assert!(app.pending_page.is_empty());
}

// A CLICK MUST REPAINT ON THE CLICK. The page load is several round trips; the
// sidebar highlight, the header title and the document cannot wait for it, or
// the app reads as dead for seconds. Everything asserted here is the state of
// the very next frame — nothing has landed yet.
#[test]
fn a_page_click_repaints_before_the_load_lands() {
    let mut app = reading_alpha();

    let _ = app.__update(__DucktapeMessage::ChoosePage("beta".into()));

    assert_eq!(app.active_page, "beta", "the sidebar highlight moves now");
    assert_eq!(
        app.active_page_title, "Beta",
        "the header title comes from the page list already in hand"
    );
    assert!(
        app.active_page_parent.is_empty(),
        "the breadcrumb of the page she left must not hang over the new one"
    );
    assert!(
        app.blocks.is_empty(),
        "the previous document's blocks must leave the pane"
    );
    assert!(
        page_document_text(&app).is_empty(),
        "the previous document's text must leave the pane"
    );
    assert!(app.loading, "the load is still in flight");
    // The buffer is honest about holding nothing: `buffer_page` is what the
    // install decision reads, and the baseline moves with the buffer so an
    // empty pane never reads as dirty to the save tick.
    assert!(app.buffer_page.is_empty());
    assert!(app.page_saved_text.is_empty());
}

// `buffer_page`, not `active_page`, is what the install decision compares.
// Closing the front tab moves the selection while the buffer is still the old
// page's and still DIRTY — read against `active_page` the landing document is
// a same-page refresh, the dirty buffer refuses it, and Beta opens showing
// Alpha's text.
#[test]
fn the_landing_document_installs_when_the_page_actually_moved() {
    let mut app = reading_alpha();
    app.page_editor = compose("Alpha\nalpha body, still typing");

    let _ = app.__update(__DucktapeMessage::CloseDocTab("alpha".into()));
    assert_eq!(app.active_page, "beta", "the tab close moved the selection");
    assert_eq!(app.buffer_page, "alpha", "the buffer is still Alpha's");

    let _ = app.__update(__DucktapeMessage::PagesUpdated(page_load(
        "beta",
        "Beta",
        "beta body",
    )));

    assert_eq!(page_document_text(&app), "Beta\nbeta body");
    assert_eq!(app.page_saved_text, "Beta\nbeta body");
    assert_eq!(app.buffer_page, "beta");
    assert_eq!(app.blocks.len(), 1);
    assert_eq!(app.blocks[0].id, "beta-1");
}

// THE KEYSTROKE-EATING GUARD, which the split must not cost us: a reload of
// the page the user is typing in leaves her words alone — even when the text
// it carries is genuinely newer than the baseline (somebody else edited the
// page). A same-page refresh whose text merely equals the baseline would
// install nothing anyway, and would prove nothing here.
#[test]
fn a_refresh_never_overwrites_a_dirty_buffer_on_the_same_page() {
    let mut app = reading_alpha();
    app.page_editor = compose("Alpha\nalpha body, still typing");

    let _ = app.__update(__DucktapeMessage::PagesUpdated(page_load(
        "alpha",
        "Alpha",
        "alpha body, edited by somebody else",
    )));

    assert_eq!(
        page_document_text(&app),
        "Alpha\nalpha body, still typing",
        "a reload must never eat keystrokes"
    );
    assert_eq!(
        app.page_saved_text, "Alpha\nalpha body",
        "the baseline stays with the buffer — the drift is what makes the next tick save"
    );
    assert_eq!(app.buffer_page, "alpha");
}

// THE SAME GUARD THE CHAT COMPOSER NEVER HAD. `live_resynced` rebuilt
// `message_editor` from `message_draft` — the SETTLED stash, which reads "" the
// whole time somebody is typing — so any resync emptied a half-written message:
// a `files` write in another window, a teammate joining the huddle, any plane
// op on the chain at all. Nothing writes the composer here now; it owns its own
// text and no resync produces a new one.
#[test]
fn a_resync_never_eats_the_message_being_typed() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.shell_tab = ShellTab::Chat;
    app.active_channel = "general".into();
    app.hydration_generation = 4;
    app.message_editor = compose("half a paragraph, mid-word");

    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
        4,
        "general",
        vec![message(7, "somebody else posted", false)],
        "",
        Vec::new(),
    )));

    assert_eq!(
        composer(&app),
        "half a paragraph, mid-word",
        "a resync must never eat keystrokes either"
    );
    assert_eq!(
        app.messages.len(),
        1,
        "and it still installs the timeline it answered with"
    );
}

// Reconnect is the same-endpoint retry now — the picker owns endpoint
// changes — so typed drafts survive it untouched.
#[test]
fn same_endpoint_reconnect_preserves_unsent_drafts() {
    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.connected_rpc = "http://node-a".into();
    app.message_editor = compose("next message");
    app.failed_message_draft = "unsent message".into();

    let _ = app.__update(__DucktapeMessage::Reconnect);

    assert_eq!(app.connected_rpc, "http://node-a");
    assert_eq!(composer(&app), "next message");
    assert_eq!(app.failed_message_draft, "unsent message");
}

/// SURVIVING THE RECONNECT IS NOT THE SAME AS SURVIVING IT IN THE RIGHT ROOM.
///
/// The reconnect is one room switch spread over two handlers, and that is how it
/// escaped the park: `reconnect` carries the live composer across and blanks
/// `active_channel`, then `workspace_connected` lands on
/// `landing_channel(channels)` — the first room with traffic, rarely the room
/// she left. So #private-ops' half-typed incident note stood over #general's
/// Send, and the next pick parked those words under #general's id: she found
/// #private-ops empty and her sentence filed in a room she never typed it in.
/// The rail's composer had it worse — the reconnect simply ate it.
#[test]
fn a_reconnect_lands_each_composer_in_the_room_it_was_typed_in() {
    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.connected_rpc = "http://node".into();
    app.active_channel = "private-ops".into();
    app.active_thread_seq = 3;
    app.message_editor = compose("the incident started at");
    app.reply_editor = compose("half a reply");

    let _ = app.__update(__DucktapeMessage::Reconnect);

    let mut landed = workspace("general");
    landed.generation = app.connect_generation;
    landed.channels = vec![room("private-ops", 10), room("general", 20)];
    let _ = app.__update(__DucktapeMessage::WorkspaceConnected(landed));

    assert_eq!(
        app.active_channel, "general",
        "the connect picks the landing"
    );
    assert!(
        composer(&app).is_empty(),
        "#general's composer is #general's — the note she was writing next door \
         is not armed to send here"
    );

    app.mutation_phase = MutationPhase::Idle;
    let _ = app.__update(__DucktapeMessage::ChooseChannel("private-ops".into()));
    assert_eq!(
        composer(&app),
        "the incident started at",
        "it is waiting in the room she was writing it in"
    );

    let _ = app.__update(__DucktapeMessage::OpenThreadFor(3));
    assert_eq!(
        reply_composer(&app),
        "half a reply",
        "and the rail the reconnect closed kept its reply too"
    );
}

/// AND THE ROOM SHE LEFT DOES NOT HAUNT THE NEXT ONE AS AN "UNSENT MESSAGE".
///
/// `reconnect`'s editor harvest predated the park and left `message_draft` —
/// the settled stash — holding the LEFT room's text after the landing. Its one
/// consumer, `live_resynced`'s `remember_failed_draft(…, "channel",
/// message_draft, …)`, fires when a chat-carrying resync lands on a different
/// room, so opening a DM after reconnecting out of a room raised the
/// failed-draft plate offering to restore the old room's words into the DM
/// composer. The park owns the trip now; the stash stays empty across it.
#[test]
fn a_reconnect_does_not_leak_the_left_rooms_draft_into_the_failed_plate() {
    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.connected_rpc = "http://node".into();
    app.active_channel = "private-ops".into();
    app.message_editor = compose("the incident started at");

    let _ = app.__update(__DucktapeMessage::Reconnect);
    let mut landed = workspace("general");
    landed.generation = app.connect_generation;
    landed.channels = vec![room("private-ops", 10), room("general", 20)];
    let _ = app.__update(__DucktapeMessage::WorkspaceConnected(landed));

    // A chat-carrying resync lands on another room — the exact trip that used
    // to stash the harvest into `failed_message_draft`.
    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
        app.hydration_generation,
        "dm-with-alice",
        Vec::new(),
        "",
        Vec::new(),
    )));

    assert_eq!(app.active_channel, "dm-with-alice");
    assert!(
        app.failed_message_draft.is_empty(),
        "the room she left is parked under its own id, not offered to the room \
         she is in"
    );
}

#[test]
fn the_page_surface_is_one_editor_with_no_click_to_edit_left() {
    let components = inlined(include_str!("ui/components/pages.ice"));
    let handlers = inlined(include_str!("ui/handlers/pages.ice"));
    let view = inlined(include_str!("ui/screens/pages.ice"));

    // THE TITLE IS LINE 0 OF THE BUFFER, not a control. The click-to-edit
    // title editor is gone the same way the click-to-edit blocks are; these
    // stay as refusals so neither creeps back.
    assert!(!components.contains("PageTitleEditor"));
    assert!(!components.contains("task widget focus #title-input"));
    assert!(!components.contains("defer_focus"));
    assert!(!handlers.contains("focus_page_title"));
    assert!(!inlined(include_str!("ui/extern/backend.ice")).contains("defer_focus"));
    // THE CANVAS HAS NO MENUS LEFT TO PLACE. The block-actions popover, its
    // pointer tracking and the insert row's type dropdown are gone with the
    // click-to-edit model — a page is one editor, and `# ` is the block-type
    // menu.
    assert!(!view.contains("pages_pointer"));
    assert!(!view.contains("mouse move="));
    assert!(!view.contains(
        "scroll dir=vertical w=fill h=fill bar=hidden\n              box w=fill max-w=720.0"
    ));
    assert!(!view.contains("BlockActionsMenu"));
    assert!(!view.contains("block_menu_x"));
    assert!(!view.contains("InlineBlockInsert"));
    assert!(!view.contains("slash_kind_matches"));
    assert!(!components.contains("component DocumentBlock"));
    // The one overlay the surface still raises is the page-delete confirm.
    assert!(view.contains("overlay when=page_delete_armed"));
    // The document column opens directly on the one editor.
    assert!(view.contains("extern page_document(page_editor, dark,"));
}

#[test]
fn shell_uses_canonical_glass_and_opaque_content() {
    let ui = inlined(concat!(
        include_str!("ui/app.ice"),
        include_str!("ui/extern/backend.ice"),
        include_str!("ui/state/types.ice"),
        include_str!("ui/state/core.ice"),
        include_str!("ui/state/chat.ice"),
        include_str!("ui/state/shell.ice"),
        include_str!("ui/state/explorer.ice"),
        include_str!("ui/state/roster.ice"),
        include_str!("ui/state/forge.ice"),
        include_str!("ui/state/node.ice"),
        include_str!("ui/state/files.ice"),
        include_str!("ui/state/overlays.ice"),
        include_str!("ui/state/pages.ice"),
        include_str!("ui/state/onboarding.ice"),
        include_str!("ui/state/huddle.ice"),
        include_str!("ui/state/derived.ice"),
        include_str!("ui/theme.ice"),
        include_str!("ui/view.ice"),
        include_str!("ui/components/chat.ice"),
        include_str!("ui/components/dm.ice"),
        include_str!("ui/components/files.ice"),
        include_str!("ui/components/forge.ice"),
        include_str!("ui/components/huddle.ice"),
        include_str!("ui/components/icon.ice"),
        include_str!("ui/components/kit.ice"),
        include_str!("ui/components/node.ice"),
        include_str!("ui/components/onboarding.ice"),
        include_str!("ui/components/overlay.ice"),
        include_str!("ui/components/pages.ice"),
        include_str!("ui/components/patterns.ice"),
        include_str!("ui/components/roster.ice"),
        include_str!("ui/components/shell.ice"),
        include_str!("ui/handlers/lifecycle.ice"),
        include_str!("ui/handlers/chat.ice"),
        include_str!("ui/handlers/pages.ice"),
        include_str!("ui/handlers/shell.ice"),
    ));
    for gradient in ["linear(", "radial(", "conic("] {
        assert!(!ui.contains(gradient), "{gradient}");
        assert!(!SCREENS.contains(gradient), "{gradient}");
    }
    // The window is opaque. iced has no backdrop blur, so the chrome paints
    // the artifact's own non-glass ladder — desk/rail/sidebar/content — and
    // never a translucent tint that would composite over the desktop.
    let app = inlined(include_str!("ui/app.ice"));
    assert!(!app.contains("\n    transparent true"));
    assert!(!app.contains("\n    blur true"));
    assert!(app.contains("\n  bg app_background"));
    assert!(app.contains("\n  fg app_text"));
    let core_state = inlined(include_str!("ui/state/core.ice"));
    assert!(!core_state.contains("app_background"));
    assert!(!core_state.contains("app_text"));
    assert!(core_state.contains("appearance:Appearance = Appearance.system"));
    let derived = inlined(include_str!("ui/state/derived.ice"));
    assert!(derived.contains(
        "app_background = keep_str(appearance == Appearance.dark, \"#1b1a16\", \"#fdfdfb\")"
    ));
    assert!(
        derived.contains(
            "app_text = keep_str(appearance == Appearance.dark, \"#e8e6df\", \"#2c2b27\")"
        )
    );
    let lifecycle = inlined(include_str!("ui/handlers/lifecycle.ice"));
    assert!(!lifecycle.contains("app_background ="));
    assert!(!lifecycle.contains("app_text ="));
    assert!(!lifecycle.contains("appearance = \""));
    assert!(app.contains("titlebar-transparent true"));
    assert!(app.contains("fullsize-content-view true"));
    assert!(app.contains("font \"../../../crates/design/assets/fonts/Geist[wght].ttf\""));
    assert!(!ui.contains("white/"));
    assert!(!ui.contains("bg=glass_"));
    assert!(!SCREENS.contains("white/"));
    assert!(!SCREENS.contains("bg=glass_"));

    // The palette moved with the theme: 2.0 permits one contract and one
    // palette, and the vendored kit copy no longer carries either.
    let defaults = inlined(include_str!("ui/theme.ice"));
    for material in [
        "bg         #fdfdfb",
        "surface    #ffffff",
        "fg         #2c2b27",
        "muted_bg   #f6f5f2",
        "primary    #26251f",
        "brand      #a05a3c",
        "ring       #26251f",
        "glass_thin #fdfcfa80",
        "glass_regular #fdfcfa9e",
        "glass_sheet #fdfcfadb",
        "shadow_popover #28262221",
        "shadow_toast #28262238",
        "shadow_modal #2826224d",
    ] {
        assert!(defaults.contains(material), "{material}");
    }
    let theme = inlined(include_str!("ui/theme.ice"));
    assert!(theme.contains("font ui family=\"Geist\" weight=normal"));
    assert!(theme.contains("font display family=\"Geist\" weight=semibold"));
    assert!(theme.contains("font strong family=\"Geist\" weight=bold"));
    assert!(theme.contains("font code_medium family=\"Geist Mono\" weight=medium"));
    assert!(theme.contains("font code_semibold family=\"Geist Mono\" weight=semibold"));
    for app_token in [
        "desk #e3e1d9",
        "rail #fafaf8",
        "window_line #d6d4cc",
        "card_line #ece9e1",
        "caption #9a988f",
        "meta #a7a59b",
        "hint #b3b1a8",
        "label #bdbbb1",
        "icon_idle #cbc9bf",
        "sidebar #fbfbf9",
        "elevated #f3f2ef",
        "subtle #ecebe6",
        "row_hover #f8f7f3",
        "rail_hover #f0efea",
        "separator #efeee9",
        "scrim #28262257",
    ] {
        assert!(theme.contains(app_token), "{app_token}");
    }

    let shell = inlined(include_str!("ui/components/shell.ice"));
    // the shell is titlebar + optional degradation banner over the panes.
    assert!(shell.contains(
        "component TitleBar(network:str, height:i64, sync_line:str, loading:bool, degraded:bool, bell_badge:i64, bell_sev:str, tier:str, answered:bool, root_hash:str, consensus_view:str, quorum:str, reachable:str, last_finalized:i64, wall_now:i64)"
    ));
    // The bar exists only in the console window now — the launch window
    // wears OS chrome — so the chip and the status/bell cluster are
    // unconditional: no `phase` discriminant may return here.
    let bar = shell.split_once("component TitleBar(").unwrap().1;
    let bar = bar.split_once("\ncomponent ").unwrap().0;
    assert!(bar.contains("NetworkChip name=network"));
    assert!(!bar.contains("phase"));
    assert!(shell.contains("component ConnectionBanner(status:str)"));
    // The degradation band rides in the CONTENT column, below the rail row —
    // mounted above it, it pushed the whole nav rail down by its own height
    // whenever the connection wobbled, so a click at a remembered rail position
    // landed one item off. Pin the order, not the indent.
    let tabs = shell.split_once("component WorkspaceTabs(").unwrap().1;
    let rail_at = tabs.find("NavRail #rail").expect("the rail mounts here");
    let banner_at = tabs
        .find("ConnectionBanner status=status")
        .expect("the band mounts here");
    assert!(banner_at > rail_at, "the band must not sit above the rail");
    assert!(shell.contains("box #root w=74.0 h=fill pt=13.0 pb=10.0 bg=rail"));
    // The status tooltip ALWAYS overflows the window's right edge, and iced
    // snaps an overflowing tip hard against it. The paper therefore belongs to
    // StatusCard and the tooltip frame stays transparent, so the `pr` gutter
    // can hold the card off the wall on the bell card's line.
    assert!(bar.contains("tooltip position=bottom gap=13.5 p=0.0 delay=90 style=transparent"));
    // Per-frame extern bans. These two walk the disk (workspace tomls) or
    // deep-clone the whole timeline through the extern ABI, so the view reads
    // their STATE MIRRORS (`network_name`, `has_older_history`) instead. If
    // either name returns to a view or screen file, the per-frame tax is back.
    assert!(!SCREENS.contains("network_label("));
    assert!(!inlined(include_str!("ui/view.ice")).contains("network_label("));
    assert!(!SCREENS.contains("history_has_older("));
    assert!(!inlined(include_str!("ui/view.ice")).contains("history_has_older("));
    assert!(bar.contains("box pr=13.0\n              StatusCard "));
    assert!(shell.contains(
        "box #root w=284.0 pl=14.0 pr=14.0 pt=13.0 pb=13.0 bg=surface border=border border-w=1.0 r=13.0 shadow=shadow_modal shadow-y=16.0 shadow-blur=40.0"
    ));
    assert!(SCREENS.contains("box w=236.0 h=fill bg=sidebar clip=true"));
    assert!(SCREENS.contains("box w=230.0 h=fill bg=sidebar clip=true"));

    // The endpoint field is GONE from Settings — the launch window's picker
    // owns which network; Settings keeps only Reconnect / Switch network.
    assert!(!SCREENS.contains("#rpc"));
    assert!(SCREENS.contains("emit(switch_network)"));
    assert!(SCREENS.contains("input \"\" #key-password <-> key_pw label=\"Key password\""));
    assert!(SCREENS.contains("if active_thread_seq > 0 && !channel_settings_open"));
    // Both chat composers wear the SAME plate now — the rail dropped its old
    // transparent fg/12 frame for the stream's surface/control_line/r12 chrome.
    assert_eq!(
        SCREENS
            .matches("box w=fill bg=surface border=control_line border-w=1.0 r=12.0 clip=true")
            .count(),
        2
    );
    // the palette card moved into the overlay layer with the rest of the
    // window-level surfaces; the assertion follows the code it guards.
    let overlays = inlined(include_str!("ui/screens/overlays.ice"));
    assert!(overlays.contains(
        "bg=surface border=border border-w=1.0 r=14.0 shadow=shadow_modal shadow-y=24.0 shadow-blur=60.0"
    ));

    let authored_pages = inlined(include_str!("ui/components/pages.ice"));
    for authored in [&shell, &authored_pages, &*SCREENS] {
        assert!(!authored.contains("shadow=black/"));
        assert!(!authored.contains("shadow=shadow "));
    }
}

#[test]
fn compact_controls_share_a_single_geometry_and_type_scale() {
    assert!(SCREENS.contains("p=6.2 text-size=13.0 line-h=1.2"));
    // The composer geometry moved into the `rich_composer` extern args
    // (min_h, max_h, pad); type scale (13.5/1.3) is owned by the adapter.
    // Both chat composers share one plate; the forge note runs compact.
    assert_eq!(SCREENS.matches(", 44.0, 150.0, 10.0) #").count(), 2);
    assert!(SCREENS.contains(", 38.0, 120.0, 6.0) #forge-note"));
    assert!(SCREENS.contains("button \"Send\" disabled="));
    assert!(SCREENS.contains(
        "h=29.0 @primary_action @px-12px @py-7px -> emit(composer_event, composer_submit_event())"
    ));
    assert!(
        SCREENS
            .matches("box w=fill h=fill align-x=center align-y=center")
            .count()
            >= 10
    );
    for line in SCREENS
        .lines()
        .filter(|line| line.trim_start().starts_with("input "))
    {
        assert!(!line.contains(" h="), "{line}");
    }

    let components = inlined(concat!(
        include_str!("ui/components/shell.ice"),
        include_str!("ui/components/chat.ice"),
        include_str!("ui/components/pages.ice"),
    ));
    // the pane header is ONE geometry: a 50px plate holding a `gap=9.0`
    // centered row. Chat and pages both draw it, from their screens — the
    // components carry the pane bodies, never a second header shape.
    let pane_headers: Vec<_> = SCREENS
        .lines()
        .zip(SCREENS.lines().skip(1))
        .filter(|(plate, _)| {
            let plate = plate.trim_start();
            plate == "box w=fill h=50.0 pl=18.0 pr=18.0"
                || plate == "box w=fill h=50.0 pl=22.0 pr=22.0"
        })
        .map(|(_, row)| row.trim_start())
        .collect();
    assert_eq!(pane_headers, ["row w=fill h=fill gap=9.0 align=center"; 2]);
    assert!(!components.contains("row w=fill h=fill gap=9.0 align=center"));
    // The `+`/`⋮⋮` gutter cluster went with the block canvas.
    assert!(!components.contains("Insert block below"));
    for line in SCREENS.lines().chain(components.lines()).filter(|line| {
        [
            "button \"+\" label",
            "button \"×\" label",
            "button \"…\" label",
        ]
        .iter()
        .any(|needle| line.contains(needle))
    }) {
        assert!(line.contains("w="), "{line}");
        assert!(line.contains("h="), "{line}");
    }
}

#[test]
fn semantic_recipes_own_action_focus_and_status_colors() {
    fn assert_recipe_owns_states(name: &str, source: &str, recipe: &str) {
        let lines: Vec<_> = source.lines().collect();
        for (index, line) in lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.contains(recipe))
        {
            let indentation = line.len() - line.trim_start().len();
            for child in &lines[index + 1..] {
                let trimmed = child.trim_start();
                if trimmed.is_empty() {
                    continue;
                }
                let child_indentation = child.len() - trimmed.len();
                if child_indentation <= indentation {
                    break;
                }
                let is_direct_state = child_indentation == indentation + 2
                    && ["active ", "hovered ", "pressed ", "disabled "]
                        .iter()
                        .any(|state| trimmed.starts_with(state));
                assert!(
                    !is_direct_state,
                    "{name}: {recipe} must own its state colors: {child:?}"
                );
            }
        }
    }

    /// AN ICON-ONLY CONTROL'S GLYPH MUST INHERIT ITS BUTTON'S INK. A button's
    /// status styling reaches its content as an INHERITED text color: a
    /// `text` child carrying a `@text-*` class emits an explicit color and
    /// ignores it for every status, and an svg opts into the channel with
    /// `color=inherit` (ducktape-ui#606) — the glyph then draws the button's
    /// status-resolved text color, written AFTER the disabled pass, so its
    /// hover ink keys on the BUTTON's bounds and its disabled ink on the
    /// status ladder. A glyph outside the channel is the #1072 defect: the
    /// plate lit under the cursor and the glyph stayed muted — in the message
    /// hover bar, between a ♡ and a ⋯ that both brightened.
    ///
    /// SINGLE-GLYPH BUTTONS ONLY, AND THE EXCLUSION IS DELIBERATE — say what it
    /// lets through rather than let the `[glyph]` destructure quietly decide.
    /// Thirteen multi-glyph buttons across these two files light a plate over
    /// glyphs that all name their own colour: the ⋯ menu's icon+label rows
    /// (`Icon tone="muted"` beside a `@text-accent_fg` label under
    /// `hovered … text=fg`), the channel rows, the reaction chips, the search
    /// hits. They are NOT the defect this hunts. On a row the PLATE is the
    /// hover affordance and each glyph's colour is its ROLE — a `@text-danger`
    /// "Delete message…" that brightened to `fg` under the cursor would be the
    /// bug, and a muted icon beside an accent label is a two-tone hierarchy
    /// somebody chose. An icon-ONLY control has no plate hierarchy and no
    /// second glyph: its ink is the whole signal, which is why it is the one
    /// shape held to inheritance here. (The dead `text=` term such a row
    /// carries is inert, not wrong — it is the recipe's default reaching
    /// nothing.)
    ///
    /// AND THE RAMP MACHINERY STAYS DELETED. The old app-side `IconAction`
    /// component carried an opt-in hover ramp whose known ceiling was
    /// structural: `svg::Status::Hovered` keys on the svg's OWN bounds, so the
    /// glyph brightened over the icon instead of the plate, and `disabled` had
    /// to be a mount parameter instead of a status arm. ducktape-ui#606's
    /// inherit channel supersedes the whole path; a returning `IconAction`
    /// mount, or an svg glyph carrying `style=`/`hover=` ink of its own, is a
    /// second owner of ink the button already resolves.
    fn assert_icon_controls_inherit_ink(name: &str, source: &str) {
        assert!(
            !source.contains("IconAction"),
            "{name}: the IconAction ramp is deleted — a button's glyph is a direct \
             `svg … color=inherit` child drawing the button's status ink \
             (ducktape-ui#606), never a ramp of its own"
        );
        let lines: Vec<_> = source.lines().collect();
        for (index, _) in lines
            .iter()
            .enumerate()
            .filter(|(_, line)| line.trim_start().starts_with("button"))
        {
            let children = block_children(&lines, index);
            let lights_its_ink = children
                .iter()
                .any(|child| child.starts_with("hovered ") && child.contains(" text="));
            let glyphs: Vec<&&str> = children
                .iter()
                .filter(|child| {
                    child.starts_with("text ")
                        || child.starts_with("Icon")
                        || child.starts_with("svg ")
                })
                .collect();
            let [glyph] = glyphs[..] else { continue };
            if glyph.starts_with("svg ") {
                assert!(
                    glyph.contains("color=inherit"),
                    "{name}: a button's one svg glyph draws the button's status ink — \
                     declare `color=inherit` on the mount line, never a `style=` tint or \
                     `hover=` arm of its own: {glyph:?}"
                );
                continue;
            }
            let names_its_own_color = glyph.contains("@text-") || glyph.starts_with("Icon ");
            assert!(
                !lights_its_ink || !names_its_own_color,
                "{name}: this button's `hovered … text=` cannot reach a glyph that names its \
                 own colour — drop the `@text-*`, or inline the icon as \
                 `svg … color=inherit`: {glyph:?}"
            );
        }
    }

    /// The lines of the block a node opens: everything indented deeper than it,
    /// trimmed, up to the next sibling.
    fn block_children<'a>(lines: &[&'a str], index: usize) -> Vec<&'a str> {
        let opener = lines[index];
        let indentation = opener.len() - opener.trim_start().len();
        lines[index + 1..]
            .iter()
            .map(|line| (line.len() - line.trim_start().len(), line.trim_start()))
            .take_while(|(child_indentation, trimmed)| {
                trimmed.is_empty() || *child_indentation > indentation
            })
            .map(|(_, trimmed)| trimmed)
            .collect()
    }

    let view = inlined(include_str!("ui/view.ice"));
    let shell = inlined(include_str!("ui/components/shell.ice"));
    let chat = inlined(include_str!("ui/components/chat.ice"));
    let chat_screen = inlined(include_str!("ui/screens/chat.ice"));
    let pages = inlined(include_str!("ui/components/pages.ice"));
    let kit = inlined(include_str!("ui/components/kit.ice"));
    let forge = inlined(include_str!("ui/components/forge.ice"));

    assert_recipe_owns_states("screens", &SCREENS, "@primary_action");
    assert_recipe_owns_states("screens", &SCREENS, "@danger_action");
    assert_recipe_owns_states("chat.ice", &chat, "@danger_action");
    assert_recipe_owns_states("pages.ice", &pages, "@danger_action");
    assert!(!SCREENS.contains("active bg=brand text=fg"));
    assert!(!SCREENS.contains("hovered bg=brand/10"));
    assert!(!SCREENS.contains("hovered bg=brand/12"));
    assert!(!SCREENS.contains("font=code @text-brand"));
    assert!(!chat.contains("bg=brand/10 border=brand/22"));
    assert!(!chat.contains("bg=brand/9 border=brand/20"));
    assert!(SCREENS.contains("Badge.Outline label=\"Members only\""));
    // a tracker row's kind is carried by the PLATE behind the glyph, not by
    // a second badge next to the state — one `match item.kind`, two plates.
    assert!(forge.contains(
        "match item.kind\n            \"pr\"\n              PrStatePlate state=item.state"
    ));
    assert!(forge.contains("IssueStateGlyph state=item.state"));
    assert!(!SCREENS.contains("Badge.Outline label=item.kind"));
    // a degraded node speaks the ALERT family, never a second red language:
    // the status dot and the banner share `alert_*`, and the healthy dot is
    // the same plate in `success_dot`.
    assert!(shell.contains("bg=success_dot r=(plate / 2.0)"));
    assert!(shell.contains("bg=alert_dot r=(plate / 2.0)"));
    assert!(shell.contains("bg=alert_bg border=alert_line"));
    assert!(shell.contains("bg=alert_dot r=3.5"));
    assert!(!shell.contains("danger_"));
    assert!(
        SCREENS.contains("KeyValueRow label=\"Key state\" value=settings_key_state last=false")
    );
    assert!(SCREENS.contains("KeyValueRow label=\"Key path\" value=settings_key_path last=false"));

    for target in [
        "rename_channel_submit",
        "add_channel_member_submit",
        "fs_mkdir_submit",
        "fs_new_file_submit",
        "gov_execute",
        "account_rename_submit",
    ] {
        let kit_components = inlined(include_str!("ui/components/kit.ice"));
        let action = SCREENS
            .lines()
            .chain(kit_components.lines())
            .find(|line| line.trim_start().starts_with("button ") && line.contains(target))
            .unwrap_or_else(|| panic!("missing action target {target}"));
        assert!(action.contains("@secondary_action"), "{action}");
    }
    // A divider is `---` typed into the document now, not a button.
    assert!(!SCREENS.contains("Insert divider"));

    // The chat surface's own icon controls, both files. The same dead-ink
    // shape exists on other surfaces, but there `tone=` carries STATE (a muted
    // mic against a danger one, a checked tab against an idle one), so those
    // are a design decision rather than a defect and are not swept here.
    // icon.ice is swept so the deleted `IconAction` ramp component itself
    // cannot quietly return.
    assert_icon_controls_inherit_ink("chat.ice", &chat);
    assert_icon_controls_inherit_ink("screens/chat.ice", &chat_screen);
    assert_icon_controls_inherit_ink(
        "components/icon.ice",
        &inlined(include_str!("ui/components/icon.ice")),
    );
    // The three composer editors carried ad-hoc `focused border=ring` status
    // blocks; their focus ring now lives in the rich composer adapter
    // (`editor::composer_style`), and the fs editor is the one authored
    // `editor` ring left. Inputs inherit `@control`'s ring — see
    // `control_focus_ring_survives_the_active_base`.
    assert_eq!(
        SCREENS
            .matches("focused bg=muted_bg border=ring border-w=1.0")
            .count(),
        1
    );
    // ZERO, and it must stay zero: those two `opened` blocks styled the
    // block-type dropdowns — the one parked at the right of every insert row
    // and the one inside the `⋮⋮` menu. Pages has no block-type picker at all
    // now; the markdown prefix is the picker.
    assert_eq!(
        pages
            .matches("opened text=fg placeholder=muted handle=fg bg=fg/11 border=ring")
            .count(),
        0
    );
    assert!(!SCREENS.contains("selection=brand"));
    assert_eq!(
        SCREENS
            .matches("focused bg=transparent border=transparent value=transparent border-w=0.0")
            .count(),
        6
    );

    for binding in [
        "StatusBadge label=forge_item_state",
        "StatusBadge label=op.disposition",
    ] {
        assert!(SCREENS.contains(binding), "{binding}");
    }
    for mapping in [
        "\"active\"\n        Badge.Success label=label",
        "\"paused\"\n        Badge.Warning label=label",
        "\"open\"\n        Badge.Success label=label",
        "\"closed\"\n        Badge.Destructive label=label",
        "\"merged\"\n        Badge.Success label=label",
        "\"passed\"\n        Badge.Success label=label",
        "\"rejected\"\n        Badge.Destructive label=label",
        "\"applied\"\n        Badge.Success label=label",
        "\"discarded\"\n        Badge.Warning label=label",
    ] {
        // `StatusBadge` is the kit's now, so the state→badge table is read
        // where the component lives.
        assert!(kit.contains(mapping), "{mapping}");
    }
    assert!(SCREENS.contains("bg=danger_bg border=danger_line"));
    assert!(SCREENS.contains("bg=danger_dot"));
    assert!(SCREENS.contains("bg=success_dot"));
    // the semantic status plate is the kit's, so every screen that reports
    // a good outcome paints the same three tokens.
    assert!(kit.contains("bg=success_bg border=success_line border-w=1.0"));
    for source in [&view, &shell, &chat, &pages, &kit, &forge, &*SCREENS] {
        assert!(!source.contains("bg=success/"));
        assert!(!source.contains("border=success/"));
    }

    // EVERY action recipe styles its keyboard focus ring with the same token
    // inputs use (`focus:border-ring` on @control). The ring is origin-aware
    // (ducktape-ui#611): it paints only on keyboard/AT-acquired focus, so a
    // mouse click on a nav item no longer wears it — orthory#804's cosmetic
    // item. Swept over every `for button` recipe rather than a name list, so
    // the next action recipe added without the arm fails here.
    let recipes = [
        include_str!("ui/ducktape-ui/recipes.ice"),
        include_str!("ui/theme.ice"),
    ]
    .join("\n");
    let button_recipes: Vec<_> = recipes
        .lines()
        .zip(recipes.lines().skip(1))
        .filter(|(header, _)| header.starts_with("recipe ") && header.ends_with(" for button"))
        .collect();
    assert!(!button_recipes.is_empty(), "the action recipes moved");
    for (header, styles) in button_recipes {
        assert!(
            styles.contains("focus-visible:border-ring"),
            "{header}: an action recipe styles its keyboard focus ring \
             `focus-visible:border-ring` — the origin-aware overlay in the app's \
             ring token, at the button's own radius (ducktape-ui#611, orthory#804)"
        );
    }
}

/// THE RING HAS TO REACH THE WIDGET — asserted against the GENERATED code,
/// because the hazard lives in codegen, not in our sources. `recipe control`
/// ends in `focus:border-ring`, and `active` is the base of every status;
/// until ducktape-ui#600 the focus conditional was emitted BEFORE the authored
/// `active` base, so any input declaring `active border=` overwrote the ring
/// and no field showed the caret's seat. #1072's workaround (an authored
/// `focused … border=ring` on every such input, plus a lint requiring it) is
/// deleted; this probe is what remains.
///
/// The probe reads the palette input's generated style closure — an input with
/// an authored `active border=fg/16` base and no authored `focused` arm — and
/// pins the emission ORDER: the base's alpha write must precede the
/// `Status::Focused` conditional. Emission order is a property of ui-lang's
/// `input.rs` codegen, not of any one input, so one probe covers every
/// `@control` field; if a pin bump regresses the ordering, this fails. It does
/// NOT catch an input authoring a NEW `focused border=<not ring>` arm that
/// deliberately repaints the focused border — that is a design choice, not a
/// regression.
#[test]
fn control_focus_ring_survives_the_active_base() {
    let generated_dir = std::path::Path::new(env!("OUT_DIR")).join("ui-lang-generated");
    let entries = std::fs::read_dir(&generated_dir).expect("generated ui-lang dir");
    // TWO closures answer to `/palette-input`: the overlay's real input, and
    // the ice-test fixture's plain stub (`ui/tests/app.ice`), which authors no
    // `active` base. The fragment files are named by content hash, so read_dir
    // order reshuffles on any source change — the probe must select the
    // closure that CARRIES the authored base, not whichever file lands first.
    // The regression this probe exists for is an ORDER flip, which keeps the
    // write present, so the selection cannot mask it; only deleting the
    // authored base itself trips the expect below.
    let palette_input = entries
        .filter_map(|entry| std::fs::read_to_string(entry.expect("dir entry").path()).ok())
        .flat_map(|source| {
            source
                .lines()
                .filter(|line| line.contains("/palette-input") && line.contains("text_input("))
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .find(|line| line.contains("__color.a = 0.160000"))
        .expect("the authored `active border=fg/16` base write in the palette input's closure");
    // `active … border=fg/16` is the only alpha-0.16 write in this closure;
    // the ring is the recipe's `focus:border-ring` conditional.
    let active_base = palette_input
        .find("__color.a = 0.160000")
        .expect("just selected on this marker");
    let focus_ring = palette_input
        .find("Status::Focused")
        .expect("the recipe's `focus:border-ring` conditional");
    assert!(
        active_base < focus_ring,
        "the authored `active` base is emitted AFTER `focus:border-ring`, so the base \
         border overwrites the ring on every focused @control input (ducktape-ui#600 \
         regressed — see orthory#1089)"
    );
}

/// App-authored text sizes stay on the app design scale, while the shared
/// Ice palette stays identical to the retained ducktape-ui theme.
#[test]
fn ice_sources_hold_to_the_design_system() {
    let sources = [
        ("view.ice", inlined(include_str!("ui/view.ice"))),
        ("chat.ice", inlined(include_str!("ui/components/chat.ice"))),
        ("dm.ice", inlined(include_str!("ui/components/dm.ice"))),
        (
            "files.ice",
            inlined(include_str!("ui/components/files.ice")),
        ),
        (
            "forge.ice",
            inlined(include_str!("ui/components/forge.ice")),
        ),
        (
            "huddle.ice",
            inlined(include_str!("ui/components/huddle.ice")),
        ),
        ("icon.ice", inlined(include_str!("ui/components/icon.ice"))),
        ("kit.ice", inlined(include_str!("ui/components/kit.ice"))),
        ("node.ice", inlined(include_str!("ui/components/node.ice"))),
        (
            "onboarding.ice",
            inlined(include_str!("ui/components/onboarding.ice")),
        ),
        (
            "overlay.ice",
            inlined(include_str!("ui/components/overlay.ice")),
        ),
        (
            "pages.ice",
            inlined(include_str!("ui/components/pages.ice")),
        ),
        (
            "patterns.ice",
            inlined(include_str!("ui/components/patterns.ice")),
        ),
        (
            "roster.ice",
            inlined(include_str!("ui/components/roster.ice")),
        ),
        (
            "shell.ice",
            inlined(include_str!("ui/components/shell.ice")),
        ),
        // the screens carry the console's authored type scale now that
        // view.ice holds only the mounts.
        ("screens", SCREENS.clone()),
    ];
    for (name, source) in sources {
        for line in source.lines() {
            for token in line.split_whitespace() {
                let Some(value) = token
                    .strip_prefix("size=")
                    .or_else(|| token.strip_prefix("text-size="))
                else {
                    continue;
                };
                let Ok(size) = value.parse::<f64>() else {
                    // a prop name, not a step — the literal is at the call site
                    continue;
                };
                assert!(
                    design::type_scale::ALL.contains(&size),
                    "{name}: {size} is off the design scale — change design::type_scale, not the view: {line:?}"
                );
            }
        }
    }
    // NO size→family/weight pairing is asserted, and that is a finding, not
    // an omission: the canonical artifact draws EVERY step of the scale in
    // both faces and at several weights (12.5px alone appears at 400, 500
    // and 600, and 11px splits 226/195 mono-vs-sans). A step therefore
    // fixes size and nothing else — a guard pinning `size=11.0` to
    // `font=code_medium` was describing an older, smaller app, not the
    // design system, and would reject correct markup on nine screens.

    // the font identity: theme roles bind to the design crate's families,
    // and the app embeds exactly the crate's font assets.
    let theme = inlined(include_str!("ui/theme.ice"));
    assert!(theme.contains(&format!("family=\"{}\"", design::fonts::FAMILY_UI)));
    assert!(theme.contains(&format!("family=\"{}\"", design::fonts::FAMILY_MONO)));
    let app = inlined(include_str!("ui/app.ice"));
    for asset in design::fonts::ASSETS {
        assert!(
            app.contains(&format!("font \"../../../crates/design/{asset}\"")),
            "app.ice must embed {asset}"
        );
    }
    assert!(app.contains(&format!("text-size {}", design::type_scale::BODY)));

    let palette = ui_lang_components::ui::theme::LIGHT.palette;
    for (token, color) in [
        ("bg", palette.background),
        ("surface", palette.card),
        ("fg", palette.foreground),
        ("muted", palette.muted_foreground),
        ("muted_bg", palette.muted),
        ("primary", palette.primary),
        ("primary_fg", palette.primary_foreground),
        ("secondary", palette.secondary),
        ("secondary_fg", palette.secondary_foreground),
        ("accent", palette.accent),
        ("accent_fg", palette.accent_foreground),
        ("brand", palette.brand),
        ("brand_fg", palette.brand_foreground),
        ("brand_bg", palette.brand_background),
        ("brand_line", palette.brand_line),
        ("danger", palette.destructive),
        ("danger_fg", palette.destructive_foreground),
        ("danger_bg", palette.destructive_background),
        ("danger_line", palette.destructive_line),
        ("danger_dot", palette.destructive_dot),
        ("success", palette.success),
        ("success_fg", palette.success_foreground),
        ("success_bg", palette.success_background),
        ("success_line", palette.success_line),
        ("success_dot", palette.success_dot),
        ("warning", palette.warning),
        ("warning_fg", palette.warning_foreground),
        ("warning_bg", palette.warning_background),
        ("warning_line", palette.warning_line),
        ("warning_dot", palette.warning_dot),
        ("avatar_bg", palette.avatar),
        ("avatar_fg", palette.avatar_foreground),
        ("toast_bg", palette.toast_background),
        ("toast_fg", palette.toast_foreground),
        ("border", palette.border),
        ("control_line", palette.control_line),
        ("input", palette.input),
        ("ring", palette.ring),
    ] {
        assert_eq!(default_ice_color(token), color, "{token}");
    }
}

/// Text painted on a status fill must stay readable in BOTH themes.
///
/// `destructive` and `warning` do not invert between light and dark, they
/// SHIFT — and a foreground that does not shift with them loses contrast as
/// they do. The old React console painted a hardcoded `#fff` on them and
/// measured 3.62:1 in dark (#459). The palette now carries a real
/// `*_foreground` for each fill, which is the right shape; this asserts the
/// VALUES actually clear AA, because nothing else would notice a palette
/// that stopped clearing it.
///
/// That matters here specifically because the palette is vendored from
/// another repo by git `rev`: a routine rev bump could darken a fill with no
/// review in this tree at all.
#[test]
fn every_status_fill_carries_a_readable_foreground_in_both_themes() {
    /// WCAG 2.1 relative luminance — the sRGB channel transfer, then the
    /// standard weights.
    fn luminance(color: iced::Color) -> f32 {
        fn channel(c: f32) -> f32 {
            match c <= 0.039_28 {
                true => c / 12.92,
                false => ((c + 0.055) / 1.055).powf(2.4),
            }
        }
        0.2126 * channel(color.r) + 0.7152 * channel(color.g) + 0.0722 * channel(color.b)
    }
    fn contrast(a: iced::Color, b: iced::Color) -> f32 {
        let (x, y) = (luminance(a), luminance(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }
    /// AA for small text. Every one of these fills carries body-size text.
    const AA_SMALL: f32 = 4.5;

    for (theme_name, theme) in [
        ("light", ui_lang_components::ui::theme::LIGHT),
        ("dark", ui_lang_components::ui::theme::DARK),
    ] {
        let p = theme.palette;
        for (fill_name, fill, foreground) in [
            ("destructive", p.destructive, p.destructive_foreground),
            ("warning", p.warning, p.warning_foreground),
            ("success", p.success, p.success_foreground),
            ("primary", p.primary, p.primary_foreground),
            ("brand", p.brand, p.brand_foreground),
            ("accent", p.accent, p.accent_foreground),
            ("secondary", p.secondary, p.secondary_foreground),
            ("toast", p.toast_background, p.toast_foreground),
        ] {
            let ratio = contrast(fill, foreground);
            assert!(
                ratio >= AA_SMALL,
                "{theme_name}/{fill_name}: {ratio:.2}:1 is below WCAG AA {AA_SMALL}:1 — \
                 a fill that shifts needs a foreground that shifts with it"
            );
        }
    }
}

// The Enter/Shift+Enter send contract moved with the binding: it lives in
// `editor::tests::plain_enter_submits_and_shift_enter_edits`, against the
// classify seam the rich composer actually routes through.

/// The artifact hangs comments off the document as a docked 306px rail on
/// the sidebar ladder, NOT as a floating card over it — a card would cover
/// the block it is about the moment the block sits on the right half.
#[test]
fn block_comments_dock_a_rail_beside_the_document() {
    // the pages screen is its own file now, so the slot slicing is gone.
    let pages = inlined(include_str!("ui/screens/pages.ice"));
    // the rail is a sibling of the document, separated by the same 1px rule
    // every other docked column uses — never an overlay layer.
    let rail = pages
        .split_once("if connected && !empty(active_page) && block_comments_open\n")
        .unwrap()
        .1;
    let mut opening = rail.lines().map(str::trim);
    assert_eq!(opening.next(), Some("box w=1.0 h=fill bg=separator"));
    assert_eq!(opening.next(), Some("space w=1.0 h=1.0"));
    assert_eq!(
        opening.next(),
        Some("box w=306.0 h=fill bg=sidebar clip=true")
    );
    assert!(!pages.contains("close_block_comments backdrop=transparent"));
    assert!(pages.contains("-> emit(close_block_comments)"));
    assert!(pages.contains("#page-comment(scope_key(connected_rpc, active_page))"));
    assert!(!pages.contains("button \"Save\""));
    assert!(!pages.contains("Saving"));

    // The control is a DOCUMENT ACTION in the header now, not a row buried in
    // a per-block menu — the rail was always page-scoped.
    assert!(pages.contains("button label=\"Comments\""));
    assert!(pages.contains("-> emit(toggle_block_comments)"));
    let components = inlined(include_str!("ui/components/pages.ice"));
    assert!(!components.contains("component BlockActionsMenu"));

    let handlers = inlined(include_str!("ui/handlers/pages.ice"));
    assert!(handlers.contains("on post_block_comment_submit"));
    // A NEW comment anchors on the CARET's block (the thread's own target on
    // a reply) — never blindly on the page.
    assert!(handlers.contains(
        "run every post_block_comment(connected_rpc, password, active_thread_target, active_block_comment_thread"
    ));
    assert!(handlers.contains(
        "let fresh_target = keep_str(!empty(caret_comment_target), caret_comment_target, active_page)"
    ));
    // Opening a thread rides the thread's OWN anchor — a block-anchored
    // thread opened with the page id is refused by the node.
    assert!(handlers.contains("on open_block_comment_thread(id, target)"));
    // The document wears its comment story: washes from the load, resolve
    // available from the open thread. The editor is handed the BLOCKS and the
    // raw hit list rather than a precomputed line set, because the chip in the
    // margin spells how many threads sit on the line and the count is the
    // repetition in `commented_block_hits` — a precomputed `[i64]` of lines
    // has already thrown it away.
    assert!(pages.contains(
        "page_document(page_editor, dark, (loading || !connected), blocks, commented_block_hits)"
    ));
    assert!(pages.contains("-> emit(resolve_thread_submit, true)"));
}

#[test]
fn comment_pages_merge_by_identity_and_ordinal() {
    let thread = |id: &str, count: i64| backend::PageCommentThread {
        id: id.into(),
        target: "page".into(),
        author: "user".into(),
        meta: count.to_string(),
        resolved: false,
        comment_count: count,
    };
    let comment = |ordinal, text: &str| backend::PageComment {
        id: format!("comment-{ordinal}"),
        ordinal,
        author: "user".into(),
        meta: format!("#{ordinal}"),
        text: text.into(),
    };

    let threads = backend::append_page_comment_threads(
        vec![thread("b", 1), thread("a", 1)],
        vec![thread("b", 2), thread("c", 1)],
    );
    assert_eq!(
        threads
            .iter()
            .map(|thread| thread.id.as_str())
            .collect::<Vec<_>>(),
        ["a", "b", "c"]
    );
    assert_eq!(threads[1].comment_count, 2);

    let comments = backend::append_page_comments(
        vec![comment(1, "first"), comment(3, "old")],
        vec![comment(2, "second"), comment(3, "new")],
    );
    assert_eq!(
        comments
            .iter()
            .map(|comment| comment.ordinal)
            .collect::<Vec<_>>(),
        [1, 2, 3]
    );
    assert_eq!(comments[2].text, "new");
}

/// A PLANE'S OP REFETCHES THAT PLANE AND NO OTHER.
///
/// These five modules feed surfaces that were correct only at connect and at
/// tab-switch time: a validator joining, a proposal being voted, a device being
/// renamed, an agent registering, a file being committed — none of it reached a
/// console already looking at the page that shows it.
///
/// The generation counters ARE the assertion: each is the refetch's own guard,
/// so one moving means exactly that plane was asked for, and the others holding
/// means nothing else was.
#[test]
fn a_plane_op_refetches_only_the_plane_it_names() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;

    let plane = |app: &mut Ducktape, module: &str| {
        let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
            kind: "plane".into(),
            status: "Live".into(),
            height: 12,
            module: module.into(),
            ..backend::LiveUpdate::default()
        }));
    };

    let (members, gov, agents, account, dm, fs) = (
        app.members_generation,
        app.gov_generation,
        app.agents_generation,
        app.account_generation,
        app.dm_peers_generation,
        app.fs_generation,
    );

    plane(&mut app, "valset");
    assert_eq!(app.members_generation, members + 1, "valset feeds members");
    assert_eq!(app.gov_generation, gov, "and nothing else");
    assert_eq!(app.fs_generation, fs);

    plane(&mut app, "governance");
    assert_eq!(app.gov_generation, gov + 1);
    assert_eq!(
        app.members_generation,
        members + 1,
        "unchanged by governance"
    );

    // identity feeds TWO surfaces: the account card and the DM directory.
    plane(&mut app, "identity");
    assert_eq!(app.account_generation, account + 1);
    assert_eq!(app.dm_peers_generation, dm + 1);

    plane(&mut app, "agent");
    assert_eq!(app.agents_generation, agents + 1);

    plane(&mut app, "files");
    assert_eq!(app.fs_generation, fs + 1);

    // A module with no plane of its own moves nothing.
    let before = app.members_generation;
    plane(&mut app, "tagging");
    assert_eq!(
        app.members_generation, before,
        "an unrouted module is inert"
    );
}

/// A REMOTE RENAME REACHES LINE 0, WHICH IS WHERE THE SAVE READS THE TITLE.
///
/// `UpdateText` on the page's own block is the rename op, and it classifies as
/// `text` like any body edit — so it folds, and nothing reloads. Before the
/// title fold it landed nowhere at all: `apply_page_text` cannot see the page
/// head (the block list drops it), so the reader kept the old name on screen
/// AND in buffer line 0. Their next keystroke then ran `save_page_document`,
/// which reads the node fresh, found line 0 disagreeing with the node's new
/// title, and wrote the OLD one back — reverting someone else's rename on
/// chain, with nothing on screen.
///
/// Asserting the buffer, not just the label, is the point: line 0 is the only
/// copy of the title the save ever reads.
#[test]
fn a_folded_rename_moves_the_title_the_page_row_and_line_zero() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_page = "page".into();
    app.buffer_page = "page".into();
    app.active_page_title = "Old Name".into();
    app.pages = vec![page_item("page", "Old Name"), page_item("other", "Other")];
    app.blocks = vec![page_block("b1", "page", "body")];
    // A CLEAN buffer: baseline and buffer agree, so the rebuild is allowed.
    app.page_editor = compose("Old Name\nbody");
    app.page_saved_text = "Old Name\nbody".into();
    let before = app.hydration_generation;

    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: "pages".into(),
        status: "Live".into(),
        height: 11,
        module: "pages".into(),
        pages: backend::PagesDelta {
            kind: "text".into(),
            block_id: "page".into(),
            text: "New Name".into(),
        },
        ..backend::LiveUpdate::default()
    }));

    assert_eq!(app.active_page_title, "New Name", "the open page's title");
    assert_eq!(app.pages[0].title, "New Name", "and its row in the list");
    assert_eq!(app.pages[1].title, "Other", "and only its row");
    assert_eq!(
        page_document_text(&app),
        "New Name\nbody",
        "line 0 is the title the save reads — a stale one writes it back over the rename"
    );
    assert_eq!(
        app.page_saved_text, "New Name\nbody",
        "the baseline moves with the buffer, or the next save plans a title change nobody made"
    );
    assert_eq!(
        app.hydration_generation, before,
        "a rename still folds — it must not buy back the reload this PR removed"
    );
}

/// The dirty-buffer rule is UNCHANGED by the title fold: a reader mid-sentence
/// keeps their words and their caret. The label and the list still move (they
/// are not the reader's text), but the buffer does not.
#[test]
fn a_folded_rename_never_overwrites_a_dirty_buffer() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_page = "page".into();
    app.buffer_page = "page".into();
    app.active_page_title = "Old Name".into();
    app.pages = vec![page_item("page", "Old Name")];
    app.blocks = vec![page_block("b1", "page", "body")];
    // DIRTY: she has typed since the last save.
    app.page_editor = compose("Old Name\nbody mid-sentence");
    app.page_saved_text = "Old Name\nbody".into();

    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: "pages".into(),
        status: "Live".into(),
        height: 12,
        module: "pages".into(),
        pages: backend::PagesDelta {
            kind: "text".into(),
            block_id: "page".into(),
            text: "New Name".into(),
        },
        ..backend::LiveUpdate::default()
    }));

    assert_eq!(
        page_document_text(&app),
        "Old Name\nbody mid-sentence",
        "her buffer is hers until she saves"
    );
    assert_eq!(
        app.active_page_title, "New Name",
        "the title itself still moved — it is not part of her unsaved text"
    );
}

/// A COMMITTED EDIT LANDS WITHOUT RE-READING THE DOCUMENT IT LANDED IN.
///
/// The page autosave commits one `UpdateText` per tick while a reader types,
/// and every one used to set `load_pages` — buying a `live_resync_load` and its
/// three sequential queries, against a read path that is checkpoint-gated. Your
/// own keystrokes came back on your own stream and made you re-read the page
/// you were typing into.
///
/// `hydration_generation` is the reload's own counter, so an unchanged one IS
/// the assertion that nothing was fetched.
#[test]
fn a_folded_text_edit_updates_the_block_and_fetches_nothing() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_page = "page".into();
    app.buffer_page = "page".into();
    // THE TITLE IS HERE TO PROVE A BODY EDIT CANNOT MOVE IT. `apply_page_title`
    // rests entirely on `delta.block_id == active_page`; drop that term and
    // every body edit renames the open page — on chain, via line 0 — which is
    // the bug this fold exists to fix, pointed the other way. Nothing else in
    // the suite constrains it.
    app.active_page_title = "Doc".into();
    app.blocks = vec![
        page_block("b1", "page", "old"),
        page_block("b2", "page", "untouched"),
    ];
    let before = app.hydration_generation;

    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: "pages".into(),
        status: "Live".into(),
        height: 9,
        module: "pages".into(),
        pages: backend::PagesDelta {
            kind: "text".into(),
            block_id: "b1".into(),
            text: "typed".into(),
        },
        ..backend::LiveUpdate::default()
    }));

    assert_eq!(
        app.blocks[0].text, "typed",
        "the edit folded into its block"
    );
    assert_eq!(app.blocks[1].text, "untouched", "and only into its block");
    assert_eq!(
        app.active_page_title, "Doc",
        "a body edit must never move the page's title"
    );
    assert_eq!(
        app.hydration_generation, before,
        "a folded edit must not start a reload — that is the whole point"
    );

    // A block this document does not hold belongs to another page. Fold
    // nothing, fetch nothing.
    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: "pages".into(),
        status: "Live".into(),
        height: 10,
        module: "pages".into(),
        pages: backend::PagesDelta {
            kind: "text".into(),
            block_id: "elsewhere".into(),
            text: "another page".into(),
        },
        ..backend::LiveUpdate::default()
    }));
    assert_eq!(app.blocks[0].text, "typed");
    assert_eq!(app.hydration_generation, before);
}

/// THE RACE #1041 RECORDS: a fold is not reverted by a reload that was
/// already in flight when it landed.
///
/// A fold does not bump `hydration_generation` — folding instead of reloading
/// is its whole point — so a `live_resync_load` issued BEFORE the fold still
/// passes `live_resynced`'s generation guard when it answers AFTER it,
/// carrying a pre-fold snapshot: the sidebar row, the header title and line 0
/// all reverted, and stayed reverted until the next structural op on the page
/// happened to buy a fresh read. The fold serial is the ordering token the
/// reply must clear — and it gates ONLY the fold-owned fields, so the reply
/// still delivers the structural change it was issued for. Neither staleness
/// is traded for the other.
#[test]
fn a_fold_landing_during_a_resync_flight_is_not_reverted_by_the_reply() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_page = "page".into();
    app.buffer_page = "page".into();
    app.active_page_title = "Old Name".into();
    app.pages = vec![page_item("page", "Old Name"), page_item("other", "Other")];
    app.blocks = vec![page_block("b1", "page", "body")];
    app.page_editor = compose("Old Name\nbody");
    app.page_saved_text = "Old Name\nbody".into();

    // Someone inserts a block: the structural delta buys the debounced resync.
    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: "pages".into(),
        status: "Live".into(),
        height: 20,
        module: "pages".into(),
        load_pages: true,
        debounce: true,
        pages: backend::PagesDelta {
            kind: "touched".into(),
            ..backend::PagesDelta::default()
        },
        ..backend::LiveUpdate::default()
    }));
    let resync_generation = app.hydration_generation;
    let request_fold_serial = app.pages_fold_serial;

    // A rename folds while the resync's three reads are still executing.
    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: "pages".into(),
        status: "Live".into(),
        height: 21,
        module: "pages".into(),
        pages: backend::PagesDelta {
            kind: "text".into(),
            block_id: "page".into(),
            text: "New Name".into(),
        },
        ..backend::LiveUpdate::default()
    }));
    assert_eq!(app.active_page_title, "New Name");
    assert_eq!(
        app.hydration_generation, resync_generation,
        "a fold buys no reload — which is exactly why the in-flight reply stays current"
    );
    assert_ne!(
        app.pages_fold_serial, request_fold_serial,
        "the fold moved the serial the in-flight request snapshotted"
    );

    // The reply lands afterwards, built from the PRE-fold snapshot — but
    // carrying the inserted block, the very thing it was issued to fetch.
    let _ = app.__update(__DucktapeMessage::LiveResynced(backend::LiveRefresh {
        pages: vec![page_item("page", "Old Name"), page_item("other", "Other")],
        active_page_title: "Old Name".into(),
        fold_serial: request_fold_serial,
        ..live_refresh(
            resync_generation,
            "",
            Vec::new(),
            "page",
            vec![
                page_block("b1", "page", "body"),
                page_block("b2", "page", "inserted"),
            ],
        )
    }));

    assert_eq!(
        app.active_page_title, "New Name",
        "the fold owns the header — the pre-fold reply must not revert it"
    );
    assert_eq!(app.pages[0].title, "New Name", "and the sidebar row");
    assert_eq!(
        app.pages[1].title, "Other",
        "and only the folded row's title"
    );
    assert_eq!(
        app.blocks
            .iter()
            .map(|block| block.text.as_str())
            .collect::<Vec<_>>(),
        vec!["body", "inserted"],
        "while the reply still delivers the structural half it was issued for"
    );
    assert_eq!(
        page_document_text(&app),
        "New Name\nbody\ninserted",
        "line 0 is rebuilt from the KEPT title, so header, row and editor agree"
    );
    assert_eq!(app.page_saved_text, "New Name\nbody\ninserted");
}

/// THE HALF BOTH OF #1041's REJECTED DESIGNS LOST: the reply is NOT discarded
/// wholesale. A generation bump on the fold — or a serial gating the whole
/// pages half — would throw away the structural data the read was issued for,
/// trading one staleness for another. Only the fold-owned fields (titles,
/// block texts) are kept; every reply-owned field still lands.
#[test]
fn a_fold_in_the_window_does_not_discard_the_replys_pages_half() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_page = "page".into();
    app.buffer_page = "page".into();
    app.active_page_title = "Old Name".into();
    app.pages = vec![page_item("page", "Old Name")];
    app.blocks = vec![page_block("b1", "page", "body")];
    app.page_editor = compose("Old Name\nbody");
    app.page_saved_text = "Old Name\nbody".into();

    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: "pages".into(),
        status: "Live".into(),
        height: 30,
        module: "pages".into(),
        load_pages: true,
        debounce: true,
        pages: backend::PagesDelta {
            kind: "touched".into(),
            ..backend::PagesDelta::default()
        },
        ..backend::LiveUpdate::default()
    }));
    let resync_generation = app.hydration_generation;
    let request_fold_serial = app.pages_fold_serial;

    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: "pages".into(),
        status: "Live".into(),
        height: 31,
        module: "pages".into(),
        pages: backend::PagesDelta {
            kind: "text".into(),
            block_id: "page".into(),
            text: "New Name".into(),
        },
        ..backend::LiveUpdate::default()
    }));

    // The pre-fold reply carries page-list structure (a row state has never
    // seen), a fresh comment census and a parent — all reply-owned.
    let _ = app.__update(__DucktapeMessage::LiveResynced(backend::LiveRefresh {
        pages: vec![
            page_item("page", "Old Name"),
            page_item("brand-new", "Brand New"),
        ],
        active_page_title: "Old Name".into(),
        active_page_parent: "parent-page".into(),
        comment_thread_total: 4,
        commented_block_hits: vec!["b1".into()],
        fold_serial: request_fold_serial,
        ..live_refresh(
            resync_generation,
            "",
            Vec::new(),
            "page",
            vec![page_block("b1", "page", "body")],
        )
    }));

    assert_eq!(app.active_page_title, "New Name", "the folded title holds");
    assert_eq!(
        app.pages
            .iter()
            .map(|page| (page.id.as_str(), page.title.as_str()))
            .collect::<Vec<_>>(),
        vec![("page", "New Name"), ("brand-new", "Brand New")],
        "the list takes the reply's structure and the fold's title"
    );
    assert_eq!(
        app.active_page_parent, "parent-page",
        "no fold writes a parent, so the reply's lands"
    );
    assert_eq!(app.block_comment_thread_total, 4);
    assert_eq!(app.commented_block_hits, vec!["b1".to_string()]);
}

/// THE OWNERSHIP CALL #1041 LEFT OPEN, PINNED: block STRUCTURE is the
/// reply's, block TEXT is the fold's.
///
/// `apply_page_text` folds body edits exactly as the rename folds the title
/// (#1027), so a body edit landing in the resync window is clobbered the same
/// way — and not merely on screen: a clean buffer rebuilt from the reply's
/// pre-fold text makes the reader's next keystroke plan the OLD text back
/// onto the chain (`document_plan` is a two-way diff, and body lines have no
/// authorship guard the way the title has `title_write_owed`). The LIST is
/// still the reply's: keeping current blocks wholesale would discard the
/// inserted block the read was issued for.
#[test]
fn a_body_text_fold_keeps_its_text_and_takes_the_replys_structure() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_page = "page".into();
    app.buffer_page = "page".into();
    app.active_page_title = "Doc".into();
    app.pages = vec![page_item("page", "Doc")];
    app.blocks = vec![page_block("b1", "page", "body")];
    app.page_editor = compose("Doc\nbody");
    app.page_saved_text = "Doc\nbody".into();

    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: "pages".into(),
        status: "Live".into(),
        height: 40,
        module: "pages".into(),
        load_pages: true,
        debounce: true,
        pages: backend::PagesDelta {
            kind: "touched".into(),
            ..backend::PagesDelta::default()
        },
        ..backend::LiveUpdate::default()
    }));
    let resync_generation = app.hydration_generation;
    let request_fold_serial = app.pages_fold_serial;

    // A peer's body edit folds into b1 while the reads are executing.
    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: "pages".into(),
        status: "Live".into(),
        height: 41,
        module: "pages".into(),
        pages: backend::PagesDelta {
            kind: "text".into(),
            block_id: "b1".into(),
            text: "peer edit".into(),
        },
        ..backend::LiveUpdate::default()
    }));
    assert_eq!(app.blocks[0].text, "peer edit");

    let _ = app.__update(__DucktapeMessage::LiveResynced(backend::LiveRefresh {
        pages: vec![page_item("page", "Doc")],
        active_page_title: "Doc".into(),
        fold_serial: request_fold_serial,
        ..live_refresh(
            resync_generation,
            "",
            Vec::new(),
            "page",
            vec![
                page_block("b1", "page", "body"),
                page_block("b2", "page", "inserted"),
            ],
        )
    }));

    assert_eq!(
        app.blocks
            .iter()
            .map(|block| (block.id.as_str(), block.text.as_str()))
            .collect::<Vec<_>>(),
        vec![("b1", "peer edit"), ("b2", "inserted")],
        "the fold owns b1's text, the reply owns the list — including b2"
    );
    assert_eq!(
        page_document_text(&app),
        "Doc\npeer edit\ninserted",
        "a buffer rebuilt from the reply's pre-fold text would write it back \
         on the next keystroke — body lines have no title_write_owed"
    );
    assert_eq!(app.page_saved_text, "Doc\npeer edit\ninserted");
}

/// The gate RELEASES: a request issued after the fold snapshots the moved
/// serial, so its reply — which carries the fold's own values — lands
/// wholesale. The keep is scoped to replies the fold actually outran, not a
/// permanent title freeze.
#[test]
fn a_request_issued_after_the_fold_lands_its_title_normally() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_page = "page".into();
    app.buffer_page = "page".into();
    app.active_page_title = "Old Name".into();
    app.pages = vec![page_item("page", "Old Name")];
    app.blocks = vec![page_block("b1", "page", "body")];
    app.page_editor = compose("Old Name\nbody");
    app.page_saved_text = "Old Name\nbody".into();

    // The rename folds FIRST, then a structural delta buys the resync: the
    // request snapshots the post-fold serial.
    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: "pages".into(),
        status: "Live".into(),
        height: 50,
        module: "pages".into(),
        pages: backend::PagesDelta {
            kind: "text".into(),
            block_id: "page".into(),
            text: "New Name".into(),
        },
        ..backend::LiveUpdate::default()
    }));
    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: "pages".into(),
        status: "Live".into(),
        height: 51,
        module: "pages".into(),
        load_pages: true,
        debounce: true,
        pages: backend::PagesDelta {
            kind: "touched".into(),
            ..backend::PagesDelta::default()
        },
        ..backend::LiveUpdate::default()
    }));

    // Its reply reads post-fold state — including a SECOND rename the stream
    // has not delivered yet. Serials match, so the reply's title lands.
    let _ = app.__update(__DucktapeMessage::LiveResynced(backend::LiveRefresh {
        pages: vec![page_item("page", "Renamed Again")],
        active_page_title: "Renamed Again".into(),
        fold_serial: app.pages_fold_serial,
        ..live_refresh(
            app.hydration_generation,
            "",
            Vec::new(),
            "page",
            vec![page_block("b1", "page", "body")],
        )
    }));

    assert_eq!(
        app.active_page_title, "Renamed Again",
        "no fold outran this reply — its title is the freshest reading"
    );
    assert_eq!(app.pages[0].title, "Renamed Again");
}

#[test]
fn live_comment_refresh_updates_threads_without_touching_the_draft() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.mutation_phase = MutationPhase::Idle;
    app.active_page = "page".into();
    app.block_comments_open = true;
    // the rail is DOCUMENT-scoped: its anchor is the page it was opened
    // on, never the block selection that opened it.
    app.block_comments_target = "page".into();
    app.block_comment_draft = "draft stays".into();
    app.block_comment_threads_has_more = true;
    app.active_block_comment_thread = "deleted-thread".into();
    app.block_thread_comments = vec![backend::PageComment {
        id: "stale-comment".into(),
        ordinal: 1,
        author: "user".into(),
        meta: "#1".into(),
        text: "stale".into(),
    }];

    // a pages comment op arrives: the delta starts the debounced reload
    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: "pages".into(),
        status: "Live".into(),
        height: 8,
        load_pages: true,
        debounce: true,
        pages: backend::PagesDelta {
            kind: "touched".into(),
            ..backend::PagesDelta::default()
        },
        ..backend::LiveUpdate::default()
    }));
    let resync_generation = app.hydration_generation;
    let stale_generation = app.block_comments_generation;
    let _ = app.__update(__DucktapeMessage::LoadMoreBlockThreads);
    assert_ne!(app.block_comments_generation, stale_generation);

    // a comment refresh from a superseded generation is dropped whole
    let _ = app.__update(__DucktapeMessage::BlockThreadsLoaded(
        backend::BlockThreadListData {
            generation: stale_generation,
            target: "page".into(),
            from: 0,
            threads: Vec::new(),
            total: 0,
            next_from: 0,
            has_more: false,
        },
    ));
    assert_eq!(app.block_comment_draft, "draft stays");

    // the scoped reload lands and re-arms the comment refresh
    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
        resync_generation,
        "",
        Vec::new(),
        "page",
        vec![backend::PageBlock {
            key: 0,
            id: "block-1".into(),
            parent: "page".into(),
            kind: "Text".into(),
            text: "block".into(),
            pending: false,
            checked: false,
            prefix: String::new(),
            child_count: 0,
        }],
    )));
    let generation = app.block_comments_generation;

    let _ = app.__update(__DucktapeMessage::BlockThreadsLoaded(
        backend::BlockThreadListData {
            generation,
            target: app.block_comments_target.clone(),
            from: 0,
            threads: vec![backend::PageCommentThread {
                id: "thread-1".into(),
                target: "page".into(),
                author: "user".into(),
                meta: "1".into(),
                resolved: false,
                comment_count: 1,
            }],
            total: 3,
            next_from: 0,
            has_more: false,
        },
    ));

    assert_eq!(app.block_comment_thread_total, 3);
    assert_eq!(app.block_comment_draft, "draft stays");
    assert!(!app.block_comment_threads_loading);
    // the live refresh carries the THREAD LIST only. An open comment page
    // is not reloaded under the reader — a task group must be a handler's
    // final statement, so the reply load cannot be guarded on an open
    // thread, and firing it unguarded queries thread "" and paints its
    // failure over the rail on every page edit. Replies arrive on post and
    // on reopen instead.
    assert_eq!(app.active_block_comment_thread, "deleted-thread");
    assert_eq!(app.block_thread_comments.len(), 1);
}

#[test]
fn deltas_fold_during_thread_pagination_without_disturbing_it() {
    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.active_channel = "general".into();
    app.messages = vec![message(7, "root", false)];
    app.active_thread_seq = 7;
    app.thread_generation = 4;
    app.thread_loading = true;
    app.hydration_generation = 9;

    // a delta folds immediately — pagination in flight is not a gate
    let _ = app.__update(__DucktapeMessage::LiveUpdated(posted_delta(
        "general",
        message(8, "landed mid-pagination", false),
    )));
    assert_eq!(app.messages.len(), 2);
    assert_eq!(
        app.hydration_generation, 9,
        "a folded delta starts no reload"
    );
    assert!(app.thread_loading);

    // the pending thread page still lands on its own generation
    let _ = app.__update(__DucktapeMessage::ThreadPageLoaded(
        backend::ThreadPageData {
            generation: 4,
            messages: Vec::new(),
            next_reply_offset: 0,
            has_more: false,
        },
    ));
    assert!(!app.thread_loading);
    assert_eq!(app.hydration_generation, 9);
}

#[test]
fn block_comment_recovery_always_unlocks_mutations() {
    let (mut failed, _) = Ducktape::__boot();
    failed.block_comments_open = true;
    failed.block_comments_generation = 7;
    failed.block_comment_threads_loading = true;
    failed.mutation_phase = MutationPhase::Recovering;
    let _ = failed.__update(__DucktapeMessage::BlockThreadsRecoveryFailed(
        backend::HydrationError {
            generation: 7,
            message: "recovery read failed".into(),
        },
    ));
    assert_eq!(failed.mutation_phase, MutationPhase::Idle);
    assert!(!failed.block_comment_threads_loading);

    let (mut recovered, _) = Ducktape::__boot();
    recovered.block_comments_open = true;
    recovered.block_comments_target = "block-1".into();
    recovered.block_comments_generation = 8;
    recovered.block_comment_threads_loading = true;
    recovered.mutation_phase = MutationPhase::Recovering;
    recovered.error = "write result was uncertain".into();
    let _ = recovered.__update(__DucktapeMessage::BlockThreadsRecovered(
        backend::BlockThreadListData {
            generation: 8,
            target: "block-1".into(),
            from: 0,
            threads: Vec::new(),
            total: 0,
            next_from: 0,
            has_more: false,
        },
    ));
    assert_eq!(recovered.mutation_phase, MutationPhase::Idle);
    assert!(recovered.error.is_empty());

    // AND IT UNLOCKS ONLY WHAT IT LOCKED. "recovering" has a second terminal —
    // `live_resynced` ends the one `mutation_failed` parks — and it cannot tell
    // whose recovery it landed on, so this pair can arrive to find the lock
    // already released and a FRESH mutation holding it. Writing "idle" flatly
    // there re-enables a button whose write is still in flight, which is a
    // double submit one click away.
    let (mut overtaken, _) = Ducktape::__boot();
    overtaken.block_comments_open = true;
    overtaken.block_comments_target = "block-1".into();
    overtaken.block_comments_generation = 8;
    overtaken.mutation_phase = MutationPhase::Channel;
    let _ = overtaken.__update(__DucktapeMessage::BlockThreadsRecovered(
        backend::BlockThreadListData {
            generation: 8,
            target: "block-1".into(),
            from: 0,
            threads: Vec::new(),
            total: 0,
            next_from: 0,
            has_more: false,
        },
    ));
    assert_eq!(
        overtaken.mutation_phase,
        MutationPhase::Channel,
        "a stale recovery does not unlock the mutation that came after it"
    );

    // BOTH ARMS, because both took the term. A failed recovery is no more
    // entitled to a lock it no longer holds, and its arm would revert to a flat
    // "idle" with everything above still green.
    let (mut overtaken_failure, _) = Ducktape::__boot();
    overtaken_failure.block_comments_open = true;
    overtaken_failure.block_comments_generation = 8;
    overtaken_failure.block_comment_threads_loading = true;
    overtaken_failure.mutation_phase = MutationPhase::Channel;
    let _ = overtaken_failure.__update(__DucktapeMessage::BlockThreadsRecoveryFailed(
        backend::HydrationError {
            generation: 8,
            message: "recovery read failed".into(),
        },
    ));
    assert_eq!(
        overtaken_failure.mutation_phase,
        MutationPhase::Channel,
        "and neither does the failure arm"
    );
    assert!(!overtaken_failure.block_comment_threads_loading);
}

#[test]
fn live_thread_refresh_preserves_the_reply_draft_and_rejects_other_scopes() {
    let (mut app, _) = Ducktape::__boot();
    app.active_channel = "general".into();
    app.active_thread_seq = 7;
    app.thread_target_seq = 9;
    app.reply_editor = compose("typing");
    app.thread_messages = backend::optimistic_message(
        backend::optimistic_message(Vec::new(), "pending first".into(), "pending-first".into()),
        "pending second".into(),
        "pending-second".into(),
    );

    let _ = app.__update(__DucktapeMessage::LiveThreadRefreshed(
        backend::LiveThreadData {
            channel_id: "other".into(),
            root_seq: 7,
            target_seq: 0,
            messages: Vec::new(),
            next_reply_offset: 99,
            has_more: true,
        },
    ));
    assert_eq!(app.thread_next_reply_offset, 0);

    let _ = app.__update(__DucktapeMessage::LiveThreadRefreshed(
        backend::LiveThreadData {
            channel_id: "general".into(),
            root_seq: 7,
            target_seq: 0,
            messages: Vec::new(),
            next_reply_offset: 5,
            has_more: true,
        },
    ));
    assert_eq!(reply_composer(&app), "typing");
    assert_eq!(app.thread_target_seq, 0);
    assert_eq!(app.thread_next_reply_offset, 5);
    assert!(app.thread_has_more);
    assert!(
        app.thread_messages
            .iter()
            .any(|message| { message.pending && message.id == "pending-first" })
    );
    assert!(
        app.thread_messages
            .iter()
            .any(|message| { message.pending && message.id == "pending-second" })
    );

    let _ = app.__update(__DucktapeMessage::CloseThread);
    let _ = app.__update(__DucktapeMessage::LiveThreadRefreshed(
        backend::LiveThreadData {
            channel_id: "general".into(),
            root_seq: 7,
            target_seq: 9,
            messages: Vec::new(),
            next_reply_offset: 99,
            has_more: true,
        },
    ));
    assert_eq!(app.thread_next_reply_offset, 0);
    assert!(!app.thread_has_more);
}

#[test]
fn reconnect_recovers_active_drafts_for_the_same_endpoint() {
    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.rpc = "http://node".into();
    app.connected_rpc = "http://node".into();
    app.active_page = "page".into();
    app.block_comment_draft = "unfinished comment".into();

    let _ = app.__update(__DucktapeMessage::Reconnect);

    // A half-typed COMMENT still survives a reconnect. The page body does not
    // need the same rescue: it is one buffer whose every keystroke is already
    // heading for the node on the save tick, and it is reinstalled from the
    // node's own text on the next load.
    assert_eq!(app.orphaned_comment_drafts, ["unfinished comment"]);
}

/// "NOTHING OPEN" IS TWO DIFFERENT FACTS AND THE PLATE MUST NOT CONFLATE THEM.
/// One message served both, so a workspace that had never held a vote read
/// `0 open · 0 settled` in its header and "every decision on this network is
/// finalized" in its body — asserting a history of decisions nobody ever made.
/// Driven on the running app: the demo network shows exactly that.
///
/// Pinned as COMPLEMENTARY CONDITIONS, not as copy. Asserting the sentences
/// alone would stay green if both arms fired at once, or if the new arm were
/// unreachable.
#[test]
fn approvals_tells_a_first_run_apart_from_a_finished_one() {
    let source = inlined(include_str!("ui/screens/governance.ice"));
    let arms: Vec<&str> = source
        .lines()
        .filter(|line| line.trim_start().starts_with("if ") && line.contains("answered"))
        .map(|line| line.trim())
        .collect();

    assert!(
        arms.contains(&"if connected && empty(rows) && answered"),
        "a network with no proposals at all needs its own arm, found {arms:?}"
    );
    assert!(
        arms.contains(&"if connected && open_proposals(rows) <= 0 && !empty(rows) && answered"),
        "the settled arm must exclude the empty case, found {arms:?}"
    );

    // The two plates must never be able to fire together: the first requires an
    // empty list, the second an inhabited one.
    let first_run = source
        .split("if connected && empty(rows) && answered")
        .nth(1)
        .expect("first-run arm");
    assert!(
        first_run.contains("No proposals yet"),
        "the first-run plate says nothing has happened yet"
    );
    assert!(
        !first_run
            .split("EmptyPlate")
            .nth(1)
            .unwrap_or("")
            .contains("finalized"),
        "the first-run plate must not claim decisions were finalized"
    );
}

/// ACCOUNT FACTS ONLY WHEN THERE IS AN ACCOUNT. With no account bound,
/// `load_account` returns zeros for every field, and the identity card printed
/// `0 keys 0 nodes` one line under "· validator keypair on this device" — a
/// count of the ACCOUNT's keys reading as a count of THIS DEVICE's, and the two
/// contradicting each other inside one card.
///
/// `account_bound` is the fact that tells an empty account from no account. It
/// was already in state, already gating the Rename submit, and simply was not
/// given to the screen.
#[test]
fn the_identity_card_counts_only_a_bound_account() {
    let settings = inlined(include_str!("ui/screens/settings.ice"));
    assert!(
        settings.contains("account_bound:bool"),
        "the screen has to be handed the fact before it can use it"
    );
    let card = settings
        .split("if account_bound")
        .nth(1)
        .expect("the counts sit under the bound gate")
        .split("\n        col ")
        .next()
        .expect("card region");
    for reading in ["account_members", "account_nodes", "Copy key"] {
        assert!(
            card.contains(reading),
            "{reading} is an account reading and belongs under the gate"
        );
    }

    // And view.ice actually passes it, or the screen renders a default.
    let view = inlined(include_str!("ui/view.ice"));
    assert!(view.contains("account_bound"), "the mount has to supply it");

    // THE SEPARATOR BELONGS TO THE KEY. #998 stopped the card counting an
    // account that does not exist; the dot that had joined those counts stayed
    // ungated, and an unbound account has no `account_id` either — so the line
    // led with it: `· validator keypair on this device`. Every other separator
    // in the console is gated by the run it introduces (forge.ice's repo-count
    // dot carries the reasoning in its own comment).
    let key_line = settings
        .split_once("text account_id")
        .expect("the key line")
        .1
        .split_once(r#"text "keypair on this device""#)
        .expect("the custody clause closes it")
        .0;
    let dot = key_line.find(r#"text "·""#).expect("the separator");
    let guard = key_line
        .find("if !empty(account_id)")
        .expect("the separator's guard");
    assert!(guard < dot, "the dot is gated by the key it introduces");
}

/// A ZERO IS A CLAIM, AND THIS APP SAYS IT WITH BLANK. `count_label` returns
/// "" below one and every count in the app routes through it — except two,
/// which printed the digit right beside a plate that had just said the same
/// thing in words:
///
///   - the bell read `Alerts 0 unread` directly above "Nothing yet — mentions
///     and deliveries land here", while its own `Mark all read` was already
///     gated on `bell_unread <= 0`;
///   - Channel details read `MEMBERS 0` above "No members added."
#[test]
fn the_two_counts_that_printed_a_zero_now_say_nothing() {
    assert_eq!(backend::count_label(0), "", "the convention");
    assert_eq!(backend::count_label(3), "3");

    // The GATE is the load-bearing half — with it, the digit never reaches the
    // screen at zero whatever formats it. Both the number and the word "unread"
    // must carry it, or the panel reads a bare "unread".
    let view = inlined(include_str!("ui/view.ice"));
    let alerts = view
        .split("text \"Alerts\"")
        .nth(1)
        .expect("the bell's Alerts header")
        .split("space w=fill")
        .next()
        .expect("header region");
    assert_eq!(
        alerts.matches("if bell_unread > 0").count(),
        2,
        "the count and the word both stand down at zero"
    );
    assert!(
        alerts.contains("count_label(bell_unread)"),
        "and the digit routes through the app's own blank-at-zero label"
    );

    // Scoped to the drawer's MEMBERS eyebrow. The OTHER member count in this
    // file — the `· N added` run in the channel header — is already correct: it
    // sits under `if !empty(channel_members)` and its comment says why
    // ("`· 0 added` on every normal channel is noise"). A file-wide negative
    // would flag that one too, which is how this assertion first failed.
    let chat = inlined(include_str!("ui/screens/chat.ice"));
    let eyebrow = chat
        .split("Eyebrow label=\"MEMBERS\"")
        .nth(1)
        .expect("the drawer's MEMBERS eyebrow")
        .split("Eyebrow ")
        .next()
        .expect("eyebrow region");
    assert!(
        eyebrow.contains("count_label(len(channel_members))"),
        "the member eyebrow blanks at zero like every other count"
    );
    assert!(
        !eyebrow.contains("text len(channel_members)"),
        "the raw len is what printed `0`"
    );
}

/// AN EMPTY STATE MAY ONLY NAME A MECHANISM THAT EXISTS. Forge's two tracker
/// plates each promised a route into the list, and one of them was wrong while
/// the other named nothing at all:
///
///   - "a PR **pushed to** this repo appears here" — a push does not open a PR.
///     The only production emitter of `ForgeMsg::OpenPr` is the runs sink
///     (`crates/modules/apps/runs/src/sink.rs`), reached from `response.rs`
///     when a run with a PR sink DELIVERS. A push can update an already-open
///     PR; it cannot open one.
///   - "an issue opened against this repo appears here" — passive, naming
///     neither who opens it nor from where. `OpenIssue` has NO production
///     sender at all: only the tests and the demo seeder emit it, there is no
///     CLI verb, and the Code tab on the same screen says the app is view only.
///
/// The third plate on this screen has always done it right — the repo overview
/// says forge IS a git remote and prints the push command — so this is the
/// house style, not a new one.
#[test]
fn forge_empty_states_name_only_routes_that_exist() {
    let forge = inlined(include_str!("ui/screens/forge.ice"));

    assert!(
        forge.contains("No pull requests — an agent run opens one when it delivers its work."),
        "a PR comes from a delivering run, not from a push"
    );
    assert!(
        !forge.contains("a PR pushed to this repo"),
        "the push route was never real"
    );
    assert!(
        forge.contains("No issues — this app reads the tracker but cannot open one yet."),
        "nothing in the shipped surface opens an issue; say so rather than implying a route"
    );

    // AND THE CODE PANE MUST NOT CALL A MIRROR FETCH "UNBORN". The first fetch
    // can take seconds for a real repository; only the loader's born bit may
    // decide that no branch exists, and an empty born commit is distinct too.
    assert!(
        forge.contains("if code_phase == ForgeCodePhase.tree_loading"),
        "the in-flight tree has its own visible state"
    );
    assert!(
        forge.contains("empty(tree_entries) && !tree_born"),
        "unborn is driven by branch presence, not an empty listing"
    );
    assert!(
        forge.contains("empty(tree_entries) && tree_born"),
        "a born empty commit does not get called unborn"
    );
}

#[test]
fn interaction_state_stays_with_the_screen_that_owns_it() {
    fn component<'a>(source: &'a str, name: &str) -> &'a str {
        let opener = format!("component {name}(");
        let tail = source
            .split_once(&opener)
            .unwrap_or_else(|| panic!("{name} exists"))
            .1;
        tail.split_once("\ncomponent ")
            .map_or(tail, |(body, _)| body)
    }

    fn local_state(component: &str) -> Vec<&str> {
        component
            .lines()
            .skip_while(|line| line.trim() != "state")
            .skip(1)
            .take_while(|line| line.trim().is_empty() || line.starts_with("    "))
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect()
    }

    let members = component(SCREENS.as_str(), "MembersScreen");
    let members_state = local_state(members);
    for field in [
        "filter:MembersFilter = MembersFilter.all",
        "selected = \"\"",
    ] {
        assert!(
            members_state.contains(&field),
            "MembersScreen owns `{field}`"
        );
    }
    for handler in ["on pick_members_filter(next)", "on open_member(key)"] {
        assert!(members.contains(handler), "MembersScreen owns `{handler}`");
    }

    let explorer = component(SCREENS.as_str(), "ExplorerScreen");
    let explorer_state = local_state(explorer);
    for field in [
        "query = \"\"",
        "kind = \"all\"",
        "hits:[ExplorerHit] = []",
        "kinds:[KindCount] = []",
        "partial = \"\"",
        "searching = false",
        "selected:i64 = 0",
    ] {
        assert!(
            explorer_state.contains(&field),
            "ExplorerScreen owns `{field}`"
        );
    }
    for handler in [
        "on explorer_search_submit(rpc, online)",
        "on explorer_results_loaded(next)",
        "on clear_explorer_search",
        "on pick_explorer_kind(next)",
        "on select_explorer_block(height)",
    ] {
        assert!(
            explorer.contains(handler),
            "ExplorerScreen owns `{handler}`"
        );
    }

    let chat = component(SCREENS.as_str(), "ChatScreen");
    let chat_state = local_state(chat);
    for field in [
        "message_action_focus = \"\"",
        "chat_pointer_y = 0.0",
        "chat_height = 720.0",
        "thread_pointer_y = 0.0",
        "thread_height = 720.0",
    ] {
        assert!(chat_state.contains(&field), "ChatScreen owns `{field}`");
    }
    for handler in [
        "on chat_pointer_pressed(_x, y)",
        "on chat_resized(_width, height)",
        "on thread_pointer_pressed(_x, y)",
        "on thread_resized(_width, height)",
    ] {
        assert!(chat.contains(handler), "ChatScreen owns `{handler}`");
    }

    let files = component(SCREENS.as_str(), "FilesScreen");
    assert!(local_state(files).contains(&"history_open = false"));
    assert!(files.contains("on fs_toggle_history"));

    let root_state = inlined(&ice_sources_in("state"));
    let root_view = inlined(include_str!("ui/view.ice"));
    for field in [
        "members_filter",
        "members_selected",
        "explorer_query",
        "explorer_kind",
        "explorer_hits",
        "explorer_kinds",
        "explorer_partial",
        "explorer_searching",
        "explorer_selected",
        "message_action_focus",
        "chat_pointer_y",
        "chat_height",
        "message_menu_y",
        "thread_pointer_y",
        "thread_height",
        "thread_menu_y",
        "fs_history_open",
        "pending_message",
        "pending_reply",
        "live_settle",
        "page_landing",
        "page_install",
        "pages_answer_is_current",
        "pages_fold_outran_reply",
        "escape_key",
        "content_scroll",
        "palette_key",
        "settings_endpoint",
    ] {
        assert!(
            !root_state.contains(field),
            "root state reclaimed `{field}`"
        );
        assert!(!root_view.contains(field), "root view plumbs `{field}`");
    }
    for route in [
        "pick_members_filter ->",
        "open_member ->",
        "explorer_search_submit ->",
        "clear_explorer_search ->",
        "pick_explorer_kind ->",
        "select_explorer_block ->",
        "fs_toggle_history ->",
        "chat_pointer_pressed ->",
        "chat_resized ->",
        "thread_pointer_pressed ->",
        "thread_resized ->",
    ] {
        assert!(
            !root_view.contains(route),
            "root view still routes `{route}`"
        );
    }

    // These fields only name values used during one handler invocation.
    let chat_handlers = inlined(include_str!("ui/handlers/chat.ice"));
    for handler in [
        "on chat_pointer_pressed",
        "on chat_resized",
        "on thread_pointer_pressed",
        "on thread_resized",
    ] {
        assert!(
            !chat_handlers.contains(handler),
            "app handler reclaimed component geometry route `{handler}`"
        );
    }
    for local in ["pending_message_id", "pending_reply_id"] {
        assert!(!root_state.contains(local), "root state holds `{local}`");
        assert!(
            chat_handlers.contains(&format!("let {local} =")),
            "`{local}` is minted as a handler local"
        );
    }
    assert!(
        !inlined(include_str!("ui/state/chat.ice"))
            .lines()
            .any(|line| line.trim_start().starts_with("reply_draft =")),
        "root state reclaimed `reply_draft`"
    );
    let page_handlers = inlined(include_str!("ui/handlers/pages.ice"));
    assert!(!root_state.contains("closing_doc_tab"));
    assert!(page_handlers.contains("on close_doc_tab(id)"));
    assert!(page_handlers.contains("doc_tabs = doc_tabs_without(doc_tabs, id)"));
    assert!(!root_state.contains("page_link"));
    assert!(page_handlers.contains("let page_link = page_link_of(event)"));

    let native_surfaces = concat!(
        include_str!("backend/live.rs"),
        include_str!("backend/load.rs"),
        include_str!("backend/mod.rs"),
        include_str!("backend/model.rs"),
        include_str!("backend/storage.rs"),
        include_str!("frame_probe.rs"),
    );
    for removed in ["files_tree", "active_channel_huddle_count"] {
        for (path, source) in ice_sources() {
            assert!(!source.contains(removed), "{path} restored `{removed}`");
        }
        assert!(
            !native_surfaces.contains(removed),
            "native app code restored `{removed}`"
        );
    }
}

/// THE EXPLORER NAMES WHAT IT SHOWS. Several defects on one screen, all the
/// same shape — the pixels were right and the words were absent or false.
///
/// The header claimed the list holds "the blocks this node verified for
/// itself". It does not. `GET /v1/blocks` serves the derived-index rows, and
/// `bin/noded`'s projection writes NO row for a block whose members are all the
/// `consensus.nop` heartbeat ("a pure nop/idle block — the explorer hides it").
/// Measured against the demo node: `/v1/status` height 419718, while the
/// hundred rows `/v1/blocks` served spanned heights 102907-366045 — a
/// hundred-row "recent" window covering 263k heights, which no lag explains. A
/// reader comparing the top row against the height in the titlebar concludes
/// the node is fifty thousand blocks behind.
///
/// Naming the set truthfully then obliges the list to HOLD that set in every
/// role, which it did not: a node following from a checkpoint writes one
/// op-less boundary row, and that row printed `0 ops` under the new sentence.
///
/// The op detail then printed two values nobody named: `chat(+0m/+0e)`, whose
/// units are spelled out only in `crates/kernel/host`, and a hash that differs
/// from the row's hash on the left because one is the frame id and the other
/// the op payload digest.
///
/// This pins the DERIVATION, the SET, and the LABEL SITES rather than the
/// sentences: the copy may be rewritten, but a count must carry its noun, a
/// value must carry its name, one set must not have two names on one screen,
/// and no row may contradict the name the screen prints over it.
///
/// AND WHAT THE SCREEN LOADS MUST LAND IN ITS OWN STATE. Search is interaction
/// state owned by `ExplorerScreen`; its reply and both reset paths must still
/// carry every field the view reads.
#[test]
fn the_explorer_names_what_it_shows() {
    // The dispatch trace, fed the `operations` shape `bin/noded`'s projection
    // serves. Both units appear once singular and once plural, so a hand-rolled
    // `{n} msgs` that skips the `plural` seam fails here.
    let hops = vec![
        serde_json::json!({
            "module": "chat", "origin": "external",
            "emitted_msgs": 1, "emitted_events": 0,
        }),
        serde_json::json!({
            "module": "tagging", "origin": "module:chat",
            "emitted_msgs": 0, "emitted_events": 2,
        }),
    ];
    assert_eq!(
        backend::explorer_trace(Some(&hops)),
        "chat · 1 msg · 0 events → tagging · 0 msgs · 2 events",
        "every count in the trace names what it counts"
    );

    let explorer = SCREENS
        .split_once("component ExplorerScreen(")
        .expect("the Explorer screen exists")
        .1;
    let explorer = explorer
        .split_once("\ncomponent ")
        .map_or(explorer, |(body, _)| body);

    // ONE SET, ONE NAME. The subtitle and the "No blocks yet" plate describe
    // the same list and had drifted — only the plate knew the list is filtered.
    // The clause lives here once so a rewrite has to move both sites together.
    const SET: &str = "locks that carried operations";
    for (site, opener) in [
        ("subtitle", "ScreenTitle title=\"Explorer\" detail=\""),
        (
            "empty plate",
            "EmptyState title=\"No blocks yet\" description=\"",
        ),
    ] {
        let sentence = explorer
            .split_once(opener)
            .unwrap_or_else(|| panic!("the Explorer {site} is where it was"))
            .1
            .split_once('"')
            .expect("a quoted string closes")
            .0;
        assert!(
            sentence.contains(SET),
            "the Explorer {site} describes the block list as `{sentence}` \
             instead of naming the set the node actually serves"
        );
    }

    // AND NO ROW MAY CONTRADICT THAT SENTENCE — which is a claim about the
    // DATA, not about the copy, because `/v1/blocks` is not uniformly filtered.
    // Three of its four row writers drop an op-less block; the fourth,
    // `boundary_block_row` (`bin/node/src/explorer.rs`, applied in
    // `replica/park.rs`), writes the follower's ascension tip with `hash: ""`
    // and no ops. `bin/node/src/main.rs` routes every key that is neither a
    // validator nor seated by the checkpoint into `replica::run` — every joined
    // member until promotion — so that row drew a blank hash and `0 ops`
    // directly under the subtitle asserted above, and opened to an empty pane.
    let served = [
        serde_json::json!({
            "height": 41, "hash": "", "commit_hash": "aa11bb22cc33dd44", "ops": [],
        }),
        serde_json::json!({
            "height": 42, "hash": "ee55ff66aa77bb88", "commit_hash": "cc99dd00ee11ff22",
            "ops": [{
                "proposer": "abc123def456789a", "disposition": "applied", "target": "chat",
                "op_hash": "0f1e2d3c4b5a6978", "payload": "hi", "operations": hops,
            }],
        }),
    ];
    let window = backend::explorer_window(0, &served);
    assert_eq!(
        window
            .blocks
            .iter()
            .map(|block| (block.height, block.op_count))
            .collect::<Vec<_>>(),
        vec![(42, 1)],
        "the Explorer listed a block carrying no operations under a subtitle \
         that says every row carried some"
    );
    assert!(
        window.ops.iter().all(|op| op.height == 42),
        "an op was attributed to a block the list does not hold"
    );

    // AND EVERY VALUE IN THE OP DETAIL CARRIES ITS NAME. `by` was already
    // right and is pinned with the two that were not, so the rule reads as a
    // rule instead of an exception for these two fields. `hash` and not `op`:
    // the list's third column already spends `op` as a count noun (`1 op`).
    let detail = explorer
        .split_once("for op in explorer_ops_at(ops, selected)")
        .expect("the op detail pane iterates the selected block's ops")
        .1;
    for (label, value) in [
        ("hash", "text op.op_hash"),
        ("by", "text op.proposer"),
        ("dispatch", "text op.trace"),
    ] {
        let named = format!("text \"{label}\"");
        let label_at = detail
            .find(&named)
            .unwrap_or_else(|| panic!("`{value}` is drawn with no `{named}` beside it"));
        let value_at = detail
            .find(value)
            .unwrap_or_else(|| panic!("the op detail no longer draws `{value}`"));
        assert!(
            label_at < value_at,
            "`{named}` must precede `{value}`, or it labels something else"
        );
    }

    // EVERY FIELD OF THE REPLY IS CARRIED, OR THE SCREEN SHOWS A DEFAULT AND
    // CALLS IT AN ANSWER. `partial` is the field this rule was written for:
    // without it the strip's kinds and the hit count are still rendered, so the
    // screen goes back to presenting whatever survived as the whole truth.
    let loaded = explorer
        .split_once("on explorer_results_loaded(next)")
        .expect("the Explorer's results handler")
        .1
        .split_once("\n  on ")
        .expect("the next handler closes it")
        .0;
    // AND A FACT ABOUT THE LAST SEARCH DIES WITH IT. Both resets already clear
    // the hits and the strip; a `partial` left standing keeps naming a source
    // that failed to answer a query the reader has since cleared or replaced.
    let resets = ["on explorer_search_submit", "on clear_explorer_search"].map(|opener| {
        explorer
            .split_once(opener)
            .unwrap_or_else(|| panic!("`{opener}` is where it was"))
            .1
            .split_once("\n  on ")
            .expect("the next handler closes it")
            .0
    });
    for (field, cleared) in [("hits", "[]"), ("kinds", "[]"), ("partial", r#""""#)] {
        assert!(
            loaded.contains(&format!("{field} = next.{field}")),
            "`{field}` comes back from the search and nothing lands it in \
             the screen's local state"
        );
        for reset in &resets {
            assert!(
                reset.contains(&format!("{field} = {cleared}")),
                "a reset that leaves `{field}` standing shows the last search's \
                 answer over the next one"
            );
        }
    }
}

/// A LIST THAT DRIVES A DETAIL PANE MUST SAY WHICH ROW IT IS SHOWING. The
/// Explorer's block rows carried `active bg=transparent` on BOTH states, so
/// clicking a row filled the pane on the right and left the list identical to
/// the pixel — measured on the running app at 36.78/255 before and after the
/// click, pointer parked away. The only visible response was `hovered`, which
/// follows the pointer rather than the selection. In a list whose every row is a
/// height and a truncated hash there is no landmark to re-find your place by.
///
/// Pinned as a DIFFERENCE, not as a colour: asserting `selected_row` alone
/// would stay green if the unselected arm adopted it too, which is the same
/// both-arms-identical bug in a new coat.
#[test]
fn the_explorer_marks_the_block_row_whose_detail_is_open() {
    let source = inlined(include_str!("ui/screens/storage.ice"));
    let row = source
        .split("component ExplorerBlockRow")
        .nth(1)
        .expect("ExplorerBlockRow is declared")
        .split("\ncomponent ")
        .next()
        .expect("component body");

    let selected = row.split("if !selected").next().expect("selected arm");
    let unselected = row.split("if !selected").nth(1).expect("unselected arm");

    assert!(
        selected.contains("active bg=selected_row"),
        "the open row wears a plate"
    );
    assert!(
        unselected.contains("active bg=transparent"),
        "every other row stays flat"
    );
    // The whole point: the two arms must not paint the same thing.
    let plate_of = |arm: &str| {
        arm.lines()
            .find_map(|line| {
                line.trim()
                    .strip_prefix("active bg=")
                    .map(|rest| rest.split_whitespace().next().unwrap_or("").to_owned())
            })
            .expect("each arm sets active bg=")
    };
    assert_ne!(
        plate_of(selected),
        plate_of(unselected),
        "a selected row that paints the unselected plate marks nothing"
    );
}

/// ONE TOKEN MEANS "THIS IS THE ROW YOU ARE ON". The Forge file tree marked its
/// selected row with `tree_selected`; the Files tree marked its own with
/// `subtle` — #35322a against #31302b, 2.3/255 apart in dark. Nobody saw the
/// difference, which IS the defect: two tokens for one meaning drift apart the
/// moment either is retuned. `subtle` could never have been the selection plate
/// anyway, because it is also the PRESSED plate on those very components —
/// pressing an unselected channel painted it the colour of the selected one.
/// Thirteen surfaces mark a current row; ten of them were split across `subtle`
/// and `elevated`. All thirteen now read one token, named for the meaning it
/// carries rather than for the first tree that needed it: `selected_row`.
///
/// Pinned as the CONVENTION in two halves, because neither holds alone:
///   - NEGATIVE, self-extending: no arm that opens on "this is the current one"
///     may rest on `subtle` or `elevated`. This is the half that fails on the
///     NEXT surface someone writes, which a pin on any one call site cannot.
///   - COVERAGE: the set of sources carrying the token is fixed, so a surface
///     cannot quietly slide off the convention onto some third plate.
///
/// Marks that are not a plate at all rest on their own tokens ON PURPOSE and
/// are commented where they live: the tab underline and the filter chip invert
/// to ink, the matrix cell takes the faintest wash because its column HEAD
/// already wears the mark, and the huddle camera toggle is an engaged control
/// A CHIP INSIDE A MESSAGE ROW MUST NOT WEAR A PLATE THE ROW ITSELF COULD BE
/// WEARING. #1008 gave the current row `selected_row`; the reacted chip was
/// painting `brand_bg`, and both of its other states deflated to washes. On a
/// message you had open the mark and the card were the same colour.
///
/// Luminance means computed from `ui/theme.ice`, distance from `selected_row`,
/// light / dark:
///
/// | plate | before | after |
/// |---|---|---|
/// | reacted chip | `brand_bg` 6.65 / 9.12 | `brand` **128.43 / 102.41** |
/// | un-reacted chip | `elevated`, `subtle` on hover/press | `muted_bg` 9.02 / 13.37, held in every state |
/// | thread chip | `muted_bg`, `elevated` on hover/press | `surface` 19.06 / 17.14, held in every state |
///
/// This pins the three arms by their own state lines rather than by a scanner.
/// A scanner was tried across four rounds of #1016 and grew a fresh hole each
/// time — an unbounded forward walk that measured the next component, a
/// `transparent` skip, then `border-w=0`. The rule that generalises already
/// exists directly below (`every_current_row_marker_rests_on_one_selection_token`);
/// this one only has to say what these three chips wear.
#[test]
fn no_chip_inside_a_message_row_deflates_onto_the_row() {
    let chat = inlined(include_str!("ui/components/chat.ice"));
    for (chip, states) in [
        (
            "reacted",
            [
                "active bg=brand text=brand_fg border=brand border-w=1.0 r=11.0",
                "hovered bg=brand text=brand_fg border=brand_line",
                "pressed bg=brand text=brand_fg border=brand_fg",
            ],
        ),
        (
            "un-reacted",
            [
                "active bg=muted_bg text=muted border=control_line border-w=1.0 r=11.0",
                "hovered bg=muted_bg text=fg border=brand_line",
                "pressed bg=muted_bg text=fg border=brand",
            ],
        ),
        (
            "thread",
            [
                "active bg=surface text=brand border=control_line border-w=1.0 r=8.0",
                "hovered bg=surface text=brand border=brand_line",
                "pressed bg=surface text=brand border=brand",
            ],
        ),
    ] {
        for state in states {
            assert!(
                chat.contains(state),
                "the {chip} chip must hold its plate in every state: {state}"
            );
        }
    }

    // And none of them may go back to a wash that sits within ~13/255 of the
    // row plate. Scoped to the two chip components, because `elevated` and
    // `subtle` are legitimate elsewhere in this file.
    let chips = chat
        .split_once("component ReactionChip(")
        .expect("the reaction chip")
        .1
        .split_once("\ncomponent ")
        .expect("it ends")
        .0;
    for wash in ["bg=brand_bg", "bg=elevated", "bg=subtle"] {
        assert!(
            !chips.contains(wash),
            "a chip on the row you are reading may not rest on {wash}"
        );
    }
}

/// rather than a row you navigated to.
#[test]
fn every_current_row_marker_rests_on_one_selection_token() {
    // Every arm that opens on "this is the current one", paired with each
    // plate it RESTS on: the `bg=` of the arm's own first node, and any
    // button's `active bg=` inside it. `hovered`/`pressed` are not resting
    // plates, and a descendant's dot or badge is not the row's plate.
    fn current_row_plates(source: &str) -> Vec<(String, String)> {
        let lines: Vec<&str> = source.lines().collect();
        let mut out = Vec::new();
        for (at, line) in lines.iter().enumerate() {
            let Some(cond) = line.trim().strip_prefix("if ") else {
                continue;
            };
            // A negation is the OTHER arm; a comparison is a count guard
            // (`if selected > 0`), not a selection.
            let names_the_current_one = cond
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .any(|word| word == "selected" || word == "active");
            let opens_a_marker = names_the_current_one && !cond.contains(['!', '<', '>']);
            if !opens_a_marker {
                continue;
            }
            let indent = line.len() - line.trim_start().len();
            let arm: Vec<&&str> = lines[at + 1..]
                .iter()
                .take_while(|body| {
                    body.trim().is_empty() || body.len() - body.trim_start().len() > indent
                })
                .collect();
            let plate_of = |text: &str| {
                text.split_whitespace()
                    .find_map(|token| token.strip_prefix("bg="))
                    .map(str::to_owned)
            };
            let own_plate = arm
                .iter()
                .find(|body| !body.trim().is_empty())
                .and_then(|body| plate_of(body));
            let button_plates = arm
                .iter()
                .filter(|body| body.trim().starts_with("active "))
                .filter_map(|body| plate_of(body));
            for plate in own_plate.into_iter().chain(button_plates) {
                out.push((cond.to_owned(), plate));
            }
        }
        out
    }

    // EVERY authored ice source, each paired with its own path so a failure
    // names the file. The scan must see all of them: a source left out is a
    // surface the convention stops covering.
    macro_rules! ice_sources {
        ($($path:literal),* $(,)?) => { [$(($path, include_str!($path))),*] };
    }
    let sources = ice_sources![
        "ui/components/chat.ice",
        "ui/components/dm.ice",
        "ui/components/files.ice",
        "ui/components/forge.ice",
        "ui/components/huddle.ice",
        "ui/components/icon.ice",
        "ui/components/kit.ice",
        "ui/components/node.ice",
        "ui/components/onboarding.ice",
        "ui/components/overlay.ice",
        "ui/components/pages.ice",
        "ui/components/patterns.ice",
        "ui/components/roster.ice",
        "ui/components/shell.ice",
        "ui/screens/chat.ice",
        "ui/screens/forge.ice",
        "ui/screens/governance.ice",
        "ui/screens/overlays.ice",
        "ui/screens/pages.ice",
        "ui/screens/roster.ice",
        "ui/screens/settings.ice",
        "ui/screens/shell.ice",
        "ui/screens/storage.ice",
        "ui/view.ice",
    ];

    let mut carriers: Vec<&str> = Vec::new();
    for (name, raw) in sources {
        // `with` blocks fold back onto their node line, so a plate and the
        // node it paints stay ONE line however the source was wrapped.
        let source = inlined(raw);
        if source.contains("bg=selected_row") {
            carriers.push(name);
        }
        for (cond, plate) in current_row_plates(&source) {
            assert!(
                plate != "subtle" && plate != "elevated",
                "{name}: `if {cond}` marks the current row with `{plate}` — \
                 that is the track grey / the raised surface, not `selected_row`"
            );
        }
    }

    // The surfaces that mark a current row, all of them, on one token: the
    // channel and the DM you are reading, the page you are editing, the tree
    // directory and the open object, the tree file and the repo switcher, the
    // matrix column head, the network you picked, the nav rail tab and
    // Settings, the member whose card is open, the Explorer block you
    // inspected. A source that drops off this list has either lost its mark or
    // invented a second token for it; both are the bug this test exists for.
    assert_eq!(
        carriers,
        [
            "ui/components/chat.ice",
            "ui/components/dm.ice",
            "ui/components/files.ice",
            "ui/components/forge.ice",
            "ui/components/node.ice",
            "ui/components/onboarding.ice",
            "ui/components/pages.ice",
            "ui/components/shell.ice",
            "ui/screens/roster.ice",
            "ui/screens/shell.ice",
            "ui/screens/storage.ice",
        ],
        "every surface that marks a current row reads `selected_row`"
    );
}

/// BOTH COMPOSERS RE-ASK THE GATE AT APPLY TIME, AND BOTH ARE PINNED HERE. A
/// composer's `disabled=` was decided a frame ago, so a channel that went
/// archived — or a members-only roster that dropped her — between the keystroke
/// and the Enter would otherwise let the send through and surface as a server
/// rejection she cannot act on. The optimistic row is the tell: it is written
/// BEFORE the request, so a refused send that still appends one has skipped the
/// gate.
#[test]
fn neither_composer_sends_into_a_channel_that_refuses_the_post() {
    // The two reasons `post_gate` names, each driven through both composers.
    for (reason, archived, members_only) in [
        ("channel_archived", true, false),
        ("members_only", false, true),
    ] {
        let (mut app, _) = Ducktape::__boot();
        app.connected = true;
        app.loading = false;
        app.active_channel = "general".into();
        app.active_channel_archived = archived;
        app.active_channel_members_only = members_only;
        // Empty roster: she is not seated, which is what `members_only` refuses.
        app.channel_members = Vec::new();
        app.settings_user_key = "me".into();
        // The gate the composers re-ask is the MIRROR every handler that moves
        // one of those four inputs writes — so the fixture writes it the same
        // way, through `post_gate` itself, and the two reasons are still real.
        app.post_refusal = backend::post_gate(
            archived,
            members_only,
            app.channel_members.clone(),
            app.settings_user_key.clone(),
        );
        assert_eq!(app.post_refusal, reason);

        app.message_editor = compose("into the void");
        let _ = app.__update(__DucktapeMessage::ChatComposerEvent(
            editor::composer_submit_event(),
        ));
        assert!(
            app.messages.is_empty(),
            "the main composer must refuse a {reason} channel at apply time"
        );
        // The words are still hers — a refusal is not a discard.
        assert_eq!(composer(&app), "into the void");

        app.active_thread_seq = 7;
        app.reply_editor = compose("into the void");
        let _ = app.__update(__DucktapeMessage::ReplyComposerEvent(
            editor::composer_submit_event(),
        ));
        assert!(
            app.thread_messages.is_empty(),
            "the reply composer must refuse a {reason} channel at apply time"
        );
        assert_eq!(reply_composer(&app), "into the void");
    }

    // AND THE GATE IS NOT A BLANKET REFUSAL: seated in the same members-only
    // channel, both composers send. Without this the asserts above would pass
    // against a composer that refused everything.
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_channel = "general".into();
    app.active_channel_members_only = true;
    app.channel_members = vec![backend::ChatMember {
        key: "me".into(),
        label: "me".into(),
    }];
    app.settings_user_key = "me".into();
    app.post_refusal = backend::post_gate(
        false,
        true,
        app.channel_members.clone(),
        app.settings_user_key.clone(),
    );
    assert!(
        app.post_refusal.is_empty(),
        "a seated member is not refused"
    );

    app.message_editor = compose("hello");
    let _ = app.__update(__DucktapeMessage::ChatComposerEvent(
        editor::composer_submit_event(),
    ));
    assert_eq!(app.messages.len(), 1, "a seated member still posts");

    app.active_thread_seq = 7;
    app.reply_editor = compose("hello back");
    let _ = app.__update(__DucktapeMessage::ReplyComposerEvent(
        editor::composer_submit_event(),
    ));
    assert_eq!(
        app.thread_messages.len(),
        1,
        "a seated member still replies"
    );
}

/// A REPLY IS FORMATTABLE, through both of the doors the stream's composer has.
///
/// The rail's composer had NEITHER: no toolbar (the seat row was hint + Send),
/// and the Cmd/Ctrl chord — supposedly the keyboard half of the same table —
/// was hard-wired to `message_editor` in `handlers/overlays.ice`. The chord
/// rides the app's ONE global key subscription, which sees no widget focus, so
/// Cmd+B pressed with the caret in a thread reply wrapped the CHANNEL draft
/// instead: a silent write into a composer the user was not looking at.
///
/// Both halves are pinned because they are separate mechanisms with the same
/// failure. The toolbar half needs its MOUNTS asserted too: `ComposerMarks` is
/// one component seated twice, and the two seats must route to DIFFERENT
/// editors — a same-name `forward` would have collapsed them back onto one,
/// which is exactly the defect in miniature.
#[test]
fn a_thread_reply_takes_marks_from_its_own_toolbar_and_the_chord() {
    // THE MOUNTS. Two seats of one component, two routes.
    assert_eq!(SCREENS.matches("ComposerMarks disabled=(").count(), 2);
    assert!(SCREENS.contains("mark -> emit(composer_mark, _)"));
    assert!(SCREENS.contains("mark -> emit(reply_composer_mark, _)"));

    // THE TOOLBAR half: the rail's Bold wraps the REPLY and nothing else.
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.shell_tab = ShellTab::Chat;
    app.active_channel = "general".into();
    app.active_thread_seq = 7;
    app.message_editor = compose("channel draft");
    app.reply_editor = compose("reply draft");
    app.reply_editor
        .perform(iced::widget::text_editor::Action::SelectAll);
    let _ = app.__update(__DucktapeMessage::ReplyComposerMark("bold".into()));
    assert_eq!(reply_composer(&app), "**reply draft**");
    assert_eq!(
        composer(&app),
        "channel draft",
        "the stream draft is not the reply's"
    );

    // THE CHORD half, caret in the reply. A click into an editor arrives as a
    // composer event — that event is what stamps the focus the subscription
    // cannot read, so drive it rather than poking the field.
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.shell_tab = ShellTab::Chat;
    app.active_channel = "general".into();
    app.active_thread_seq = 7;
    let _ = app.__update(__DucktapeMessage::ReplyComposerEvent(
        editor::ComposerEvent::Apply(editor::RichAction::Edit(
            iced::widget::text_editor::Action::Move(iced::widget::text_editor::Motion::DocumentEnd),
        )),
    ));
    app.message_editor = compose("channel draft");
    app.reply_editor = compose("reply draft");
    app.reply_editor
        .perform(iced::widget::text_editor::Action::SelectAll);
    let _ = app.__update(__DucktapeMessage::GlobalKeyPressed(command_chord(
        iced::keyboard::key::Code::KeyB,
    )));
    assert_eq!(reply_composer(&app), "**reply draft**");
    assert_eq!(
        composer(&app),
        "channel draft",
        "Cmd+B in a reply is not a channel edit"
    );

    // AND IT IS NOT A BLANKET REDIRECT: the same chord with the caret back in
    // the stream's composer still marks the stream's draft, rail open or not.
    // Without this arm the asserts above would pass against a chord that had
    // simply been rewired to the reply editor.
    let _ = app.__update(__DucktapeMessage::ChatComposerEvent(
        editor::ComposerEvent::Apply(editor::RichAction::Edit(
            iced::widget::text_editor::Action::Move(iced::widget::text_editor::Motion::DocumentEnd),
        )),
    ));
    app.message_editor = compose("channel draft");
    app.message_editor
        .perform(iced::widget::text_editor::Action::SelectAll);
    let reply_before = reply_composer(&app);
    let _ = app.__update(__DucktapeMessage::GlobalKeyPressed(command_chord(
        iced::keyboard::key::Code::KeyB,
    )));
    assert_eq!(composer(&app), "**channel draft**");
    assert_eq!(reply_composer(&app), reply_before);
}

/// A CLAIM ON THE CARET HAS TO DIE WHEN THE CARET LEAVES. `composer_focus`
/// stands in for widget focus the app cannot read: the rich editor drops its
/// own focus on any press landing outside it (`rich_text_editor.rs` sets
/// `state.focus = None` in the else-arm of its press handler) and publishes
/// NOTHING when it does. So a discriminant stamped on entry is honest only for
/// as long as the set of handlers that retire it is complete — and #1005
/// shipped the claim with no retire at all, leaving Cmd+B marking the reply
/// draft while the caret sat in an inline edit box, on another tab, or in a
/// channel two switches away.
///
/// The enforcement is three MECHANICAL rules, not a remembered list, because
/// the hole was never in a route that existed — it was in the one nobody
/// thought to write. A handler carrying a `task widget focus` moves the caret
/// by hand; a handler writing `shell_tab` unmounts the composer under it; a
/// handler writing a literal `active_thread_seq = 0` tears the rail, and the
/// reply composer, out from under it. Any of the three must RETIRE — `"none"`
/// and nothing else, since a mover by definition took the caret somewhere that
/// is not a chat composer. The pinned set then catches the two the rules cannot
/// name, `open_thread_for`'s reset included: deleting that line used to fail
/// nothing.
///
/// Every rule here records the VALUE and not merely the assignment. A retire
/// flipped to `"message"` is a claim on a composer the caret is not in — the
/// exact defect — and a lint that only counted assignments called that green.
///
/// The rules cannot reach every rail close, and the last arm here is the one
/// they miss on purpose — which is what the chord's own `active_thread_seq > 0`
/// term is for. Both halves are driven, so neither the guard nor the retires
/// can be deleted quietly.
/// OPENING THE CHANNEL DRAWER IS NOT A REQUEST TO CLOSE THE THREAD.
/// `toggle_channel_settings` cleared `active_thread_seq`, the thread's messages
/// and `reply_editor` on the way in, so a part-typed reply was gone and closing
/// the drawer gave back an empty one. The main composer's draft survives the
/// same trip, which is the app's own standard — `reconnect` parks it
/// deliberately rather than letting a transition eat it.
///
/// A NOTE ON HOW THIS WAS FOUND, because the first account of it was wrong.
/// The live drive that "reproduced" it had clicked (1408, 164) — which with the
/// rail open is the RAIL's own `×`, not the channel header's `⋯` (that moves to
/// 1077 when the rail narrows the column). `close_thread` discarding a reply is
/// by design. The defect is real on the drawer's path and this test is what
/// proves it: restoring the teardown fails the first assertion below. The fix
/// was then driven correctly — drawer opened at 1077, Escape, reply intact.
///
/// The teardown was never what hid the rail: the screen draws it under
/// `if active_thread_seq > 0 && !channel_settings_open`. `close_thread` remains
/// the one route that discards a reply, because that one is a request to.
#[test]
fn the_channel_drawer_does_not_eat_a_reply_you_are_typing() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.shell_tab = ShellTab::Chat;
    app.active_channel = "general".into();
    app.active_thread_seq = 7;
    app.thread_messages = vec![message(7, "the root", false)];
    app.reply_editor = compose("half a reply");

    let _ = app.__update(__DucktapeMessage::ToggleChannelSettings);
    assert!(app.channel_settings_open, "the drawer opened");
    assert_eq!(
        reply_composer(&app),
        "half a reply",
        "the drawer does not discard a reply in progress"
    );
    assert_eq!(app.active_thread_seq, 7, "and it does not close the thread");
    assert_eq!(
        app.thread_messages.len(),
        1,
        "nor throw away the thread it was reading"
    );

    // Closing it gives the rail back exactly as it was.
    let _ = app.__update(__DucktapeMessage::ToggleChannelSettings);
    assert!(!app.channel_settings_open);
    assert_eq!(reply_composer(&app), "half a reply");
    assert_eq!(app.active_thread_seq, 7);

    // The screen is what hides the rail while the drawer is up — the handler
    // never needed to.
    assert!(
        SCREENS.contains("if active_thread_seq > 0 && !channel_settings_open"),
        "the rail is drawn under the drawer's own gate"
    );
    // And the claim on the caret still retires, because the drawer lays its own
    // inputs over a composer that stays mounted.
    let chat = inlined(include_str!("ui/handlers/chat.ice"));
    let arm = chat
        .split_once("on toggle_channel_settings")
        .expect("the handler")
        .1
        .split_once("\non ")
        .expect("it ends")
        .0;
    // Statements, not prose: the comment above this handler NAMES the
    // teardown it no longer does, and a substring check over the arm would
    // read that as the teardown itself. Third time tonight.
    let statements: Vec<&str> = arm
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with("//"))
        .collect();
    assert!(statements.contains(&"composer_focus = ComposerFocus.unfocused"));
    assert!(
        !statements.contains(&"active_thread_seq = 0"),
        "the drawer must not tear the rail down"
    );
}

#[test]
fn every_handler_that_moves_the_caret_retires_the_composer_focus() {
    // THE BEHAVIOUR, on the route the rules are about: a claim, then a handler
    // that takes the caret with the rail still open — so neither the
    // `active_thread_seq > 0` gate nor the tab gate can save it — then the
    // chord. It must mark NEITHER draft.
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.shell_tab = ShellTab::Chat;
    app.active_channel = "general".into();
    app.active_thread_seq = 7;
    let _ = app.__update(__DucktapeMessage::ReplyComposerEvent(
        editor::ComposerEvent::Apply(editor::RichAction::Edit(
            iced::widget::text_editor::Action::Move(iced::widget::text_editor::Motion::DocumentEnd),
        )),
    ));
    let _ = app.__update(__DucktapeMessage::BeginMessageEdit(7, "hello".into(), 2));
    app.message_editor = compose("channel draft");
    app.reply_editor = compose("reply draft");
    app.reply_editor
        .perform(iced::widget::text_editor::Action::SelectAll);
    let _ = app.__update(__DucktapeMessage::GlobalKeyPressed(command_chord(
        iced::keyboard::key::Code::KeyB,
    )));
    assert_eq!(
        reply_composer(&app),
        "reply draft",
        "the caret is in the inline edit box, so Cmd+B is not a reply edit"
    );
    assert_eq!(
        composer(&app),
        "channel draft",
        "and it is not a channel edit either — a retired claim marks neither"
    );

    // THE SAME BEHAVIOUR ON THE RAIL'S OWN OPEN, which is where the VALUE of a
    // retire is load-bearing. `open_thread_for` inherits whatever the channel
    // composer claimed, and the click that opened the rail landed on a message
    // row — the caret is in NEITHER box. The rail is open, so `"reply"` is as
    // live as `"message"` here: every wrong value this one line could carry
    // marks a draft, which is why the assertion is on both drafts and not on
    // the presence of the line.
    let (mut rail, _) = Ducktape::__boot();
    rail.connected = true;
    rail.loading = false;
    rail.shell_tab = ShellTab::Chat;
    rail.active_channel = "general".into();
    let _ = rail.__update(__DucktapeMessage::ChatComposerEvent(
        editor::ComposerEvent::Apply(editor::RichAction::Edit(
            iced::widget::text_editor::Action::Move(iced::widget::text_editor::Motion::DocumentEnd),
        )),
    ));
    let _ = rail.__update(__DucktapeMessage::OpenThreadFor(7));
    rail.message_editor = compose("channel draft");
    rail.reply_editor = compose("reply draft");
    rail.message_editor
        .perform(iced::widget::text_editor::Action::SelectAll);
    let _ = rail.__update(__DucktapeMessage::GlobalKeyPressed(command_chord(
        iced::keyboard::key::Code::KeyB,
    )));
    assert_eq!(
        composer(&rail),
        "channel draft",
        "opening the rail moved the caret off the channel composer, so Cmd+B \
         is not a channel edit"
    );
    assert_eq!(
        reply_composer(&rail),
        "reply draft",
        "and the rail's own composer never had it either — the click landed on \
         a message row"
    );

    // THE ONE RAIL CLOSE NO RETIRE CAN COVER, which is the whole job of the
    // chord's `active_thread_seq > 0` term. Someone deletes the thread root
    // while you are typing a reply: `live_resynced` answers 0 for a root it
    // finds deleted (`refreshed_known_message_seq`) and the rail — with the
    // reply composer in it — is gone. That handler ALSO runs on every ordinary
    // resync while the rail stays open and you keep typing, so it cannot
    // retire unconditionally the way the user-driven teardowns do. The claim
    // survives on purpose; the READ side is what has to be honest.
    let (mut gone, _) = Ducktape::__boot();
    gone.connected = true;
    gone.loading = false;
    gone.shell_tab = ShellTab::Chat;
    gone.active_channel = "general".into();
    gone.hydration_generation = 4;
    gone.active_thread_seq = 7;
    let _ = gone.__update(__DucktapeMessage::ReplyComposerEvent(
        editor::ComposerEvent::Apply(editor::RichAction::Edit(
            iced::widget::text_editor::Action::Move(iced::widget::text_editor::Motion::DocumentEnd),
        )),
    ));
    let _ = gone.__update(__DucktapeMessage::LiveResynced(live_refresh(
        4,
        "general",
        vec![message(7, "the root", true)],
        "",
        Vec::new(),
    )));
    assert_eq!(
        gone.active_thread_seq, 0,
        "a deleted root closes the rail under the caret"
    );
    assert_eq!(
        gone.composer_focus,
        ComposerFocus::Reply,
        "and nothing retires the claim on that route — if this ever stops \
         holding, the arm below has gone vacuous and this gate needs a new pin"
    );
    // Both drafts are seated after the resync — this arm is about which box the
    // chord lands in, not about what a resync leaves in them.
    gone.message_editor = compose("channel draft");
    gone.reply_editor = compose("reply draft");
    gone.reply_editor
        .perform(iced::widget::text_editor::Action::SelectAll);
    let _ = gone.__update(__DucktapeMessage::GlobalKeyPressed(command_chord(
        iced::keyboard::key::Code::KeyB,
    )));
    assert_eq!(
        reply_composer(&gone),
        "reply draft",
        "a closed rail is never the chord's target, however stale the claim is"
    );
    assert_eq!(
        composer(&gone),
        "channel draft",
        "and a stale \"reply\" does not fall through to the channel draft either"
    );

    // THE RULES. Every handler file, so a focus mover added to a screen nobody
    // is thinking about today still has to answer.
    const HANDLERS: [(&str, &str); 11] = [
        ("chat", include_str!("ui/handlers/chat.ice")),
        ("files", include_str!("ui/handlers/files.ice")),
        ("forge", include_str!("ui/handlers/forge.ice")),
        ("huddle", include_str!("ui/handlers/huddle.ice")),
        ("lifecycle", include_str!("ui/handlers/lifecycle.ice")),
        ("node", include_str!("ui/handlers/node.ice")),
        ("onboarding", include_str!("ui/handlers/onboarding.ice")),
        ("overlays", include_str!("ui/handlers/overlays.ice")),
        ("pages", include_str!("ui/handlers/pages.ice")),
        ("roster", include_str!("ui/handlers/roster.ice")),
        ("shell", include_str!("ui/handlers/shell.ice")),
    ];

    // `app.ice` is the real registry; the list above is a hand copy of it, and
    // a twelfth handler file would otherwise ship unscanned.
    for line in include_str!("ui/app.ice").lines() {
        let Some(rest) = line.trim_start().strip_prefix("use \"handlers/") else {
            continue;
        };
        let Some(file) = rest.strip_suffix(".ice\"") else {
            continue;
        };
        assert!(
            HANDLERS.iter().any(|(scanned, _)| *scanned == file),
            "app.ice registers handlers/{file}.ice and this lint does not read \
             it — add it to HANDLERS, or the next focus mover lands there \
             unchecked"
        );
    }

    let mut moves_the_caret: Vec<String> = Vec::new();
    // Handler AND value: `composer_focus = ComposerFocus.message` in a retire is the defect
    // itself, so recording only the handler name pins nothing worth pinning.
    let mut writes_the_focus: Vec<String> = Vec::new();
    for (file, source) in HANDLERS {
        // Per FILE, not per sweep: carrying the previous file's last handler in
        // here credits it with any statement standing above the first `on `.
        let mut handler = format!("{file}::<above the first handler>");
        for line in source.lines() {
            if let Some(rest) = line.strip_prefix("on ") {
                handler = format!("{file}::{}", rest.split('(').next().unwrap_or(rest).trim());
            }
            let statement = line.trim_start();
            let takes_the_caret = statement.starts_with("task widget focus");
            let unmounts_the_tab = statement.starts_with("shell_tab = ");
            // The LITERAL zero only. A computed write (`= seq`,
            // `= next.active_thread_seq`, `= refreshed_known_message_seq(…)`)
            // may leave the rail open, so it is not a teardown and a retire
            // there would fire mid-typing; the chord's own `> 0` gate covers
            // what those can produce, and the last behaviour arm drives it.
            let closes_the_rail = statement == "active_thread_seq = 0";
            if takes_the_caret || unmounts_the_tab || closes_the_rail {
                moves_the_caret.push(handler.clone());
            }
            if let Some(value) = statement.strip_prefix("composer_focus = ") {
                writes_the_focus.push(format!("{handler} = {}", value.trim()));
            }
        }
    }
    moves_the_caret.sort();
    moves_the_caret.dedup();
    writes_the_focus.sort();
    writes_the_focus.dedup();

    let silent: Vec<&String> = moves_the_caret
        .iter()
        .filter(|mover| !writes_the_focus.contains(&format!("{mover} = ComposerFocus.unfocused")))
        .collect();
    assert!(
        silent.is_empty(),
        "these handlers move the caret (`task widget focus`), unmount the \
         composer under it (`shell_tab = `), or tear the thread rail out from \
         under it (`active_thread_seq = 0`) without RETIRING the claim on it — \
         each needs `composer_focus = ComposerFocus.unfocused`, and `unfocused` is the only \
         honest value: a mover took the caret somewhere that is not a chat \
         composer: {silent:?}"
    );

    assert_eq!(
        writes_the_focus,
        [
            "chat::arm_message_delete = ComposerFocus.unfocused",
            "chat::arm_thread_message_delete = ComposerFocus.unfocused",
            "chat::begin_message_edit = ComposerFocus.unfocused",
            "chat::begin_thread_message_edit = ComposerFocus.unfocused",
            "chat::chat_composer_event = ComposerFocus.message",
            "chat::choose_channel = ComposerFocus.unfocused",
            "chat::choose_dm = ComposerFocus.unfocused",
            "chat::close_thread = ComposerFocus.unfocused",
            "chat::open_chat_search_hit = ComposerFocus.unfocused",
            "chat::open_message_actions = ComposerFocus.unfocused",
            "chat::open_message_reactions = ComposerFocus.unfocused",
            "chat::open_thread_for = ComposerFocus.unfocused",
            "chat::open_thread_message_actions = ComposerFocus.unfocused",
            "chat::open_thread_message_reactions = ComposerFocus.unfocused",
            "chat::reply_composer_event = ComposerFocus.reply",
            "chat::toggle_channel_create = ComposerFocus.unfocused",
            "chat::toggle_channel_settings = ComposerFocus.unfocused",
            "huddle::huddle_go_channel = ComposerFocus.unfocused",
            "lifecycle::reconnect = ComposerFocus.unfocused",
            "lifecycle::select_shell_tab = ComposerFocus.unfocused",
            "onboarding::console_opened = ComposerFocus.unfocused",
            "overlays::global_key_pressed = ComposerFocus.unfocused",
            "pages::open_page_search_hit = ComposerFocus.unfocused",
            "pages::toggle_page_create = ComposerFocus.unfocused",
        ],
        "a handler started, stopped, or CHANGED what it says about the caret: \
         exactly two may CLAIM it (the two composer-event handlers, and only \
         with their own composer's name), everyone else here RETIRES it to \
         `unfocused` — decide which yours is, then update this list"
    );
}

/// One Cmd/Ctrl chord, shaped the way the keyboard subscription delivers it.
/// AN ORDINARY KEYSTROKE IS NOT A CHORD, AND MUST NOT BE CHARGED AS ONE.
/// `global_key_pressed` rides the app's ONE keyboard subscription, so it sees
/// every letter typed into a composer. Its three `editor` self-assignments each
/// lower to `mem::take(&mut self.<editor>)`, which leaves a `Content::default()`
/// behind — a fresh cosmic-text buffer built under a WRITE lock on the
/// process-global font system — so a letter used to pay three of them on the
/// literal typing path, serialized against whatever the renderer was shaping.
/// The handler now resolves all four verdicts up front and returns when the
/// press names none of them.
///
/// The saving is invisible in state (a take hands the same document straight
/// back), so the guard's POSITION is pinned in the source and its only real
/// failure mode — refusing a press that should act — is driven here, one press
/// per class the guard tests.
#[test]
fn an_inert_key_press_leaves_the_handler_before_it_rebuilds_an_editor() {
    let overlays = inlined(include_str!("ui/handlers/overlays.ice"));
    let body = overlays
        .split_once("\non global_key_pressed(event)")
        .expect("the keyboard handler")
        .1;
    let guard = body
        .find("  return if empty(escape_key)")
        .expect("the inert-press guard");
    for take in [
        "message_editor = composer_toggle_mark(",
        "reply_editor = composer_toggle_mark(",
        "page_editor = page_history_key(",
    ] {
        let at = body.find(take).expect(take);
        assert!(
            guard < at,
            "`{take}…` takes the editor, so it must sit BELOW the inert-press guard"
        );
    }

    fn plain(code: iced::keyboard::key::Code, key: iced::keyboard::Key) -> __IceKeyPress {
        __IceKeyPress {
            key,
            modified_key: iced::keyboard::Key::Unidentified,
            physical_key: iced::keyboard::key::Physical::Code(code),
            location: iced::keyboard::Location::Standard,
            modifiers: iced::keyboard::Modifiers::empty(),
            text: None,
            repeat: false,
        }
    }
    let escape = || {
        plain(
            iced::keyboard::key::Code::Escape,
            iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
        )
    };

    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.shell_tab = ShellTab::Chat;
    app.active_channel = "general".into();
    let _ = app.__update(__DucktapeMessage::ChatComposerEvent(
        editor::ComposerEvent::Apply(editor::RichAction::Edit(
            iced::widget::text_editor::Action::Move(iced::widget::text_editor::Motion::DocumentEnd),
        )),
    ));
    app.message_editor = compose("draft");

    // Inert: a bare letter marks nothing and opens nothing.
    let _ = app.__update(__DucktapeMessage::GlobalKeyPressed(plain(
        iced::keyboard::key::Code::KeyB,
        iced::keyboard::Key::Character("b".into()),
    )));
    assert_eq!(composer(&app), "draft", "a bare letter is not a mark");
    assert!(!app.palette_open);

    // …and every class the guard tests still gets through it.
    let _ = app.__update(__DucktapeMessage::GlobalKeyPressed(command_chord(
        iced::keyboard::key::Code::KeyB,
    )));
    assert_eq!(composer(&app), "****draft", "the chord still marks");

    let _ = app.__update(__DucktapeMessage::GlobalKeyPressed(command_chord(
        iced::keyboard::key::Code::KeyK,
    )));
    assert!(app.palette_open, "Cmd+K still opens the palette");
    let _ = app.__update(__DucktapeMessage::GlobalKeyPressed(escape()));
    assert!(!app.palette_open, "Escape still closes it");

    app.channel_settings_open = true;
    let _ = app.__update(__DucktapeMessage::GlobalKeyPressed(escape()));
    assert!(
        !app.channel_settings_open,
        "and the escape ladder still runs below the guard"
    );

    // The pages chord is the third take, so it is driven too.
    app.shell_tab = ShellTab::Pages;
    app.page_editor = iced::widget::text_editor::Content::with_text("one");
    pages::history::record(|| ("".to_owned(), app.page_editor.cursor()));
    let _ = app.__update(__DucktapeMessage::GlobalKeyPressed(command_chord(
        iced::keyboard::key::Code::KeyZ,
    )));
    assert_eq!(
        app.page_editor.text(),
        "",
        "Cmd+Z on the pages tab still reaches the buffer"
    );
    pages::history::reset();
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

#[test]
fn failed_optimistic_send_rolls_back_and_restores_the_draft() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_channel = "general".into();
    app.message_editor = compose("retry me");

    let _ = app.__update(__DucktapeMessage::ChatComposerEvent(
        editor::composer_submit_event(),
    ));
    let operation_id = app.messages[0].id.clone();
    let _ = app.__update(__DucktapeMessage::MessageSendFailed(
        backend::OptimisticMutationError {
            message: "rejected".into(),
            committed: false,
            operation_id,
            scope_id: "general".into(),
            body: "retry me".into(),
        },
    ));

    assert_eq!(composer(&app), "retry me");
    assert_eq!(app.message_draft, "retry me");
    assert!(app.failed_message_draft.is_empty());
    assert!(app.messages.is_empty());
    assert_eq!(app.error, "rejected");
    assert_eq!(app.mutation_phase, MutationPhase::Idle);
}

#[test]
fn failed_send_preserves_the_next_and_unsent_drafts() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_channel = "general".into();
    app.message_editor = compose("first");

    let _ = app.__update(__DucktapeMessage::ChatComposerEvent(
        editor::composer_submit_event(),
    ));
    let operation_id = app.messages[0].id.clone();
    app.message_editor = compose("second");
    let _ = app.__update(__DucktapeMessage::MessageSendFailed(
        backend::OptimisticMutationError {
            message: "rejected".into(),
            committed: false,
            operation_id,
            scope_id: "general".into(),
            body: "first".into(),
        },
    ));

    assert_eq!(composer(&app), "second");
    assert_eq!(app.failed_message_draft, "first");
    app.message_editor = compose("");
    let _ = app.__update(__DucktapeMessage::RestoreFailedMessage);
    assert_eq!(composer(&app), "first");
    assert_eq!(app.message_draft, "first");
    assert!(app.failed_message_draft.is_empty());
}

/// A FAILURE THAT ARRIVES AFTER SHE LEFT THE ROOM IS STILL HER TEXT.
///
/// The whole handler used to return on the room check, so a send refused while
/// she was reading another channel left no error, no unsent stash, and no row —
/// and the last thing she saw was the message sitting in the timeline. The room
/// check now scopes the timeline surgery only: the stash and the banner are
/// written above it, and the composer she is typing in NOW is not touched.
#[test]
fn a_send_that_fails_after_she_moved_rooms_still_reaches_her() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_channel = "general".into();
    app.message_editor = compose("the deploy is at 4pm");

    let _ = app.__update(__DucktapeMessage::ChatComposerEvent(
        editor::composer_submit_event(),
    ));
    let operation_id = app.messages[0].id.clone();

    // She switches rooms while the write is in flight, and starts a new message
    // there. `choose_channel` blanks the timeline; the pending row is gone.
    let _ = app.__update(__DucktapeMessage::ChooseChannel("random".into()));
    app.message_editor = compose("different thought");

    let _ = app.__update(__DucktapeMessage::MessageSendFailed(
        backend::OptimisticMutationError {
            message: "rejected".into(),
            committed: false,
            operation_id,
            scope_id: "general".into(),
            body: "the deploy is at 4pm".into(),
        },
    ));

    assert_eq!(app.error, "rejected", "the refusal must be said out loud");
    assert_eq!(
        app.failed_message_draft, "the deploy is at 4pm",
        "and the body she typed must be recoverable, not gone"
    );
    assert_eq!(
        composer(&app),
        "different thought",
        "the composer belongs to the room she is in now — a restore here would \
         overwrite the message she is writing"
    );

    // THE SAME HOLE ON THE REPLY PATH, and wider: `close_thread` empties
    // `thread_messages`, so merely closing the rail under an in-flight reply
    // made the pending check fail and dropped the failure whole.
    let (mut rail, _) = Ducktape::__boot();
    rail.connected = true;
    rail.loading = false;
    rail.active_channel = "general".into();
    rail.active_thread_seq = 7;
    rail.reply_editor = compose("on it");
    let _ = rail.__update(__DucktapeMessage::ReplyComposerEvent(
        editor::composer_submit_event(),
    ));
    let reply_id = rail.thread_messages[0].id.clone();
    let _ = rail.__update(__DucktapeMessage::CloseThread);
    let _ = rail.__update(__DucktapeMessage::ThreadReplySendFailed(
        backend::OptimisticMutationError {
            message: "reply rejected".into(),
            committed: false,
            operation_id: reply_id,
            scope_id: "general".into(),
            body: "on it".into(),
        },
    ));

    assert_eq!(rail.error, "reply rejected");
    assert_eq!(
        rail.failed_reply_draft, "on it",
        "a closed rail is not a reason to throw the reply away"
    );
}

/// A PENDING ROW HAS NO SEQ, SO IT CANNOT ANSWER FOR THE TOP OF THE TIMELINE.
///
/// `optimistic_message` mints a descending negative seq, which sorts ahead of
/// every real message. Sorting it numerically into a prepended page put an in-flight send
/// at the top of months-old scrollback, and then `history_has_older` read
/// `-1 > 1` and hid "Load older" outright — the pending send locked the reader
/// out of her own history until it settled.
#[test]
fn a_pending_send_survives_a_history_page_without_poisoning_it() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.mutation_phase = MutationPhase::Idle;
    app.active_channel = "general".into();
    app.messages = vec![message(40, "the oldest loaded root", false)];
    app.has_older_history = true;
    app.message_editor = compose("still sending");

    let _ = app.__update(__DucktapeMessage::ChatComposerEvent(
        editor::composer_submit_event(),
    ));
    assert!(app.messages[1].pending, "the send is in flight at the tail");

    let _ = app.__update(__DucktapeMessage::LoadMoreHistory);
    assert!(
        app.history_loading,
        "an in-flight send must not block paging"
    );
    let _ = app.__update(__DucktapeMessage::HistoryLoaded(backend::HistoryPageData {
        channel_id: "general".into(),
        messages: vec![message(20, "older", false)],
    }));

    let ordering: Vec<i64> = app.messages.iter().map(|message| message.seq).collect();
    assert_eq!(
        ordering,
        vec![20, 40, -1],
        "the page prepends, the pending row stays at the tail"
    );
    assert!(
        app.has_older_history,
        "seq 20 is not the channel's first message, so `Load older` stays live"
    );
    assert_eq!(
        backend::oldest_message_seq(app.messages.clone()),
        20,
        "and the next page is asked for from the oldest COMMITTED row"
    );
}

/// A SEND CONTINUES THE READER'S OWN RUN.
///
/// The optimistic row used to be minted with a hand-written `"You"` while every
/// committed row of the reader's own renders `"you"`, so `mark_message_groups`
/// opened a run on it: a send that followed one of your own drew a full avatar +
/// header that vanished — shifting the row up by the header's height — the
/// moment the settle delta replaced it. The COMMITTED row below is the fence:
/// without it both rows are minted by the same call and carry the same label
/// whatever literal it uses.
#[test]
fn consecutive_sends_stay_in_one_author_run() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_channel = "general".into();
    app.messages = vec![backend::ChatMessage {
        author: "you".into(),
        ..message(40, "landed a minute ago", false)
    }];

    for body in ["first", "second"] {
        app.message_editor = compose(body);
        let _ = app.__update(__DucktapeMessage::ChatComposerEvent(
            editor::composer_submit_event(),
        ));
    }

    let authors: Vec<&str> = app
        .messages
        .iter()
        .map(|message| message.author.as_str())
        .collect();
    assert_eq!(
        authors,
        vec!["you", "you", "you"],
        "the mint renders the reader the way a committed row of hers does"
    );
    let headers: Vec<bool> = app
        .messages
        .iter()
        .map(|message| message.show_author)
        .collect();
    assert_eq!(
        headers,
        vec![true, false, false],
        "the committed row opens the run and both sends continue it — no header \
         to draw and then take away"
    );
}

/// THE RAIL IS NOT A PLAIN RUN, SO THE MINT MUST NOT RE-MARK IT.
///
/// A thread's vec is `[root] ++ replies` and the root renders as its own divided
/// block, so `load_thread_data` marks the REPLIES only. Re-marking the whole vec
/// when a reply is minted folds the first reply under a root that shares its
/// author and swallows that reply's header — which then comes back on the next
/// thread load: the same render jump, one pane over.
#[test]
fn a_minted_reply_keeps_the_first_reply_header() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_channel = "general".into();
    app.active_thread_seq = 7;
    app.thread_messages = vec![
        backend::ChatMessage {
            author: "you".into(),
            ..message(7, "the root", false)
        },
        backend::ChatMessage {
            author: "you".into(),
            ..message(8, "the first reply", false)
        },
    ];
    app.reply_editor = compose("and one more");

    let _ = app.__update(__DucktapeMessage::ReplyComposerEvent(
        editor::composer_submit_event(),
    ));

    let headers: Vec<bool> = app
        .thread_messages
        .iter()
        .map(|message| message.show_author)
        .collect();
    assert_eq!(
        headers,
        vec![true, true, true],
        "the root's header and the first reply's both stand"
    );
}

#[test]
fn committed_mutation_keeps_optimistic_state_until_refresh() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.connected_rpc = "http://node".into();
    app.active_channel = "general".into();
    app.message_editor = compose("committed once");

    let _ = app.__update(__DucktapeMessage::ChatComposerEvent(
        editor::composer_submit_event(),
    ));
    let operation_id = app.messages[0].id.clone();
    let _ = app.__update(__DucktapeMessage::MessageSendFailed(
        backend::OptimisticMutationError {
            message: "read failed after commit".into(),
            committed: true,
            operation_id,
            scope_id: "general".into(),
            body: "committed once".into(),
        },
    ));

    assert!(app.message_draft.is_empty());
    assert_eq!(app.messages.len(), 1);
    assert!(app.messages[0].pending);
    assert_eq!(app.mutation_phase, MutationPhase::Idle);

    app.message_editor = compose("still available");
    let _ = app.__update(__DucktapeMessage::ChatComposerEvent(
        editor::composer_submit_event(),
    ));
    assert_eq!(app.messages.len(), 2);
    assert_eq!(app.mutation_phase, MutationPhase::Idle);
}

#[test]
fn committed_message_change_cannot_be_submitted_twice() {
    let (mut app, _) = Ducktape::__boot();
    app.connected_rpc = "http://node".into();
    app.active_channel = "general".into();
    app.selected_message_seq = 7;
    app.selected_message_rev = 2;
    app.message_action = MessageAction::Editing;
    app.message_edit_draft = "committed edit".into();
    app.mutation_phase = MutationPhase::MessageEdit;

    let _ = app.__update(__DucktapeMessage::MutationFailed(backend::AppError {
        message: "read failed after commit".into(),
        committed: true,
    }));

    assert_eq!(app.selected_message_seq, 0);
    assert_eq!(app.selected_message_rev, 0);
    assert_eq!(app.message_action, MessageAction::Toolbar);
    assert!(app.message_edit_draft.is_empty());
    assert_eq!(app.mutation_phase, MutationPhase::Recovering);
}

/// AND "recovering" HAS A TERMINAL. It is the phase a write the node COMMITTED
/// but could not read back parks in — ordinary enough, a `/v1/query` can block
/// past the RPC timeout (#1018) — and the resync `mutation_failed` launches is
/// the recovery. Nothing released it: every other writer of "idle" sits behind
/// a `mutation_phase != MutationPhase.idle` guard it can no longer pass, so the sidebar went
/// dead (no room click, no DM, no search hit, no scrollback, no edit or delete)
/// under a titlebar stuck on "Syncing…", with Settings → Reconnect the only way
/// out and no reason for anyone to guess at it.
#[test]
fn a_committed_mutation_failure_unlocks_when_its_recovery_lands() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.loading = false;
    app.active_channel = "general".into();
    app.mutation_phase = MutationPhase::Channel;

    let _ = app.__update(__DucktapeMessage::MutationFailed(backend::AppError {
        message: "read failed after commit".into(),
        committed: true,
    }));
    assert_eq!(app.mutation_phase, MutationPhase::Recovering);

    // a resync belonging to an abandoned chain answers for nothing
    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
        app.hydration_generation - 1,
        "general",
        Vec::new(),
        "",
        Vec::new(),
    )));
    assert_eq!(
        app.mutation_phase,
        MutationPhase::Recovering,
        "a stale answer is not it"
    );

    let _ = app.__update(__DucktapeMessage::LiveResynced(live_refresh(
        app.hydration_generation,
        "general",
        Vec::new(),
        "",
        Vec::new(),
    )));
    assert_eq!(
        app.mutation_phase,
        MutationPhase::Idle,
        "the state the lock protected is known good now"
    );
    assert!(app.error.is_empty());
}

#[test]
fn optimistic_thread_replies_settle_independently_out_of_order() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_channel = "general".into();
    app.active_thread_seq = 1;
    app.reply_editor = compose("first");

    let _ = app.__update(__DucktapeMessage::ReplyComposerEvent(
        editor::composer_submit_event(),
    ));
    let first_id = app.thread_messages[0].id.clone();
    assert_eq!(app.mutation_phase, MutationPhase::Idle);
    assert!(reply_composer(&app).is_empty());
    assert!(app.thread_messages[0].pending);

    app.reply_editor = compose("second");
    let _ = app.__update(__DucktapeMessage::ReplyComposerEvent(
        editor::composer_submit_event(),
    ));
    let second_id = app.thread_messages[1].id.clone();
    assert_ne!(first_id, second_id);
    assert_eq!(app.thread_messages.len(), 2);
    assert!(app.thread_messages.iter().all(|message| message.pending));

    let mut second = message(3, "second", false);
    second.id = second_id.clone();
    second.thread_seq = 1;
    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: "chat".into(),
        status: "Live".into(),
        height: 3,
        chat: backend::ChatDelta {
            kind: "reply".into(),
            channel_id: "general".into(),
            seq: 3,
            root_seq: 1,
            message: second,
            ..backend::ChatDelta::default()
        },
        ..backend::LiveUpdate::default()
    }));
    assert_eq!(app.thread_messages.len(), 2);
    assert!(
        app.thread_messages
            .iter()
            .any(|message| message.id == first_id && message.pending)
    );
    assert!(
        app.thread_messages
            .iter()
            .any(|message| message.body == "second" && !message.pending)
    );

    let mut first = message(2, "first", false);
    first.id = first_id.clone();
    first.thread_seq = 1;
    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: "chat".into(),
        status: "Live".into(),
        height: 4,
        chat: backend::ChatDelta {
            kind: "reply".into(),
            channel_id: "general".into(),
            seq: 2,
            root_seq: 1,
            message: first,
            ..backend::ChatDelta::default()
        },
        ..backend::LiveUpdate::default()
    }));
    assert_eq!(app.thread_messages.len(), 2);
    assert!(app.thread_messages.iter().all(|message| !message.pending));
    assert_eq!(app.thread_messages[0].body, "first");
    assert_eq!(app.thread_messages[1].body, "second");
}

#[test]
fn failed_thread_reply_rolls_back_only_itself_and_preserves_the_newer_draft() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_channel = "general".into();
    app.active_thread_seq = 1;
    app.reply_editor = compose("first");

    let _ = app.__update(__DucktapeMessage::ReplyComposerEvent(
        editor::composer_submit_event(),
    ));
    let first_id = app.thread_messages[0].id.clone();
    app.reply_editor = compose("second");
    let _ = app.__update(__DucktapeMessage::ReplyComposerEvent(
        editor::composer_submit_event(),
    ));
    let second_id = app.thread_messages[1].id.clone();
    app.reply_editor = compose("newer draft");

    let _ = app.__update(__DucktapeMessage::ThreadReplySendFailed(
        backend::OptimisticMutationError {
            message: "rejected".into(),
            committed: false,
            operation_id: first_id,
            scope_id: "general".into(),
            body: "first".into(),
        },
    ));
    assert_eq!(reply_composer(&app), "newer draft");
    assert_eq!(app.failed_reply_draft, "first");
    assert_eq!(app.thread_messages.len(), 1);
    assert_eq!(app.thread_messages[0].id, second_id);
    assert!(app.thread_messages[0].pending);
    assert_eq!(app.mutation_phase, MutationPhase::Idle);
    assert!(!app.thread_loading);

    let _ = app.__update(__DucktapeMessage::RestoreFailedReply);
    assert_eq!(reply_composer(&app), "newer draft");
    assert_eq!(app.failed_reply_draft, "first");
    app.reply_editor = compose("");
    let _ = app.__update(__DucktapeMessage::RestoreFailedReply);
    assert_eq!(reply_composer(&app), "first");
    assert!(app.failed_reply_draft.is_empty());
}

#[test]
fn committed_thread_reply_refreshes_without_blocking_the_composer() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.connected_rpc = "http://node".into();
    app.active_channel = "general".into();
    app.active_thread_seq = 1;
    app.reply_editor = compose("committed");

    let _ = app.__update(__DucktapeMessage::ReplyComposerEvent(
        editor::composer_submit_event(),
    ));
    let operation_id = app.thread_messages[0].id.clone();
    let _ = app.__update(__DucktapeMessage::ThreadReplySendFailed(
        backend::OptimisticMutationError {
            message: "read failed after commit".into(),
            committed: true,
            operation_id,
            scope_id: "general".into(),
            body: "committed".into(),
        },
    ));
    assert_eq!(app.thread_messages.len(), 1);
    assert!(app.thread_messages[0].pending);
    assert!(reply_composer(&app).is_empty());
    assert!(app.failed_reply_draft.is_empty());
    assert_eq!(app.mutation_phase, MutationPhase::Idle);
    assert!(!app.thread_loading);

    app.reply_editor = compose("still available");
    let _ = app.__update(__DucktapeMessage::ReplyComposerEvent(
        editor::composer_submit_event(),
    ));
    assert_eq!(app.thread_messages.len(), 2);
    assert!(app.thread_messages.iter().all(|message| message.pending));
    assert!(reply_composer(&app).is_empty());
}

#[test]
fn a_channel_switch_freezes_the_unread_divider_while_a_same_channel_refresh_does_not() {
    let channel = |id: &str, head: i64| backend::ChatChannel {
        id: id.into(),
        name: id.into(),
        archived: false,
        members_only: false,
        huddle_count: 0,
        head_seq: head,
    };

    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.active_channel = "general".into();
    app.channels = vec![channel("general", 100), channel("random", 50)];
    // I last read #random at seq 30; it has since grown to head 50.
    app.channel_reads = vec![backend::ChannelRead {
        channel: "random".into(),
        seq: 30,
    }];

    // Switching INTO #random freezes the divider above the first unread
    // (>30) and marks #random read up to head so its sidebar badge clears.
    // The freeze must survive the REAL click path: `choose_channel` takes the
    // header and highlight optimistically, so by `chat_updated` current ==
    // next and the load-time freeze self-defers to the click-time one.
    let _ = app.__update(__DucktapeMessage::ChooseChannel("random".into()));
    assert_eq!(app.active_channel, "random");
    assert_eq!(app.unread_boundary, 30);
    assert!(app.messages.is_empty());
    app.loading = false;
    let mut switched = chat_data(
        "random",
        vec![
            message(31, "a", false),
            message(40, "b", false),
            message(50, "c", false),
        ],
    );
    switched.channels = vec![channel("general", 100), channel("random", 50)];
    switched.generation = app.chat_generation;
    let _ = app.__update(__DucktapeMessage::ChatUpdated(switched));
    assert_eq!(app.active_channel, "random");
    assert_eq!(app.unread_boundary, 30);
    assert_eq!(
        backend::first_unread_seq(app.messages.clone(), app.unread_boundary),
        31
    );
    assert!(
        !app.rooms
            .iter()
            .any(|row| row.channel.id == "random" && row.unread)
    );

    // A same-channel live delta that brings a NEW message must NOT move
    // the frozen boundary — the divider would jump as you read.
    let _ = app.__update(__DucktapeMessage::LiveUpdated(posted_delta(
        "random",
        message(60, "d", false),
    )));
    assert_eq!(app.active_channel, "random");
    assert_eq!(app.unread_boundary, 30);
    assert_eq!(app.messages.len(), 4);

    // Arriving at a caught-up channel shows no divider (boundary 0).
    app.channel_reads =
        backend::mark_channel_read(app.channel_reads.clone(), "general".into(), 100);
    let mut caught_up = chat_data("general", vec![message(100, "x", false)]);
    caught_up.channels = vec![channel("general", 100), channel("random", 60)];
    caught_up.generation = app.chat_generation;
    let _ = app.__update(__DucktapeMessage::ChatUpdated(caught_up));
    assert_eq!(app.active_channel, "general");
    assert_eq!(app.unread_boundary, 0);
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

/// THE LAST CLICK WINS. `choose_channel` used to open `return if loading`, and
/// `loading` covers the whole switch it starts — so the second and third clicks
/// of a fast A→B→C were discarded on the way out, with nothing on screen
/// admitting it. The clicks are taken now and the SUPERSEDED REPLY is dropped:
/// B answering after C must not drag the reader back into B.
#[test]
fn a_burst_of_channel_clicks_lands_on_the_last_one_and_drops_the_replies_it_passed() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.active_channel = "a".into();
    app.channels = vec![room("a", 10), room("b", 20), room("c", 30)];

    let _ = app.__update(__DucktapeMessage::ChooseChannel("b".into()));
    let for_b = app.chat_generation;
    // The click DURING the load is what used to vanish.
    assert!(app.loading);
    let _ = app.__update(__DucktapeMessage::ChooseChannel("c".into()));
    assert_eq!(app.active_channel, "c", "the second click moved the reader");
    assert_ne!(app.chat_generation, for_b);

    // One refreshed row is the whole channel list a window loader answers with.
    let mut late_b = chat_data("b", vec![message(20, "from b", false)]);
    late_b.channels = vec![room("b", 20)];
    late_b.generation = for_b;
    let _ = app.__update(__DucktapeMessage::ChatUpdated(late_b));
    assert_eq!(app.active_channel, "c", "b's reply must not take the pane");
    assert!(app.messages.is_empty());
    assert!(app.loading, "c is still in flight — the plate stays up");

    let mut for_c = chat_data("c", vec![message(30, "from c", false)]);
    for_c.channels = vec![room("c", 30)];
    for_c.generation = app.chat_generation;
    let _ = app.__update(__DucktapeMessage::ChatUpdated(for_c));
    assert_eq!(app.active_channel, "c");
    assert_eq!(app.messages.len(), 1);
    assert!(!app.loading);
}

/// A SWITCH REPLY FOLDS INTO THE SIDEBAR, IT DOES NOT REPLACE IT.
///
/// The window loader is handed the list the reader is already looking at and
/// answers with the one row it refreshed, so everything the live stream landed
/// DURING the round trip has to survive the reply: a peer's post in a THIRD
/// room and the unread badge it lit, and a channel someone created while she
/// waited. Nothing re-pages the list afterwards — `load_chat` is raised only
/// for `kind == "ready"`, i.e. a websocket reconnect — so a revert here is not
/// a frame of staleness, it is permanent.
#[test]
fn a_switch_reply_keeps_what_the_live_stream_folded_while_it_was_in_flight() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.active_channel = "general".into();
    app.channels = vec![room("general", 10), room("random", 20), room("eng", 40)];
    app.channel_reads = backend::initial_channel_reads(app.channels.clone(), Vec::new());

    let _ = app.__update(__DucktapeMessage::ChooseChannel("random".into()));
    let switch = app.chat_generation;

    // Mid-RTT: a peer posts into a third room, and another creates a channel.
    let _ = app.__update(__DucktapeMessage::LiveUpdated(posted_delta(
        "eng",
        message(41, "from a peer", false),
    )));
    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: "chat".into(),
        status: "Live".into(),
        height: 1,
        chat: backend::ChatDelta {
            kind: "channel-created".into(),
            channel: room("brand-new", 0),
            ..backend::ChatDelta::default()
        },
        ..backend::LiveUpdate::default()
    }));
    assert!(
        app.rooms
            .iter()
            .any(|row| row.channel.id == "eng" && row.unread)
    );

    let mut landed = chat_data("random", vec![message(20, "from random", false)]);
    landed.channels = vec![room("random", 20)];
    landed.generation = switch;
    let _ = app.__update(__DucktapeMessage::ChatUpdated(landed));

    assert_eq!(
        app.channels
            .iter()
            .map(|row| row.id.as_str())
            .collect::<Vec<_>>(),
        vec!["general", "random", "eng", "brand-new"],
        "the room created mid-switch is still in the sidebar"
    );
    assert_eq!(
        backend::channel_head_seq(app.channels.clone(), "eng".into()),
        41,
        "and the third room's head did not walk back to the pre-click snapshot"
    );
    assert!(
        app.rooms
            .iter()
            .any(|row| row.channel.id == "eng" && row.unread),
        "so its badge survives the switch it had nothing to do with"
    );
}

/// AND NEITHER DOES A RESYNC'S REPLY — same rule, same seam, wider blast.
///
/// `live_resync_load` is a checkpoint-gated multi-query read whose latency the
/// repo measures in seconds, so every delta the live stream folds inside its
/// round trip is the NEWER fact. A flat assignment walked a third room's
/// `head_seq` back to the snapshot — while `channel_reads` was NOT reverted with
/// it — so `head_seq > last_read` went false and the badge the reader never saw
/// blinked out, dark until that room got another message.
#[test]
fn a_resync_keeps_the_badge_the_live_stream_lit_while_it_was_in_flight() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.loading = false;
    app.active_channel = "general".into();
    app.channels = vec![room("general", 10), room("eng", 40)];
    app.channel_reads = backend::initial_channel_reads(app.channels.clone(), Vec::new());

    // mid-RTT: a peer posts into a third room, and another creates a channel
    let _ = app.__update(__DucktapeMessage::LiveUpdated(posted_delta(
        "eng",
        message(41, "from a peer", false),
    )));
    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: "chat".into(),
        status: "Live".into(),
        height: 1,
        chat: backend::ChatDelta {
            kind: "channel-created".into(),
            channel: room("brand-new", 0),
            ..backend::ChatDelta::default()
        },
        ..backend::LiveUpdate::default()
    }));
    assert!(
        app.rooms
            .iter()
            .any(|row| row.channel.id == "eng" && row.unread)
    );

    // the resync answers off a snapshot taken before either of them
    let mut landed = live_refresh(
        app.hydration_generation,
        "general",
        Vec::new(),
        "",
        Vec::new(),
    );
    landed.channels = vec![room("general", 10), room("eng", 40)];
    let _ = app.__update(__DucktapeMessage::LiveResynced(landed));

    assert_eq!(
        backend::channel_head_seq(app.channels.clone(), "eng".into()),
        41,
        "the third room's head does not walk back to the snapshot"
    );
    assert!(
        app.rooms
            .iter()
            .any(|row| row.channel.id == "eng" && row.unread),
        "so the badge it lit survives a resync it had nothing to do with"
    );
    assert!(
        app.channels.iter().any(|row| row.id == "brand-new"),
        "and the room created mid-resync is still in the sidebar"
    );
}

/// NOBODY READS A PANE THAT IS NOT MOUNTED.
///
/// The live feed is subscribed on `connected`, not on the tab, so an arrival in
/// the open room while the reader was in Settings or Files marked it read on the
/// spot: she came back to no divider and no way to tell the new rows from the
/// ones she had already read, and every OTHER room badged normally while that
/// one stayed dark. The rows still fold in — only the cursor waits for her.
#[test]
fn messages_that_arrive_off_tab_wait_for_the_reader_to_come_back() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;
    app.active_channel = "general".into();
    app.channels = vec![room("general", 10), room("eng", 40)];
    app.channel_reads = backend::initial_channel_reads(app.channels.clone(), Vec::new());
    app.messages = vec![message(10, "the last thing she read", false)];

    let _ = app.__update(__DucktapeMessage::SelectShellTab(ShellTab::Settings));
    let _ = app.__update(__DucktapeMessage::LiveUpdated(posted_delta(
        "general",
        message(11, "while she was away", false),
    )));

    assert_eq!(
        app.messages.len(),
        2,
        "the row folds in either way — it is on screen when she returns"
    );
    assert!(
        app.rooms
            .iter()
            .any(|row| row.channel.id == "general" && row.unread),
        "but the room she left open is unread like any other room"
    );

    // AND THEN SHE SAVES A FILE. A plane op resyncs the client — files, valset,
    // identity, agent and governance all land in `live_resynced`, carrying no
    // chat at all — and the read cursor used to move to the head on the way
    // past, retiring the badge and the divider for a room she has not looked at
    // since. It is traffic she generates herself, so the off-tab gate above
    // survived roughly one keystroke without this.
    let plane_only = backend::LiveRefresh {
        chat_loaded: false,
        pages_loaded: false,
        ..live_refresh(
            app.hydration_generation,
            "general",
            Vec::new(),
            "",
            Vec::new(),
        )
    };
    let _ = app.__update(__DucktapeMessage::LiveResynced(plane_only));
    assert_eq!(
        app.messages.len(),
        2,
        "a resync that carried no chat leaves the window alone"
    );
    assert!(
        app.rooms
            .iter()
            .any(|row| row.channel.id == "general" && row.unread),
        "and it does not catch her up on a room she is not on the tab for"
    );

    let _ = app.__update(__DucktapeMessage::SelectShellTab(ShellTab::Chat));
    assert_eq!(
        app.unread_marker_seq, 11,
        "coming back freezes the divider on what arrived while she was gone"
    );
    assert!(
        !app.rooms
            .iter()
            .any(|row| row.channel.id == "general" && row.unread),
        "and only then is she caught up"
    );

    // a tab round trip with nothing new must not throw the divider away
    let _ = app.__update(__DucktapeMessage::SelectShellTab(ShellTab::Files));
    let _ = app.__update(__DucktapeMessage::SelectShellTab(ShellTab::Chat));
    assert_eq!(app.unread_marker_seq, 11);
}

/// A SUPERSEDED SWITCH'S FAILURE STAYS WITH IT. Nothing serializes the room
/// pickers any more, so B's error can arrive after the reader has clicked on to
/// C — and ungated it would clear `loading` under C (swapping C's plate for "No
/// messages yet") and put B's message in the banner until C lands.
#[test]
fn a_failed_switch_the_reader_clicked_past_does_not_land_on_the_room_she_is_in() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.active_channel = "a".into();
    app.channels = vec![room("a", 10), room("b", 20), room("c", 30)];

    let _ = app.__update(__DucktapeMessage::ChooseChannel("b".into()));
    let for_b = app.chat_generation;
    let _ = app.__update(__DucktapeMessage::ChooseChannel("c".into()));

    let _ = app.__update(__DucktapeMessage::ChatLoadFailed(backend::HydrationError {
        generation: for_b,
        message: "b is unreachable".into(),
    }));
    assert!(app.loading, "c is still in flight — the plate stays up");
    assert!(app.error.is_empty(), "and b's failure is not c's");

    let for_c = app.chat_generation;
    let _ = app.__update(__DucktapeMessage::ChatLoadFailed(backend::HydrationError {
        generation: for_c,
        message: "c is unreachable too".into(),
    }));
    assert!(!app.loading);
    assert_eq!(app.error, "c is unreachable too");
}

/// A→B→A PAINTS IN ONE FRAME. The room being left is parked with its member
/// roll; the room being entered is restored from that park with `loading`
/// false, so the switch back costs no round trip to become readable — and the
/// composer's gate comes back with it rather than reading the room she left.
#[test]
fn switching_back_to_a_parked_room_paints_its_rows_without_waiting_on_the_node() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.settings_user_key = "me".into();
    app.active_channel = "a".into();
    app.channels = vec![room("a", 10), room("b", 20)];
    app.messages = vec![message(9, "older", false), message(10, "newest", false)];
    app.channel_members = vec![backend::ChatMember {
        key: "me".into(),
        label: "me".into(),
    }];

    let _ = app.__update(__DucktapeMessage::ChooseChannel("b".into()));
    assert!(app.messages.is_empty(), "#b has never been read");
    assert!(app.loading);

    let _ = app.__update(__DucktapeMessage::ChooseChannel("a".into()));
    assert_eq!(app.messages.len(), 2, "#a came back from the park");
    assert!(!app.loading, "a parked room is not a loading one");
    assert_eq!(app.channel_members.len(), 1, "its member roll came with it");
    assert!(app.has_older_history, "derived from the restored rows");
    assert!(app.post_refusal.is_empty());
}

/// AND THE COMPOSER IS PER-ROOM TOO — the one piece of per-room state no
/// switch handler touched.
///
/// `choose_channel` resets a dozen fields and the rail's editor, and left
/// `message_editor` exactly as it found it: half a sentence typed in
/// #private-ops followed the reader into whatever room she clicked next, sat
/// there above a live Send, and was prepended to the next thing she typed and
/// posted THERE. A chain post is permanent in history even after a tombstone
/// delete, and the leaked text is by construction from the room she just left.
///
/// The rule is the one `chat.ice` already states for a failed send — "the
/// composer belongs to the room she is in now" — finally applied to the live
/// buffer, and drafts survive the switch instead of being thrown away for it.
#[test]
fn the_composer_belongs_to_the_room_she_is_in_and_waits_in_the_one_she_left() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.mutation_phase = MutationPhase::Idle;
    app.active_channel = "private-ops".into();
    app.channels = vec![room("private-ops", 10), room("general", 20)];
    app.message_editor = compose("the incident started at");

    let _ = app.__update(__DucktapeMessage::ChooseChannel("general".into()));
    assert!(
        composer(&app).is_empty(),
        "#general's composer is #general's — nothing from next door is armed to \
         send here"
    );

    app.message_editor = compose("ok");
    let _ = app.__update(__DucktapeMessage::ChooseChannel("private-ops".into()));
    assert_eq!(
        composer(&app),
        "the incident started at",
        "and the sentence she was writing is waiting where she left it"
    );

    let _ = app.__update(__DucktapeMessage::ChooseChannel("general".into()));
    assert_eq!(composer(&app), "ok", "both rooms keep their own");

    // A SENT DRAFT DOES NOT COME BACK. The composer empties on submit, and the
    // park that runs on the way out drops the entry rather than storing "".
    // (#general has never been read here, so the switch left `loading` up.)
    app.loading = false;
    let _ = app.__update(__DucktapeMessage::ChatComposerEvent(
        editor::composer_submit_event(),
    ));
    assert!(composer(&app).is_empty(), "the send emptied the box");
    let _ = app.__update(__DucktapeMessage::ChooseChannel("private-ops".into()));
    let _ = app.__update(__DucktapeMessage::ChooseChannel("general".into()));
    assert!(
        composer(&app).is_empty(),
        "a message she already sent must not be handed back as a draft"
    );
}

/// AND CREATING A CHANNEL IS A ROOM SWITCH, so the composer parks there too.
///
/// `channel_created` writes `active_channel = next.active_channel` — the reader
/// lands IN the room she just made, which is why `create_channel_submit`
/// abandons the old room's window load. With no park the sentence she was
/// half-way through in #private-ops arrived in #new-channel above a live Send,
/// and the NEXT switch parked it under #new-channel's id: silently
/// reattributed, and gone when she went back to #private-ops for it.
#[test]
fn creating_a_channel_leaves_the_old_rooms_draft_in_the_old_room() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.mutation_phase = MutationPhase::Idle;
    app.active_channel = "private-ops".into();
    app.channels = vec![room("private-ops", 10)];
    app.message_editor = compose("the incident started at");

    let mut created = chat_data("new-channel", Vec::new());
    created.generation = app.chat_generation;
    created.channels = vec![room("private-ops", 10), room("new-channel", 0)];
    let _ = app.__update(__DucktapeMessage::ChannelCreated(created));

    assert_eq!(
        app.active_channel, "new-channel",
        "the create lands her in it"
    );
    assert!(
        composer(&app).is_empty(),
        "and the new channel's composer is the new channel's — nothing from the \
         room she left is armed to send here"
    );

    let _ = app.__update(__DucktapeMessage::ChooseChannel("private-ops".into()));
    assert_eq!(
        composer(&app),
        "the incident started at",
        "the sentence is waiting in the room she was writing it in"
    );
}

/// A DM CLICK LANDS THE WHOLE ROOM, NOT JUST THE FACE.
///
/// `choose_dm` used to move `active_dm_peer` and nothing else about the room,
/// so for the several blocks `open_dm` takes — a channel create plus two
/// membership seats on a first open — the peer's name sat beside the ARCHIVED
/// badge, the "· N added" count and the composer refusal of the room she left.
/// The id is derivable here (`dm_channel_id` is the same deterministic hash
/// `open_dm` resolves), which also makes a re-opened DM eligible for the park.
#[test]
fn a_dm_click_takes_the_room_with_it_instead_of_wearing_the_last_ones_badges() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.settings_user_key = "me".into();
    app.active_channel = "locked".into();
    app.active_channel_name = "locked".into();
    app.active_channel_archived = true;
    app.active_channel_members_only = true;
    app.channel_members = vec![backend::ChatMember {
        key: "someone-else".into(),
        label: "Someone else".into(),
    }];
    app.post_refusal = "channel_archived".into();
    app.messages = vec![message(10, "in the room she leaves", false)];
    app.dm_peers = vec![backend::DmPeer {
        key: "peer".into(),
        name: "Peer".into(),
        initials: "P".into(),
        is_agent: false,
        channel_id: backend::dm_channel_id("me".into(), "peer".into()),
    }];

    let _ = app.__update(__DucktapeMessage::ChooseDm("peer".into()));
    let dm = backend::dm_channel_id("me".into(), "peer".into());
    assert_eq!(app.active_channel, dm, "the DM's own room, on the click");
    assert_eq!(app.active_dm_peer, "peer");
    assert!(!app.active_channel_archived, "not the left room's badge");
    assert!(!app.active_channel_members_only);
    assert!(app.channel_members.is_empty(), "nor its member count");
    assert!(app.post_refusal.is_empty(), "nor its composer refusal");
    assert!(!app.history_view, "a DM open is a live tail");
    assert!(app.loading, "this peer has never been read");

    // And back out and in again: the DM is an ordinary room in the park now.
    let mut landed = chat_data(&dm, vec![message(30, "from the peer", false)]);
    landed.generation = app.chat_generation;
    let _ = app.__update(__DucktapeMessage::ChatUpdated(landed));
    let _ = app.__update(__DucktapeMessage::ChooseChannel("locked".into()));
    let _ = app.__update(__DucktapeMessage::ChooseDm("peer".into()));
    assert_eq!(app.messages.len(), 1, "the DM came back from the park");
    assert!(!app.loading, "a parked DM is not a loading one");
}

/// A SEARCH HIT PAINTS THE ROOM IT IS JUMPING TO, NOT THE ROOM IT LEFT.
///
/// Every landing field used to move only in `chat_hit_loaded`, so a hit that
/// lives in another room kept that room's header, rows and sidebar highlight
/// for the whole walk — the one navigation whose entire purpose is to jump
/// somewhere else, and the only one still showing the "did my click land?" void
/// #1059 removed from the pickers. The park is deliberately NOT restored: a hit
/// is a history window, and an empty timeline under the skeleton is honest.
#[test]
fn opening_a_search_hit_moves_the_room_on_the_click() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.settings_user_key = "me".into();
    app.active_channel = "general".into();
    app.active_channel_name = "general".into();
    app.active_channel_archived = true;
    app.channels = vec![room("general", 10), room("design", 40)];
    app.messages = vec![message(10, "in general", false)];
    app.channel_members = vec![backend::ChatMember {
        key: "me".into(),
        label: "me".into(),
    }];

    let _ = app.__update(__DucktapeMessage::OpenChatSearchHit("design".into(), 7, 7));
    assert_eq!(
        app.active_channel, "design",
        "the sidebar moves on the click"
    );
    assert_eq!(app.active_channel_name, "design", "and so does the header");
    assert!(!app.active_channel_archived, "not general's badge");
    assert!(app.channel_members.is_empty(), "nor general's roll");
    assert!(app.post_refusal.is_empty());
    assert!(app.messages.is_empty(), "general's rows leave with general");
    assert!(
        app.loading,
        "so the skeleton draws for the room being entered"
    );
    assert!(app.history_view, "a hit is a window around one old message");

    // Leaving parks general, so the way back out of the hit is one frame.
    let parked = backend::cached_window(app.message_cache.clone(), "general".into());
    assert_eq!(parked.messages.len(), 1);
}

/// THE THREAD RAIL OPENS ON THE MESSAGE IT IS ABOUT.
///
/// `open_thread_for` emptied `thread_messages` and the rail's only body is a
/// loop over it, with both loading arms gated on `thread_has_more` — which the
/// same handler clears. So the click produced a 330px pane of bare background
/// with no root row, no skeleton and a disabled composer for the whole round
/// trip, and a load that FAILED left it that way until Close. The clicked
/// message is already in hand; seeding it costs one filter.
#[test]
fn opening_a_thread_seeds_its_root_row_instead_of_a_blank_rail() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.active_channel = "a".into();
    app.messages = vec![message(7, "the message it is about", false)];

    let _ = app.__update(__DucktapeMessage::OpenThreadFor(7));
    assert_eq!(app.active_thread_seq, 7);
    assert!(app.thread_loading, "the replies are still out");
    assert_eq!(
        app.thread_messages
            .iter()
            .map(|row| row.seq)
            .collect::<Vec<_>>(),
        vec![7],
        "the root draws on the click, before the node answers"
    );

    // A FAILURE LEAVES THE ROOT STANDING. `thread_failed` clears the busy term
    // and routes the text to the app banner; it never touches the rail, so an
    // unseeded rail stayed blank for as long as it was open.
    let _ = app.__update(__DucktapeMessage::ThreadFailed(backend::HydrationError {
        generation: app.thread_generation,
        message: "the node did not answer".into(),
    }));
    assert!(!app.thread_loading);
    assert_eq!(
        app.thread_messages.len(),
        1,
        "the rail still says which thread"
    );

    // AND A RE-ROOT ONTO A REPLY WORKS TOO: that seq lives in the rail's own
    // vec, never in the timeline, because the button is on a reply card.
    let mut loaded = backend::ThreadLoadData {
        generation: 0,
        root_seq: 7,
        target_seq: 0,
        messages: vec![message(7, "root", false), message(9, "a reply", false)],
        next_reply_offset: 0,
        has_more: false,
    };
    let _ = app.__update(__DucktapeMessage::OpenThreadFor(7));
    loaded.generation = app.thread_generation;
    let _ = app.__update(__DucktapeMessage::ThreadLoaded(loaded));
    let _ = app.__update(__DucktapeMessage::OpenThreadFor(9));
    assert_eq!(
        app.thread_messages
            .iter()
            .map(|row| row.seq)
            .collect::<Vec<_>>(),
        vec![9],
        "the reply the reader re-rooted on is the new rail's root"
    );
}

/// A PARKED WINDOW IS NOT A SETTLED ONE, AND HISTORY MAY NOT PAGE FROM IT. The
/// cache hit paints the rows she left behind with `loading` false, so `loading`
/// — the only in-flight term the two history routes had — stopped covering the
/// round trip the switch is still inside. #a grew while she was away, so the
/// walk answers with a LATER window: a page requested from the PARKED window's
/// oldest seq lands after that replacement and prepends under rows it does not
/// touch, deleting the seqs between them from the MIDDLE of the timeline with no
/// gap marker, and `has_older_history` then walks backwards past the hole
/// forever. `chat_window_loading` is the term that covers it.
#[test]
fn a_switch_still_in_flight_refuses_the_history_page_its_parked_rows_would_ask_for() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.mutation_phase = MutationPhase::Idle;
    app.active_channel = "a".into();
    app.channels = vec![room("a", 100), room("b", 20)];
    app.messages = vec![
        message(50, "a-fifty", false),
        message(100, "a-hundred", false),
    ];

    // Away and back: #a comes off the park in one frame, and its refetch is out.
    let _ = app.__update(__DucktapeMessage::ChooseChannel("b".into()));
    let _ = app.__update(__DucktapeMessage::ChooseChannel("a".into()));
    assert_eq!(app.messages.len(), 2, "the parked window is on screen");
    assert!(!app.loading, "a parked room is not a loading one");
    assert!(app.has_older_history, "so the paging routes are live");

    // Neither route may spend a request on seqs the walk is about to replace —
    // the scroll prefetch and the button both.
    let _ = app.__update(__DucktapeMessage::ChatScrolled(0.0, 900.0, 0.0, 0.95));
    let _ = app.__update(__DucktapeMessage::LoadMoreHistory);
    assert!(
        !app.history_loading,
        "no page may be requested from a window a switch is still replacing"
    );

    // The walk answers, and it answers with a window that starts LATER than the
    // parked one — the whole reason its seqs could not be paged from.
    let mut fresh = chat_data(
        "a",
        vec![
            message(90, "a-ninety", false),
            message(140, "a-head", false),
        ],
    );
    fresh.channels = vec![room("a", 140)];
    fresh.generation = app.chat_generation;
    let _ = app.__update(__DucktapeMessage::ChatUpdated(fresh));
    assert_eq!(app.messages.first().map(|row| row.seq), Some(90));

    // Now the button is honest work again, and the page joins the window it was
    // actually requested from.
    let _ = app.__update(__DucktapeMessage::LoadMoreHistory);
    assert!(app.history_loading, "the settled window pages normally");
    let _ = app.__update(__DucktapeMessage::HistoryLoaded(backend::HistoryPageData {
        channel_id: "a".into(),
        messages: vec![
            message(88, "a-eightyeight", false),
            message(89, "a-eightynine", false),
        ],
    }));
    let seqs: Vec<i64> = app.messages.iter().map(|row| row.seq).collect();
    assert_eq!(
        seqs,
        vec![88, 89, 90, 140],
        "the timeline is continuous — no seq vanished from its middle"
    );
}

/// A PENDING SEND IS NOT PARKED. Its settle handlers only touch the timeline of
/// the room the reader is IN, so a parked pending row would have no writer left
/// to retire it and would come back as a permanent "Sending…".
#[test]
fn the_parked_window_keeps_only_committed_rows() {
    let parked = backend::cache_channel_window(
        Vec::new(),
        "a".into(),
        vec![message(10, "committed", false), {
            let mut row = message(-1, "in flight", false);
            row.pending = true;
            row
        }],
        Vec::new(),
        false,
    );
    let rows = backend::cached_window(parked.clone(), "a".into()).messages;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].seq, 10);

    // A history window is a page around one old message, not the tail: parking
    // it would repaint months-old scrollback as the live conversation.
    let refused = backend::cache_channel_window(
        Vec::new(),
        "a".into(),
        vec![message(10, "committed", false)],
        Vec::new(),
        true,
    );
    assert!(
        backend::cached_window(refused, "a".into())
            .messages
            .is_empty()
    );
}

/// SCROLLING NEAR THE TOP STARTS THE PAGE. The offset arrives relative to the
/// scrollable's anchor, and the stream is bottom-anchored, so 1.0 is the top.
#[test]
fn approaching_the_top_of_the_scrollback_prefetches_the_older_page() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.active_channel = "a".into();
    app.messages = vec![message(40, "oldest loaded", false)];
    app.has_older_history = true;

    // Mid-scrollback: nothing happens, and no message means no view pass spent.
    let _ = app.__update(__DucktapeMessage::ChatScrolled(0.0, 120.0, 0.0, 0.4));
    assert!(!app.history_loading);

    // Content that FITS reports 0/0. Nothing scrolls, so nothing is approached
    // — and the explicit button is already on screen.
    let _ = app.__update(__DucktapeMessage::ChatScrolled(0.0, 0.0, 0.0, f64::NAN));
    assert!(!app.history_loading);

    let _ = app.__update(__DucktapeMessage::ChatScrolled(0.0, 900.0, 0.0, 0.95));
    assert!(app.history_loading, "the older page is already on its way");
    // And it does not fan out: the in-flight page holds the next steps off.
    let _ = app.__update(__DucktapeMessage::ChatScrolled(0.0, 950.0, 0.0, 0.98));
    assert!(app.history_loading);
}

#[test]
fn unread_indicators_are_wired_client_local_only() {
    // Sidebar badge: ChannelButton takes an `unread` flag and paints the
    // brand treatment + dot when set.
    let components = inlined(include_str!("ui/components/chat.ice"));
    assert!(components.contains(
        "component ChannelButton(channel:ChatChannel, selected:bool, unread:bool, disabled:bool)"
    ));
    assert!(components.contains("if unread\n                box w=7.0 h=7.0 bg=brand r=3.5"));
    // The name rides a `box w=fill clip=true`: `wrap=none` text lays out at its
    // INTRINSIC width whatever box it is given, so an unclipped long channel
    // name inflated the whole row past the 236px pane and the pane's own clip
    // sliced the row plate square through its rounded corner.
    // Unread is WEIGHT, not just ink — the same signal `ChannelButton` gives
    // an unread row over a read one (`font=medium` there, `font=display`
    // here), the conventional stronger signal.
    assert!(components.contains(
        "if unread\n                box w=fill clip=true\n                  text channel.name size=13.0 wrap=none font=display @text-fg"
    ));

    let screen = inlined(include_str!("ui/screens/chat.ice"));
    // The prepared row owns the scalar. No list-taking extern runs in either
    // sidebar loop.
    assert!(screen.contains(
        "ChannelButton channel=room.channel selected=(room.channel.id == active_channel) unread=room.unread"
    ));
    // In-channel divider anchored on the first message past the frozen
    // boundary. The seq is a STATE FIELD recomputed where messages or the
    // boundary change — `first_unread_seq(messages, …)` in the view sat
    // inside `for message in messages`, and the extern's by-value ABI deep-
    // cloned the whole timeline once per row per frame.
    assert!(screen.contains("if unread_boundary > 0 && message.seq == unread_marker_seq"));
    assert!(!screen.contains("first_unread_seq("));
    // The eyebrow spelling: FIELD_LABEL scale (10.0, mono semibold caps) —
    // every other structural label in the console reads this way, and a
    // 12.5px sentence-case run inside the message column read as a MESSAGE
    // at first glance.
    assert!(screen.contains("text \"NEW\" size=10.0 wrap=none font=code_semibold @text-brand"));

    // Freeze happens on a real channel change; connect seeds caught-up.
    let lifecycle = inlined(include_str!("ui/handlers/lifecycle.ice"));
    assert!(
        lifecycle.contains("channel_reads = initial_channel_reads(next.channels, channel_reads)")
    );
    // navigation loads freeze on the real channel change (chat.ice);
    // the resync path freezes against the possibly-unchanged channel.
    let chat = inlined(include_str!("ui/handlers/chat.ice"));
    assert!(chat.contains(
        "unread_boundary = frozen_unread_boundary(channel_reads, next.channels, active_channel, next.active_channel, unread_boundary)"
    ));
    assert!(chat.contains(
        "channel_reads = mark_channel_read(channel_reads, next.active_channel, channel_head_seq(next.channels, next.active_channel))"
    ));
    assert!(lifecycle.contains(
        "unread_boundary = frozen_unread_boundary(channel_reads, channels, active_channel, active_channel, unread_boundary)"
    ));
    // AND NEITHER LANDING MARKS A ROOM READ UNNAMED. Every read-cursor write
    // outside a deliberate channel entry goes through a gated channel name, so
    // that the gate cannot be dropped without this failing: a plane-only resync
    // (every files/agent/identity op) and an off-tab arrival both reach these
    // lines, and `mark_channel_read` refuses an empty channel.
    for gated in [
        "channel_reads = mark_channel_read(channel_reads, live_tail_channel, channel_head_seq(channels, live_tail_channel))",
        "channel_reads = mark_channel_read(channel_reads, resync_tail_channel, channel_head_seq(channels, resync_tail_channel))",
        "channel_reads = mark_channel_read(channel_reads, chat_tab_channel, channel_head_seq(channels, chat_tab_channel))",
    ] {
        assert!(lifecycle.contains(gated), "{gated}");
    }
    for gate in [
        "let live_tail_channel = keep_str(!history_view && shell_tab == ShellTab.chat, active_channel, \"\")",
        "let resync_tail_channel = keep_str(!history_view && shell_tab == ShellTab.chat, active_channel, \"\")",
        "let chat_tab_channel = keep_str(shell_tab == ShellTab.chat && !history_view, active_channel, \"\")",
    ] {
        assert!(lifecycle.contains(gate), "{gate}");
    }

    // Client-local only: no wire read-cursor leaked into the module surface.
    let backend_ice = inlined(include_str!("ui/extern/backend.ice"));
    assert!(!backend_ice.contains("read_cursor"));
    assert!(!backend_ice.contains("mark_read(rpc"));
}

/// THE "Not connected" WORDING, ONCE. Every data screen swaps its empty-state
/// claim for this exact plate, so the console reads as one app rather than eight
/// dialects of "I don't know".
const NOT_CONNECTED_PLATE: &str = concat!(
    "EmptyState title=\"Not connected\" ",
    "description=\"Click the network name in the titlebar to pick or reconnect a network.\""
);

/// A SCREEN THAT CANNOT REACH THE NODE MUST NOT REPORT ON ITS CONTENTS. With the
/// node down the status bar already says "Connection degraded · Offline", and the
/// bodies used to answer underneath it with `0 files · 0 dirs` and "Empty
/// directory — nothing is committed under this path." — a claim about CONTENT
/// made from a request that never went out. Chat and Pages already got this
/// right; the other six asserted an emptiness nobody measured.
///
/// THE SCREEN LIST IS THE INVARIANT, SO THE SCREEN LIST IS PINNED. A ninth data
/// screen has to decide what it says with the node down, and nothing about
/// writing one would prompt that thought — which is exactly how six of them got
/// written. This fails the build on a new screen so the decision is forced.
/// Exemptions are named with their reason, never left implicit.
#[test]
fn every_data_screen_answers_a_dead_node_with_not_connected() {
    /// Settings owns connection repair and Node owns the daemon diagnostics;
    /// both remain useful while the node is down instead of claiming the
    /// network's module contents are empty.
    const EXEMPT: [&str; 2] = ["NodeScreen", "SettingsScreen"];

    let mut screens: Vec<&str> = SCREENS
        .lines()
        .filter_map(|line| line.strip_prefix("component "))
        .map(|rest| rest.split('(').next().unwrap_or(rest).trim())
        .filter(|name| name.ends_with("Screen"))
        .collect();
    screens.sort_unstable();

    assert_eq!(
        screens,
        [
            "AgentsScreen",
            "ChatScreen",
            "ExplorerScreen",
            "FilesScreen",
            "ForgeScreen",
            "GovernanceScreen",
            "MembersScreen",
            "NodeScreen",
            "PagesScreen",
            "SettingsScreen",
            "ShellScreen",
        ],
        "a screen appeared or vanished: decide what it says with the node down, \
         then add it here or to EXEMPT with a reason"
    );

    // Scoped to each component's OWN body — a sweep over the whole file would
    // pass on six screens off Chat's single arm.
    for screen in screens.iter().filter(|name| !EXEMPT.contains(name)) {
        let body = SCREENS
            .split(&format!("\ncomponent {screen}("))
            .nth(1)
            .unwrap_or_else(|| panic!("{screen} is a component"))
            .split("\ncomponent ")
            .next()
            .expect("component body");
        assert!(
            body.contains("if !connected\n"),
            "{screen} draws readings of a network it may not be able to reach, \
             so it needs an `if !connected` arm in place of its empty-state claim"
        );
        assert!(
            body.contains(NOT_CONNECTED_PLATE),
            "{screen} must use the shared \"Not connected\" wording verbatim"
        );
    }
}

/// AND THE PLATE IS ONLY HALF OF IT — the arms BESIDE it must stand down too.
/// The first cut gated each screen's empty-state claim and left its POPULATED
/// arms open, so a screen rendered "Not connected" and the stale register
/// underneath it at the same time: Approvals showed the plate above three vote
/// cards with live Approve buttons, Members showed it beside a detail panel
/// carrying Promote and Remove, and Explorer showed it under a strip of search
/// hits nobody had run. No register is ever cleared on disconnect — they are
/// only overwritten by a successful load — so `connected == false` is routinely
/// reached with a full set of stale rows in hand.
///
/// The invariant, stated mechanically: a screen may touch a LIST-TYPED register
/// prop only under a `connected` gate, or on a line that carries `connected`
/// itself (a header subtitle folding rows through `members_summary(connected,
/// rows)` is already honest).
#[test]
fn a_disconnected_screen_stands_its_registers_down_too() {
    const EXEMPT: [&str; 2] = ["NodeScreen", "SettingsScreen"];

    for source in [
        include_str!("ui/screens/governance.ice"),
        include_str!("ui/screens/roster.ice"),
        include_str!("ui/screens/storage.ice"),
        include_str!("ui/screens/forge.ice"),
    ] {
        for chunk in source.split("\ncomponent ").skip(1) {
            let name = chunk.split('(').next().unwrap_or("").trim();
            if !name.ends_with("Screen") || EXEMPT.contains(&name) {
                continue;
            }
            // The registers are the list-typed params of the screen's own
            // signature — the readings only a live node can deliver.
            // AFTER the opening paren: splitting the whole head on `,` leaves
            // the first param as `GovernanceScreen(rows`, whose name never
            // matches a word in the body — which silently disarmed the check
            // for every screen whose only register is its first param.
            let signature = chunk
                .split(')')
                .next()
                .unwrap_or("")
                .split_once('(')
                .map(|(_, params)| params)
                .unwrap_or("");
            let registers: Vec<&str> = signature
                .split(',')
                .filter(|param| param.contains(":["))
                .filter_map(|param| param.split(':').next())
                .map(|name| name.trim().trim_start_matches("bind "))
                .filter(|name| !name.is_empty())
                .collect();
            assert!(!registers.is_empty(), "{name} has no register to guard");

            // Walk the body tracking which `if` arms are open above each line;
            // a line is covered when it, or any arm enclosing it, says
            // `connected`.
            let mut open: Vec<(usize, bool)> = Vec::new();
            for line in chunk.lines() {
                let trimmed = line.trim_start();
                if trimmed.is_empty() || trimmed.starts_with("//") {
                    continue;
                }
                let indent = line.len() - trimmed.len();
                open.retain(|(at, _)| *at < indent);
                let says_connected = line.contains("connected");
                if trimmed.starts_with("if ") {
                    open.push((indent, says_connected));
                }
                let covered = says_connected || open.iter().any(|(_, gated)| *gated);
                if covered {
                    continue;
                }
                // Prose is not a register read: the Explorer's own subtitle
                // says "read the blocks this node verified", and a screen may
                // describe what it shows while showing nothing.
                let code: String = trimmed.split('"').step_by(2).collect::<Vec<_>>().join(" ");
                if let Some(register) = registers.iter().find(|reg| {
                    code.split(|c: char| !c.is_alphanumeric() && c != '_')
                        .any(|word| word == **reg)
                }) {
                    panic!(
                        "{name} touches `{register}` with no `connected` gate above it:\n  {trimmed}\n\
                         a register nobody could read must not be drawn beside the \"Not connected\" plate"
                    );
                }
            }
        }
    }
}

/// A DIRECTORY YOU HAVE NOT LISTED HAS NO CONTENTS TO REPORT. Measured live:
/// clicking `reports` inside `/shared` moved the crumb to
/// `duckfs /shared/reports` while the rows below it, and the `0 files · 1 dir`
/// beside the crumb, still described `/shared`. Both were `/shared`'s reading,
/// printed under `/shared/reports`'s name — and `/shared/reports` is in fact
/// empty, so every word of it was wrong.
///
/// `fs_path` is the path asked for (the crumb moves on the click, deliberately
/// — a click that repaints nothing reads as a dead app). `fs_listed_path` is
/// the path the rows describe. Same split as `active_page`/`buffer_page`.
#[test]
fn the_files_pane_reports_only_a_directory_it_has_listed() {
    let entry = |path: &str, kind: &str| backend::FsEntry {
        key: 0,
        path: path.into(),
        name: path.rsplit('/').next().unwrap_or(path).into(),
        kind: kind.into(),
        size: 0,
        object: String::new(),
    };
    let listing = |generation: i64, path: &str, entries: Vec<backend::FsEntry>| {
        __DucktapeMessage::FsListed(backend::FsListing {
            generation,
            path: path.into(),
            entries,
        })
    };

    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    let _ = app.__update(listing(
        app.fs_generation,
        "/shared",
        vec![entry("/shared/reports", "dir")],
    ));
    assert_eq!(
        app.fs_listed_path, app.fs_path,
        "the listing answered for it"
    );
    assert_eq!(
        backend::fs_counts_summary(app.connected, true, app.fs_entries.clone()),
        "0 files · 1 dir"
    );

    // Navigate. The crumb moves at once; the rows have not.
    let _ = app.__update(__DucktapeMessage::FsOpenDir("/shared/reports".into()));
    assert_eq!(
        app.fs_path, "/shared/reports",
        "the crumb moves on the click"
    );
    assert_eq!(
        app.fs_listed_path, "/shared",
        "the rows still describe where you came from"
    );
    assert_ne!(
        app.fs_listed_path, app.fs_path,
        "`listed` is false for the whole of the navigation, and every reading \
         of `entries` on the screen is gated on it"
    );
    assert_eq!(
        backend::fs_counts_summary(app.connected, false, app.fs_entries.clone()),
        "",
        "no tally for a directory nobody has answered for"
    );

    // The answer lands and the two agree again. The directory is empty, so the
    // tally stays silent — the pane's own plate says "Empty directory" in
    // words, and a subtitle of nothing but zeros repeats it in digits.
    let _ = app.__update(listing(app.fs_generation, "/shared/reports", Vec::new()));
    assert_eq!(app.fs_listed_path, app.fs_path);
    assert_eq!(
        backend::fs_counts_summary(app.connected, true, app.fs_entries.clone()),
        ""
    );

    // A same-path refresh — what a write kicks off — must NOT blank the pane:
    // the rows on hand still describe the path in the crumb.
    let _ = app.__update(listing(
        app.fs_generation,
        "/shared/reports",
        vec![entry("/shared/reports/q3.md", "file")],
    ));
    assert_eq!(app.fs_listed_path, app.fs_path, "a refresh never disagrees");
    assert_eq!(
        backend::fs_counts_summary(app.connected, true, app.fs_entries.clone()),
        "1 file · 0 dirs",
        "and it speaks again as soon as there is something to count"
    );

    // And the screen actually gates on it, at every reading of the rows.
    let storage = inlined(include_str!("ui/screens/storage.ice"));
    let files = storage
        .split_once("component FilesScreen(")
        .expect("the screen")
        .1
        .split_once("\ncomponent ")
        .expect("the screen ends")
        .0;
    assert!(
        files.contains("listed:bool"),
        "the screen is handed the fact"
    );
    for gate in [
        "meta=fs_counts_summary(connected, listed, entries)",
        "if connected && listed && empty(directories)",
        "if connected && listed",
        "if listed && empty(entries)",
        "if listed && !empty(entries)",
    ] {
        assert!(files.contains(gate), "ungated reading of the rows: {gate}");
    }
    let view = inlined(include_str!("ui/view.ice"));
    assert!(
        view.contains("listed=(fs_listed_path == fs_path)"),
        "the mount has to compute it"
    );
}

/// AND THE HEADER SUBTITLES ARE CLAIMS TOO — the subtler half. `Agents 0 agents ·
/// 0 working` is a measured zero about a register nobody read, and it sits ABOVE
/// the body arm above, so it survives it. Every subtitle fold takes `connected`
/// as its first argument and returns "" without it; this pins that no call site
/// can quietly drop the guard.
#[test]
fn every_header_subtitle_is_gated_on_the_connection() {
    let sites: Vec<&str> = SCREENS
        .match_indices("_summary(")
        .map(|(at, _)| {
            let head = SCREENS[..at]
                .rfind(|c: char| !c.is_alphanumeric() && c != '_')
                .map_or(0, |before| before + 1);
            let close = at + SCREENS[at..].find(')').expect("a call site closes");
            &SCREENS[head..=close]
        })
        .collect();

    assert_eq!(
        sites,
        [
            "proposals_summary(connected, rows)",
            "members_summary(connected, rows)",
            "agents_summary(connected, rows)",
            "members_summary(connected, members_rows)",
            "fs_counts_summary(connected, listed, entries)",
        ],
        "a header subtitle folds rows only a live node delivers: pass `connected` \
         first so it says nothing rather than a confident zero"
    );
}

/// THE CONSOLE HEALS ITSELF FROM A CONNECT FAILURE. The steady-state path has
/// always retried forever (`live_resync_failed`), so the app recovered from
/// every interruption except the one that gets it running: `on failed` set
/// Offline and stopped, leaving the console dead against a node answering
/// `/v1/status` in under a millisecond, with no way back but the network
/// picker. Issue #1018 makes that failure ordinary — a `/v1/query` can block
/// until the node writes its next checkpoint, outlasting the client's 30s
/// timeout.
#[test]
fn a_failed_connect_retries_instead_of_giving_up() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    let before = app.connect_generation;

    let fail = |generation: i64| {
        __DucktapeMessage::ConnectFailed(backend::HydrationError {
            generation,
            message: "error sending request".into(),
        })
    };
    let _ = app.__update(fail(app.connect_generation));
    assert_eq!(
        app.hydration_retry_attempt, 1,
        "the first failure is attempt 1"
    );
    assert_eq!(app.status, "Offline", "and it is offline while it retries");
    assert!(
        app.connect_generation > before,
        "each attempt owns a generation, so an abandoned one cannot answer"
    );

    // The counter CLIMBS — that is what feeds the backoff. A reset here would
    // retry at 1s forever against a genuinely dead endpoint.
    let _ = app.__update(fail(app.connect_generation));
    let _ = app.__update(fail(app.connect_generation));
    assert_eq!(app.hydration_retry_attempt, 3);

    // A CONNECT IS NOT GUARDED ON `hydration_generation`, AND THIS IS WHY.
    // Thirty-seven handlers bump that counter for reasons of their own —
    // `choose_channel` is one of them — and a connect is in flight for seconds,
    // or for up to 30s while the node sits in issue #1018's checkpoint stall.
    // Guarded on the shared counter, one click on a channel mid-connect drops
    // the successful reply; because it SUCCEEDED no failure arm fires and
    // nothing retries, so the console sits Offline forever. Strictly worse than
    // the defect this PR fixes.
    let (mut wired, _) = Ducktape::__boot();
    wired.connected_rpc = "http://127.0.0.1:38259".into();
    let connect_gen = wired.connect_generation;
    let shared_before = wired.hydration_generation;
    let _ = wired.__update(__DucktapeMessage::ChooseChannel("general".into()));
    assert!(
        wired.hydration_generation > shared_before,
        "an ordinary channel click bumps the SHARED counter"
    );
    assert_eq!(
        wired.connect_generation, connect_gen,
        "and leaves the connect's own alone — only the three routes that start a connect may touch it"
    );

    // AND A FAILURE FROM AN ABANDONED CHAIN IS DROPPED UNREAD. Without this,
    // two chains retry forever and each one's generation bump can reject the
    // other's success — measured live as two interleaved retry series 5.2s and
    // 10.8s apart, summing to one 16s cap.
    let stale = app.connect_generation - 1;
    let attempts = app.hydration_retry_attempt;
    let _ = app.__update(fail(stale));
    assert_eq!(
        app.hydration_retry_attempt, attempts,
        "an abandoned chain must not start a second retry loop"
    );

    // The handler re-runs the connect, and the reply is generation-guarded.
    let lifecycle = inlined(include_str!("ui/handlers/lifecycle.ice"));
    let arm = lifecycle
        .split_once("on connect_failed(cause)")
        .expect("connect owns its failure arm, not the shared one")
        .1
        .split_once("\non ")
        .expect("the arm ends")
        .0;
    assert!(
        arm.contains("hydration_retry_attempt = hydration_retry_attempt + 1"),
        "the attempt climbs"
    );
    assert!(
        arm.contains(
            "run replace lane=connect connect(connected_rpc, hydration_retry_attempt, connect_generation) -> workspace_connected _ | connect_failed _"
        ),
        "and it goes round again, carrying the attempt into the backoff"
    );
    // Scoped to the ARM, not to the rest of the file: `live_resynced` further
    // down guards on `hydration_generation` and is right to — that one really
    // is the live plane's counter.
    let connected_rest = lifecycle
        .split_once("on workspace_connected(next)")
        .expect("the success arm")
        .1;
    let connected = connected_rest
        .split_once("\non ")
        .map_or(connected_rest, |(arm, _)| arm);
    // NO `||` HERE. An alternative that is trivially true short-circuits the
    // half that matters — the first version of this assertion accepted the
    // presence of a COMMENT and stayed green with the guard pointed back at the
    // shared counter, which is the wedge this test exists to prevent.
    assert!(
        connected.contains("return if next.generation != connect_generation"),
        "the connect is guarded on its OWN generation, never the shared one"
    );
    assert!(
        !connected.contains("return if next.generation != hydration_generation"),
        "the shared counter is bumped by 37 handlers; guarding on it drops a \
         successful connect and nothing retries"
    );

    // The SHARED `failed` arm still belongs to the six page/chat loaders that
    // route to it, and must NOT have grown a connect retry.
    // `on failed` is the LAST handler in the file, so there may be no `\non `
    // after it to cut at — take the remainder when there is not.
    let shared_rest = lifecycle
        .split_once("on failed(cause)")
        .expect("the shared arm")
        .1;
    let shared = shared_rest
        .split_once("\non ")
        .map_or(shared_rest, |(arm, _)| arm);
    assert!(
        !shared.contains("run connect("),
        "a failed page load must not restart the workspace connect"
    );
}

/// THE BEHAVIOUR THE SWEEPS ABOVE ONLY SPELL. Boot the console, drop the
/// connection, and the four subtitles go silent instead of reporting the zeros
/// an unfetched listing folds to.
#[test]
fn a_disconnected_console_reports_no_counts_at_all() {
    let (mut app, _) = Ducktape::__boot();
    app.members_rows = vec![backend::MemberRow {
        key: "aa".into(),
        label: "aa".into(),
        role: "validator".into(),
        is_this_node: true,
        is_agent: false,
        model: String::new(),
        live: true,
    }];
    app.fs_entries = vec![backend::FsEntry {
        key: 0,
        path: "/shared/notes".into(),
        name: "notes".into(),
        kind: "file".into(),
        size: 0,
        object: String::new(),
    }];

    app.connected = true;
    assert_eq!(
        backend::members_summary(app.connected, app.members_rows.clone()),
        "1 human · 0 agents"
    );
    assert_eq!(
        backend::fs_counts_summary(app.connected, true, app.fs_entries.clone()),
        "1 file · 0 dirs"
    );
    // The two registers this boot leaves EMPTY are silent while connected too —
    // an all-zero subtitle repeats, in digits, the plate that already said
    // "No agents registered" / "No proposals yet". `a_subtitle_that_is_all_zeros_
    // says_nothing_at_all` (backend/tests.rs) is where the speaking case is
    // proved with real rows; here they are empty on purpose.
    assert_eq!(
        backend::agents_summary(app.connected, app.agents_rows.clone()),
        ""
    );
    assert_eq!(
        backend::proposals_summary(app.connected, app.gov_rows.clone()),
        ""
    );

    // The node goes down. Everything above was a reading; none of it is one now.
    app.connected = false;
    for (screen, meta) in [
        (
            "Members",
            backend::members_summary(app.connected, app.members_rows.clone()),
        ),
        (
            "Agents",
            backend::agents_summary(app.connected, app.agents_rows.clone()),
        ),
        (
            "Approvals",
            backend::proposals_summary(app.connected, app.gov_rows.clone()),
        ),
        (
            "Files",
            backend::fs_counts_summary(app.connected, true, app.fs_entries.clone()),
        ),
    ] {
        assert_eq!(
            meta, "",
            "{screen} printed `{meta}` off a node that answered nothing"
        );
    }
}

/// A SEARCH THAT ERRORED IS NOT A SEARCH THAT FOUND NOTHING. `search_chat_submit`
/// empties `chat_search_hits` on its way out, so a phase left non-idle by the
/// failure route floats "No messages match" — a confident zero-result card beside
/// an error banner saying the request never landed. One discriminant makes that
/// state unrepresentable: the float reads `SearchPhase`, so the failure arm
/// returns it to `Idle` instead of claiming a completed empty result.
#[test]
fn a_failed_message_search_closes_the_float_instead_of_claiming_zero_results() {
    let (mut app, _) = Ducktape::__boot();
    app.loading = false;
    app.chat_search_draft = "ledger".into();
    let _ = app.__update(__DucktapeMessage::SearchChatSubmit);
    assert_eq!(app.chat_search_phase, SearchPhase::Searching);

    let _ = app.__update(__DucktapeMessage::ChatSearchFailed(backend::AppError {
        message: "rpc unreachable".into(),
        committed: false,
    }));
    assert_eq!(
        app.chat_search_phase,
        SearchPhase::Idle,
        "the float has nothing honest to say about a search that never ran"
    );
    assert!(app.chat_search_hits.is_empty());
    assert_eq!(app.error, "rpc unreachable");

    // And the empty result IS still reachable — "done" with no hits is the miss.
    let _ = app.__update(__DucktapeMessage::SearchChatSubmit);
    let _ = app.__update(__DucktapeMessage::ChatSearchLoaded(
        backend::ChatSearchData { hits: Vec::new() },
    ));
    assert_eq!(app.chat_search_phase, SearchPhase::Done);
}

/// THE DESIGN PASS, PINNED. Every one of these is a measurement someone made
/// against the artifact and a later edit can silently undo: a number in a `with`
/// block reverts as easily as it landed, and none of them fails a build. The
/// grouping rhythm, the line measure, the header row, the selection mark and the
/// loading/failure states are all here so a revert is a red test, not a
/// screenshot nobody takes.
#[test]
fn the_chat_surface_holds_to_its_measured_geometry() {
    let components = inlined(include_str!("ui/components/chat.ice"));
    let screen = inlined(include_str!("ui/screens/chat.ice"));
    let dm = inlined(include_str!("ui/components/dm.ice"));

    // ONE LINE MEASURE. Unbounded, the body ran ~130 characters at the default
    // window and ~320 maximized — past every readability bound there is.
    assert!(components.contains("col w=fill max-w=760.0 gap=5.0"));

    // GROUPING RHYTHM: 11px inside an author run, 25px across one (11 + the
    // 14px spacer). The spacer sits OUTSIDE the hover capsule, so the gap
    // between runs does not light up as part of the row below it.
    assert_eq!(
        components
            .matches("if message.show_author\n      space w=1.0 h=14.0")
            .count(),
        2,
        "both the message card and the thread card open a run with the spacer"
    );
    assert_eq!(
        components
            .matches("row w=fill gap=11.0 align=start")
            .count(),
        3,
        "the message row, the thread ROOT and the skeleton row share one 41px \
         text gutter — the root used to sit 3px off the replies it heads"
    );

    // ONE HEADER HEIGHT across the four panes, so their hairlines land on one y.
    assert_eq!(
        screen.matches("h=50.0").count(),
        4,
        "sidebar, message, thread and details headers are one row height"
    );

    // THE SELECTION MARK IS BRAND, NOT GREY. `pressed` on an unselected row
    // scores 0.00 from `selected_row` by the repo's own metric, so both sidebar
    // lists carry the 2.5px bar no press state can imitate.
    for (name, source) in [("ChannelButton", &components), ("DmButton", &dm)] {
        let selected = source
            .split("if !selected")
            .next()
            .expect("every row component opens with its selected arm");
        assert!(
            selected.contains("box w=2.5 h=fill bg=brand r=1.25"),
            "{name}'s selected arm needs a mark grey cannot forge"
        );
    }
    // And the unread dot stays on the UNSELECTED arm in both, for the same
    // reason: the open row clears its unread the moment it opens.
    assert!(dm.contains("if unread && !selected"));

    // THREE SKELETON ROWS while a room loads, one inside the search float —
    // one component, four mounts, not four copies of the same eight lines.
    assert!(components.contains("component SkeletonRow()"));
    assert_eq!(
        screen
            .lines()
            .filter(|line| line.trim() == "SkeletonRow")
            .count(),
        4,
        "three while a room loads, one inside the search float"
    );

    // A FAILED SEND WEARS GateNote's REVERSIBLE-DANGER PLATE, not a muted
    // sentence quieter than the archived-channel notice above it.
    assert!(screen.contains(
        "box w=fill px=13.0 py=11.0 bg=danger_zone_bg border=danger_zone_line border-w=1.0 r=9.0"
    ));

    // THE DM ROW CARRIES THE SAME PREPARED UNREAD MARK AS A CHANNEL ROW.
    assert!(screen.contains("unread=dm.unread"));
}

/// THE ZERO-HIT SEARCH CARD MUST STAY DISMISSABLE. The Clear-search × used to
/// gate on `!empty(search_hits)`, but the float itself opens on
/// `search_phase != SearchPhase.idle` — so a `done && empty(search_hits)` search drew
/// "No messages match" with no way to close it: not the × (hidden), not
/// Escape (`chat-search` carries no `escape_target` layer), not re-pressing
/// Enter (lands `done`+empty again), not clearing the field (the submit
/// handler returns early on an empty query, leaving the phase untouched).
/// Only a channel/DM switch or a reconnect ever wrote "idle" again. The ×
/// must share the float's own gate, not a narrower one.
#[test]
fn the_clear_search_button_survives_a_zero_hit_result() {
    let screen = inlined(include_str!("ui/screens/chat.ice"));
    assert!(
        screen.contains("if search_phase != SearchPhase.idle\n"),
        "the float and the clear button read the same discriminant"
    );
    assert!(
        !screen.contains("if !empty(search_hits)\n"),
        "the clear control must not gate on hits — a done+empty search has none"
    );
}

// ===========================================================================
// THE FRAME-COST LINTS.
//
// `src/frame_probe.rs` asserts one number — allocations per keystroke on a
// 256-row channel. A number catches a regression the day someone runs it; a
// source sweep catches the two SHAPES that produce that regression at the
// moment they are written, and names them. CLAUDE.md's own rule: guard a
// load-bearing shape with a lint, not a comment.
//
// Both sweeps walk the whole `.ice` view tree rather than a list of files,
// and both carry an allowlist that must stay LIVE — an entry matching nothing
// fails the test, so the ledger cannot rot into a blanket exemption.
// ===========================================================================

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

/// A REPEATED MOUNT IS A PER-FRAME COST MULTIPLIED BY A LIST LENGTH.
///
/// Iced 0.14 has no dirty check: every message rebuilds the whole mounted
/// tree. A plain `col` culls only `draw` — `update`, `mouse_interaction`,
/// `overlay` and `layout` walk every child on every event and every frame. So
/// a `for` that mounts a component pays its whole subtree, per row, forever,
/// unless a `col virtual-row=` above it culls all six passes against the
/// viewport.
///
/// `lazy` is NOT a substitute and this sweep deliberately does not accept one:
/// it memoizes the element and its layout node, so it saves the BUILD and the
/// text re-shape of an unchanged row — and saves nothing at all on the walks,
/// which still recurse into every cached subtree. The chat stream carries both,
/// and needs both.
///
/// Everything not virtualized is listed below, which is the point: the list is
/// the app's inventory of un-culled repetition, argued in three bounded buckets.
///
/// This is the lint that catches a deleted `virtual-row=` — the exact edit a
/// later "make the a11y test see this row" change would make, because an
/// offscreen child is not in the a11y tree.
#[test]
fn every_repeated_component_mount_is_culled_or_argued() {
    // (file, the loop head as authored), grouped by why the bounded walk is
    // acceptable without virtualization.
    //
    // `messages` and `thread_messages` are deliberately absent: both are
    // chain-fed, both grow with a "load older" click, and both are virtualized.
    const ARGUED: &[(&str, &str)] = &[
        // 1. WORKSPACE-SHAPED — channels, DMs, members, repos, pages,
        //    validators, peers. Length tracks how big the workspace is, not
        //    how long the chain has run, and it moves on a delta, not a scroll.
        ("screens/chat.ice", "for room in rooms"),
        ("screens/chat.ice", "for dm in dm_rows"),
        ("screens/chat.ice", "for member in channel_members"),
        ("screens/pages.ice", "for page in pages"),
        ("screens/pages.ice", "for child in subpage_blocks(blocks)"),
        ("screens/forge.ice", "for repo in repos"),
        ("screens/forge.ice", "for entry in tree_entries"),
        ("screens/forge.ice", "for review in forge_item_reviews"),
        (
            "screens/roster.ice",
            "for member in filter_members(rows, filter)",
        ),
        ("screens/roster.ice", "for member in rows"),
        ("screens/roster.ice", "for agent in rows"),
        ("screens/governance.ice", "for proposal in rows"),
        (
            "screens/governance.ice",
            "for proposal in settled_proposals(rows)",
        ),
        ("screens/node.ice", "for peer in node_peers"),
        ("components/huddle.ice", "for tile in rows"),
        ("components/node.ice", "for entry in rows"),
        ("components/onboarding.ice", "for row in networks"),
        ("components/onboarding.ice", "for step in steps"),
        ("components/forge.ice", "for item in items"),
        // 2. WITHIN ONE ROW — one message's blocks and reactions, one
        //    proposal's quorum dots, the nav bar. Bounded by the row that
        //    contains them, and the chat ones already sit inside the stream's
        //    own `lazy`, so they are built once per row change, not per frame.
        ("components/chat.ice", "for block in message.blocks"),
        ("components/chat.ice", "for reaction in message.reactions"),
        (
            "components/kit.ice",
            "for seat in quorum_dots(proposal.approvals, proposal.required_yes)",
        ),
        (
            "components/shell.ice",
            "for item in shell_nav(tab, approvals, agent_live)",
        ),
        ("screens/storage.ice", "for kind_count in kinds"),
        ("screens/storage.ice", "for block in blocks"),
        // One provider turn, hard-capped to MAX_ACTIVITY_ROWS in the backend.
        (
            "screens/shell.ice",
            "keyed row in activity by=row.id #activity w=fill gap=8.0",
        ),
        // 3. QUERY-CAPPED — whatever one query answered with. The list is
        //    replaced wholesale by the next query, never appended to.
        ("screens/chat.ice", "for hit in search_hits"),
        ("screens/pages.ice", "for hit in page_search_hits"),
        ("screens/pages.ice", "for comment_row in block_comment_rows"),
        (
            "screens/pages.ice",
            "for page_comment in block_thread_comments",
        ),
        ("screens/storage.ice", "for hit in hits"),
        (
            "screens/storage.ice",
            "for op in explorer_ops_at(ops, selected)",
        ),
    ];

    let mut unculled: Vec<String> = Vec::new();
    let mut matched: Vec<(&str, &str)> = Vec::new();
    for (path, source) in view_sources() {
        let path = path.replace('\\', "/");
        let lines: Vec<String> = inlined(&source).lines().map(str::to_owned).collect();
        for (index, line) in lines.iter().enumerate() {
            let code = code_of(line);
            let head = code.trim();
            let repeated = head.starts_with("for ") || head.starts_with("keyed ");
            if !repeated {
                continue;
            }
            let loop_indent = indent_of(code);
            let repeats_a_component = lines[index + 1..]
                .iter()
                .take_while(|next| next.trim().is_empty() || indent_of(next) > loop_indent)
                .any(|line| mounts_a_component(code_of(line)));
            if !repeats_a_component {
                continue;
            }
            let mut virtualized = head.contains("virtual-row=");
            let mut depth = loop_indent;
            for above in lines[..index].iter().rev() {
                if above.trim().is_empty() || indent_of(above) >= depth {
                    continue;
                }
                depth = indent_of(above);
                virtualized |= above.contains("virtual-row=");
                if virtualized || depth == 0 {
                    break;
                }
            }
            if virtualized {
                continue;
            }
            match ARGUED
                .iter()
                .find(|(file, loop_head)| path.ends_with(file) && *loop_head == head)
            {
                Some(entry) => matched.push(*entry),
                None => unculled.push(format!("(\"{path}\", \"{head}\"),")),
            }
        }
    }

    assert!(
        unculled.is_empty(),
        "a repeated component mount walks every row on every event and every \
         frame. Put it under a `col virtual-row=` — a `lazy` row does not cull \
         a walk — or argue it into ARGUED above with the bucket it belongs \
         to:\n{}",
        unculled.join("\n")
    );
    for entry in ARGUED {
        assert!(
            matched.contains(entry),
            "{entry:?} argues for a loop that no longer exists — delete the \
             entry so the ledger keeps saying something true"
        );
    }
}

/// EVERY EXTERN ARGUMENT IS BY VALUE, SO A LIST ARGUMENT IS A DEEP CLONE.
///
/// `sync`/`pure f(rows:[T])` called from a view expression clones the whole list into
/// the call, once per frame, per call site — and if the site sits inside a
/// `for`, once per row per frame. Feature state records three fields
/// (`unread_marker_seq`, `has_older_history`, `rooms`) that exist only because
/// this was measured and paid for; the fix is always the same, mirror the
/// answer into state where the list is written.
///
/// So: a view may not hand a list to an extern. The ledger below is every
/// place that still does — pre-existing, on screens whose frame cost nobody
/// has measured. It is a ratchet, not a blessing: the next one fails.
#[test]
fn no_view_expression_hands_an_extern_a_list() {
    let list_taking: Vec<String> = ice_sources()
        .into_iter()
        .filter(|(path, _)| path.replace('\\', "/").contains("/extern/"))
        .flat_map(|(_, source)| {
            source
                .lines()
                .filter_map(|line| {
                    let declaration = line
                        .trim()
                        .strip_prefix("sync ")
                        .or_else(|| line.trim().strip_prefix("pure "))?;
                    let (name, rest) = declaration.split_once('(')?;
                    let (args, _) = rest.split_once(')')?;
                    args.contains(":[").then(|| name.to_owned())
                })
                .collect::<Vec<_>>()
        })
        .collect();
    assert!(
        list_taking.len() > 50,
        "the extern sweep found the declarations, not an empty file"
    );

    // (file, extern). Every entry clones once per frame and predates this
    // sweep; the sweep exists so the next one does not.
    const ARGUED: &[(&str, &str)] = &[
        // 1. ONCE PER FRAME — one clone of a workspace-shaped list per
        //    rebuild. The mount file's props are the bulk of it: `view.ice`
        //    is evaluated once, not per row.
        ("view.ice", "member_tier"),
        ("view.ice", "members_is_admin"),
        ("view.ice", "bell_worst_severity"),
        ("view.ice", "open_proposals"),
        ("view.ice", "any_agent_active"),
        ("screens/node.ice", "member_tier"),
        ("screens/node.ice", "members_is_admin"),
        ("screens/settings.ice", "member_tier"),
        ("screens/settings.ice", "members_is_admin"),
        ("screens/settings.ice", "members_summary"),
        ("screens/pages.ice", "doc_tab_rows"),
        ("screens/pages.ice", "subpage_blocks"),
        ("screens/pages.ice", "thread_is_resolved"),
        ("screens/pages.ice", "comment_compose_hint"),
        ("screens/forge.ice", "forge_open_count"),
        ("screens/forge.ice", "filter_forge_items"),
        ("screens/forge.ice", "forge_comment_cap_reached"),
        ("screens/storage.ice", "fs_counts_summary"),
        ("screens/storage.ice", "explorer_ops_at"),
        ("screens/governance.ice", "proposals_summary"),
        ("screens/governance.ice", "open_proposals"),
        ("screens/governance.ice", "pending_label"),
        ("screens/governance.ice", "settled_proposals"),
        ("screens/roster.ice", "members_summary"),
        ("screens/roster.ice", "agents_summary"),
        ("screens/roster.ice", "filter_members"),
    ];

    let mut cloned: Vec<String> = Vec::new();
    let mut matched: Vec<(&str, &str)> = Vec::new();
    for (path, source) in view_sources() {
        let path = path.replace('\\', "/");
        for line in inlined(&source).lines() {
            let code = code_of(line);
            for name in &list_taking {
                if !calls(code, name) {
                    continue;
                }
                match ARGUED
                    .iter()
                    .find(|(file, extern_name)| path.ends_with(file) && extern_name == name)
                {
                    Some(entry) => matched.push(*entry),
                    None => cloned.push(format!("(\"{path}\", \"{name}\"), // {}", code.trim())),
                }
            }
        }
    }

    assert!(
        cloned.is_empty(),
        "a view expression handed a list to an extern — the ABI is by value, so \
         that is a deep clone of the whole list on every frame. Mirror the answer \
         into a feature-state field written where the list is written:\n{}",
        cloned.join("\n")
    );
    for entry in ARGUED {
        assert!(
            matched.contains(entry),
            "{entry:?} argues for a call that is gone — delete the entry"
        );
    }
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
