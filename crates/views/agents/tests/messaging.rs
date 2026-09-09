//! The messages pane: what the network said is what the screen says.
//!
//! Every test here defends one sentence of the agent-messaging contract that a
//! plausible implementation gets wrong: a refusal drawn as an empty
//! conversation, a delivery state drawn as work done, an unread receipt drawn
//! as `stored`, a draft thrown away by a refused send, or a page drawn as if it
//! were the whole conversation.

use agents_view::host::{
    AgentsProps, MessagingBinding, MessagingMessage, MessagingProps, MessagingSeat, OpenConversation,
    SendMessage,
};
use agents_view::{boot_native, tick_native};
use ui_lang_guest::testing::{has_text, item, keys, pick, press, texts, type_into};
use ui_lang_guest::wire::{Frame, Node, Request};

const BODY_HINT: &str = "what to send…";
const PARTICIPANT_HINT: &str = "the participant id you own…";
const CONVERSATION_HINT: &str = "the conversation id…";
const MESSAGES_TAB: &str = "Show agent messages";

fn seat(participant: &str, role: &str, you: bool) -> MessagingSeat {
    MessagingSeat {
        participant: participant.into(),
        role: role.into(),
        you,
    }
}

fn message(seq: i64, delivery: &str) -> MessagingMessage {
    MessagingMessage {
        seq,
        sender: "codex-b".into(),
        recipient: "claude-a".into(),
        kind: "question".into(),
        body: "does the review cover the migration?".into(),
        body_bytes: 36,
        shown_bytes: 36,
        references: String::new(),
        reply_to: 0,
        task: String::new(),
        task_attempt: 0,
        delivery: delivery.into(),
        delivery_reason: String::new(),
        mine: false,
        expires_at: 90_000,
        admitted_at: 41,
    }
}

/// A conversation this reader is a sending member of.
fn open(messages: Vec<MessagingMessage>) -> MessagingProps {
    MessagingProps {
        participant: "claude-a".into(),
        conversation: "standup".into(),
        network: "duck-1".into(),
        topic: "release review".into(),
        roster: vec![seat("claude-a", "member", true), seat("codex-b", "member", false)],
        binding: MessagingBinding {
            present: true,
            device: "laptop".into(),
            credential: "4".into(),
            detached: false,
        },
        messages,
        may_read: true,
        may_send: true,
        floor_seq: 0,
        from_seq: 0,
        next_seq: 12,
        undelivered: 1,
        queued_bytes: 512,
        max_body_bytes: 16 * 1024,
        answered: true,
        visibility: "Committed state on this network is replicated in plaintext.".into(),
        ..MessagingProps::default()
    }
}

fn register(messaging: MessagingProps) -> Vec<u8> {
    serde_json::to_vec(&AgentsProps {
        rows: Vec::new(),
        capabilities: Vec::new(),
        actions: Vec::new(),
        account: "7".into(),
        committed: 0,
        connected: true,
        answered: true,
        dark: false,
        messaging,
    })
    .expect("props encode")
}

/// Boot, subscribe, push the panel, and switch to the messages pane.
fn opened(messaging: MessagingProps) -> (u64, Frame) {
    boot_native();
    let frame = tick_native(Vec::new());
    let subscription = frame.requests[0].id;
    let frame = tick_native(vec![item(subscription, &register(messaging))]);
    let frame = tick_native(press(&frame, MESSAGES_TAB));
    (subscription, frame)
}

fn one_intent(frame: &Frame) -> &Request {
    let [intent] = frame.requests.as_slice() else {
        panic!("one intent, got {:?}", frame.requests);
    };
    intent
}

/// The full key of the control whose path ends in `suffix`: the tree's own key
/// path, so a nesting change moves with the test instead of breaking it.
fn key_ending(frame: &Frame, suffix: &str) -> String {
    keys(frame)
        .into_iter()
        .find(|key| key.ends_with(suffix))
        .unwrap_or_else(|| panic!("no key ending {suffix:?} in {:?}", keys(frame)))
}

#[test]
fn a_refused_read_is_a_refusal_and_never_an_empty_conversation() {
    let refused = MessagingProps {
        participant: "claude-a".into(),
        conversation: "standup".into(),
        network: "duck-1".into(),
        denied: "not_permitted".into(),
        answered: true,
        ..MessagingProps::default()
    };
    let (_, frame) = opened(refused);
    assert!(
        has_text(&frame, "This participant is revoked, or is not on this conversation's roster."),
        "{:?}",
        texts(&frame)
    );
    // the module's own token stays on screen beside the sentence
    assert!(has_text(&frame, "not_permitted"), "{:?}", texts(&frame));
    // THE COUNTEREXAMPLE: a panel that drew its empty-list plate whenever the
    // message list is empty would say this conversation has no messages in it.
    assert!(
        !has_text(&frame, "No messages in the retained range this page covers."),
        "a refusal was drawn as an empty conversation: {:?}",
        texts(&frame)
    );
    assert!(!has_text(&frame, "Send"), "{:?}", texts(&frame));
}

#[test]
fn an_accepted_delivery_never_claims_the_model_read_it_or_the_task_ran() {
    let mut accepted = message(7, "adapter_accepted");
    accepted.task = "job-19".into();
    accepted.task_attempt = 2;
    let (_, frame) = opened(open(vec![accepted]));
    assert!(has_text(&frame, "ACCEPTED"), "{:?}", texts(&frame));
    assert!(
        texts(&frame).iter().any(|text| text.contains(
            "not that the model read it, understood it, acted on it, or claimed a task"
        )),
        "{:?}",
        texts(&frame)
    );
    // a task reference is a reference: this screen resolves no execution state
    assert!(
        has_text(&frame, "task job-19 · attempt 2 · execution status not resolved here"),
        "{:?}",
        texts(&frame)
    );
}

#[test]
fn a_receipt_this_app_could_not_read_is_unknown_rather_than_stored() {
    // the app leaves `delivery` empty when it could not resolve the record —
    // the counterexample is defaulting it to the most reassuring state
    let (_, frame) = opened(open(vec![message(7, "")]));
    assert!(has_text(&frame, "UNKNOWN"), "{:?}", texts(&frame));
    assert!(
        has_text(&frame, "This app does not know this delivery state."),
        "{:?}",
        texts(&frame)
    );
    assert!(!has_text(&frame, "STORED"), "{:?}", texts(&frame));
}

#[test]
fn the_composer_sends_exactly_what_was_typed() {
    let (_, frame) = opened(open(vec![message(7, "queued")]));
    let recipient = key_ending(&frame, "/recipient");
    let kind = key_ending(&frame, "/kind");
    let frame = tick_native(pick(&frame, &recipient, "codex-b"));
    let frame = tick_native(pick(&frame, &kind, "question"));
    // trailing space and newline included: a body is submitted as written
    let frame = tick_native(type_into(&frame, BODY_HINT, "please review \n"));
    assert!(frame.requests.is_empty(), "a draft leaves nothing: {:?}", frame.requests);

    let frame = tick_native(press(&frame, "Send message"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "agents.messaging_send");
    assert_eq!(
        serde_json::from_slice::<SendMessage>(&intent.payload).expect("decodes"),
        SendMessage {
            kind: "question".into(),
            recipient: "codex-b".into(),
            body: "please review \n".into(),
            reply_to: 0,
        }
    );
}

#[test]
fn a_reply_names_the_message_it_answers_and_offers_the_result_kind() {
    let (_, frame) = opened(open(vec![message(7, "queued")]));
    // `result` is not offered until there is something for it to answer
    let kind = key_ending(&frame, "/kind");
    let Some(Node::PickList { options, .. }) = ui_lang_guest::testing::find(&frame, &kind) else {
        panic!("no kind pick in {:?}", keys(&frame));
    };
    assert_eq!(options.as_slice(), ["notice", "question", "task_request"]);

    let frame = tick_native(press(&frame, "Reply to this message"));
    assert!(has_text(&frame, "replying to sequence 7"), "{:?}", texts(&frame));
    let Some(Node::PickList { options, .. }) = ui_lang_guest::testing::find(&frame, &kind) else {
        panic!("no kind pick in {:?}", keys(&frame));
    };
    assert_eq!(
        options.as_slice(),
        ["notice", "question", "task_request", "result"]
    );

    let recipient = key_ending(&frame, "/recipient");
    let frame = tick_native(pick(&frame, &recipient, "codex-b"));
    let frame = tick_native(type_into(&frame, BODY_HINT, "it does"));
    let frame = tick_native(press(&frame, "Send message"));
    let sent: SendMessage =
        serde_json::from_slice(&one_intent(&frame).payload).expect("decodes");
    assert_eq!(sent.reply_to, 7);
}

#[test]
fn a_refused_send_keeps_the_draft_and_an_admitted_one_clears_it() {
    let (subscription, frame) = opened(open(vec![message(7, "queued")]));
    let recipient = key_ending(&frame, "/recipient");
    let frame = tick_native(pick(&frame, &recipient, "codex-b"));
    let frame = tick_native(type_into(&frame, BODY_HINT, "ship it"));
    let frame = tick_native(press(&frame, "Send message"));
    assert_eq!(one_intent(&frame).kind, "agents.messaging_send");

    // THE NETWORK REFUSED. The draft is still in the box, in the same scope,
    // and the refusal is on screen with it.
    let refused = MessagingProps {
        send_error: "the mailbox is full — retry later".into(),
        ..open(vec![message(7, "queued")])
    };
    let frame = tick_native(vec![item(subscription, &register(refused))]);
    assert!(has_text(&frame, "ship it"), "the draft was lost: {:?}", texts(&frame));
    assert!(
        has_text(&frame, "the mailbox is full — retry later"),
        "{:?}",
        texts(&frame)
    );
    // and nothing was drawn into the conversation on the way out
    assert!(!has_text(&frame, "ship it • sent"), "{:?}", texts(&frame));

    // THE NETWORK ADMITTED IT. Only now does the box empty.
    let admitted = MessagingProps {
        sent_seq: 13,
        ..open(vec![message(7, "queued")])
    };
    let frame = tick_native(vec![item(subscription, &register(admitted))]);
    assert!(!has_text(&frame, "ship it"), "{:?}", texts(&frame));
    assert!(has_text(&frame, BODY_HINT), "{:?}", texts(&frame));
}

#[test]
fn a_bounded_body_says_what_it_is_showing_and_what_is_stored() {
    let mut clipped = message(7, "queued");
    clipped.body = "a".repeat(1024);
    clipped.body_bytes = 4096;
    clipped.shown_bytes = 1024;
    let (_, frame) = opened(open(vec![clipped]));
    assert!(
        has_text(
            &frame,
            "showing 1024 of 4096 bytes — the stored message is unchanged"
        ),
        "{:?}",
        texts(&frame)
    );
}

#[test]
fn a_history_gap_offers_a_resync_instead_of_a_page() {
    let gapped = MessagingProps {
        history_gap: true,
        floor_seq: 40,
        ..open(Vec::new())
    };
    let (_, frame) = opened(gapped);
    assert!(
        texts(&frame)
            .iter()
            .any(|text| text.starts_with("This conversation was pruned past the page")),
        "{:?}",
        texts(&frame)
    );
    // the paging row is NOT drawn over a gap: advancing the cursor is exactly
    // what must not happen here
    assert!(!has_text(&frame, "Older"), "{:?}", texts(&frame));
    let frame = tick_native(press(&frame, "Resync from the retained floor"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "agents.messaging_page");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&intent.payload).expect("decodes")["from_seq"],
        40
    );
}

#[test]
fn an_observer_seat_is_told_why_it_cannot_send() {
    let observing = MessagingProps {
        may_send: false,
        roster: vec![seat("claude-a", "observer", true), seat("codex-b", "member", false)],
        ..open(vec![message(7, "queued")])
    };
    let (_, frame) = opened(observing);
    assert!(
        has_text(
            &frame,
            "You hold an observer seat on this conversation: you receive it and do not send on it."
        ),
        "{:?}",
        texts(&frame)
    );
    assert!(has_text(&frame, "observer · subscribes, does not send"), "{:?}", texts(&frame));
    // the messages still read — an observer subscribes
    assert!(has_text(&frame, "does the review cover the migration?"), "{:?}", texts(&frame));
    assert!(!has_text(&frame, BODY_HINT), "{:?}", texts(&frame));
}

#[test]
fn opening_a_conversation_names_the_participant_and_conversation_the_reader_typed() {
    let (_, frame) = opened(MessagingProps::default());
    let frame = tick_native(type_into(&frame, PARTICIPANT_HINT, " claude-a "));
    let frame = tick_native(type_into(&frame, CONVERSATION_HINT, "standup"));
    let frame = tick_native(press(&frame, "Open conversation"));
    let intent = one_intent(&frame);
    assert_eq!(intent.kind, "agents.messaging_open");
    assert_eq!(
        serde_json::from_slice::<OpenConversation>(&intent.payload).expect("decodes"),
        OpenConversation {
            participant: "claude-a".into(),
            conversation: "standup".into(),
        }
    );
}

#[test]
fn the_panel_discloses_the_binding_the_roster_and_the_networks_visibility() {
    let (_, frame) = opened(open(vec![message(7, "queued")]));
    assert!(has_text(&frame, "Bound to laptop under credential 4."), "{:?}", texts(&frame));
    assert!(has_text(&frame, "1 undelivered · 512 bytes queued"), "{:?}", texts(&frame));
    assert!(has_text(&frame, "duck-1"), "{:?}", texts(&frame));
    assert!(
        has_text(
            &frame,
            "Committed state on this network is replicated in plaintext."
        ),
        "{:?}",
        texts(&frame)
    );
    // the page is shown against the conversation's own tip: no silent tail
    assert!(has_text(&frame, "1 shown · sequences 7–7 of 12"), "{:?}", texts(&frame));
}
