use super::*;

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
        chain_id: "mynet#d0cdf950".into(),
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
    assert_eq!(app.node_root_hash, "hash-new");
    assert_eq!(app.network_chain_id, "mynet#d0cdf950");
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
    let view = inlined(include_str!("../ui/view.ice"));
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

/// STATUS EVERYWHERE, PEERS ONLY WHERE IT IS DRAWN — pinned as sets, because a
/// `contains` is satisfied by a commented-out line and equally by a SECOND,
/// wrongly-gated subscription sitting beside the right one.
#[test]
fn the_node_streams_carry_the_gates_their_costs_require() {
    let lifecycle = inlined(include_str!("../ui/handlers/lifecycle.ice"));
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
    let settings = include_str!("../ui/screens/settings.ice");
    let node = include_str!("../ui/screens/node.ice");
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

    let shell = include_str!("../ui/components/shell.ice");
    assert!(shell.contains("ShellTab.node\n                  slot node"));
    let view = include_str!("../ui/view.ice");
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
/// `crates/noded/src/peers.rs` serves `peer` / `connected` / `role`; it has never
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
        ("backend/node.rs", include_str!("../backend/node.rs")),
        ("backend/roster.rs", include_str!("../backend/roster.rs")),
    ];
    for (name, source) in READERS {
        for wrong in ["peer[\"key\"]", "peer[\"live\"]", "peer[\"height\"]"] {
            assert!(
                !source.contains(wrong),
                "{name} reads {wrong}, which `/v1/peers` does not serve — \
                 see crates/noded/src/peers.rs for the names it does"
            );
        }
        assert!(
            source.contains("peer[\"peer\"]") || source.contains("peer[\"connected\"]"),
            "{name} was expected to read the peers view; if it no longer does, \
             drop it from this lint rather than leaving the guard vacuous"
        );
    }
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
    let lifecycle = inlined(include_str!("../ui/handlers/lifecycle.ice"));

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
    ] {
        let live = format!(
            "from done load_request(plane_live_hit(next.kind, next.module, \"{module}\"), connected_rpc, \"\", {generation})\n          try request -> done request\n          done -> {selected} _"
        );
        assert!(
            lifecycle.contains(&live),
            "a {module} commit must refresh {loader} on any tab: {live}"
        );
    }

    // THE AGENTS PROJECTION IS THE ONE PLANE TWO MODULES WRITE, so its live
    // arm is the one that does not ride `plane_live_hit`: `agent` commits the
    // registration and `runs` commits the liveness `AgentRow.live` is read
    // from (`agents_with_a_run_in_flight`). BOTH lines take the predicate —
    // a bump without the load refetches nothing, and a load without the bump
    // answers on a generation `agents_loaded` rejects. Narrow either back to
    // `"agent"` and the Forge seat's dot goes dark for the length of a run.
    for line in [
        "agents_generation = keep_i64(agents_plane_hit(next.kind, next.module), agents_generation + 1, agents_generation)",
        "from done load_request(agents_plane_hit(next.kind, next.module), connected_rpc, \"\", agents_generation)\n          try request -> done request\n          done -> agents_load_selected _",
    ] {
        assert!(
            lifecycle.contains(line),
            "the agents live arm must ride the two-module predicate: {line}"
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
/// chat's seating key, chat returns above the tab block, and nothing chat does
/// ever re-issues the load. Lose it and the key stays "" for the session —
/// which `post_gate` reads as "not seated", refusing the composer on every
/// members-only room.
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

/// THE AGENTS BUMP IS THE SAME HALF, AND `run replace` DOES NOT COVER IT.
/// Replacing a lane aborts work still running there, but it cannot retract a
/// completion the runtime has already queued — that reply is delivered
/// anyway, and an unconditional bump on the way out is precisely what makes
/// `agents_loaded` throw it away. The Forge seat's live dot reads those rows
/// on EVERY tab, so opening the destination pane does not re-earn them: the
/// next `agent` or `runs` op does, and for a run that just started that op is
/// the one that ends it.
#[test]
fn a_move_off_the_agents_tab_keeps_a_live_load_that_already_answered() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.loading = false;

    // the run's own commit is what asks for the rows; its generation is the
    // one the reply below carries.
    let _ = app.__update(__DucktapeMessage::LiveUpdated(backend::LiveUpdate {
        kind: LiveKind::Plane,
        status: "Live".into(),
        height: 12,
        module: "runs".into(),
        ..backend::LiveUpdate::default()
    }));
    let in_flight = app.agents_generation;

    let _ = app.__update(__DucktapeMessage::SelectShellTab(ShellTab::Members));
    let _ = app.__update(__DucktapeMessage::AgentsLoaded(backend::AgentsData {
        generation: in_flight,
        agents: vec![backend::AgentRow {
            id: "agent-1".into(),
            name: "Quackbot".into(),
            initials: "QU".into(),
            capability: "mock-llm-1".into(),
            status: "active".into(),
            owner_handle: String::new(),
            live: true,
            skill_count: 0,
            cap_count: 0,
        }],
    }));
    assert!(
        backend::any_agent_active(&app.agents_rows),
        "the move off-tab must not revoke the run's own refetch — the dot is drawn on every tab"
    );

    // and the tab that DOES draw the rows still re-reads on entry.
    let _ = app.__update(__DucktapeMessage::SelectShellTab(ShellTab::Agents));
    assert_ne!(
        app.agents_generation, in_flight,
        "entering Agents must issue a fresh read"
    );
}

/// THE JOIN LANDS IN THE DOCK, AND OPENS NO WINDOW. Every face, every shared
/// screen and every media control lives in the in-window dock; a join routed
/// back to the generic `chat_acked` would leave someone sitting in a live call
/// watching a static pill, which is indistinguishable from a huddle that does
/// not work — so the route is pinned here.
///
/// The ack used to open the popped huddle window, and that is the defect this
/// now pins the inverse of: the second OS window fell behind the console the
/// moment anything in the console was clicked, and the console said nothing
/// about the call at all. Popping out is an explicit click on the dock now, so
/// an `open huddle` task reappearing in this ack is a regression, not a
/// convenience.
#[test]
fn joining_a_huddle_lands_in_the_in_window_dock() {
    let handler = inlined(include_str!("../ui/handlers/chat.ice"));
    assert!(
        handler
            .contains("join_huddle(connected_rpc, password, active_channel) -> huddle_joined_ack"),
        "the join's ack is its own, not the generic one"
    );
    let ack = handler
        .split_once("on huddle_joined_ack")
        .expect("the join ack handler exists")
        .1;
    let ack = ack.split_once("\non ").map_or(ack, |split| split.0);
    assert!(
        !ack.contains("task window open huddle"),
        "the join lands in the dock — the popped window is an explicit choice"
    );
    assert!(
        ack.contains("huddle_dock_collapsed = false"),
        "a join is a huddle to look at, whatever the last one was folded to"
    );
    // AND THE ACK LANDS THE JOINED STATE ITSELF. `huddle_joined` has no other
    // writer on the way in: it is answered off a chat load's roster, and the
    // load that answered it used to be the popped window's. With no window,
    // an ack that only cleared the mutation left the write committed, the
    // chain roster listing her, and the app showing the start button with no
    // media session — `call_session` is gated on this very flag.
    for landed in [
        "huddle_joined = true",
        "huddle_channel = active_channel",
        "huddle_channel_name = active_channel_name",
        "huddle_joined_at = huddle_now",
    ] {
        assert!(ack.contains(landed), "the join ack lands {landed}");
    }
    assert!(
        ack.contains(
            "run replace lane=chat_load load_channel_window(connected_rpc, active_channel, \
             chat_generation) -> chat_updated"
        ),
        "and asks that channel for the roster the tiles and the reconciler need"
    );
    let huddle = inlined(include_str!("../ui/handlers/huddle.ice"));
    let popped = huddle
        .split_once("on pop_huddle")
        .expect("popping out still exists")
        .1;
    assert!(
        popped.contains("task window open huddle"),
        "popping out is what opens the window now"
    );
}

/// THE HUDDLE IS SHOWN WHEREVER YOU ARE, AND DRAWN ONCE.
///
/// A call you are in does not stop being live because you opened Pages or
/// clicked another room — the media session is subscribed on `huddle_joined`
/// and nothing else (handlers/lifecycle.ice). The UI used to disagree: the
/// docked pill carried a `shell_tab`/`active_channel` term, the panel lived in
/// a second OS window, and between them a live huddle could be invisible on
/// the screen you were actually looking at. So the dock's visibility rule may
/// not read WHERE you are at all, and this is the guard that says so — it
/// fails on the next `shell_tab ==` term anybody adds to either arm.
///
/// It also pins the other half: the dock and the panel each mount the
/// `extern call_video_*` widgets, and each of those runs its own 4 ms repaint
/// clock while a tile is live (video.rs). Two of them up at once would be two
/// clocks for one call, so the two mounts must be mutually exclusive — the
/// dock under `!huddle_popped`, the panel only inside the window whose
/// existence IS `huddle_popped` (state/derived.ice).
#[test]
fn the_huddle_dock_rides_every_tab_and_keeps_one_video_surface() {
    let view = inlined(include_str!("../ui/view.ice"));

    // 1. THE TWO ARMS of the window-level huddle slot, as authored.
    let slot = view
        .split_once("\n        huddle:\n")
        .expect("the window-level huddle slot")
        .1;
    let slot = slot
        .split_once("\n        palette:\n")
        .map_or(slot, |split| split.0);
    let arms: Vec<&str> = slot
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("if "))
        .collect();
    assert_eq!(
        arms,
        [
            "if huddle_joined && !huddle_popped && !huddle_dock_collapsed",
            "if huddle_joined && !huddle_popped && huddle_dock_collapsed",
        ],
        "the dock is expanded or folded to its pill, and nothing else gates it"
    );
    for arm in &arms {
        for elsewhere in ["shell_tab", "active_channel", "huddle_channel "] {
            assert!(
                !arm.contains(elsewhere),
                "the huddle rides every tab and every channel: {arm:?} reads {elsewhere}"
            );
        }
    }
    // `inlined` folds each mount's `with` block onto its own line, so a mount
    // is the head of a line — which is also what keeps `HuddleDock` from
    // matching inside `HuddleDockedPill`.
    let mounts: Vec<&str> = slot
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .filter(|head| head.starts_with("Huddle"))
        .collect();
    assert_eq!(
        mounts,
        ["HuddleDock", "HuddleDockedPill"],
        "the expanded card and the folded pill, and nothing else in this slot"
    );

    // 2. THE PANEL IS THE WINDOW'S, and only the window's.
    let panel_at = view
        .find("      HuddlePanel #huddle")
        .expect("the popped panel is mounted");
    let guard = view[..panel_at]
        .lines()
        .rev()
        .find(|line| line.trim().starts_with("if "))
        .expect("the panel is guarded");
    assert_eq!(
        guard.trim(),
        "if huddle_win == some(window)",
        "the panel draws inside the huddle window and nowhere else"
    );
    let derived = inlined(include_str!("../ui/state/derived.ice"));
    assert!(
        derived.contains("huddle_popped = huddle_win != none"),
        "`!huddle_popped` on the dock is what makes the two mounts exclusive"
    );

    // 3. AND THE VIDEO WIDGETS LIVE IN EXACTLY THOSE TWO COMPONENTS.
    let components = inlined(include_str!("../ui/components/huddle.ice"));
    let mut mounting: Vec<&str> = Vec::new();
    let mut component = "";
    for line in components.lines() {
        if let Some(rest) = line.strip_prefix("component ") {
            component = rest.split('(').next().unwrap_or(rest);
        }
        if line.trim().starts_with("extern call_video_") {
            mounting.push(component);
        }
    }
    mounting.sort_unstable();
    mounting.dedup();
    assert_eq!(
        mounting,
        ["HuddleDock", "HuddlePanel"],
        "a third video surface is a third repaint clock on one call"
    );
}

/// THE HUDDLE RESERVES SPACE; IT DOES NOT FLOAT OVER THE MODULE.
///
/// A card in the corner of the window covers whatever the module put at that
/// corner — the chat composer's Send, the Pages editor's own bottom bar, the
/// last rows of every timeline — and no single inset clears all of them on
/// every tab. So the huddle is a CELL IN THE CONTENT ROW: it sits beside the
/// module, the module's content box gets `fill(5)` against its `fill(2)`, and
/// overlap is impossible by construction rather than by arithmetic.
///
/// This walks the three sources that have to agree on that: shell.ice mounts
/// the slot in the row, view.ice fills it with a portion-width column whose
/// clamp is `huddle_dock_width`, and the card itself carries no width at all.
#[test]
fn the_huddle_reserves_a_column_instead_of_floating_over_the_module() {
    let shell = include_str!("../ui/components/shell.ice");
    let row = shell
        .split_once("        row w=fill h=fill\n")
        .expect("the rail/content row")
        .1;
    let row = row.split_once("\n      slot ").map_or(row, |split| split.0);
    assert!(
        row.contains("          slot huddle"),
        "the huddle is a cell in the content row, not a layer over the window"
    );
    assert!(
        row.contains("              w=fill\n"),
        "the module's content box takes whatever the huddle column leaves"
    );
    // And it is NOT in the window-level stack any more: `palette` and `bell`
    // are the layers that legitimately float, and the huddle left them.
    let layers = shell
        .rsplit_once("\n      slot palette")
        .expect("the floating layers")
        .1;
    assert!(
        !layers.contains("slot huddle"),
        "a floating huddle is the overlap this test exists to prevent"
    );

    let view = inlined(include_str!("../ui/view.ice"));
    let slot = view
        .split_once("\n        huddle:\n")
        .expect("the huddle slot")
        .1;
    let slot = slot
        .split_once("\n        palette:\n")
        .map_or(slot, |split| split.0);
    let column = slot
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("box "))
        .unwrap_or_default();
    assert_eq!(
        column,
        "box w=huddle_dock_width(huddle_joined, huddle_popped, huddle_dock_collapsed) h=fill align-y=end",
        "one number owns the column's width, and zero is what removes it"
    );
    // NO INSET ANYWHERE. A `pb=` big enough to clear a composer is the shape
    // of the floating dock this replaced, and it belongs to no tab now.
    assert!(
        !slot.contains("huddle_dock_inset"),
        "reserving space is the fix; an inset is the workaround it replaced"
    );

    // THE CARD CARRIES NO WIDTH. Its width is the column's, so a `max-w` or a
    // fixed `w=` on any of the huddle's own surfaces is a second owner of the
    // number — and the reason a joined huddle used to sit in a 312px card on a
    // 2560px screen.
    let components = inlined(include_str!("../ui/components/huddle.ice"));
    let mut component = "";
    for line in components.lines() {
        if let Some(rest) = line.strip_prefix("component ") {
            component = rest.split('(').next().unwrap_or(rest);
        }
        let reflows =
            ["HuddleDock", "HuddlePanel", "HuddleTile", "HuddleControls"].contains(&component);
        assert!(
            !(reflows && line.contains("max-w=")),
            "{component} must reflow with the window it is given: {line:?}"
        );
    }
    let dock = components
        .split_once("component HuddleDock(")
        .expect("the dock")
        .1;
    assert!(
        dock.contains("box #root w=fill bg=surface"),
        "the dock is as wide as the column hands it and no wider"
    );
}

/// ONE HUDDLE SURFACE AT A TIME.
///
/// The chat header used to carry two of its own — a LIVE pill in the huddle's
/// room and a "call in progress" chip in every other one — beside a dock that
/// says both things on every screen, with faces, a clock and a way in. Both
/// are gone. What is left in the header is the one state the dock cannot
/// speak for: a huddle popped out into its own OS window, where the pill is
/// how you raise it.
#[test]
fn the_chat_header_carries_no_second_huddle_surface() {
    let screen = inlined(include_str!("../ui/screens/chat.ice"));
    assert!(
        !screen.contains("HuddleElsewhere"),
        "the dock names the huddle's room on every screen — the header chip          was the same sentence twice"
    );
    let at = screen
        .find("HuddleLivePill")
        .expect("the header's live pill");
    let pill = screen[..at]
        .lines()
        .rev()
        .find(|line| line.trim().starts_with("if "))
        .map(str::trim)
        .expect("its guard");
    assert_eq!(
        pill, "if huddle_joined && huddle_channel == active_channel && huddle_popped",
        "the header pill draws only while the dock cannot: the huddle is in its own window"
    );
    let components = inlined(include_str!("../ui/components/huddle.ice"));
    assert!(
        !components.contains("component HuddleElsewhere"),
        "deleted, not deprecated"
    );
    assert!(
        !components.contains("component HuddleLivePill(name:str"),
        "the pill names no channel now: the header it sits in already says which room this is"
    );
}

#[test]
fn a_failed_huddle_leave_keeps_the_retained_roster_visible() {
    let handler = inlined(include_str!("../ui/handlers/huddle.ice"));
    let after = |marker: &str| {
        let body = handler
            .split_once(marker)
            .unwrap_or_else(|| panic!("{marker} exists"))
            .1;
        // ONE handler, not the rest of the file: the ack below legitimately
        // blanks what the request must not.
        body.split_once("\non ")
            .map_or(body, |split| split.0)
            .to_string()
    };

    let leave = after("on leave_huddle_here");
    assert!(leave.contains("call_peers = []"));
    assert!(
        leave.contains("huddle_rows = huddle_tile_rows(huddle_roster, call_peers, call_muted)")
    );
    assert!(
        !leave.contains("huddle_rows = []"),
        "an uncommitted leave failure retains the roster, so blanking its mirror is permanent"
    );

    // THE COMMITTED LEAVE IS WHAT ENDS IT. The resync behind it loads the room
    // on screen, which the popped huddle window outlives — so the ack, not the
    // load, has to be the thing that takes her off this device's huddle.
    let acked = after("on huddle_left");
    for cleared in [
        "huddle_joined = false",
        "huddle_roster = []",
        "huddle_rows = []",
        "huddle_channel = \"\"",
    ] {
        assert!(acked.contains(cleared), "the leave ack clears {cleared}");
    }
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
        kind: LiveKind::Ready,
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
    let source = inlined(include_str!("../ui/screens/governance.ice"));
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

/// ACCOUNT FACTS ONLY WHEN THERE IS AN ACCOUNT. With the local key in no
/// account, `load_account` returns zeros for every field, and the identity
/// card printed `0 keys` one line under "· validator keypair on this device" —
/// a count of the ACCOUNT's keys reading as a count of THIS DEVICE's, and the
/// two contradicting each other inside one card.
///
/// `account_exists` is the fact that tells an empty account from no account.
/// It was already in state, already gating the Rename submit, and simply was
/// not given to the screen.
#[test]
fn the_identity_card_counts_only_an_existing_account() {
    let settings = inlined(include_str!("../ui/screens/settings.ice"));
    assert!(
        settings.contains("account_exists:bool"),
        "the screen has to be handed the fact before it can use it"
    );
    let card = settings
        .split("if account_exists")
        .nth(1)
        .expect("the counts sit under the exists gate")
        .split("\n        col ")
        .next()
        .expect("card region");
    for reading in ["account_keys", "Copy number"] {
        assert!(
            card.contains(reading),
            "{reading} is an account reading and belongs under the gate"
        );
    }

    // And view.ice actually passes it, or the screen renders a default.
    let view = inlined(include_str!("../ui/view.ice"));
    assert!(
        view.contains("account_exists"),
        "the mount has to supply it"
    );

    // THE SEPARATOR BELONGS TO THE NUMBER. #998 stopped the card counting an
    // account that does not exist; the dot that had joined those counts stayed
    // ungated, and a key in no account has no `account_number` either — so the
    // line led with it: `· validator keypair on this device`. Every other
    // separator in the console is gated by the run it introduces (forge.ice's
    // repo-count dot carries the reasoning in its own comment).
    let number_line = settings
        .split_once("text account_number")
        .expect("the number line")
        .1
        .split_once(r#"text "keypair on this device""#)
        .expect("the custody clause closes it")
        .0;
    let dot = number_line.find(r#"text "·""#).expect("the separator");
    let guard = number_line
        .find("if !empty(account_number)")
        .expect("the separator's guard");
    assert!(guard < dot, "the dot is gated by the number it introduces");
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
    let view = inlined(include_str!("../ui/view.ice"));
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
    let chat = inlined(include_str!("../ui/screens/chat.ice"));
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
    let root_view = inlined(include_str!("../ui/view.ice"));
    for (path, source) in ice_sources() {
        let in_state_directory = std::path::Path::new(&path)
            .parent()
            .is_some_and(|parent| parent.ends_with("src/ui/state"));
        let owns_app_state = source
            .lines()
            .any(|line| matches!(line, "state" | "derived"));
        assert!(
            in_state_directory || !owns_app_state,
            "{path}: top-level app state belongs under ui/state"
        );
    }
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
    let chat_handlers = inlined(include_str!("../ui/handlers/chat.ice"));
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
    // The operation id is a value of ONE send, and it is minted where the
    // send begins: the composer instance mints it as it emits
    // (ducktape-ui#697), and the app receives it as a handler parameter.
    // Neither half may park it in state.
    for local in ["pending_message_id", "pending_reply_id", "pending_id"] {
        assert!(!root_state.contains(local), "root state holds `{local}`");
    }
    let chat_components = inlined(include_str!("../ui/components/chat.ice"));
    assert!(
        chat_components.contains("fresh_operation_id(composer_op_prefix("),
        "the composer mints its own operation id as it emits"
    );
    assert!(
        chat_handlers.contains("on composer_submitted(kind, pending_body, pending_id)"),
        "and the app takes it as a parameter, not from state"
    );
    assert!(
        !inlined(include_str!("../ui/state/chat.ice"))
            .lines()
            .any(|line| line.trim_start().starts_with("reply_draft =")),
        "root state reclaimed `reply_draft`"
    );
    let page_handlers = inlined(include_str!("../ui/handlers/pages.ice"));
    assert!(!root_state.contains("closing_doc_tab"));
    assert!(page_handlers.contains("on close_doc_tab(id)"));
    assert!(page_handlers.contains("doc_tabs = doc_tabs_without(doc_tabs, id)"));
    assert!(!root_state.contains("page_link"));
    assert!(page_handlers.contains("let page_link = page_link_of(event)"));

    let native_surfaces = concat!(
        include_str!("../backend/live.rs"),
        include_str!("../backend/load.rs"),
        include_str!("../backend/mod.rs"),
        include_str!("../backend/model.rs"),
        include_str!("../backend/storage.rs"),
        include_str!("../frame_probe.rs"),
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
    let source = inlined(include_str!("../ui/screens/storage.ice"));
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
        backend::fs_counts_summary(app.connected, true, &app.fs_entries),
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
        backend::fs_counts_summary(app.connected, false, &app.fs_entries),
        "",
        "no tally for a directory nobody has answered for"
    );

    // The answer lands and the two agree again. The directory is empty, so the
    // tally stays silent — the pane's own plate says "Empty directory" in
    // words, and a subtitle of nothing but zeros repeats it in digits.
    let _ = app.__update(listing(app.fs_generation, "/shared/reports", Vec::new()));
    assert_eq!(app.fs_listed_path, app.fs_path);
    assert_eq!(
        backend::fs_counts_summary(app.connected, true, &app.fs_entries),
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
        backend::fs_counts_summary(app.connected, true, &app.fs_entries),
        "1 file · 0 dirs",
        "and it speaks again as soon as there is something to count"
    );

    // And the screen actually gates on it, at every reading of the rows.
    let storage = inlined(include_str!("../ui/screens/storage.ice"));
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
    let view = inlined(include_str!("../ui/view.ice"));
    assert!(
        view.contains("listed=(fs_listed_path == fs_path)"),
        "the mount has to compute it"
    );
}

/// THE FILES PREVIEW IS THE FORGE READER, NOT A PLAIN TEXT NODE.
///
/// Text files read as numbered, syntect-coloured rows through the same
/// `forge_code` extern the forge blob pane mounts (behind the same `lazy`
/// memo boundary), and a Markdown path reads as a document through
/// `agent_markdown`. Pinned so the pane cannot quietly fall back to the one-ink
/// `text preview_text` it shipped with — and so a pick clears the previous
/// body before the read lands, instead of showing A's text under B's path.
#[test]
fn the_files_preview_reads_text_through_the_forge_reader() {
    let storage = inlined(include_str!("../ui/screens/storage.ice"));
    let files = storage
        .split_once("component FilesScreen(")
        .expect("the screen")
        .1
        .split_once("\ncomponent ")
        .expect("the screen ends")
        .0;
    assert!(
        files.contains("lazy preview_text by preview_text, preview_path, dark as cached_source"),
        "the reader's memo boundary is the mount's lazy"
    );
    assert!(
        files.contains("extern forge_code(cached_source, preview_path, dark) #fs-code"),
        "text mounts the highlighted reader"
    );
    assert!(
        files.contains("if !preview_binary && !preview_picture && markdown_path(preview_path)")
            && files
                .contains("lazy preview_text by preview_text, preview_path, dark as cached_doc")
            && files.contains("extern agent_markdown(cached_doc, dark) #fs-markdown"),
        "a markdown path reads as a document"
    );
    assert!(
        files.contains("if preview_picture\n")
            && files.contains("extern picture(\"files\", preview_path) #fs-picture"),
        "a picture draws through the viewer"
    );
    assert!(
        files.contains("if !preview_binary && !preview_picture && !editing && !preview_truncated"),
        "a picture has no Edit"
    );
    assert!(
        !files.contains("text preview_text\n"),
        "no arm falls back to the one-ink text node"
    );

    let view = inlined(include_str!("../ui/view.ice"));
    let mount = view
        .split_once("FilesScreen new_name<->fs_new_name")
        .expect("the mount")
        .1
        .split_once("\n        members:")
        .expect("the mount ends")
        .0;
    assert!(mount.contains("dark\n"), "the mount hands the screen the appearance");
    assert!(
        mount.contains("open_message_link -> open_message_link _"),
        "markdown links route through the shell's link seam"
    );

    let handlers = include_str!("../ui/handlers/files.ice");
    let open_file = handlers
        .split_once("on fs_open_file(path)")
        .expect("the handler")
        .1
        .split_once("\non ")
        .expect("the handler ends")
        .0;
    let cleared = open_file.find("fs_preview_text = \"\"").expect("the old body is cleared");
    let unpictured = open_file.find("fs_preview_picture = false").expect("the old picture flag is cleared");
    let read = open_file.find("run replace lane=files_preview").expect("the read");
    assert!(cleared < read && unpictured < read, "the body is cleared before the read is issued");
}

/// ESCAPE CLOSES WHAT IS ON SCREEN, AND THE THREAD RAIL IS NOT.
///
/// Channel details unmounts the rail — `if active_thread_seq > 0 &&
/// !channel_settings_open` in `screens/chat.ice` — and nothing clears the ⋯
/// flag on the way in, so opening a thread action and then the drawer is a
/// mouse-reachable state where the ladder's first rung named a menu nobody
/// could see: the press wiped a half-typed `thread_edit_draft` and left the
/// drawer standing. Same rule as the tab scoping, one level down — a rung
/// answers only while its surface is mounted.
#[test]
fn escape_closes_the_drawer_over_a_thread_menu_the_drawer_unmounted() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.shell_tab = ShellTab::Chat;
    app.active_channel = "general".into();
    app.thread_selected_seq = 7;
    app.thread_message_action = MessageAction::Editing;
    app.thread_edit_draft = "half typed".into();
    app.channel_settings_open = true;

    let _ = app.__update(__DucktapeMessage::GlobalKeyPressed(escape_press()));
    assert!(
        !app.channel_settings_open,
        "the first Escape closes the drawer the reader is looking at"
    );
    assert_eq!(
        app.thread_edit_draft, "half typed",
        "and leaves the unmounted rail's draft where she left it"
    );
    assert_eq!(app.thread_message_action, MessageAction::Editing);

    // With the drawer down the rail is mounted again, and its rung answers.
    let _ = app.__update(__DucktapeMessage::GlobalKeyPressed(escape_press()));
    assert_eq!(app.thread_message_action, MessageAction::Toolbar);
    assert_eq!(app.thread_edit_draft, "");
}

/// THE FILES DELETE CONFIRM IS AN OVERLAY, SO IT ANSWERS ESCAPE.
///
/// `fs_delete_target` arms a scrim + `ConfirmDelete` over duckfs
/// (`screens/storage.ice`). Its dismiss route is the backdrop click and the
/// Cancel button — a destructive confirm with no keyboard exit, which is the
/// state the drawer was in before #1132 gave it a rung.
#[test]
fn escape_disarms_the_files_delete_confirm_from_the_files_tab_only() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.shell_tab = ShellTab::Files;
    app.fs_delete_target = "/shared/report.md".into();

    let _ = app.__update(__DucktapeMessage::GlobalKeyPressed(escape_press()));
    assert_eq!(
        app.fs_delete_target, "",
        "Escape is the keyboard way out of a destructive confirm"
    );

    // And the rung is scoped like every other per-tab rung: from another tab
    // the confirm is not on screen, so the press names no layer at all.
    app.shell_tab = ShellTab::Node;
    app.fs_delete_target = "/shared/report.md".into();
    app.bell_open = true;
    let _ = app.__update(__DucktapeMessage::GlobalKeyPressed(escape_press()));
    assert!(!app.bell_open, "the bell rides every tab and answers first");
    assert_eq!(app.fs_delete_target, "/shared/report.md");
}

/// THE LADDER'S TAB SCOPING IS READ OFF THE MOUNT LAYOUT — SO THE LAYOUT PINS IT.
///
/// #1132 scoped every per-tab rung in `topmost_overlay` by reading
/// `components/shell.ice`'s `match tab`: a rung whose surface is mounted under
/// an arm answers only from that arm's tab, and one whose surface sits OUTSIDE
/// the match (the palette, the bell, the create modal) rides every tab. That
/// reading was done once, by hand, and nothing held it: move a screen to
/// another arm — or add a rung and forget its guard — and the pre-#1132 symptom
/// comes back silently, a stale flag eating the first Escape on every other
/// tab while the visible screen swallows the press.
///
/// So the rule is derived here instead of restated: `match tab` says which slot
/// each tab mounts, `view.ice` says which state each slot is handed, and the
/// ladder itself says which flags each rung reads and which tab it is guarded
/// on. Nothing in this test names a tab, a slot or a rung — the three sources
/// have to agree on their own.
#[test]
fn every_ladder_rung_is_scoped_to_the_tab_that_mounts_its_surface() {
    /// `contains`, but a whole identifier — `message_action` must not match
    /// inside `thread_message_action`.
    fn mentions(haystack: &str, needle: &str) -> bool {
        let part = |c: char| c.is_alphanumeric() || c == '_';
        haystack.match_indices(needle).any(|(at, _)| {
            !haystack[..at].chars().next_back().is_some_and(part)
                && !haystack[at + needle.len()..]
                    .chars()
                    .next()
                    .is_some_and(part)
        })
    }

    // 1. THE MOUNT LAYOUT. Every `slot` the shell declares, and the tab arm
    //    that mounts it — `None` for the window-level layers outside the match.
    let shell = include_str!("../ui/components/shell.ice");
    assert_eq!(
        shell
            .lines()
            .filter(|line| line.trim() == "match tab")
            .count(),
        1,
        "one tab match, and this walk found it"
    );
    let arms = shell.split_once("match tab\n").expect("the tab match").1;
    let mut mounted: Vec<(&str, &str)> = Vec::new();
    let mut arm: Option<&str> = None;
    for line in arms.lines() {
        let line = line.trim();
        if let Some(tab) = line.strip_prefix("ShellTab.") {
            arm = Some(tab);
        } else if let Some(slot) = line.strip_prefix("slot ") {
            // A `slot` with no arm above it is past the match — the palette,
            // the bell and the huddle, which ride every screen.
            let Some(tab) = arm.take() else { break };
            mounted.push((slot, tab));
        }
    }
    let declared: Vec<&str> = shell
        .lines()
        .filter_map(|line| line.trim().strip_prefix("slot "))
        .collect();
    assert!(
        mounted.len() > 8 && declared.len() > mounted.len(),
        "the walk found the arms ({}) and the layers outside them ({})",
        mounted.len(),
        declared.len()
    );

    // 2. WHAT EACH SLOT IS HANDED. `view.ice` fills them, and the state a
    //    surface is plumbed is what says which surface a flag belongs to.
    let view = include_str!("../ui/view.ice");
    let filled: Vec<(&str, Option<&str>, String)> = declared
        .iter()
        .map(|slot| {
            let head = format!("\n        {slot}:\n");
            let body = view
                .split_once(&head)
                .unwrap_or_else(|| panic!("`{slot}:` is filled in view.ice"))
                .1
                .lines()
                .take_while(|line| line.trim().is_empty() || line.starts_with("          "))
                .collect::<Vec<_>>()
                .join("\n");
            let tab = mounted
                .iter()
                .find(|(mounted, _)| mounted == slot)
                .map(|(_, tab)| *tab);
            (*slot, tab, body)
        })
        .collect();

    // 3. THE LADDER: its layer flags, its tab predicates, and its rungs.
    let explorer = include_str!("../backend/explorer.rs");
    let ladder = explorer
        .split_once("pub fn topmost_overlay(")
        .expect("the ladder")
        .1;
    let (signature, body) = ladder.split_once(") -> String {").expect("the ladder");
    let body = body.split_once("\n}\n").expect("the ladder ends").0;
    let layers: Vec<&str> = signature
        .lines()
        .filter_map(|line| line.trim().split_once(':'))
        .map(|(name, _)| name)
        .collect();
    let guards: Vec<(&str, &str)> = body
        .lines()
        .filter_map(|line| line.trim().strip_prefix("let "))
        .filter_map(|line| line.split_once(" = shell_tab == crate::ShellTab::"))
        .map(|(name, tab)| (name, tab.trim_end_matches(';')))
        .collect();
    let mut rungs: Vec<(&str, &str)> = Vec::new();
    let mut condition: Option<&str> = None;
    for line in body.lines() {
        let line = line.trim();
        if let Some(open) = line.strip_prefix("if ").and_then(|c| c.strip_suffix(" {")) {
            condition = Some(open);
        } else if let Some(rung) = line
            .strip_prefix("return \"")
            .and_then(|rung| rung.strip_suffix("\".into();"))
            && let Some(open) = condition.take()
        {
            rungs.push((rung, open));
        }
    }
    assert_eq!(
        rungs.len(),
        body.matches("return \"").count(),
        "every rung's condition parsed — one `if <condition> {{` per rung"
    );

    // 4. AND THE THREE HAVE TO AGREE.
    for (rung, condition) in rungs {
        let read: Vec<&&str> = layers
            .iter()
            .filter(|layer| mentions(condition, layer))
            .collect();
        assert!(!read.is_empty(), "rung `{rung}` reads no layer flag");
        let mut tabs: Vec<&str> = Vec::new();
        let mut rides_every_tab = false;
        for layer in read {
            let plumbed: Vec<&(&str, Option<&str>, String)> = filled
                .iter()
                .filter(|(_, _, body)| mentions(body, layer))
                .collect();
            assert!(
                !plumbed.is_empty(),
                "`{layer}` reaches no slot — rung `{rung}` is scoped against nothing"
            );
            for (_, tab, _) in plumbed {
                match tab {
                    Some(tab) => tabs.push(tab),
                    None => rides_every_tab = true,
                }
            }
        }
        let guard = guards
            .iter()
            .find(|(predicate, _)| mentions(condition, predicate));
        // A layer plumbed into a slot outside `match tab` stays on screen
        // across a switch, so its rung must keep answering from every tab.
        if rides_every_tab {
            assert!(
                guard.is_none(),
                "rung `{rung}` is mounted outside the tab match and must not be scoped"
            );
            continue;
        }
        tabs.dedup();
        assert_eq!(
            tabs.len(),
            1,
            "rung `{rung}` reads state from more than one tab's slot: {tabs:?}"
        );
        let (predicate, scoped_to) = guard.unwrap_or_else(|| {
            panic!(
                "rung `{rung}` mounts under `{}` and must be scoped to it",
                tabs[0]
            )
        });
        assert!(
            scoped_to.eq_ignore_ascii_case(tabs[0]),
            "rung `{rung}` is scoped by `{predicate}` to {scoped_to}, but the shell mounts \
             its surface under ShellTab.{}",
            tabs[0]
        );
    }
}

/// A TAB MOVE RETIRES THE MENUS THE TAB IT LEFT OWNED.
///
/// Nothing did: `select_shell_tab` left every menu flag set, which is the whole
/// reason the escape ladder has to be scoped tab by tab (#1132). The scoping
/// stays — a rung must not answer for a surface that is not on screen, however
/// the flag got there — and this is the other half of it: an armed delete
/// confirm that survives a tab round trip is a mouse click away from deleting
/// the page the reader forgot she armed, and a ⋯ menu is not state anyone
/// expects to come back to.
#[test]
fn a_tab_move_retires_the_menu_only_state_of_the_screen_it_left() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.shell_tab = ShellTab::Chat;
    app.selected_message_seq = 4;
    app.selected_message_rev = 2;
    app.message_action = MessageAction::More;
    app.message_edit_draft = "half typed".into();
    app.thread_selected_seq = 7;
    app.thread_selected_rev = 1;
    app.thread_message_action = MessageAction::Editing;
    app.thread_edit_draft = "half typed too".into();
    app.forge_repo_menu = true;
    app.page_delete_armed = true;
    app.fs_delete_target = "/shared/report.md".into();

    let _ = app.__update(__DucktapeMessage::SelectShellTab(ShellTab::Node));

    assert_eq!(app.message_action, MessageAction::Toolbar);
    assert_eq!(app.message_edit_draft, "");
    assert_eq!(app.selected_message_seq, 0);
    assert_eq!(app.selected_message_rev, 0);
    assert_eq!(app.thread_message_action, MessageAction::Toolbar);
    assert_eq!(app.thread_edit_draft, "");
    assert_eq!(app.thread_selected_seq, 0);
    assert_eq!(app.thread_selected_rev, 0);
    assert!(!app.forge_repo_menu);
    assert!(
        !app.page_delete_armed,
        "an armed delete never rides a tab move"
    );
    assert_eq!(app.fs_delete_target, "");

    // The disconnected path returns before the generation bumps, and retires
    // the same set — the clear sits above both early returns, like `error`.
    let (mut app, _) = Ducktape::__boot();
    app.shell_tab = ShellTab::Files;
    app.fs_delete_target = "/shared/report.md".into();
    let _ = app.__update(__DucktapeMessage::SelectShellTab(ShellTab::Chat));
    assert_eq!(app.fs_delete_target, "");
    assert_eq!(app.shell_tab, ShellTab::Chat);

    // AND A RE-SELECT IS NOT A MOVE. The rail emits `select_shell_tab(item.id)`
    // from the seat that is already active, and Settings' rows emit their own
    // tab while the reader is on it — so an unconditional retire is one click
    // from destroying an inline edit on the screen she never left.
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.shell_tab = ShellTab::Chat;
    app.selected_message_seq = 4;
    app.selected_message_rev = 2;
    app.message_action = MessageAction::Editing;
    app.message_edit_draft = "still typing".into();
    let _ = app.__update(__DucktapeMessage::SelectShellTab(ShellTab::Chat));
    assert_eq!(
        app.message_edit_draft, "still typing",
        "clicking the tab you are on retires nothing"
    );
    assert_eq!(app.message_action, MessageAction::Editing);
    assert_eq!(app.selected_message_seq, 4);
    assert_eq!(app.selected_message_rev, 2);
}

/// A RUN IN FLIGHT IS NOT A CAGE.
///
/// The Shell screen used to refuse, while a task or a session was live: the
/// surface switch, both pickers, and the reset — `return if shell_terminal_busy
/// || shell_terminal_running || shell_chat_busy` at the top of every one. That
/// left exactly one state with no exit: a run whose event stream stalls holds
/// `shell_chat_busy` forever, and every control that could have moved the
/// operator off it is disabled by the same flag. The tab is wedged until the
/// app restarts.
///
/// Two properties close it, and both are pinned here: the switch always lands,
/// and `shell_chat_detach` — the operator's way out — is reachable from INSIDE
/// the busy state it exits.
#[test]
fn a_running_turn_never_locks_the_screen_that_started_it() {
    let saga = format!("origin/sched\u{1f}{}", "a".repeat(64));
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.shell_tab = ShellTab::Shell;
    app.shell_provider = "codex".into();
    app.shell_chat_entries =
        backend::agent_chat_push_user(Vec::new(), "do it".into(), "codex".into());
    app.shell_chat_busy = true;
    app.shell_chat_saga = saga.clone();

    let _ = app.__update(__DucktapeMessage::ShellSurfaceChanged(
        ShellSurface::Terminal,
    ));
    assert_eq!(
        app.shell_surface,
        ShellSurface::Terminal,
        "a live task must not pin the operator to the surface that started it"
    );

    let _ = app.__update(__DucktapeMessage::ShellChatDetach);
    assert!(!app.shell_chat_busy, "detaching leaves the busy state");
    assert_eq!(
        app.shell_detached_saga, saga,
        "STOP WATCHING, NOT STOP RUNNING: the turn keeps the id that reaches \
         the still-executing saga"
    );
    let closed = app.shell_chat_entries.last().expect("the turn is closed");
    assert_eq!(closed.status, "detached");
    assert_eq!(closed.saga_id, saga);

    // and discarding it hands the composer back.
    let _ = app.__update(__DucktapeMessage::ShellChatDiscard);
    assert!(app.shell_detached_saga.is_empty());
    assert_eq!(app.shell_chat_entries.len(), 1);
}

/// ONE PICK, BOTH ANSWERS. `--cred` decides the provider, so choosing an
/// identity settles the credential AND the provider, and re-narrows the host
/// list to the peers that announced it — a peer that cannot serve the pick must
/// not survive the change that made it unreachable.
#[test]
fn choosing_an_identity_settles_the_provider_and_re_narrows_the_hosts() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.shell_tab = ShellTab::Shell;
    app.shell_host_nodes = vec![backend::AgentHostNode {
        key: "b".repeat(64),
        label: "bo".into(),
        providers: vec!["codex".into()],
    }];
    app.shell_identities = backend::agent_identities(vec![
        backend::AgentCredential {
            name: "x1".into(),
            provider: "codex".into(),
        },
        backend::AgentCredential {
            name: "c1".into(),
            provider: "claude".into(),
        },
    ]);

    let _ = app.__update(__DucktapeMessage::ShellIdentityChanged("x1 · Codex".into()));
    assert_eq!(app.shell_provider, "codex");
    assert_eq!(app.shell_credential, "x1");
    assert_eq!(app.shell_host_node_options, ["This node", "bo"]);

    let _ = app.__update(__DucktapeMessage::ShellHostNodeChanged("bo".into()));
    assert_eq!(app.shell_host_node_key, "b".repeat(64));

    // bo announces no claude provider, so the run it would bounce is not on
    // offer — and the pick that pointed at it falls back to the local row.
    let _ = app.__update(__DucktapeMessage::ShellIdentityChanged(
        "c1 · Claude Code".into(),
    ));
    assert_eq!(app.shell_provider, "claude");
    assert_eq!(app.shell_host_node_options, ["This node"]);
    assert_eq!(app.shell_host_node, "This node");
    assert_eq!(app.shell_host_node_key, "");
}

/// THE FOUR IDENTITY OPS LAND IN ONE PLACE. `account_changed` is the only
/// handler that re-reads the account for them, and it clears every draft an op
/// consumed — a ticket left on screen after its device joined is a stale blob
/// that looks like a secret, and a create draft after the account exists is a
/// second Create waiting to be refused.
#[test]
fn a_committed_identity_op_rereads_the_account_and_clears_its_drafts() {
    let (mut app, _) = Ducktape::__boot();
    app.connected = true;
    app.connected_rpc = "http://node".into();
    app.account_busy = true;
    app.account_create_draft = "me".into();
    app.account_join_draft = "{}".into();
    app.account_ticket = "{}".into();
    let before = app.account_generation;

    let _ = app.__update(__DucktapeMessage::AccountChanged(true));

    assert!(!app.account_busy, "the op is over");
    assert_eq!(app.account_generation, before + 1, "the account is re-read");
    assert!(app.account_create_draft.is_empty());
    assert!(app.account_join_draft.is_empty());
    assert!(app.account_key_draft.is_empty());
    assert!(app.account_key_label_draft.is_empty());
    assert!(app.account_ticket.is_empty());
}

/// THE BROWSER CEREMONIES ARE WIRED LIKE THE PASTED OPS: each button emits
/// its own signal, each handler runs its backend fn on the connected chain
/// under the signing seat, and every one lands in `account_changed` /
/// `account_op_failed` — the one pair that re-reads the account and frees
/// the card. And each is offered only where consensus would accept it:
/// registering/linking with an account, logging in without one.
#[test]
fn the_browser_ceremonies_land_where_the_pasted_ops_do() {
    let settings = include_str!("../ui/screens/settings.ice");
    let roster = include_str!("../ui/handlers/roster.ice");
    for (button, signal, backend) in [
        (
            "In this browser",
            "account_passkey_desktop",
            "register_passkey",
        ),
        ("Link a wallet", "account_wallet_submit", "link_wallet"),
        (
            "Log in with a passkey",
            "account_login_submit",
            "login_with_passkey",
        ),
    ] {
        assert!(
            settings.contains(&format!(r#"button "{button}" -> emit({signal})"#)),
            "{button} emits {signal}"
        );
        let handler = roster
            .split(&format!("\non {signal}\n"))
            .nth(1)
            .unwrap_or_else(|| panic!("a handler for {signal}"))
            .split("\non ")
            .next()
            .unwrap();
        assert!(
            handler.contains(&format!(
                "run every {backend}(connected_rpc, password, network_chain_id"
            )),
            "{signal} runs {backend} on the connected chain"
        );
        assert!(
            handler.contains("-> account_changed _ | account_op_failed _"),
            "{signal} lands where the pasted ops do"
        );
        assert!(
            handler.contains("empty(password)"),
            "{signal} needs the signing seat"
        );
    }
    let login = settings
        .find(r#"button "Log in with a passkey""#)
        .expect("the login button");
    let last_gate = |upto: usize| {
        let with = settings[..upto].rfind("if account_exists");
        let without = settings[..upto].rfind("if !account_exists");
        (with, without)
    };
    let (with, without) = last_gate(login);
    assert!(
        without > with,
        "login is offered only while there is no account"
    );
    for button in ["On your phone", "In this browser", "Link a wallet"] {
        let at = settings
            .find(&format!(r#"button "{button}""#))
            .expect(button);
        let (with, without) = last_gate(at);
        assert!(with > without, "{button} is offered only with an account");
    }
}

/// A MINTED TICKET COMMITS NOTHING: it is shown to copy, the inputs that
/// produced it clear, and the account is NOT re-read — the other device's
/// join is what moves it.
#[test]
fn a_minted_ticket_is_shown_and_consumes_its_drafts_without_a_reread() {
    let (mut app, _) = Ducktape::__boot();
    app.account_busy = true;
    app.account_key_draft = "ab".into();
    app.account_key_label_draft = "phone".into();
    let before = app.account_generation;

    let _ = app.__update(__DucktapeMessage::AccountTicketMinted(
        r#"{"add_key":{}}"#.into(),
    ));

    assert!(!app.account_busy);
    assert_eq!(app.account_ticket, r#"{"add_key":{}}"#);
    assert!(app.account_key_draft.is_empty());
    assert!(app.account_key_label_draft.is_empty());
    assert_eq!(app.account_generation, before, "minting re-reads nothing");
}

/// THE CARD OFFERS ONLY WHAT CONSENSUS WOULD ACCEPT: founding only while there
/// is no account, and never the removal of the last key (the module refuses
/// it, and a button that always refuses is a lie).
#[test]
fn the_account_card_gates_founding_and_the_last_key() {
    let settings = include_str!("../ui/screens/settings.ice");
    let create = settings.find("#account-create").expect("the create input");
    assert!(
        settings[..create].rfind("if !account_exists").is_some(),
        "founding is offered only while there is no account"
    );
    let remove = settings
        .split_once(r#"button "Remove" -> emit(account_key_remove, row.pubkey)"#)
        .expect("the remove button")
        .1;
    let gate_line = remove
        .split_once("disabled=")
        .expect("its gate")
        .1
        .lines()
        .next()
        .unwrap_or_default();
    assert!(
        gate_line.contains("account_keys <= 1"),
        "the last key is never offered for removal: {gate_line}"
    );
}

/// THE COLUMN'S THREE WIDTHS, and the band the open one has to land in.
///
/// `max-w` is the whole space rule for the huddle column: view.ice's two arms
/// decide what is DRAWN, and this decides whether the cell takes any of the
/// row at all. A non-zero answer with nothing to draw is a dead gutter beside
/// every module; a pill-sized answer is what gives the room back when the card
/// is folded, which a 28% column holding one pill would not.
#[test]
fn the_huddle_column_closes_folds_and_opens() {
    use crate::backend::huddle_dock_width;

    let closed = [
        ("no huddle", huddle_dock_width(false, false, false)),
        ("no huddle, folded", huddle_dock_width(false, false, true)),
        ("popped out", huddle_dock_width(true, true, false)),
        ("popped out, folded", huddle_dock_width(true, true, true)),
    ];
    for (state, width) in closed {
        assert_eq!(width, 0.0, "{state}: the module gets the whole row back");
    }

    let folded = huddle_dock_width(true, false, true);
    let open = huddle_dock_width(true, false, false);
    assert!(
        folded > 0.0 && folded < open,
        "folded is a pill's width, not a column's: {folded} vs {open}"
    );
    assert_eq!(open, 420.0, "the upper end of the specified 280-420 band");

    // AND THE LOWER END IS ARITHMETIC. The column takes `fill(2)` of what the
    // nav rail leaves, against the content box's `fill(5)`, so the narrowest
    // it is ever asked to be is set by the console's own minimum width — which
    // is pinned here, because that is the number the band rests on.
    let app = include_str!("../ui/app.ice");
    assert!(
        app.contains("min-size 1040 540"),
        "the console's minimum is what puts a floor under the column's portion"
    );
    const RAIL_AND_ITS_RULE: f64 = 75.0;
    let narrowest = (1040.0 - RAIL_AND_ITS_RULE) * 2.0 / 7.0;
    assert!(
        narrowest > 260.0 && narrowest < open,
        "the smallest console still hands the huddle a usable column: {narrowest}"
    );
}
