use super::*;

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

#[test]
fn message_action_toolbar_stays_compact_and_accessible() {
    let components = inlined(include_str!("../ui/components/chat.ice"));
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
    let stream = inlined(include_str!("../ui/screens/chat.ice"));
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

    let chat = inlined(include_str!("../ui/screens/chat.ice"));
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

    let handlers = inlined(include_str!("../ui/handlers/chat.ice"));
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
    assert!(!inlined(include_str!("../ui/extern/backend.ice")).contains("task focus_next()"));
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
fn the_page_surface_is_one_editor_with_no_click_to_edit_left() {
    let components = inlined(include_str!("../ui/components/pages.ice"));
    let handlers = inlined(include_str!("../ui/handlers/pages.ice"));
    let view = inlined(include_str!("../ui/screens/pages.ice"));

    // THE TITLE IS LINE 0 OF THE BUFFER, not a control. The click-to-edit
    // title editor is gone the same way the click-to-edit blocks are; these
    // stay as refusals so neither creeps back.
    assert!(!components.contains("PageTitleEditor"));
    assert!(!components.contains("task widget focus #title-input"));
    assert!(!components.contains("defer_focus"));
    assert!(!handlers.contains("focus_page_title"));
    assert!(!inlined(include_str!("../ui/extern/backend.ice")).contains("defer_focus"));
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
        include_str!("../ui/app.ice"),
        include_str!("../ui/extern/backend.ice"),
        include_str!("../ui/state/types.ice"),
        include_str!("../ui/state/core.ice"),
        include_str!("../ui/state/chat.ice"),
        include_str!("../ui/state/shell.ice"),
        include_str!("../ui/state/explorer.ice"),
        include_str!("../ui/state/roster.ice"),
        include_str!("../ui/state/forge.ice"),
        include_str!("../ui/state/node.ice"),
        include_str!("../ui/state/files.ice"),
        include_str!("../ui/state/overlays.ice"),
        include_str!("../ui/state/pages.ice"),
        include_str!("../ui/state/onboarding.ice"),
        include_str!("../ui/state/huddle.ice"),
        include_str!("../ui/state/derived.ice"),
        include_str!("../ui/theme.ice"),
        include_str!("../ui/view.ice"),
        include_str!("../ui/components/chat.ice"),
        include_str!("../ui/components/dm.ice"),
        include_str!("../ui/components/files.ice"),
        include_str!("../ui/components/forge.ice"),
        include_str!("../ui/components/huddle.ice"),
        include_str!("../ui/components/icon.ice"),
        include_str!("../ui/components/kit.ice"),
        include_str!("../ui/components/node.ice"),
        include_str!("../ui/components/onboarding.ice"),
        include_str!("../ui/components/overlay.ice"),
        include_str!("../ui/components/pages.ice"),
        include_str!("../ui/components/patterns.ice"),
        include_str!("../ui/components/roster.ice"),
        include_str!("../ui/components/shell.ice"),
        include_str!("../ui/handlers/lifecycle.ice"),
        include_str!("../ui/handlers/chat.ice"),
        include_str!("../ui/handlers/pages.ice"),
        include_str!("../ui/handlers/shell.ice"),
    ));
    for gradient in ["linear(", "radial(", "conic("] {
        assert!(!ui.contains(gradient), "{gradient}");
        assert!(!SCREENS.contains(gradient), "{gradient}");
    }
    // The window is opaque. iced has no backdrop blur, so the chrome paints
    // the artifact's own non-glass ladder — desk/rail/sidebar/content — and
    // never a translucent tint that would composite over the desktop.
    let app = inlined(include_str!("../ui/app.ice"));
    assert!(!app.contains("\n    transparent true"));
    assert!(!app.contains("\n    blur true"));
    assert!(app.contains("\n  bg app_background"));
    assert!(app.contains("\n  fg app_text"));
    let core_state = inlined(include_str!("../ui/state/core.ice"));
    assert!(!core_state.contains("app_background"));
    assert!(!core_state.contains("app_text"));
    assert!(core_state.contains("appearance:Appearance = Appearance.system"));
    let derived = inlined(include_str!("../ui/state/derived.ice"));
    assert!(derived.contains(
        "app_background = keep_str(appearance == Appearance.dark, \"#1b1a16\", \"#fdfdfb\")"
    ));
    assert!(
        derived.contains(
            "app_text = keep_str(appearance == Appearance.dark, \"#e8e6df\", \"#2c2b27\")"
        )
    );
    let lifecycle = inlined(include_str!("../ui/handlers/lifecycle.ice"));
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
    let defaults = inlined(include_str!("../ui/theme.ice"));
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
    let theme = inlined(include_str!("../ui/theme.ice"));
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

    let shell = inlined(include_str!("../ui/components/shell.ice"));
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
    assert!(!inlined(include_str!("../ui/view.ice")).contains("network_label("));
    assert!(!SCREENS.contains("history_has_older("));
    assert!(!inlined(include_str!("../ui/view.ice")).contains("history_has_older("));
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
    let overlays = inlined(include_str!("../ui/screens/overlays.ice"));
    assert!(overlays.contains(
        "bg=surface border=border border-w=1.0 r=14.0 shadow=shadow_modal shadow-y=24.0 shadow-blur=60.0"
    ));

    let authored_pages = inlined(include_str!("../ui/components/pages.ice"));
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
        include_str!("../ui/components/shell.ice"),
        include_str!("../ui/components/chat.ice"),
        include_str!("../ui/components/pages.ice"),
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

    let view = inlined(include_str!("../ui/view.ice"));
    let shell = inlined(include_str!("../ui/components/shell.ice"));
    let chat = inlined(include_str!("../ui/components/chat.ice"));
    let chat_screen = inlined(include_str!("../ui/screens/chat.ice"));
    let pages = inlined(include_str!("../ui/components/pages.ice"));
    let kit = inlined(include_str!("../ui/components/kit.ice"));
    let forge = inlined(include_str!("../ui/components/forge.ice"));

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
        let kit_components = inlined(include_str!("../ui/components/kit.ice"));
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
        &inlined(include_str!("../ui/components/icon.ice")),
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
        include_str!("../ui/ducktape-ui/recipes.ice"),
        include_str!("../ui/theme.ice"),
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
        ("view.ice", inlined(include_str!("../ui/view.ice"))),
        ("chat.ice", inlined(include_str!("../ui/components/chat.ice"))),
        ("dm.ice", inlined(include_str!("../ui/components/dm.ice"))),
        (
            "files.ice",
            inlined(include_str!("../ui/components/files.ice")),
        ),
        (
            "forge.ice",
            inlined(include_str!("../ui/components/forge.ice")),
        ),
        (
            "huddle.ice",
            inlined(include_str!("../ui/components/huddle.ice")),
        ),
        ("icon.ice", inlined(include_str!("../ui/components/icon.ice"))),
        ("kit.ice", inlined(include_str!("../ui/components/kit.ice"))),
        ("node.ice", inlined(include_str!("../ui/components/node.ice"))),
        (
            "onboarding.ice",
            inlined(include_str!("../ui/components/onboarding.ice")),
        ),
        (
            "overlay.ice",
            inlined(include_str!("../ui/components/overlay.ice")),
        ),
        (
            "pages.ice",
            inlined(include_str!("../ui/components/pages.ice")),
        ),
        (
            "patterns.ice",
            inlined(include_str!("../ui/components/patterns.ice")),
        ),
        (
            "roster.ice",
            inlined(include_str!("../ui/components/roster.ice")),
        ),
        (
            "shell.ice",
            inlined(include_str!("../ui/components/shell.ice")),
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
                    ::design::type_scale::ALL.contains(&size),
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
    let theme = inlined(include_str!("../ui/theme.ice"));
    assert!(theme.contains(&format!("family=\"{}\"", ::design::fonts::FAMILY_UI)));
    assert!(theme.contains(&format!("family=\"{}\"", ::design::fonts::FAMILY_MONO)));
    let app = inlined(include_str!("../ui/app.ice"));
    for asset in ::design::fonts::ASSETS {
        assert!(
            app.contains(&format!("font \"../../../crates/design/{asset}\"")),
            "app.ice must embed {asset}"
        );
    }
    assert!(app.contains(&format!("text-size {}", ::design::type_scale::BODY)));

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
    let chat = inlined(include_str!("../ui/components/chat.ice"));
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
        ($($path:literal),* $(,)?) => { [$(($path, include_str!(concat!("../", $path)))),*] };
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

/// THE DESIGN PASS, PINNED. Every one of these is a measurement someone made
/// against the artifact and a later edit can silently undo: a number in a `with`
/// block reverts as easily as it landed, and none of them fails a build. The
/// grouping rhythm, the line measure, the header row, the selection mark and the
/// loading/failure states are all here so a revert is a red test, not a
/// screenshot nobody takes.
#[test]
fn the_chat_surface_holds_to_its_measured_geometry() {
    let components = inlined(include_str!("../ui/components/chat.ice"));
    let screen = inlined(include_str!("../ui/screens/chat.ice"));
    let dm = inlined(include_str!("../ui/components/dm.ice"));

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
