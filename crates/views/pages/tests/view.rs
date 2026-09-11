//! The view driven natively through the wire: the kernel pushes session
//! facts, the view reads the workspace, the open document and its comment
//! threads for itself through `rpc.view`, re-reads them on every `rpc.live`
//! hit, and every act leaves as `op.submit` carrying the pages message.

use pages_view::host::{Session, sidebar_width_after_delta};
use pages_view::{boot_native, tick_native};
use ui_lang_guest::testing::{answer, has_text, item, press, texts, type_into};
use ui_lang_guest::wire::{Frame, Request};

fn boot() -> Frame {
    boot_native();
    tick_native(Vec::new())
}

fn kinds(requests: &[Request]) -> Vec<&str> {
    requests
        .iter()
        .map(|request| request.kind.as_str())
        .collect()
}

fn request<'a>(frame: &'a Frame, kind: &str) -> &'a Request {
    frame
        .requests
        .iter()
        .find(|request| request.kind == kind)
        .unwrap_or_else(|| panic!("no `{kind}` request in {:?}", frame.requests))
}

fn session(connected: bool) -> Vec<u8> {
    serde_json::to_vec(&Session {
        connected,
        dark: false,
        chain: "mynet#d0cdf950".into(),
        route_page: String::new(),
        route_serial: 0,
    })
    .expect("session encodes")
}

/// The workspace index: two top-level pages, one cursor page.
fn page_list() -> Vec<u8> {
    serde_json::json!({ "pages": {
        "pages": [
            { "id": "alpha", "title": "Alpha", "parent": null },
            { "id": "beta", "title": "Beta", "parent": null }
        ],
        "has_more": false,
        "next_after": null
    }})
    .to_string()
    .into_bytes()
}

/// One page's blocks in preorder: the page record itself, then one paragraph.
fn page_blocks() -> Vec<u8> {
    serde_json::json!({ "page": {
        "blocks": [
            {
                "block_id": "alpha", "parent": null, "kind": "page",
                "text": "Alpha", "checked": false, "children": ["alpha-1"]
            },
            {
                "block_id": "alpha-1", "parent": "alpha", "kind": "paragraph",
                "text": "the first paragraph", "checked": false, "children": []
            }
        ],
        "next_after": null
    }})
    .to_string()
    .into_bytes()
}

fn no_threads() -> Vec<u8> {
    serde_json::json!({ "threads": [] }).to_string().into_bytes()
}

/// The canned reply for one `rpc.view` ask, chosen by the query it carries:
/// the page index, one page's blocks, or the grouped thread read.
fn answered(request: &Request) -> Vec<u8> {
    let ask: serde_json::Value =
        serde_json::from_slice(&request.payload).expect("a view ask decodes");
    let query = &ask["query"];
    assert_eq!(ask["target"], "pages", "a pages view asks the pages module");
    match query {
        _ if !query["list_pages"].is_null() => page_list(),
        _ if !query["get_page"].is_null() => page_blocks(),
        _ if !query["threads_for_targets"].is_null() => no_threads(),
        other => panic!("unexpected view ask {other}"),
    }
}

/// Boots, connects, and answers every read the register costs until the view
/// goes quiet — a landed register re-keys the subscription on the page it
/// found, so it settles over more than one round. Returns the settled frame
/// and the id of the live `rpc.live` subscription.
fn connected_with_register() -> (Frame, u64) {
    let frame = boot();
    let session_id = request(&frame, "pages.props").id;
    let mut frame = tick_native(vec![item(session_id, &session(true))]);
    let mut live = request(&frame, "rpc.live").id;
    for _ in 0..16 {
        if let Some(request) = frame.requests.iter().find(|one| one.kind == "rpc.live") {
            live = request.id;
        }
        let Some(view) = frame.requests.iter().find(|one| one.kind == "rpc.view") else {
            return (frame, live);
        };
        let reply = answered(view);
        frame = tick_native(vec![answer(view.id, &reply)]);
    }
    panic!("the register never settled")
}

/// At boot the view asks for the session only; connected, it reads the
/// workspace itself and the fold is the whole screen — the sidebar, the
/// header and the document.
#[test]
fn a_connected_view_reads_its_own_workspace() {
    let frame = boot();
    assert_eq!(
        kinds(&frame.requests),
        ["pages.props"],
        "only the session at boot: {:?}",
        frame.requests
    );
    assert!(has_text(&frame, "Not connected"), "{:?}", texts(&frame));

    let (frame, _live) = connected_with_register();
    for expected in ["Pages", "Alpha", "Beta"] {
        assert!(
            has_text(&frame, expected),
            "missing {expected:?} in {:?}",
            texts(&frame)
        );
    }
}

/// A pages block re-reads the workspace through the live subscription.
#[test]
fn a_live_hit_reads_the_workspace_again() {
    let (_, live) = connected_with_register();
    let frame = tick_native(vec![item(live, b"{}")]);
    assert_eq!(kinds(&frame.requests), ["rpc.view"], "{:?}", frame.requests);
    let ask: serde_json::Value =
        serde_json::from_slice(&frame.requests[0].payload).expect("a view ask decodes");
    assert!(
        !ask["query"]["list_pages"].is_null(),
        "the re-read starts at the workspace index: {ask}"
    );
}

/// A create mints its id through the kernel and leaves as `op.submit`
/// carrying the module's own `create_page`. The guest has no clock and no
/// entropy, so the id is the one thing it cannot make for itself.
#[test]
fn a_create_mints_its_id_and_leaves_as_a_signed_op() {
    let (frame, _) = connected_with_register();
    let frame = tick_native(press(&frame, "New page"));
    let frame = tick_native(type_into(&frame, "New page", "Runbook"));
    let frame = tick_native(press(&frame, "Create page"));

    let mint = request(&frame, "host.id");
    assert_eq!(mint.payload, b"page", "the prefix names the kind of record");

    let frame = tick_native(vec![answer(mint.id, b"page-1757000000-1")]);
    let submit = request(&frame, "op.submit");
    let op: serde_json::Value = serde_json::from_slice(&submit.payload).expect("an op decodes");
    assert_eq!(
        op,
        serde_json::json!({
            "target": "pages",
            "payload": { "create_page": {
                "page_id": "page-1757000000-1", "title": "Runbook", "blocks": []
            }}
        })
    );
}

/// The sidebar's drag is the view's own arithmetic, clamped at both ends.
#[test]
fn the_sidebar_drag_is_clamped_at_both_ends() {
    assert_eq!(sidebar_width_after_delta(240.0, 40.0, 1400.0), 280.0);
    assert!(sidebar_width_after_delta(240.0, -400.0, 1400.0) >= 120.0);
    assert!(sidebar_width_after_delta(240.0, 4000.0, 1400.0) <= 1400.0);
}
