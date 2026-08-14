use super::*;

/// THE ZERO-HIT PLATE SPEAKS FOR A QUERY, AND A BOOL COULD NOT CARRY ONE —
/// page search is enter-to-submit with no `change=` route, so a keystroke runs
/// no handler and only `trim(draft) == query` can retire the plate (the full
/// rationale lives on the plate arm in `screens/pages.ice`). This test walks
/// the query's whole lifetime: captured at submit, standing through a zero-hit
/// answer, abandoned by the draft, dropped by navigation and by failure.
#[test]
fn the_zero_hit_plate_speaks_for_the_query_it_was_sent() {
    // Draft -> submit -> empty answer: the state a standing plate reads.
    let answered_pages = |draft: &str| {
        let (mut app, _) = Ducktape::__boot();
        app.connected = true;
        app.loading = false;
        app.page_search_draft = draft.into();
        // A DRAFT IS NOT A QUERY: typing alone runs nothing and captures
        // nothing, which is why the plate cannot fire on the first keystroke.
        assert!(app.page_search_query.is_empty());
        assert!(!app.page_searching);
        let _ = app.__update(__DucktapeMessage::SearchPagesSubmit);
        assert!(app.page_searching, "the round trip is not an answer either");
        let _ = app.__update(__DucktapeMessage::PageSearchLoaded(
            backend::PageSearchData { hits: vec![] },
        ));
        app
    };

    // The submit captures the TRIMMED query — the same string the node is
    // asked about — and the empty answer leaves it standing. All five of the
    // plate arm's terms hold jointly in this state.
    let mut pages = answered_pages("  zzz  ");
    assert!(pages.connected);
    assert!(pages.page_search_hits.is_empty());
    assert!(!pages.page_searching);
    assert!(!pages.page_search_query.is_empty());
    assert_eq!(pages.page_search_draft.trim(), pages.page_search_query);
    assert_eq!(pages.page_search_query, "zzz");

    // THE CLASS THE BOOL COULD NOT COVER: one more character runs no handler,
    // so the query stays put while the draft walks away from it, and the arm
    // stops matching without anything having been told.
    pages.page_search_draft = "zzzq".into();
    assert_eq!(pages.page_search_query, "zzz");
    assert_ne!(pages.page_search_draft.trim(), pages.page_search_query);

    // Every handler that drops the hits drops the query with them.
    for leaving in [
        __DucktapeMessage::OpenPageSearchHit("page".into(), "block".into()),
        __DucktapeMessage::ChoosePage("next".into()),
        __DucktapeMessage::ClearPageSearch,
    ] {
        let mut app = answered_pages("zzz");
        assert_eq!(app.page_search_query, "zzz");
        let _ = app.__update(leaving);
        assert!(
            app.page_search_query.is_empty(),
            "opening a hit or navigating must not leave the plate standing"
        );
    }

    // A FAILED search never ran, so it found nothing in no sense the plate may
    // report: the query goes, and `error` carries the cause instead.
    let (mut failed, _) = Ducktape::__boot();
    failed.loading = false;
    failed.page_search_draft = "zzz".into();
    let _ = failed.__update(__DucktapeMessage::SearchPagesSubmit);
    let _ = failed.__update(__DucktapeMessage::PageSearchFailed(backend::AppError {
        message: "node refused".into(),
        committed: false,
    }));
    assert!(!failed.page_searching);
    assert!(failed.page_search_query.is_empty());
    assert_eq!(failed.error, "node refused");

    // AN EMPTY QUERY GATES THE REPLY HANDLERS: no search is standing, so a
    // reply the dismissal could not invalidate (`close_doc_tab` rides a
    // decision no lane invalidate can) is dropped on arrival instead of
    // resurrecting the float and clobbering `error`.
    let (mut dismissed, _) = Ducktape::__boot();
    dismissed.error = "standing error".into();
    let _ = dismissed.__update(__DucktapeMessage::PageSearchLoaded(
        backend::PageSearchData {
            hits: vec![stale_page_hit()],
        },
    ));
    assert!(
        dismissed.page_search_hits.is_empty(),
        "a reply with no standing query must not restore the hits float"
    );
    assert_eq!(dismissed.error, "standing error", "nor clobber the banner");
    let _ = dismissed.__update(__DucktapeMessage::PageSearchFailed(backend::AppError {
        message: "late failure".into(),
        committed: false,
    }));
    assert_eq!(
        dismissed.error, "standing error",
        "a failure nobody is waiting on must not raise a banner"
    );

    // THE ARM. The plate may not be keyed on a flag, and may not fire during
    // the round trip its own submit opened.
    let pages_screen = inlined(include_str!("../ui/screens/pages.ice"));
    assert!(pages_screen.contains(
        "if connected && empty(page_search_hits) && !page_searching && !empty(page_search_query) && trim(page_search_draft) == page_search_query"
    ));
    let overlays = inlined(include_str!("../ui/screens/overlays.ice"));
    assert!(overlays
        .contains("if search_phase == SearchPhase.done && empty(chat_hits) && empty(page_hits)"));

    // THE PLATE IS OPAQUE. It is a sibling stack LAYER — over the live document
    // or over "No page selected" — and `EmptyPlate` is `bg=transparent`, so
    // what it denies would render straight through the sentence denying it.
    let card = pages_screen
        .split("if connected && empty(page_search_hits) && !page_searching")
        .nth(1)
        .expect("the zero-hit arm");
    let card = &card[..card.find("No pages matched").expect("the plate's message")];
    assert!(
        card.contains("bg=elevated"),
        "a zero-hit plate must not be a transparent layer"
    );
}

/// THE PLATE MUST HAVE A SEAT IN THE STATE THAT MOST NEEDS IT, AND IT MUST SIT
/// ON TOP. Nested inside `connected && !empty(active_page)` — where the whole
/// document header including the search input lives — the panel had no answer
/// for its one real arrival with no page open: `live_resynced` moving
/// `active_page` to "" under a STANDING query. Nested, that state showed "No
/// page selected" and said nothing about the query; the × is gone with the
/// header there, so picking a page would be the only exit. Hoisted to a
/// sibling layer it must be declared AFTER the document arm: a stack paints in
/// declaration order, first at the bottom, so an earlier position puts the
/// opaque card UNDER the document it is supposed to cover.
#[test]
fn the_zero_hit_plates_sit_where_the_answer_is_needed() {
    let pages_screen = inlined(include_str!("../ui/screens/pages.ice"));
    // Both needles carry the SAME ten-space indent, and the indent is the
    // sibling pin: re-nesting the plate inside the document arm deepens its
    // indent and its needle stops matching, exactly as hoisting the document
    // arm would break its own.
    let document = pages_screen
        .find("\n          if connected && !empty(active_page)\n")
        .expect("the document arm, as a stack layer");
    let plate = pages_screen
        .find("\n          if connected && empty(page_search_hits)")
        .expect("the pages zero-hit arm, as a SIBLING stack layer");
    assert!(
        document < plate,
        "the pages plate must be declared AFTER the document arm: a stack draws \
         its layers in declaration order, first at the BOTTOM, so an earlier plate \
         is painted UNDER the document it is supposed to cover"
    );
}

/// A NAVIGATION DISMISSES THE WHOLE ANSWER, NOT HALF OF IT. `channel_created`
/// and `pages_mutated` land you somewhere new exactly the way the pickers do,
/// and `close_doc_tab` does when — and only when — it closes the ACTIVE tab;
/// each must take the hits and the standing answer with it (pages: the query;
/// chat: the phase back to idle), or the results float — the one that actually
/// occludes the room or page you just landed in — travels along.
///
/// This is a DISMISSAL POLICY, not a truth requirement: both searches pass an
/// empty scope and are workspace-wide, so the answer would still be true where
/// you landed. The reason to drop it is that it is in the way.
#[test]
fn the_three_navigation_resets_take_the_hits_and_the_answer() {
    let pages_mutated = || {
        __DucktapeMessage::PagesMutated(backend::PagesData {
            pages: Vec::new(),
            blocks: Vec::new(),
            active_page: "fresh".into(),
            active_page_title: "Fresh".into(),
            active_page_parent: String::new(),
            comment_thread_total: 0,
            commented_block_hits: Vec::new(),
        })
    };

    // A CREATE LANDS YOU IN THE NEW CHANNEL — the same dismissal
    // `choose_channel` and `choose_dm` already perform: the lane invalidate
    // dropped any reply in flight, so nothing else would ever move the phase
    // again, and the phase goes idle with the hits.
    let (mut created, _) = Ducktape::__boot();
    created.loading = false;
    created.chat_search_hits = vec![stale_chat_hit()];
    created.chat_search_phase = SearchPhase::Searching;
    let _ = created.__update(__DucktapeMessage::ChannelCreated(chat_data(
        "fresh",
        Vec::new(),
    )));
    assert!(created.chat_search_hits.is_empty());
    assert_eq!(
        created.chat_search_phase,
        SearchPhase::Idle,
        "the invalidated lane drops the reply; the reset must move the phase"
    );

    let (mut mutated, _) = Ducktape::__boot();
    mutated.loading = false;
    mutated.page_search_query = "zzz".into();
    mutated.page_search_hits = vec![stale_page_hit()];
    mutated.page_searching = true;
    let _ = mutated.__update(pages_mutated());
    assert!(mutated.page_search_query.is_empty());
    assert!(mutated.page_search_hits.is_empty());
    assert!(
        !mutated.page_searching,
        "the invalidated lane drops the reply; the reset must lower the flag"
    );

    // CLOSING THE ACTIVE TAB LANDS YOU ELSEWHERE — a navigation, so it resets.
    let (mut active, _) = Ducktape::__boot();
    active.loading = false;
    active.doc_tabs = vec!["open".into(), "other".into()];
    active.active_page = "open".into();
    active.page_search_query = "zzz".into();
    active.page_search_hits = vec![stale_page_hit()];
    active.page_searching = true;
    let _ = active.__update(__DucktapeMessage::CloseDocTab("open".into()));
    assert_eq!(active.active_page, "other");
    assert!(active.page_search_query.is_empty());
    assert!(active.page_search_hits.is_empty());
    assert!(!active.page_searching);

    // CLOSING A BACKGROUND TAB DOES NOT. `next_doc_tab` returns `active`
    // unchanged when the closed tab is not the active one, and an
    // unconditional reset here would dismiss a truthful plate the user is
    // still reading. The reset rides that same decision.
    let (mut background, _) = Ducktape::__boot();
    background.loading = false;
    background.doc_tabs = vec!["open".into(), "other".into()];
    background.active_page = "other".into();
    background.page_search_query = "zzz".into();
    background.page_search_hits = vec![stale_page_hit()];
    background.page_searching = true;
    let _ = background.__update(__DucktapeMessage::CloseDocTab("open".into()));
    assert_eq!(background.active_page, "other");
    assert!(
        background.page_searching,
        "the reply still lands and lowers it — the query it answers is standing"
    );
    assert_eq!(
        background.page_search_query, "zzz",
        "closing a background tab navigates nowhere and must not dismiss the answer"
    );
    assert_eq!(background.page_search_hits.len(), 1);
}

/// A FAILED PALETTE SEARCH MUST SAY SO. `palette_search_failed` returns the
/// phase to idle and clears the hits, and idle under a live draft is reachable
/// no other way — so the panel needs an arm for exactly that pair, and the arm
/// is the palette's ONLY possible word about the failure (the rationale for
/// why no `error` assignment could speak from behind the scrim lives on the
/// arm itself).
#[test]
fn the_palette_says_so_when_a_search_fails() {
    let overlays = inlined(include_str!("../ui/screens/overlays.ice"));
    let failure = overlays
        .find("if search_phase == SearchPhase.idle && !empty(trim(query))")
        .expect("the palette's failure arm");
    // Bounded at the next sibling arm, so a "Search failed." that migrated
    // anywhere else in the file cannot satisfy this.
    let arm = &overlays[failure..];
    let arm = &arm[..arm
        .find("if !empty(chat_hits) || !empty(page_hits)")
        .unwrap_or(arm.len())];
    assert!(
        arm.contains("Search failed."),
        "a palette search that never ran must not render as a bare input"
    );
    // And the arm is not rescued by an error the scrim hides: the handler
    // deliberately sets none. The slice runs to the next handler header, so an
    // inserted blank line cannot shrink what this lint reads.
    let handler = include_str!("../ui/handlers/overlays.ice")
        .split("on palette_search_failed(cause)")
        .nth(1)
        .expect("the failure handler");
    let handler = &handler[..handler.find("\non ").unwrap_or(handler.len())];
    assert!(
        !handler.contains("error ="),
        "the console error banner is behind the palette's scrim; the arm is the report"
    );
}

/// The palette rides the SAME `SearchPhase` discriminant as chat, and honestly:
/// `palette_changed` runs on every keystroke and moves it, so no phase can
/// outlive the draft that earned it and no captured query string is needed.
/// `done` is written only where a result lands — a failure returns to idle
/// instead of claiming a completed empty answer.
#[test]
fn the_palette_does_not_call_a_failed_search_an_empty_one() {
    let (mut app, _) = Ducktape::__boot();
    app.connected_rpc = "http://node".into();
    app.palette_open = true;

    // Typing is not an answer.
    let _ = app.__update(__DucktapeMessage::PaletteChanged("zzz".into()));
    assert_eq!(app.palette_search_phase, SearchPhase::Searching);

    // A search that never ran is not an answer either — and it is the one a
    // bare `!searching` arm would mistake for one.
    app.palette_chat_hits = vec![stale_chat_hit()];
    app.palette_page_hits = vec![stale_page_hit()];
    let _ = app.__update(__DucktapeMessage::PaletteSearchFailed(backend::AppError {
        message: "node refused".into(),
        committed: false,
    }));
    assert_eq!(
        app.palette_search_phase,
        SearchPhase::Idle,
        "a failed palette search must not claim the workspace holds no match"
    );
    // THE RESULTS ARM IS KEYED ON THE HITS ALONE, so hits left standing put
    // "Search failed." directly above live rows read as its results.
    assert!(app.palette_chat_hits.is_empty());
    assert!(app.palette_page_hits.is_empty());

    // An empty result IS one.
    let _ = app.__update(__DucktapeMessage::PaletteChanged("zzz".into()));
    let _ = app.__update(__DucktapeMessage::PaletteResults(
        backend::PaletteSearchData {
            chat_hits: Vec::new(),
            page_hits: Vec::new(),
        },
    ));
    assert_eq!(app.palette_search_phase, SearchPhase::Done);

    // ...and the next keystroke retires it, so the claim never outlives its
    // query.
    let _ = app.__update(__DucktapeMessage::PaletteChanged("zzzz".into()));
    assert_eq!(app.palette_search_phase, SearchPhase::Searching);

    // BACKSPACING TO EMPTY RUNS NO SEARCH, so nothing is coming to replace the
    // rows: `palette_changed` clears them above its early return, or the last
    // query's results sit listed under a blank field forever.
    let _ = app.__update(__DucktapeMessage::PaletteResults(
        backend::PaletteSearchData {
            chat_hits: Vec::new(),
            page_hits: vec![stale_page_hit()],
        },
    ));
    assert_eq!(app.palette_page_hits.len(), 1);
    let _ = app.__update(__DucktapeMessage::PaletteChanged(String::new()));
    assert!(app.palette_page_hits.is_empty());
    assert_eq!(app.palette_search_phase, SearchPhase::Idle);
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
    const LIVE: &str = include_str!("../backend/live.rs");
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
        include_str!("../ui/screens/governance.ice"),
        include_str!("../ui/screens/roster.ice"),
        include_str!("../ui/screens/storage.ice"),
        include_str!("../ui/screens/forge.ice"),
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

/// AND THE HEADER SUBTITLES ARE CLAIMS TOO — the subtler half. `Agents 0 agents ·
/// 0 working` is a measured zero about a register nobody read, and it sits ABOVE
/// the body arm above, so it survives it. Every subtitle fold takes `connected`
/// as its first argument and returns "" without it; this pins that no call site
/// can quietly drop the guard.
#[test]
fn every_header_subtitle_is_gated_on_the_connection() {
    let mut sites: Vec<&str> = SCREENS
        .match_indices("_summary(")
        .map(|(at, _)| {
            let head = SCREENS[..at]
                .rfind(|c: char| !c.is_alphanumeric() && c != '_')
                .map_or(0, |before| before + 1);
            let close = at + SCREENS[at..].find(')').expect("a call site closes");
            &SCREENS[head..=close]
        })
        .collect();
    sites.sort_unstable();
    let mut expected = [
        "proposals_summary(connected, rows)",
        "members_summary(connected, rows)",
        "agents_summary(connected, rows)",
        "members_summary(connected, members_rows)",
        "fs_counts_summary(connected, listed, entries)",
    ];
    expected.sort_unstable();

    assert_eq!(
        sites, expected,
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
    let lifecycle = inlined(include_str!("../ui/handlers/lifecycle.ice"));
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
    let screen = inlined(include_str!("../ui/screens/chat.ice"));
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
