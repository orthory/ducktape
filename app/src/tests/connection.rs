use super::*;
use quote::ToTokens;
use syn::visit::Visit;

pub(super) const CHAT: &str = include_str!("../../../crates/views/chat/src/ui/chat.rs");
const PAGES: &str = include_str!("../../../crates/views/pages/src/ui/pages.rs");
const EXPLORER: &str = include_str!("../../../crates/views/explorer/src/lib.rs");

pub(super) fn branches(source: &str) -> Vec<(String, String, usize)> {
    struct Branches {
        depth: usize,
        rows: Vec<(String, String, usize)>,
    }
    fn tokens(value: &impl ToTokens) -> String {
        value
            .to_token_stream()
            .to_string()
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect()
    }
    impl<'ast> Visit<'ast> for Branches {
        fn visit_expr_if(&mut self, branch: &'ast syn::ExprIf) {
            self.rows.push((
                tokens(&branch.cond),
                tokens(&branch.then_branch),
                self.depth,
            ));
            self.depth += 1;
            syn::visit::visit_expr_if(self, branch);
            self.depth -= 1;
        }
    }
    let mut visitor = Branches {
        depth: 0,
        rows: Vec::new(),
    };
    visitor.visit_file(&syn::parse_file(source).expect("authored Rust view"));
    visitor.rows
}

/// THE ZERO-HIT PLATE SPEAKS FOR A QUERY, AND A BOOL COULD NOT CARRY ONE —
/// page search is enter-to-submit with no `change=` route, so a keystroke runs
/// no handler and only `trim(draft) == query` can retire the plate (the full
/// rationale lives on the plate arm in the view's own `pages.ice`). The query's
/// lifetime is the pages view's own state now; what the app still pins is the
/// SHAPE of the arm that reads it, on both surfaces that render a page hit.
#[test]
fn the_zero_hit_plate_speaks_for_the_query_it_was_sent() {
    let branches = branches(PAGES);
    let (condition, plate, _) = branches
        .iter()
        .find(|(condition, _, _)| condition.contains("search_answer_stands("))
        .expect("standing-query plate");
    for field in [
        "connected",
        "page_search_hits",
        "page_search_query",
        "page_search_draft",
        "page_searching",
    ] {
        assert!(
            condition.contains(field),
            "{field} participates in the plate guard"
        );
    }
    assert!(plate.contains("Nopagesmatched"));
    assert!(
        plate.contains("Background::Color"),
        "the plate paints a background over the document"
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
    let branches = branches(PAGES);
    let plate = branches
        .iter()
        .position(|(condition, _, _)| condition.contains("search_answer_stands("))
        .unwrap();
    let depth = branches[plate].2;
    let document = branches
        .iter()
        .enumerate()
        .find(|(_, (condition, body, at))| {
            *at == depth && condition.contains("active_page") && body.contains("Node::Editor")
        })
        .map(|(index, _)| index)
        .expect("document sibling");
    let hits = branches
        .iter()
        .enumerate()
        .find(|(_, (condition, body, at))| {
            *at == depth
                && condition.contains("page_search_hits")
                && !condition.contains("search_answer_stands")
                && body.contains("Node::Button")
        })
        .map(|(index, _)| index)
        .expect("result sibling");
    assert!(
        document < plate && document < hits,
        "search layers paint above the document"
    );
    assert!(branches[hits].1.contains("Background::Color"));
}

/// THE EXPLORER'S PLATE SPEAKS FOR THE QUERY IT WAS SENT — the same class the
/// pages plate above was fixed for, on the last surface that still keyed its
/// zero-hit sentence on the LIVE draft. Workspace search is enter-to-submit and
/// two-way bound with no `change=` route, so a keystroke after a zero-hit answer
/// runs no handler at all: only `trim(query) == sent_query` can retire the
/// plate, and the captured string is the only thing that can carry the
/// comparison. The Explorer is a module-owned view on the kernel contract, so
/// the capture, the send and the arm are all the guest's.
#[test]
fn the_explorer_plate_speaks_for_the_query_it_was_sent() {
    let source = rust_tokens(EXPLORER);
    assert!(
        source.contains("let__ice_next=(self.query).trim().to_owned();self.sent_query=__ice_next")
    );
    assert!(source.contains("workspace_search(") && source.contains("self.sent_query.clone()"));
    let condition = branches(EXPLORER)
        .into_iter()
        .find(|(condition, _, _)| condition.contains("search_answer_stands("))
        .unwrap()
        .0;
    for field in [
        "connected",
        "hits",
        "partial",
        "sent_query",
        "query",
        "searching",
    ] {
        assert!(condition.contains(field));
    }
    assert!(source.contains("let__ice_next=\"\".to_owned();self.sent_query=__ice_next"));
}

/// ONE PREDICATE, THREE SURFACES. Pages, chat and the explorer each grew their
/// own copy of the same conjunct arm, and a fourth surface would have grown a
/// fourth; the arithmetic lives in one place now, and the three arms call it.
#[test]
fn one_predicate_decides_whether_a_search_answer_still_stands() {
    // The answer speaks for the string it was sent for — trimmed, because that
    // is what was sent.
    assert!(backend::search_answer_stands("zzz", "  zzz  ", false));
    // ONE MORE CHARACTER AND IT DOES NOT. No handler ran; only this comparison
    // can tell.
    assert!(!backend::search_answer_stands("zzz", "zzzq", false));
    // A ROUND TRIP IS NOT AN ANSWER — the submit's own search is still out.
    assert!(!backend::search_answer_stands("zzz", "zzz", true));
    // AND AN EMPTY QUERY IS NO ANSWER AT ALL, which is what every dismissal
    // leaves behind: an emptied box must not match an emptied query.
    assert!(!backend::search_answer_stands("", "", false));

    for source in [PAGES, CHAT, EXPLORER] {
        assert!(
            branches(source)
                .iter()
                .any(|(condition, _, _)| condition.contains("search_answer_stands("))
        );
    }
}

/// A FAILED PALETTE SEARCH MUST SAY SO. `palette_search_failed` returns the
/// phase to idle and clears the hits, and idle under a live draft is reachable
/// no other way — so the panel needs an arm for exactly that pair, and the arm
/// is the palette's ONLY possible word about the failure (the rationale for
/// why no `error` assignment could speak from behind the scrim lives on the
/// arm itself).
#[test]
fn the_palette_says_so_when_a_search_fails() {
    let shell = rust_tokens(include_str!("../shell.rs"));
    assert!(shell.contains("SearchPhase::Idle") && shell.contains("Searchfailed."));
    assert!(shell.contains("!query.trim().is_empty()"));
    let handler = handler_body("PaletteSearchFailed");
    assert!(
        !handler.contains("self.error="),
        "the report belongs inside the visible palette"
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

    let _ = app.__update(__DucktapeMessage::ChatLoadFailed(backend::HydrationError {
        generation: app.chat_generation,
        message: "the channel did not load".into(),
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
    let native = rust_tokens(include_str!("../ui/native_view.rs"));
    for name in [
        "chat",
        "pages",
        "files",
        "forge",
        "members",
        "governance",
        "explorer",
        "settings",
        "node",
    ] {
        assert!(
            native.contains(&format!("\"{name}\"")),
            "{name} stays a dynamically loaded module view"
        );
    }
    for source in [
        CHAT,
        PAGES,
        include_str!("../../../crates/views/forge/src/ui/forge.rs"),
    ] {
        assert!(
            branches(source)
                .iter()
                .any(|(condition, body, _)| condition.contains("!self.connected")
                    && body.contains("Notconnected")),
            "a disconnected data view says why it cannot show a reading"
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
/// itself (a header subtitle that folds its rows under a `connected` gate is
/// already honest).
#[test]
fn a_disconnected_screen_stands_its_registers_down_too() {
    use std::collections::{BTreeMap, BTreeSet};
    const LISTS: &[&str] = &[
        "repos",
        "branches",
        "items",
        "forge_item_blocks",
        "diff_rows",
        "forge_item_reviews",
        "discussion",
        "linked_note",
        "merge_conflicts",
        "staged_comments",
        "tree_entries",
    ];
    fn self_field(expr: &syn::Expr, name: &str) -> bool {
        match expr {
            syn::Expr::Paren(expr) => self_field(&expr.expr, name),
            syn::Expr::Field(field) => {
                matches!(&*field.base,syn::Expr::Path(path) if path.path.is_ident("self"))
                    && matches!(&field.member,syn::Member::Named(member) if member == name)
            }
            _ => false,
        }
    }
    fn connected(expr: &syn::Expr) -> bool {
        match expr {
            syn::Expr::Paren(expr) => connected(&expr.expr),
            syn::Expr::Binary(expr) => match expr.op {
                syn::BinOp::And(_) => connected(&expr.left) || connected(&expr.right),
                syn::BinOp::Or(_) => connected(&expr.left) && connected(&expr.right),
                _ => false,
            },
            _ => self_field(expr, "connected"),
        }
    }
    #[derive(Default)]
    struct Reading {
        gated: bool,
        reads: bool,
        calls: BTreeSet<String>,
    }
    impl<'ast> Visit<'ast> for Reading {
        fn visit_expr_if(&mut self, expr: &'ast syn::ExprIf) {
            let outer = self.gated;
            self.gated = outer || connected(&expr.cond);
            self.visit_expr(&expr.cond);
            self.visit_block(&expr.then_branch);
            self.gated = outer;
            if let Some((_, otherwise)) = &expr.else_branch {
                self.visit_expr(otherwise);
            }
        }
        fn visit_expr_field(&mut self, expr: &'ast syn::ExprField) {
            let tracked = matches!(&*expr.base,syn::Expr::Path(path) if path.path.is_ident("self"))
                && matches!(&expr.member,syn::Member::Named(field) if LISTS.iter().any(|name| field == name));
            if tracked && !self.gated {
                self.reads = true;
            }
            syn::visit::visit_expr_field(self, expr);
        }
        fn visit_expr_method_call(&mut self, expr: &'ast syn::ExprMethodCall) {
            let local =
                matches!(&*expr.receiver,syn::Expr::Path(path) if path.path.is_ident("self"));
            if local && !self.gated {
                self.calls.insert(expr.method.to_string());
            }
            syn::visit::visit_expr_method_call(self, expr);
        }
        fn visit_expr_call(&mut self, expr: &'ast syn::ExprCall) {
            // A header's summary explicitly takes connected and folds its own
            // claim, just as a branch gates a list of rendered rows.
            if expr.args.iter().any(|arg| self_field(arg, "connected")) {
                return;
            }
            syn::visit::visit_expr_call(self, expr);
        }
    }
    #[derive(Default)]
    struct Methods(BTreeMap<String, Reading>);
    impl<'ast> Visit<'ast> for Methods {
        fn visit_impl_item_fn(&mut self, method: &'ast syn::ImplItemFn) {
            let mut reading = Reading::default();
            reading.visit_block(&method.block);
            self.0.insert(method.sig.ident.to_string(), reading);
        }
    }
    let mut methods = Methods::default();
    for source in [
        include_str!("../../../crates/views/forge/src/ui/app_view.rs"),
        include_str!("../../../crates/views/forge/src/ui/forge.rs"),
        include_str!("../../../crates/views/forge/src/ui/components.rs"),
        include_str!("../../../crates/views/forge/src/ui/kit.rs"),
        include_str!("../../../crates/views/forge/src/ui/icon.rs"),
    ] {
        methods.visit_file(&syn::parse_file(source).unwrap());
    }
    let mut requires_connection: BTreeSet<_> = methods
        .0
        .iter()
        .filter(|(name, reading)| name.as_str() != "__view" && reading.reads)
        .map(|(name, _)| name.clone())
        .collect();
    assert!(
        !requires_connection.is_empty(),
        "the sweep sees actual register-reading components"
    );
    loop {
        let inherited: Vec<_> = methods
            .0
            .iter()
            .filter(|(name, reading)| {
                !requires_connection.contains(*name)
                    && reading
                        .calls
                        .iter()
                        .any(|call| requires_connection.contains(call))
            })
            .map(|(name, _)| name.clone())
            .collect();
        if inherited.is_empty() {
            break;
        }
        requires_connection.extend(inherited);
    }
    assert!(
        !requires_connection.contains("__view"),
        "a disconnected entrypoint reaches an ungated register-reading component"
    );
}

/// AND THE HEADER SUBTITLES ARE CLAIMS TOO — the subtler half. `Agents 0 agents ·
/// 0 working` is a measured zero about a register nobody read, and it sits ABOVE
/// the body arm above, so it survives it. Every subtitle fold takes `connected`
/// as its first argument and returns "" without it; this pins that no call site
/// can quietly drop the guard.
#[test]
fn every_header_subtitle_is_gated_on_the_connection() {
    for source in [
        include_str!("../../../crates/views/forge/src/ui/forge.rs"),
        include_str!("../../../crates/views/members/src/lib.rs"),
    ] {
        let source = rust_tokens(source);
        for (_, tail) in source
            .match_indices("_summary(")
            .map(|(at, _)| (at, &source[at..]))
        {
            let arguments = tail.split(')').next().unwrap();
            assert!(
                arguments.contains("connected"),
                "summary must distinguish no answer from measured zero"
            );
        }
    }
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

    let failure = handler_body("ConnectFailed");
    assert!(failure.contains("self.hydration_retry_attempt=(self.hydration_retry_attempt+1)"));
    assert!(
        failure.contains("crate::backend::connect(") && failure.contains("self.connect_generation")
    );
    let connected = handler_body("WorkspaceConnected");
    assert!(connected.contains("next.generation!=self.connect_generation"));
    assert!(!connected.contains("next.generation!=self.hydration_generation"));
    assert!(!handler_bodies().iter().any(|(name, _)| name == "Failed"));
}

/// THE ZERO-HIT SEARCH CARD MUST STAY DISMISSABLE. The Clear-search × used to
/// gate on `!empty(search_hits)`, but the float itself opens on
/// `search_phase != SearchPhase.idle` — so a `done && empty(search_hits)` search drew
/// "No messages match" with no way to close it: not the × (hidden), not
/// Escape (`chat-search` carries no `escape_target` layer), not re-pressing
/// Enter (lands `done`+empty again), not clearing the field (the submit
/// handler returns early on an empty query, leaving the phase untouched).
/// Only a channel/DM switch or a reconnect ever wrote "idle" again. The ×
/// must therefore never be gated more narrowly than the float. It is gated
/// WIDER: the float's own discriminant OR a live field. Every state that
/// raises the float is covered — searching and done are both `!= idle`, and an
/// answer that still stands for the box means the box is not empty.
#[test]
fn the_clear_search_button_survives_a_zero_hit_result() {
    let branches = branches(CHAT);
    let control = branches
        .iter()
        .find(|(condition, body, _)| {
            condition.contains("search_phase")
                && condition.contains("search_draft")
                && body.contains("ClearSearch")
        })
        .expect("clear-search gate");
    assert!(control.0.contains("SearchPhase::Idle"));
    assert!(
        !control.0.contains("search_hits"),
        "the clear control cannot disappear with zero hits"
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
