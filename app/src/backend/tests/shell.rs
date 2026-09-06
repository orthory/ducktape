use super::*;

#[test]
fn the_rail_seats_collaboration_and_node_operations_separately() {
    let nav = shell_nav(ShellTab::Chat, 3, true);
    let ids: Vec<ShellTab> = nav.iter().map(|item| item.id).collect();
    assert_eq!(
        ids,
        [
            ShellTab::Chat,
            ShellTab::Shell,
            ShellTab::Pages,
            ShellTab::Forge,
            ShellTab::Agents,
            ShellTab::Files,
            ShellTab::Explorer,
            ShellTab::Node,
            ShellTab::Members,
            ShellTab::Governance
        ]
    );
    let forge = nav.iter().find(|item| item.id == ShellTab::Forge).unwrap();
    assert!(forge.live, "an engaged agent pulses the forge seat");
    assert_eq!(
        nav.iter()
            .find(|item| item.id == ShellTab::Node)
            .unwrap()
            .title,
        "Node"
    );
    assert_eq!(
        nav.iter()
            .find(|item| item.id == ShellTab::Governance)
            .unwrap()
            .badge,
        3
    );
}

/// The three folds the mounted surfaces are drawn from — the crumb bar's
/// counts, the blob gutter, and the roster the popped panel keeps.
#[test]
fn the_crumb_counts_split_the_listing_in_two() {
    let entries = ["dir", "file", "file"]
        .into_iter()
        .enumerate()
        .map(|(key, kind)| FsEntry {
            key: key as i64,
            path: format!("/shared/{kind}"),
            name: kind.into(),
            kind: kind.into(),
            size: 0,
            object: String::new(),
        })
        .collect::<Vec<_>>();
    assert_eq!(fs_dir_count(&entries), 1);
    assert_eq!(fs_file_count(&entries), 2);
    assert_eq!(
        fs_dir_count(&entries) + fs_file_count(&entries),
        3,
        "every row lands in exactly one bucket"
    );
    assert_eq!(fs_dir_count(&[]), 0);
    assert_eq!(fs_file_count(&[]), 0);
}

#[test]
fn the_selected_fs_entry_resolves_or_blanks() {
    let selected = FsEntry {
        key: 1,
        path: "/shared/notes".into(),
        name: "notes".into(),
        kind: "file".into(),
        size: 7,
        object: "abc".into(),
    };

    assert_eq!(
        fs_entry_named(vec![no_fs_entry(), selected.clone()], selected.path.clone(),),
        selected
    );
    assert_eq!(
        fs_entry_named(Vec::new(), "/shared/missing".into()),
        no_fs_entry()
    );
}

#[test]
fn directory_rows_are_prepared_from_the_listing() {
    let entry = |name: &str, kind: &str| FsEntry {
        key: 0,
        path: format!("/shared/{name}"),
        name: name.into(),
        kind: kind.into(),
        size: 0,
        object: String::new(),
    };

    assert_eq!(
        fs_directories(&[entry("docs", "dir"), entry("readme", "file")]),
        vec![entry("docs", "dir")]
    );
}

#[test]
fn a_chat_load_answers_for_the_huddle_only_when_it_loaded_the_huddles_channel() {
    let member = |is_you: bool| HuddleParticipant {
        key: "aa".into(),
        label: "aa".into(),
        initials: "A".into(),
        is_agent: false,
        is_you,
        joined_at: 0,
        node: "aa11".into(),
    };
    let idle = HuddleAfterLoad::default();

    // Not in a huddle: the loaded channel's roster is the whole answer.
    let joined = huddle_after_load(
        true,
        idle.joined,
        idle.channel.clone(),
        idle.channel_name.clone(),
        idle.roster.clone(),
        "eng".into(),
        "Engineering".into(),
        vec![member(true)],
    );
    assert!(joined.joined);
    assert_eq!(joined.channel, "eng");
    assert_eq!(joined.channel_name, "Engineering");
    assert_eq!(joined.roster.len(), 1);

    // NOW CLICK ANOTHER ROOM. Its roster is a different conversation's, and
    // reading the huddle off it used to cut the call's media (the session is
    // subscribed on `joined`) and blank the channel `leave_huddle_here` needs.
    let switched = huddle_after_load(
        true,
        joined.joined,
        joined.channel.clone(),
        joined.channel_name.clone(),
        joined.roster.clone(),
        "general".into(),
        "General".into(),
        Vec::new(),
    );
    assert_eq!(switched, joined, "another room's load is not the huddle's");

    // Back on the huddle's own channel, a roster without her ends it.
    let left = huddle_after_load(
        true,
        joined.joined,
        joined.channel.clone(),
        joined.channel_name.clone(),
        joined.roster.clone(),
        "eng".into(),
        "Engineering".into(),
        vec![member(false)],
    );
    assert_eq!(left, idle);

    // And a resync that carried no chat at all says nothing either way.
    let quiet = huddle_after_load(
        false,
        joined.joined,
        joined.channel.clone(),
        joined.channel_name.clone(),
        joined.roster.clone(),
        "eng".into(),
        "Engineering".into(),
        Vec::new(),
    );
    assert_eq!(quiet, joined);
}

#[test]
fn the_roster_answers_admin_tier_and_filters() {
    let rows = vec![
        MemberRow {
            key: "aa".into(),
            label: "aa".into(),
            role: "validator".into(),
            is_this_node: true,
            is_agent: false,
            model: String::new(),
            live: true,
        },
        MemberRow {
            key: "bb".into(),
            label: "bb".into(),
            role: "resident".into(),
            is_this_node: false,
            is_agent: false,
            model: String::new(),
            live: false,
        },
        MemberRow {
            key: "triage".into(),
            label: "triage".into(),
            role: "agent".into(),
            is_this_node: false,
            is_agent: true,
            model: "codex".into(),
            live: true,
        },
    ];
    assert!(members_is_admin(&rows));
    assert_eq!(member_tier(&rows), "validator");
    // the two halves of "no row for this node", kept apart: an unanswered
    // roster is unknown, an answered one without this node is a real guest.
    assert_eq!(member_tier(&[]), "");
    let mut answered_without_this_node = rows.clone();
    answered_without_this_node[0].is_this_node = false;
    assert_eq!(member_tier(&answered_without_this_node), "guest");
    assert_eq!(filter_members(&rows, MembersFilter::Agents).len(), 1);
    assert_eq!(filter_members(&rows, MembersFilter::Humans).len(), 2);
    assert_eq!(filter_members(&rows, MembersFilter::Validators).len(), 1);
    assert_eq!(filter_members(&rows, MembersFilter::All).len(), 3);
}

/// THE HEADER COUNTS THE LIST IT SITS ABOVE. `members_summary` used to fold the
/// two VALSET queries — validators and residents — while the roster under it
/// also draws every registered agent, which holds no valset standing at all. On
/// the demo workspace that printed `1 validator · 0 residents` over two rows:
/// both numbers true, the sentence not, because it measured a different set
/// than the one on screen. The subtitle now splits the rows on `is_agent`, the
/// same predicate the Humans / Agents chips use, so its two counts partition
/// the list and sum to the All chip.
#[test]
fn the_members_subtitle_folds_the_rows_the_screen_lists() {
    let member = |key: &str, role: &str| MemberRow {
        key: key.into(),
        label: key.into(),
        is_agent: role == "agent",
        role: role.into(),
        is_this_node: false,
        model: String::new(),
        live: true,
    };
    let rows = vec![
        member("aa", "validator"),
        member("bb", "resident"),
        member("triage", "agent"),
    ];
    assert_eq!(members_summary(true, &rows), "2 humans · 1 agent");
    // singulars, and the count that used to be the whole subtitle.
    assert_eq!(members_summary(true, &rows[..1]), "1 human · 0 agents");

    // The invariant under the wording: every number in the subtitle is a slice
    // of the list, so they add up to the row count. The valset fold never did.
    let counted: usize = members_summary(true, &rows)
        .split(" · ")
        .filter_map(|part| part.split(' ').next()?.parse::<usize>().ok())
        .sum();
    assert_eq!(
        counted,
        rows.len(),
        "the Members subtitle must sum to the roster printed under it"
    );
}

#[test]
fn the_huddle_roster_marks_the_row_this_device_holds() {
    // The wire truth: `HuddleEntry.user` is the kernel's BARE user id, never
    // `user:{hex}` — the previous fixture invented prefixed entries and
    // asserted a compare no real roster row could satisfy.
    let me = [0xaau8; 32];
    let my_passkey = [0xacu8; 32];
    let peer = [0xbbu8; 32];
    // A seat taken with the person's passkey is the person's: the directory
    // binds both keys to one account, and the roster recognises it.
    let names = NameDirectory::new(BTreeMap::from([
        (
            hex_encode(&me),
            BoundAccount {
                number: 1,
                name: "me".into(),
            },
        ),
        (
            hex_encode(&my_passkey),
            BoundAccount {
                number: 1,
                name: "me".into(),
            },
        ),
        (
            hex_encode(&peer),
            BoundAccount {
                number: 2,
                name: "peer".into(),
            },
        ),
    ]));
    let roster = huddle_roster(
        &[
            chat::index::HuddleEntry {
                user: hex_encode(&my_passkey),
                node: "0a0a".into(),
                joined_at: 10,
            },
            chat::index::HuddleEntry {
                user: hex_encode(&peer),
                node: "0b0b".into(),
                joined_at: 20,
            },
        ],
        ChatReader::new(Some(&me), &names),
    );
    assert_eq!(roster.len(), 2);
    assert!(roster[0].is_you && !roster[0].is_agent);
    assert!(!roster[1].is_you && !roster[1].is_agent);
    assert_eq!(roster[0].label, "me");
    assert!(huddle_self(roster.clone()));
    assert!(!huddle_self(vec![roster[1].clone()]));
    // The fan-out the live session polls for is this roster's NODE keys with
    // our own row removed — the hub admits and fans out by node identity, and
    // a set that carried our own key would aim this device's media at itself.
    assert_eq!(
        huddle_recipient_nodes(roster, None),
        vec!["0b0b".to_string()]
    );
}

#[test]
fn huddle_recipient_nodes_drops_any_row_naming_this_devices_own_node() {
    // A `node_proof` only proves ITS OWN user holds that node's key — nothing
    // stops a stale or replayed roster row from naming a DIFFERENT user
    // alongside THIS node's key. `is_you` alone would miss it (that row is
    // not "mine"), and fanning media to your own node is a loopback echo.
    let me = [0xaau8; 32];
    let peer = [0xbbu8; 32];
    let roster = huddle_roster(
        &[
            chat::index::HuddleEntry {
                user: hex_encode(&me),
                node: "0a0a".into(),
                joined_at: 10,
            },
            chat::index::HuddleEntry {
                user: hex_encode(&peer),
                node: "0a0a".into(),
                joined_at: 20,
            },
        ],
        Some(&me),
        &AuthorNames::default(),
    );
    assert_eq!(
        huddle_recipient_nodes(roster, Some("0a0a")),
        Vec::<String>::new(),
        "the peer row names this device's own node — never fan media there"
    );
}

#[test]
fn popover_uses_only_shared_design_roles() {
    let tokens = ui_lang_components::ui::theme::LIGHT;
    let raised = raised_style(&iced::Theme::Light);
    // OPAQUE. iced has no backdrop blur, so a glass role over a menu is just
    // transparency: the sentence behind an item and the item's own label draw
    // through each other.
    assert_eq!(
        raised.background,
        Some(iced::Background::Color(tokens.palette.popover))
    );
    assert_eq!(raised.background.map(alpha_of), Some(1.0));
    assert_eq!(
        raised_style(&iced::Theme::Dark).background.map(alpha_of),
        Some(1.0)
    );
    assert_eq!(raised.border.radius, tokens.radius.card.into());
    assert_eq!(raised.shadow, tokens.elevation.popover);
}

#[test]
fn palette_keys_use_logical_escape_and_physical_shortcut() {
    use iced::keyboard::{
        Key, Modifiers,
        key::{Code, Named, Physical},
    };

    assert_eq!(
        palette_key_action(
            Key::Named(Named::Escape),
            Physical::Code(Code::KeyA),
            Modifiers::default(),
            true,
        ),
        "close"
    );
    assert_eq!(
        palette_key_action(
            Key::Named(Named::Escape),
            Physical::Code(Code::KeyA),
            Modifiers::default(),
            false,
        ),
        "none"
    );
    assert_eq!(
        palette_key_action(
            Key::Character("x".into()),
            Physical::Code(Code::KeyK),
            Modifiers::COMMAND,
            false,
        ),
        "open"
    );
    assert_eq!(
        palette_key_action(
            Key::Character("x".into()),
            Physical::Code(Code::KeyK),
            Modifiers::COMMAND,
            true,
        ),
        "close"
    );
}

#[test]
fn escape_ladder_names_the_topmost_transient_layer_only() {
    use iced::keyboard::{Key, key::Named};

    let escape = Key::Named(Named::Escape);
    let target = |tab: ShellTab,
                  palette: bool,
                  bell: bool,
                  create: bool,
                  thread_action: MessageAction,
                  action: MessageAction,
                  drawer: bool,
                  repo_menu: bool| {
        escape_target(
            escape.clone(),
            tab,
            palette,
            bell,
            create,
            thread_action,
            action,
            drawer,
            false,
            String::new(),
            repo_menu,
        )
    };

    // Not Escape → nothing, whatever is open.
    assert_eq!(
        escape_target(
            Key::Character("x".into()),
            ShellTab::Chat,
            false,
            true,
            true,
            MessageAction::More,
            MessageAction::More,
            true,
            true,
            "/shared/q3.md".into(),
            true,
        ),
        ""
    );
    // An open palette swallows Escape — palette_key_action owns it.
    assert_eq!(
        target(
            ShellTab::Chat,
            true,
            true,
            true,
            MessageAction::More,
            MessageAction::More,
            true,
            true,
        ),
        ""
    );
    // The ladder order is the z-order: bell over the create modal, menus
    // after both, thread menu over the stream's, popovers last.
    assert_eq!(
        target(
            ShellTab::Chat,
            false,
            true,
            true,
            MessageAction::More,
            MessageAction::More,
            true,
            true,
        ),
        "bell"
    );
    assert_eq!(
        target(
            ShellTab::Chat,
            false,
            false,
            true,
            MessageAction::More,
            MessageAction::More,
            true,
            true,
        ),
        "channel_create"
    );
    assert_eq!(
        target(
            ShellTab::Chat,
            false,
            false,
            false,
            MessageAction::More,
            MessageAction::More,
            false,
            true,
        ),
        "thread_menu"
    );
    // AND THE DRAWER OUTRANKS THE THREAD MENU WHEN IT IS OPEN. The rail is not
    // mounted while Channel details is up (`if active_thread_seq > 0 &&
    // !channel_settings_open`, `screens/chat.ice`), so a ⋯ flag left set behind
    // it names no layer on screen — this test used to pin the opposite verdict,
    // where the first Escape wiped `thread_edit_draft` and left the drawer
    // standing.
    assert_eq!(
        target(
            ShellTab::Chat,
            false,
            false,
            false,
            MessageAction::More,
            MessageAction::Toolbar,
            true,
            true,
        ),
        "channel_settings"
    );
    assert_eq!(
        target(
            ShellTab::Chat,
            false,
            false,
            false,
            MessageAction::Toolbar,
            MessageAction::Editing,
            true,
            true,
        ),
        "message_menu"
    );
    // THE DRAWER SITS BETWEEN THEM. The stream's menu floats over Channel
    // details, so it wins; the repo menu lives on another tab, so it loses.
    // It had no rung at all — an `×` and no keyboard exit, while every other
    // overlay answered Escape. Measured: Escape over an open drawer changed
    // exactly zero pixels on the running app.
    assert_eq!(
        target(
            ShellTab::Chat,
            false,
            false,
            false,
            MessageAction::Toolbar,
            MessageAction::Toolbar,
            true,
            true,
        ),
        "channel_settings"
    );
    assert_eq!(
        target(
            ShellTab::Forge,
            false,
            false,
            false,
            MessageAction::Toolbar,
            MessageAction::Toolbar,
            false,
            true,
        ),
        "repo_menu"
    );
    // THE PAGES DELETE CONFIRM. A scrim and a confirm over the canvas, inside
    // the Pages screen — so it is a rung, and it answers only from Pages.
    let armed = |tab: ShellTab, page_delete: bool, fs_delete: &str| {
        escape_target(
            escape.clone(),
            tab,
            false,
            false,
            false,
            MessageAction::Toolbar,
            MessageAction::Toolbar,
            false,
            page_delete,
            fs_delete.into(),
            false,
        )
    };
    assert_eq!(armed(ShellTab::Pages, true, ""), "page_delete");
    assert_eq!(armed(ShellTab::Chat, true, ""), "");
    assert_eq!(armed(ShellTab::Files, false, "/shared/q3.md"), "fs_delete");
    assert_eq!(armed(ShellTab::Node, false, "/shared/q3.md"), "");

    // Nothing transient open → Escape is a no-op. The pages block menus are
    // gone with the surfaces they dismissed.
    assert_eq!(
        target(
            ShellTab::Chat,
            false,
            false,
            false,
            MessageAction::Toolbar,
            MessageAction::Toolbar,
            false,
            false,
        ),
        ""
    );
}

// A RUNG ANSWERS ONLY FROM THE TAB THAT MOUNTS ITS SURFACE. No tab switch
// clears menu state (`select_shell_tab` leaves every menu flag set), so a
// ⋯ menu opened on Chat is still SET while Pages is on screen —
// unscoped, that stale flag ate the first Escape on every other tab, and the
// same reading made `content_scroll_step` refuse to move a pane nothing was
// covering. The palette, bell and create modal are mounted OUTSIDE the tab
// match in `components/shell.ice` and keep answering from every tab.
#[test]
fn a_rung_answers_only_from_the_tab_that_mounts_its_surface() {
    use iced::keyboard::{Key, key::Named};

    let escape = Key::Named(Named::Escape);
    let none = String::new();

    // One closure per reader, the sibling test's `target` shape: tab first,
    // then one argument per layer.
    let overlay = |tab: ShellTab,
                   thread_action: MessageAction,
                   action: MessageAction,
                   drawer: bool,
                   repo_menu: bool| {
        topmost_overlay(
            tab,
            false,
            false,
            false,
            thread_action,
            action,
            drawer,
            false,
            "",
            repo_menu,
        )
    };
    let target =
        |tab: ShellTab, bell: bool, create: bool, thread_action: MessageAction, repo_menu: bool| {
            escape_target(
                escape.clone(),
                tab,
                false,
                bell,
                create,
                thread_action,
                MessageAction::Toolbar,
                false,
                false,
                String::new(),
                repo_menu,
            )
        };

    // A stale chat menu names no layer from another tab — for BOTH readers.
    let stale_thread_menu = |tab: ShellTab| {
        overlay(
            tab,
            MessageAction::More,
            MessageAction::Toolbar,
            false,
            false,
        )
    };
    assert_eq!(stale_thread_menu(ShellTab::Chat), "thread_menu");
    assert_eq!(stale_thread_menu(ShellTab::Pages), none);
    assert_eq!(stale_thread_menu(ShellTab::Explorer), none);

    // Same for the stream's menu and the details drawer.
    assert_eq!(
        overlay(
            ShellTab::Files,
            MessageAction::Toolbar,
            MessageAction::Editing,
            false,
            false,
        ),
        none
    );
    assert_eq!(
        overlay(
            ShellTab::Pages,
            MessageAction::Toolbar,
            MessageAction::Toolbar,
            true,
            false,
        ),
        none
    );

    // And the forge menu answers only from Forge.
    let repo_menu = |tab: ShellTab| {
        overlay(
            tab,
            MessageAction::Toolbar,
            MessageAction::Toolbar,
            false,
            true,
        )
    };
    assert_eq!(repo_menu(ShellTab::Forge), "repo_menu");
    assert_eq!(repo_menu(ShellTab::Chat), none);

    // THE LOAD-BEARING STACK: a stale chat flag must not SHADOW the menu that
    // is actually on screen. Before scoping, this named "thread_menu" and the
    // visible forge menu survived the press.
    assert_eq!(
        target(ShellTab::Forge, false, false, MessageAction::More, true),
        "repo_menu"
    );

    // Window-level layers ride every tab: mounted outside the tab match, they
    // stay on screen across a switch and must keep answering.
    assert_eq!(
        target(
            ShellTab::Governance,
            true,
            false,
            MessageAction::Toolbar,
            false
        ),
        "bell"
    );
    assert_eq!(
        target(ShellTab::Node, false, true, MessageAction::Toolbar, false),
        "channel_create"
    );
}

// THE PANE SCROLL'S THREE CONDITIONS, one assertion each, over the router
// itself rather than over one key's pixels. #1006 shipped it with only the
// modifier condition: it claimed the arrows (which a focused single-line
// `text_input` leaves UNCAPTURED — `iced_widget-0.14.2/src/text_input.rs:1245`
// falls Up/Down through to `_ => {}` — so `status=ignored` handed them here
// while a caret sat in the field), and it never asked whether a transient
// layer was over the pane it was about to move.
#[test]
fn the_content_pane_claims_only_the_keys_nothing_else_owns() {
    use iced::keyboard::{Key, Modifiers, key::Named};

    let step = |named: Named, modifiers: Modifiers, overlay: &str| {
        content_scroll_step(Key::Named(named), modifiers, overlay.into())
    };
    let free = Modifiers::empty();

    // 1. THE PANE'S OWN KEYS. Page Up/Down and Home/End: iced's text widgets
    //    capture Home/End when focused, so one only ever reaches here with
    //    nothing focused, and no widget in this console owns a Page key.
    assert!(step(Named::PageDown, free, "") > 0.0);
    assert!(step(Named::PageUp, free, "") < 0.0);
    assert!(step(Named::End, free, "") > 0.0);
    assert!(step(Named::Home, free, "") < 0.0);

    // 2. AN ARROW BELONGS TO WHATEVER HAS FOCUS. Nothing in this stack can
    //    read widget focus, and a single-line input does not capture Up/Down,
    //    so the pane cannot tell a caret's arrow from its own and must not
    //    claim one — at any time, under any layer.
    assert_eq!(step(Named::ArrowDown, free, ""), 0.0);
    assert_eq!(step(Named::ArrowUp, free, ""), 0.0);

    // 3. A TRANSIENT LAYER'S KEY IS NOT THE PANE'S. Every rung `topmost_overlay`
    //    can name stops every scroll key, so no press moves the screen behind
    //    an open palette or bell.
    for overlay in [
        "palette",
        "bell",
        "channel_create",
        "thread_menu",
        "message_menu",
        "channel_settings",
        "page_delete",
        "fs_delete",
        "repo_menu",
    ] {
        for key in [Named::PageDown, Named::PageUp, Named::End, Named::Home] {
            assert_eq!(step(key, free, overlay), 0.0, "{overlay} is over the pane");
        }
    }

    // 4. A CHORD IS NOT THE PANE'S — it belongs to its own router.
    assert_eq!(step(Named::PageDown, Modifiers::SHIFT, ""), 0.0);
    assert_eq!(step(Named::Home, Modifiers::CTRL, ""), 0.0);
}

#[test]
fn files_base64_round_trips() {
    for sample in [
        b"".as_slice(),
        b"a".as_slice(),
        b"ab".as_slice(),
        b"abc".as_slice(),
        b"hello duckfs \xf0\x9f\xa6\x86".as_slice(),
    ] {
        let encoded = base64_encode(sample);
        assert_eq!(
            base64_decode(&encoded).as_deref(),
            Some(sample),
            "{encoded}"
        );
    }
    assert_eq!(base64_encode(b"abc"), "YWJj");
    assert_eq!(base64_encode(b"ab"), "YWI=");
}

#[test]
fn bell_severity_projects_the_kind_and_defaults_to_info() {
    assert_eq!(bell_severity("run_failed"), "danger");
    assert_eq!(bell_severity("review_requested"), "warning");
    assert_eq!(bell_severity("mentioned"), "info");
    // an unnamed kind is a notice, never an alarm.
    assert_eq!(bell_severity("brand_new_kind"), "info");
}

#[test]
fn bell_badge_takes_the_worst_unread_severity() {
    let item = |seq: i64, kind: &str, read: bool| BellItem {
        seq,
        kind: kind.into(),
        body: String::new(),
        source: String::new(),
        height: 0,
        read,
    };

    assert_eq!(
        bell_worst_severity(&[item(1, "mentioned", false), item(2, "run_failed", false)]),
        "danger"
    );
    // a READ error does not keep the badge red.
    assert_eq!(
        bell_worst_severity(&[
            item(1, "run_failed", true),
            item(2, "review_requested", false)
        ]),
        "warning"
    );
    assert_eq!(bell_worst_severity(&[]), "info");
}

/// THE TAB-SWITCH GATE. Four planes used to refetch on every tab move —
/// members, governance, agents, account — regardless of the destination, so a
/// click into Files paid four `/v1/query` round trips for rows nothing on
/// screen reads.
#[test]
fn a_tab_move_only_refetches_what_its_destination_draws() {
    // EVERY tab, taken from the rail itself plus the footer's Settings, so a
    // new seat lands in this sweep instead of quietly defaulting to "reads
    // nothing" behind a hand-written negative list.
    let mut tabs: Vec<ShellTab> = shell_nav(ShellTab::Chat, 0, false)
        .into_iter()
        .map(|seat| seat.id)
        .collect();
    tabs.push(ShellTab::Settings);

    // the roster is drawn by five panes: its own, the admin gate under
    // Approvals, the forge write gate, the Node permissions, and the Settings
    // standing card. The rest are narrow: Settings draws the account card,
    // Forge the org "about", and proposals and agent rows belong to one pane
    // each.
    for (plane, drawn) in [
        (
            "members",
            &[
                ShellTab::Forge,
                ShellTab::Node,
                ShellTab::Members,
                ShellTab::Governance,
                ShellTab::Settings,
            ][..],
        ),
        ("governance", &[ShellTab::Governance][..]),
        ("agents", &[ShellTab::Agents][..]),
        ("account", &[ShellTab::Forge, ShellTab::Settings][..]),
        // an unknown plane name is nobody's — a typo must not silently reopen
        // the storm by answering true.
        ("explorer", &[][..]),
    ] {
        let readers: Vec<ShellTab> = tabs
            .iter()
            .copied()
            .filter(|tab| tab_reads_plane(*tab, plane.into()))
            .collect();
        assert_eq!(readers, drawn, "exactly these tabs draw {plane}");
    }
}

/// The launch window reads the workspace files through the crate that wrote
/// them, never a line parser: the chain id keeps its `#hex` half, a
/// two-validator descriptor (the multi-line array `node admit` writes) still
/// yields the founding key, and a wildcard `http_listen` dials loopback.
#[test]
fn workspace_facts_come_from_the_crate_that_wrote_them() {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("mynet-dir");
    std::fs::create_dir_all(&dir).unwrap();
    let founder = "aa".repeat(32);
    let admitted = "bb".repeat(32);
    workspace_config::NetworkDescriptor {
        chain_id: "mynet#a1b2c3d4".into(),
        validators: vec![founder.clone(), admitted],
        bootstrap: vec![],
        reach: vec![],
        coordination: None,
        block_time_ms: workspace_config::DEFAULT_BLOCK_TIME_MS,
        modules: vec![],
        genesis: String::new(),
    }
    .save(&dir.join("network.toml"))
    .unwrap();
    let descriptor = std::fs::read_to_string(dir.join("network.toml")).unwrap();
    assert!(
        descriptor.contains("validators = [\n"),
        "two validators serialize as a multi-line array:\n{descriptor}"
    );
    std::fs::write(
        dir.join("node.toml"),
        r#"network = "network.toml"
key_file = "node.key"
listen = "0.0.0.0:52200"
advertised = "overlay"
storage_dir = "data"
http_listen = "0.0.0.0:8844"
gateway_listen = "127.0.0.1:0"
rpc_listen = "127.0.0.1:8845"
wireguard_listen = "0.0.0.0:51820"
invite_listen = "0.0.0.0:51821"
wireguard_advertised = "auto"
primary_coordinator = "none"
coordinator_relay = "none"
checkpoint_blocks = 32
"#,
    )
    .unwrap();

    assert_eq!(
        registered_workspaces_in(root.path()),
        vec![("mynet#a1b2c3d4".to_string(), dir.clone())]
    );
    assert_eq!(workspace_identity(&dir), Some(short_label(&founder)));
    assert_eq!(
        workspace_endpoint(&dir).as_deref(),
        Some("http://127.0.0.1:8844")
    );
}
