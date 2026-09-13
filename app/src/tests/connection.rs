use super::*;
use quote::ToTokens;
use syn::visit::Visit;

pub(super) const CHAT: &str = include_str!("../../../crates/views/chat/src/ui/chat.rs");
const PAGES: &str = include_str!("../../../crates/views/pages/src/ui/pages.rs");
const EXPLORER: &str = concat!(
    include_str!("../../../crates/views/explorer/src/lib.rs"),
    "\n",
    include_str!("../../../crates/views/explorer/src/presentation.rs"),
);

pub(super) fn branches(source: &str) -> Vec<(String, String, usize)> {
    // Deep authored widget trees exceed the test harness's small thread stack.
    // Bound only this syntax walk, not the application or the whole suite.
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn_scoped(scope, || branches_on_stack(source))
            .unwrap()
            .join()
            .unwrap()
    })
}

fn branches_on_stack(source: &str) -> Vec<(String, String, usize)> {
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
        fn visit_expr_macro(&mut self, node: &'ast syn::ExprMacro) {
            if node.mac.path.is_ident("vec") {
                let tokens = &node.mac.tokens;
                let array: syn::Expr = syn::parse2(quote::quote!([#tokens])).expect("vec items");
                self.visit_expr(&array);
            }
        }
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

fn view_method(source: &str, name: &str) -> String {
    let file = syn::parse_file(source).expect("view Rust");
    file.items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Impl(item) => Some(item),
            _ => None,
        })
        .flat_map(|item| &item.items)
        .find_map(|item| match item {
            syn::ImplItem::Fn(method) if method.sig.ident == name => Some(
                method
                    .block
                    .to_token_stream()
                    .to_string()
                    .chars()
                    .filter(|character| !character.is_whitespace())
                    .collect(),
            ),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing view method {name}"))
}

// A submitted answer belongs to its captured query, not the current draft.
// The guest's wire tests cover clearing an empty answer with no selected page.
#[test]
fn the_zero_hit_plate_speaks_for_the_query_it_was_sent() {
    let pane = view_method(PAGES, "document_pane");
    assert!(pane.contains("letsearch_ready=self.connected&&crate::host::search_answer_stands(&self.page_search_query,&self.page_search_draft,self.page_searching,);"));
    assert!(pane.contains("ifsearch_ready{children.push(self.search_panel());}"));
    let panel = view_method(PAGES, "search_panel");
    assert!(panel.contains("ifself.page_search_hits.is_empty()"));
    assert!(panel.contains("Nomatchingpages"));
    assert!(panel.contains("Message::ClearPageSearch"));
}

#[test]
fn the_zero_hit_plates_sit_where_the_answer_is_needed() {
    let pane = view_method(PAGES, "document_pane");
    let surface = pane
        .find("letmutchildren=vec![surface]")
        .expect("document bottom layer");
    let search = pane
        .find("children.push(self.search_panel())")
        .expect("search layer");
    assert!(surface < search);
    let panel = view_method(PAGES, "search_panel");
    assert!(
        panel.contains("overlay("),
        "search uses the native opaque overlay"
    );
    assert!(
        panel.contains("Message::ClearPageSearch"),
        "overlay can dismiss itself"
    );
}

#[test]
fn the_explorer_plate_speaks_for_the_query_it_was_sent() {
    let source = rust_tokens(EXPLORER);
    assert!(source.contains("self.sent_query=(self.query).trim().to_owned()"));
    assert!(source.contains("host::workspace_search(self.sent_query.clone(),self.search_serial)"));
    let panel = view_method(EXPLORER, "search_results");
    assert!(panel.contains("letempty_answer=hits.is_empty()&&self.partial.is_empty()&&host::search_answer_stands(&self.sent_query,&self.query,self.searching);"));
    assert!(panel.contains("ifempty_answer{"));
    assert!(source.contains("self.sent_query=\"\".to_owned()"));
}

#[test]
fn one_predicate_decides_whether_a_search_answer_still_stands() {
    assert!(backend::search_answer_stands("zzz", "  zzz  ", false));
    assert!(!backend::search_answer_stands("zzz", "zzzq", false));
    assert!(!backend::search_answer_stands("zzz", "zzz", true));
    assert!(!backend::search_answer_stands("", "", false));
    for source in [PAGES, CHAT, EXPLORER] {
        assert!(rust_tokens(source).contains("search_answer_stands("));
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
    let (mut app, _) = Ducktape::boot();
    app.connected_rpc = "http://node".into();
    app.palette_open = true;

    // Typing is not an answer.
    let _ = app.update(AppMessage::PaletteChanged("zzz".into()));
    assert_eq!(app.palette_search_phase, SearchPhase::Searching);

    // A search that never ran is not an answer either — and it is the one a
    // bare `!searching` arm would mistake for one.
    app.palette_chat_hits = vec![stale_chat_hit()];
    app.palette_page_hits = vec![stale_page_hit()];
    let _ = app.update(AppMessage::PaletteSearchFailed(backend::AppError {
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
    let _ = app.update(AppMessage::PaletteChanged("zzz".into()));
    let _ = app.update(AppMessage::PaletteResults(backend::PaletteSearchData {
        chat_hits: Vec::new(),
        page_hits: Vec::new(),
    }));
    assert_eq!(app.palette_search_phase, SearchPhase::Done);

    // ...and the next keystroke retires it, so the claim never outlives its
    // query.
    let _ = app.update(AppMessage::PaletteChanged("zzzz".into()));
    assert_eq!(app.palette_search_phase, SearchPhase::Searching);

    // BACKSPACING TO EMPTY RUNS NO SEARCH, so nothing is coming to replace the
    // rows: `palette_changed` clears them above its early return, or the last
    // query's results sit listed under a blank field forever.
    let _ = app.update(AppMessage::PaletteResults(backend::PaletteSearchData {
        chat_hits: Vec::new(),
        page_hits: vec![stale_page_hit()],
    }));
    assert_eq!(app.palette_page_hits.len(), 1);
    let _ = app.update(AppMessage::PaletteChanged(String::new()));
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
    let (mut app, _) = Ducktape::boot();
    app.connected = true;
    app.loading = true;
    app.status = "Live".into();

    let _ = app.update(AppMessage::ChatLoadFailed(backend::HydrationError {
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
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(check_disconnected_screens)
        .unwrap()
        .join()
        .unwrap();
}

fn check_disconnected_screens() {
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
            native.contains(&format!("::{name}_view(")),
            "{name} stays a dynamically loaded module view"
        );
    }
    struct Components(Vec<(String, String)>);
    impl<'ast> Visit<'ast> for Components {
        fn visit_impl_item_fn(&mut self, method: &'ast syn::ImplItemFn) {
            self.0.push((
                method.sig.ident.to_string(),
                method
                    .block
                    .to_token_stream()
                    .to_string()
                    .chars()
                    .filter(|c| !c.is_whitespace())
                    .collect(),
            ));
        }
    }
    for (source, kit) in [
        (
            CHAT,
            include_str!("../../../crates/views/chat/src/ui/kit.rs"),
        ),
        (
            PAGES,
            include_str!("../../../crates/views/pages/src/ui/kit.rs"),
        ),
        (
            include_str!("../../../crates/views/forge/src/ui/forge.rs"),
            include_str!("../../../crates/views/forge/src/ui/kit.rs"),
        ),
    ] {
        let mut components = Components(Vec::new());
        components.visit_file(&syn::parse_file(source).unwrap());
        components.visit_file(&syn::parse_file(kit).unwrap());
        let mut bodies: Vec<_> = branches(source)
            .into_iter()
            .filter(|(guard, _, _)| guard.contains("!self.connected"))
            .map(|(_, body, _)| body)
            .collect();
        assert!(!bodies.is_empty(), "the disconnected branch exists");
        let mut seen = std::collections::BTreeSet::new();
        let mut found = false;
        while let Some(body) = bodies.pop() {
            if body.contains("Notconnected") {
                found = true;
                break;
            }
            for (name, callee) in &components.0 {
                if body.contains(&format!("self.{name}(")) && seen.insert(name.clone()) {
                    bodies.push(callee.clone());
                }
            }
        }
        assert!(
            found,
            "the disconnected branch reaches the shared Not connected plate"
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
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(check_disconnected_registers)
        .unwrap()
        .join()
        .unwrap();
}

fn check_disconnected_registers() {
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
    ] {
        methods.visit_file(&syn::parse_file(source).unwrap());
    }
    let mut requires_connection: BTreeSet<_> = methods
        .0
        .iter()
        .filter(|(name, reading)| name.as_str() != "view" && reading.reads)
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
        !requires_connection.contains("view"),
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
    struct Summaries(Vec<String>);
    impl<'ast> Visit<'ast> for Summaries {
        fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
            if let syn::Expr::Path(path) = call.func.as_ref() {
                if path
                    .path
                    .segments
                    .last()
                    .unwrap()
                    .ident
                    .to_string()
                    .ends_with("_summary")
                {
                    self.0.push(
                        call.args
                            .first()
                            .expect("summary connection argument")
                            .to_token_stream()
                            .to_string(),
                    );
                }
            }
            syn::visit::visit_expr_call(self, call);
        }
    }
    let mut summary_count = 0;
    for source in [
        include_str!("../../../crates/views/forge/src/ui/forge.rs"),
        include_str!("../../../crates/views/members/src/lib.rs"),
    ] {
        let arguments = std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(move || {
                let mut summaries = Summaries(Vec::new());
                summaries.visit_file(&syn::parse_file(source).unwrap());
                summaries.0
            })
            .unwrap()
            .join()
            .unwrap();
        summary_count += arguments.len();
        for arguments in arguments {
            assert!(
                arguments.contains("connected"),
                "summary must distinguish no answer from measured zero"
            );
        }
    }
    assert!(summary_count > 0, "measured subtitles are inspected");
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
    let (mut app, _) = Ducktape::boot();
    app.connected = true;
    let before = app.connect_generation;

    let fail = |generation: i64| {
        AppMessage::ConnectFailed(backend::HydrationError {
            generation,
            message: "error sending request".into(),
        })
    };
    let _ = app.update(fail(app.connect_generation));
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
    let _ = app.update(fail(app.connect_generation));
    let _ = app.update(fail(app.connect_generation));
    assert_eq!(app.hydration_retry_attempt, 3);

    // A CONNECT IS NOT GUARDED ON `hydration_generation`, AND THIS IS WHY.
    // Thirty-seven handlers bump that counter for reasons of their own —
    // `choose_channel` is one of them — and a connect is in flight for seconds,
    // or for up to 30s while the node sits in issue #1018's checkpoint stall.
    // Guarded on the shared counter, one click on a channel mid-connect drops
    // the successful reply; because it SUCCEEDED no failure arm fires and
    // nothing retries, so the console sits Offline forever. Strictly worse than
    // the defect this PR fixes.
    let (mut wired, _) = Ducktape::boot();
    wired.connected_rpc = "http://127.0.0.1:38259".into();
    let connect_gen = wired.connect_generation;
    let shared_before = wired.hydration_generation;
    let _ = wired.update(AppMessage::ChooseChannel("general".into()));
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
    let _ = app.update(fail(stale));
    assert_eq!(
        app.hydration_retry_attempt, attempts,
        "an abandoned chain must not start a second retry loop"
    );

    let failure = handler_body("ConnectFailed");
    assert!(failure.contains("self.hydration_retry_attempt=self.hydration_retry_attempt+1"));
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
                && body.contains("ClearChatSearch")
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
// Both sweeps walk the authored Rust view tree rather than rendered text,
// and both carry an allowlist that must stay LIVE — an entry matching nothing
// fails the test, so the ledger cannot rot into a blanket exemption.
// ===========================================================================
