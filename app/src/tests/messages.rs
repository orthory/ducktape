use super::*;

#[test]
fn every_timeline_writer_advances_its_render_revision() {
    let mut writers = 0;
    for (path, source) in ice_sources() {
        if !path.contains("/handlers/") {
            continue;
        }
        let source = format!("\n{source}");
        for handler in source.split("\non ").skip(1) {
            let body = handler.split("\non ").next().unwrap_or(handler);
            let name = body.lines().next().unwrap_or("<unknown handler>");
            for (state, revision) in [
                ("messages", "messages_revision"),
                ("thread_messages", "thread_messages_revision"),
            ] {
                let writes_state = body.lines().any(|line| {
                    line.split("//")
                        .next()
                        .unwrap_or_default()
                        .trim_start()
                        .starts_with(&format!("{state} = "))
                });
                if !writes_state {
                    continue;
                }
                writers += 1;
                let advances_revision = body.lines().any(|line| {
                    line.split("//")
                        .next()
                        .unwrap_or_default()
                        .trim_start()
                        .starts_with(&format!("{revision} = "))
                });
                assert!(
                    advances_revision,
                    "{path}: `{name}` writes `{state}` without advancing `{revision}`; the whole-list lazy would keep stale pixels"
                );
            }
        }
    }
    assert!(
        writers >= 20,
        "the ratchet found the production timeline writers"
    );
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
    let chat = inlined(include_str!("../ui/handlers/chat.ice"));
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
    let chat = inlined(include_str!("../ui/handlers/chat.ice"));
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
    let chat = inlined(include_str!("../ui/screens/chat.ice"));
    // Only the rows the viewport can see are laid out, which is what lets the
    // timeline hold a whole channel without paying a text layout per row — and
    // `by=message.view_key` is what makes per-row state and per-row MEASUREMENT
    // follow the message through prepends AND optimistic confirmation instead
    // of following the slot it happened to occupy.
    let timeline = chat
        .split_once("component MessageTimeline")
        .expect("the message timeline component")
        .1
        .split_once("keyed message in messages by=message.view_key w=fill gap=3.0 virtual-row=44.0")
        .expect("the message timeline is a KEYED virtual-row column");
    // That is only correct under an end-anchored scroll: measuring a row ABOVE
    // the viewport moves everything below it, and a bottom-anchored offset is
    // what carries the visible rows along with it. The two travel together —
    // the thread rail's own scroll sits further down the file, past the split.
    // `h=shrink` is the composer-anchored height: the virtual column reports a
    // whole-list estimate, so a long timeline still hits the box's cap.
    assert!(
        chat.contains("scroll #message-stream dir=vertical w=fill h=shrink anchor-y=end auto=true")
    );
    // The page controls stay OUTSIDE the keyed column. A keyed column repeats
    // one template over one list; a button folded into that list is a row whose
    // arrival and departure shift every index below it — the same defect one
    // level up, and `has_older_history` flips on every page.
    assert!(chat.contains("col w=fill gap=3.0 pr=6.0"));
    assert!(chat.contains("button \"Load older messages\""));
    assert!(timeline.1.contains("lazy message as cached_message"));
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

/// A MESSAGE LINE IS ONE PARAGRAPH, NOT A FLEX OF TOKENS (#1096).
///
/// ducktape-ui#639 lets a `for` expand spans inside `rich-text`, so the whole
/// span list — literal runs, mentions, links — lowers into ONE native
/// paragraph widget: real word wrapping, native selection across the line,
/// and `link=` on the widget's own route instead of a per-token button. The
/// template cannot branch, so the arm choice is DATA: exactly one `ChatSpan`
/// text field per run (the chat client's `span_arm`), and this sweep pins the
/// template that meets it.
#[test]
fn the_message_line_is_one_rich_text_paragraph() {
    let components = inlined(include_str!("../ui/components/chat.ice"));
    let rich_line = components
        .split_once("component RichLine")
        .expect("the message line component")
        .1;
    let rich_line = rich_line
        .split_once("\ncomponent ")
        .map_or(rich_line, |(body, _)| body);
    // ONE paragraph, expanded by the widget's own `for` — no wrapping flex of
    // per-token `text` widgets, and no per-token link button.
    assert!(rich_line.contains(
        "rich-text w=fill size=13.5 line-h=1.55 wrap=word-or-glyph color=accent_fg \
         -> emit(open_message_link, _)"
    ));
    assert!(rich_line.contains("for span in block.spans"));
    assert!(
        !rich_line.contains("flex") && !rich_line.contains("button"),
        "a token widget beside the paragraph is the #1071 workaround back"
    );
    // The plate is the mention's token, and ONLY the mention's — a posted URL
    // is a destination, not a person.
    let plated: Vec<&str> = rich_line
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("span ") && line.contains("bg="))
        .collect();
    assert_eq!(
        plated,
        ["span span.mention bg=brand_bg px=4.0 r=4.0 font=medium color=brand"],
        "the mention arm alone wears the plate"
    );
    // And the underline is the link's rule alone — it marks a destination,
    // not an emphasis (ducktape-ui#604).
    let underlined: Vec<&str> = rich_line
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("span ") && line.contains(" underline"))
        .collect();
    assert_eq!(
        underlined,
        ["span span.link_text link=span.link underline font=medium color=brand"],
        "the link arm alone draws the rule"
    );
    // It hands off through the SAME external-URL route the page renderer's
    // link press takes — one mechanism for one act, not a second one here.
    let handlers = include_str!("../ui/handlers/chat.ice");
    assert!(handlers.contains("on open_message_link(url)"));
    assert!(handlers.contains(
        "run every open_external_url(url) -> external_url_opened _ | external_url_failed _"
    ));
}

/// A TAIL SNAP ON AN END-ANCHORED SCROLL IS `snap … 0.0`, NEVER `snap-end`.
///
/// Both of the app's snapped scrolls (`#message-stream`, `#transcript`) are
/// `anchor-y=end`: the offset counts FROM the tail, so relative 0.0 is the
/// tail and `snap-end` (relative 1.0) is the TOP of loaded history. The
/// inverted op shipped once — every send hurled the reader to the oldest
/// loaded row — and is a silent no-op in any fixture whose content fits the
/// viewport, so only this lint stands between it and a paste-back.
#[test]
fn tail_snaps_speak_the_end_anchored_offset() {
    let chat = include_str!("../ui/handlers/chat.ice");
    let shell = include_str!("../ui/handlers/shell.ice");
    assert!(
        chat.contains("task widget snap #workspace-tabs/content/chat/message-stream 0.0 0.0"),
        "the send handler snaps the stream to its anchored tail"
    );
    for (name, source) in [("chat", chat), ("shell", shell)] {
        assert!(
            !source.contains("task widget snap-end"),
            "{name} handlers invoke snap-end, which is the TOP of an anchor-y=end scroll"
        );
    }
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
    let components = inlined(include_str!("../ui/components/chat.ice"));
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
        submit(&mut app, ComposerKind::Message, body);
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

/// THE RAIL IS NOT A PLAIN RUN, SO THE MINT RE-MARKS THE REPLIES ONLY.
///
/// A thread's vec is `[root] ++ replies` and the root renders as its own divided
/// block, so the window fold marks the REPLIES only: the first reply keeps its
/// header even under a root that shares its author, and a minted reply that
/// continues the previous reply's run folds under it — one header per run, the
/// same grouping the timeline draws.
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
    submit(&mut app, ComposerKind::Reply, "and one more");

    let headers: Vec<bool> = app
        .thread_messages
        .iter()
        .map(|message| message.show_author)
        .collect();
    assert_eq!(
        headers,
        vec![true, true, false],
        "the root's header and the first reply's both stand; the minted reply \
         continues the run"
    );
}
