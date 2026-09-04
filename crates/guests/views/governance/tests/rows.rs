//! The view driven natively: boot asks the app for the mode, the refresh
//! stream and the proposals; the module's reply becomes rows on screen; a
//! refresh item asks again.

use governance_view::{boot_native, tick_native};
use ui_lang_guest::frame::{Event, Frame, Request};
use ui_lang_guest::testing::{answer, has_text, item, redraw, texts};

fn boot() -> Frame {
    boot_native();
    tick_native(vec![
        Event::Resized {
            width: 960.0,
            height: 640.0,
        },
        redraw(),
    ])
}

fn kinds(requests: &[Request]) -> Vec<&str> {
    let mut kinds: Vec<&str> = requests
        .iter()
        .map(|request| request.kind.as_str())
        .collect();
    kinds.sort();
    kinds
}

fn request<'a>(frame: &'a Frame, kind: &str) -> &'a Request {
    frame
        .requests
        .iter()
        .find(|request| request.kind == kind)
        .unwrap_or_else(|| panic!("no `{kind}` request in {:?}", frame.requests))
}

const REPLY: &str = r#"{"proposals":[
  {"proposal_id":"p1","status":"open","action":{"signal":{"text":"Upgrade the forge"}},
   "votes":[["a",true],["b",false]],"deadline":120,"electorate":["a","b","c"],
   "voting_rule":{"threshold":{"required_yes":2}},"proposer":[1,2,3]},
  {"proposal_id":"p0","status":{"passed":{}},"action":{"set_share_mode":{"enabled":true}},
   "votes":[["a",true],["b",true]],"deadline":40,"electorate":["a","b"],
   "voting_rule":{"threshold":{"required_yes":2}},"proposer":[1,2,3]}
]}"#;

#[test]
fn boot_asks_for_the_mode_the_refreshes_and_the_proposals() {
    let frame = boot();
    assert_eq!(
        kinds(&frame.requests),
        ["host.refresh", "host.theme", "query.governance"],
        "{:?}",
        frame.requests
    );
    assert_eq!(
        request(&frame, "query.governance").payload,
        b"\"proposals\""
    );
}

#[test]
fn the_modules_reply_becomes_rows_open_first() {
    let frame = boot();
    let query = request(&frame, "query.governance").id;
    let frame = tick_native(vec![answer(query, REPLY.as_bytes()), redraw()]);
    let shown = texts(&frame);
    assert!(has_text(&frame, "1 open · 1 settled"), "{shown:?}");
    assert!(has_text(&frame, "Upgrade the forge"), "{shown:?}");
    assert!(
        has_text(&frame, "1 of 2 yes needed · 1 no · 3 eligible"),
        "{shown:?}"
    );
    assert!(has_text(&frame, "account shares"), "{shown:?}");
    let signal = shown.iter().position(|text| text == "signal");
    let share = shown.iter().position(|text| text == "set_share_mode");
    assert!(signal < share, "the open proposal draws first: {shown:?}");
}

#[test]
fn a_refresh_item_asks_for_the_proposals_again() {
    let frame = boot();
    let refresh = request(&frame, "host.refresh").id;
    let query = request(&frame, "query.governance").id;
    let frame = tick_native(vec![answer(query, REPLY.as_bytes()), redraw()]);
    assert!(frame.requests.is_empty(), "{:?}", frame.requests);
    let frame = tick_native(vec![item(refresh, &[]), redraw()]);
    assert_eq!(
        kinds(&frame.requests),
        ["query.governance"],
        "{:?}",
        frame.requests
    );
}

#[test]
fn a_refused_query_shows_its_reason() {
    let frame = boot();
    let query = request(&frame, "query.governance").id;
    let frame = tick_native(vec![
        ui_lang_guest::testing::refuse(query, "governance query failed: no node"),
        redraw(),
    ]);
    assert!(
        has_text(&frame, "governance query failed: no node"),
        "{:?}",
        texts(&frame)
    );
}
