//! The facts the host pushes are what the screen shows; every act leaves as
//! an intent carrying what the reader typed, and a committed op the host
//! reports consumes only the drafts it read.

use settings_view::host::{AccountKeyRow, KeyAdd, Name, SettingsProps, Tab, Unlock};
use settings_view::{boot_native, tick_native};
use ui_lang_guest::testing::{has_text, item, press, submit, texts, type_into};
use ui_lang_guest::wire::Frame;

fn facts() -> SettingsProps {
    SettingsProps {
        dark: false,
        connected: true,
        loading: false,
        status: "Connected".into(),
        busy: false,
        recovering: false,
        appearance: "system".into(),
        desktop_notifications: true,
        unlocked: true,
        account_name: "duck".into(),
        network_name: "testnet".into(),
        connected_rpc: "http://127.0.0.1:1".into(),
        account_ceremony_phase: String::new(),
        account_ceremony_qr: String::new(),
        account_ceremony_detail: String::new(),
        account_ceremony_left: String::new(),
        settings_key_state: "sealed".into(),
        settings_key_path: "/keys/user.key".into(),
        tier: "validator".into(),
        admin: true,
        members_line: "3 humans · 1 agent".into(),
        members_answered: true,
        account_number: "42".into(),
        account_renaming: false,
        account_exists: true,
        account_keys: 2,
        account_key_rows: vec![AccountKeyRow {
            scheme: "ed25519".into(),
            pubkey: "ab12cd34".into(),
            label: "laptop".into(),
        }],
        account_busy: false,
        account_ticket: String::new(),
        drafts_cleared: 0,
        drafts_scope: String::new(),
    }
}

fn encoded(props: &SettingsProps) -> Vec<u8> {
    serde_json::to_vec(props).expect("props encode")
}

/// Boot and push the facts; returns the subscription id and the frame.
fn shown(props: &SettingsProps) -> (u64, Frame) {
    boot_native();
    let frame = tick_native(Vec::new());
    assert_eq!(frame.requests[0].kind, "settings.props");
    let subscription = frame.requests[0].id;
    let frame = tick_native(vec![item(subscription, &encoded(props))]);
    (subscription, frame)
}

fn one_intent(frame: &Frame) -> &ui_lang_guest::wire::Request {
    let [intent] = frame.requests.as_slice() else {
        panic!("one intent, got {:?}", frame.requests);
    };
    intent
}

#[test]
fn the_facts_the_host_pushes_are_what_the_screen_shows_and_a_link_leaves_as_a_tab() {
    let (_, frame) = shown(&facts());
    for expected in ["Settings", "Theme"] {
        assert!(
            has_text(&frame, expected),
            "missing {expected:?} in {:?}",
            texts(&frame)
        );
    }
    assert!(frame.requests.is_empty(), "{:?}", frame.requests);
    let frame = tick_native(press(&frame, "Network"));
    assert!(
        has_text(&frame, "3 humans · 1 agent"),
        "{:?}",
        texts(&frame)
    );
    let frame = tick_native(press(&frame, "manage"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "settings.tab");
    assert_eq!(
        serde_json::from_slice::<Tab>(&intent.payload).expect("decodes"),
        Tab {
            tab: "members".into()
        }
    );
}

#[test]
fn the_password_leaves_only_as_an_unlock_and_never_comes_back() {
    let locked = SettingsProps {
        unlocked: false,
        ..facts()
    };
    let (subscription, frame) = shown(&locked);
    let frame = tick_native(press(&frame, "Security"));
    let frame = tick_native(type_into(&frame, "unlock signing…", "hunter2"));
    assert!(frame.requests.is_empty(), "typing runs no handler");
    let frame = tick_native(submit(&frame, "unlock signing…"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "settings.unlock");
    assert_eq!(
        serde_json::from_slice::<Unlock>(&intent.payload).expect("decodes"),
        Unlock {
            password: "hunter2".into()
        }
    );
    // the seat crosses back as a flag, never as the password
    let frame = tick_native(vec![item(subscription, &encoded(&facts()))]);
    assert!(
        has_text(&frame, "Signing unlocked for this session."),
        "{:?}",
        texts(&frame)
    );
    assert!(!has_text(&frame, "hunter2"));
}

#[test]
fn a_committed_op_consumes_only_the_drafts_it_read() {
    let (subscription, frame) = shown(&facts());
    let frame = tick_native(press(&frame, "Account"));
    let frame = tick_native(type_into(&frame, "rename account…", "  mallard  "));
    let frame = tick_native(type_into(&frame, "paste its ed25519 key (hex)…", "ff00"));
    let frame = tick_native(press(&frame, "Rename"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "settings.rename");
    assert_eq!(
        serde_json::from_slice::<Name>(&intent.payload).expect("decodes"),
        Name {
            name: "mallard".into()
        }
    );
    // the rename landed: the name draft goes, the pasted key stays …
    let renamed = SettingsProps {
        drafts_cleared: 1,
        drafts_scope: "name".into(),
        ..facts()
    };
    let frame = tick_native(vec![item(subscription, &encoded(&renamed))]);
    let frame = tick_native(press(&frame, "Mint ticket"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "settings.key_add");
    assert_eq!(
        serde_json::from_slice::<KeyAdd>(&intent.payload).expect("decodes"),
        KeyAdd {
            pubkey: "ff00".into(),
            label: String::new()
        }
    );
    // … and the same report pushed again consumes nothing more
    let frame = tick_native(vec![item(subscription, &encoded(&renamed))]);
    let frame = tick_native(press(&frame, "Mint ticket"));
    assert_eq!(one_intent(&frame).kind, "settings.key_add");
}
