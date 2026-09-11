use super::*;

#[test]
fn the_rail_seats_collaboration_and_node_operations_separately() {
    let nav = shell_nav(ShellTab::Chat, 3, true);
    let ids: Vec<ShellTab> = nav.iter().map(|item| item.id).collect();
    assert_eq!(
        ids,
        [
            ShellTab::Chat,
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
                party: "acct:1".into(),
                node: "0a0a".into(),
                joined_at: 10,
            },
            chat::index::HuddleEntry {
                party: format!("user:{}", hex_encode(&peer)),
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
    let names = NameDirectory::new(BTreeMap::new());
    let roster = huddle_roster(
        &[
            chat::index::HuddleEntry {
                party: format!("user:{}", hex_encode(&me)),
                node: "0a0a".into(),
                joined_at: 10,
            },
            chat::index::HuddleEntry {
                party: format!("user:{}", hex_encode(&peer)),
                node: "0a0a".into(),
                joined_at: 20,
            },
        ],
        ChatReader::new(Some(&me), &names),
    );
    assert_eq!(
        huddle_recipient_nodes(roster, Some("0a0a")),
        Vec::<String>::new(),
        "the peer row names this device's own node — never fan media there"
    );
}

#[test]
fn huddle_recipient_nodes_keeps_the_readers_other_device() {
    // Two devices of ONE account in the same huddle: the module dedups a
    // join by PARTY, so this device's own historical `user:{hex}` row (this
    // exact key) sits alongside the account's shared `acct:1` row a second,
    // now-bound device joined onto — and `is_you` answers by ACCOUNT, so
    // BOTH rows answer it true. Excluding on `is_you` alone used to drop
    // both, and the two devices went mutually dark. The fan-out has to tell
    // them apart by NODE, the one thing that is actually per-device, so it
    // must exclude only THIS device's own row.
    let laptop = [0xaau8; 32];
    let phone = [0xadu8; 32];
    let names = NameDirectory::new(BTreeMap::from([
        (
            hex_encode(&laptop),
            BoundAccount {
                number: 1,
                name: "me".into(),
            },
        ),
        (
            hex_encode(&phone),
            BoundAccount {
                number: 1,
                name: "me".into(),
            },
        ),
    ]));
    let roster = huddle_roster(
        &[
            chat::index::HuddleEntry {
                party: format!("user:{}", hex_encode(&laptop)),
                node: "1a1a".into(),
                joined_at: 10,
            },
            chat::index::HuddleEntry {
                party: "acct:1".into(),
                node: "2b2b".into(),
                joined_at: 20,
            },
        ],
        ChatReader::new(Some(&laptop), &names),
    );
    assert!(
        roster.iter().all(|participant| participant.is_you),
        "is_you answers by account: both rows are ours"
    );
    assert_eq!(
        huddle_recipient_nodes(roster, Some("1a1a")),
        vec!["2b2b".to_string()],
        "the phone is still a recipient; only this device's own node is excluded"
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
    let target = |tab: ShellTab, palette: bool, bell: bool, create: bool| {
        escape_target(
            escape.clone(),
            tab,
            palette,
            bell,
            create,
            false,
        )
    };

    // Not Escape -> nothing, whatever is open.
    assert_eq!(
        escape_target(
            Key::Character("x".into()),
            ShellTab::Chat,
            false,
            true,
            true,
            true,
        ),
        ""
    );
    // An open palette swallows Escape — palette_key_action owns it.
    assert_eq!(target(ShellTab::Chat, true, true, true), "");
    // The ladder order is the z-order: bell over the create modal.
    assert_eq!(target(ShellTab::Chat, false, true, true), "bell");
    assert_eq!(target(ShellTab::Chat, false, false, true), "channel_create");
    // THE PAGES DELETE CONFIRM. A scrim and a confirm over the canvas, inside
    // the Pages screen — so it is a rung, and it answers only from Pages.
    let armed = |tab: ShellTab, page_delete: bool| {
        escape_target(
            escape.clone(),
            tab,
            false,
            false,
            false,
            page_delete,
        )
    };
    assert_eq!(armed(ShellTab::Pages, true), "page_delete");
    assert_eq!(armed(ShellTab::Chat, true), "");

    // Nothing transient open -> Escape is a no-op. THE CHAT RUNGS ARE GONE
    // WITH THE CHAT SCREEN: its menus and its details drawer are the view's
    // own layers now, dismissed inside the view.
    assert_eq!(target(ShellTab::Chat, false, false, false), "");
}

// A RUNG ANSWERS ONLY FROM THE TAB THAT MOUNTS ITS SURFACE. No tab switch
// clears overlay state, so a layer armed on one tab is still SET while
// another is on screen — unscoped, that stale flag ate the first Escape
// everywhere else. The palette, bell and create modal are mounted OUTSIDE the
// tab match in `components/shell.ice` and keep answering from every tab.
#[test]
fn a_rung_answers_only_from_the_tab_that_mounts_its_surface() {
    use iced::keyboard::{Key, key::Named};

    let escape = Key::Named(Named::Escape);
    let none = String::new();

    let overlay = |tab: ShellTab, page_delete: bool| {
        topmost_overlay(tab, false, false, false, page_delete)
    };
    let target = |tab: ShellTab, bell: bool, create: bool| {
        escape_target(escape.clone(), tab, false, bell, create, false)
    };

    // A stale armed delete names no layer from another tab — for BOTH readers.
    assert_eq!(overlay(ShellTab::Pages, true), "page_delete");
    assert_eq!(overlay(ShellTab::Chat, true), none);

    // Window-level layers ride every tab: mounted outside the tab match, they
    // stay on screen across a switch and must keep answering.
    assert_eq!(target(ShellTab::Governance, true, false), "bell");
    assert_eq!(target(ShellTab::Node, false, true), "channel_create");
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
    // MALFORMED IS A REFUSAL, NOT EMPTY BYTES. The handwritten decoder this
    // replaced stopped at padding per quartet (`Zg==Zg==` read as `ff`) and
    // read a lone `Z` as nothing; a read page that decodes to `None` fails
    // the read upstream instead of showing an empty file.
    for malformed in [
        "Zg==Zg==", "Z", "Zg", "Zg=", "Zg===", "Zh==", "Y*Jj", "YWJj\n",
    ] {
        assert_eq!(base64_decode(malformed), None, "{malformed}");
    }
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
        reason: kind.into(),
        read,
        ..BellItem::default()
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
        workspaces_in(root.path()),
        vec![("mynet#a1b2c3d4".to_string(), dir.clone())]
    );
    assert_eq!(workspace_identity(&dir), Some(short_label(&founder)));
    assert_eq!(
        workspace_endpoint(&dir).as_deref(),
        Some("http://127.0.0.1:8844")
    );
}

#[test]
fn bell_renders_attribution_relation_and_change_actor() {
    let item = BellItem {
        seq: 1,
        change_seq: 4,
        source: "chat/message/general:2".into(),
        reason: "mention".into(),
        kind: "transferred_in:7".into(),
        actor: "account:9".into(),
        height: 3,
        read: false,
    };
    assert_eq!(bell_title(&item.reason), "Mention");
    let context = BellPresentation {
        seq: item.seq,
        title: "Mention changed · Alice".into(),
        detail: "Please review the launch checklist.".into(),
        ..BellPresentation::default()
    };
    assert_eq!(
        bell_presentation(&item, std::slice::from_ref(&context)),
        context
    );
    assert_eq!(bell_worst_severity(&[item]), "info");
}
