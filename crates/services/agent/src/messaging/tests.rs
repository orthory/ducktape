//! the delivery plane's own tests: binding fencing, the durable ordering, and
//! the states a delivery may and may not reach.
//!
//! These drive the plane through its real dispatch with a real outbox on disk.
//! What they deliberately do NOT do is claim anything about a model: a binding
//! here resolves to `codex queue` pointed at an executable this test owns, so
//! what is under test is the BRIDGE — ownership, fencing, ordering, and the
//! mapping from a provider's answer to a delivery state. Whether a provider
//! accepts is the provider's business, and proving that needs the provider.

use std::path::PathBuf;

use super::*;

/// the network every test plane belongs to. The outbox is bound to it, so the
/// prior-run fixture below must open under the same one.
const NETWORK: &str = "ducktape-test@aaaa";

/// a scratch directory that is this test's alone.
///
/// The name is `[a-z0-9-]` and nothing else, on purpose: it is interpolated
/// into a `/bin/sh` script below, and a thread id rendered with `{:?}` puts
/// PARENTHESES in a path — which made every stub a syntax error that never ran,
/// while the tests waiting on it hung rather than failed.
fn scratch(name: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let unique = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "ducktape-collab-{name}-{}-{unique}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// write the operator's local attachment map.
fn attach(dir: &Path, device: &str, target: Target) -> PathBuf {
    let path = dir.join("attachments.json");
    let attachments = Attachments {
        devices: HashMap::from([(device.to_string(), target)]),
    };
    std::fs::write(&path, serde_json::to_string(&attachments).expect("encodes")).expect("writes");
    path
}

/// a stand-in for the Codex CLI: a real executable that records every argv it
/// is given, one line per invocation, and exits with `exit_code`.
///
/// A REAL child process, not an in-process double, because the thing under
/// test is precisely how this plane invokes one: that a body arrives as a
/// single argument, that the exit status is what decides, and that two
/// deliveries reach it in order. It is a stub of the PROVIDER, and it proves
/// nothing whatever about a model — see the module doc.
fn stub_codex(dir: &Path, exit_code: i32) -> PathBuf {
    stub(dir, &format!("exit {exit_code}\n"))
}

/// the same stub, but its FIRST invocation does not return until `gate` — a
/// fifo — is opened for writing and closed. That gives a test a real HOLD: one
/// message is inside a provider and the ones behind it are waiting, with no
/// sleep anywhere.
///
/// The gate is SPENT once used, and that is not a detail. A stub that holds
/// every invocation strands the whole test binary the moment a mutation makes
/// a second message reach it — which is how a failing assertion turns into a
/// hung build that says nothing. Held once, the second offer returns
/// immediately and the test fails by name.
fn stub_codex_holding(dir: &Path, gate: &Path) -> PathBuf {
    std::process::Command::new("mkfifo")
        .arg(gate)
        .status()
        .expect("mkfifo runs");
    let gate = quoted(gate);
    let spent = quoted(&dir.join("gate.spent"));
    stub(
        dir,
        &format!("if [ -p {gate} ]; then read _ < {gate}; mv {gate} {spent}; fi\nexit 0\n"),
    )
}

/// write an executable stub that records its argv, then runs `tail`.
///
/// The argv is recorded as exactly ONE line per invocation: a wrapped message
/// spans many lines, and a log that inherits them cannot say how many times the
/// provider was called — which is what every one of these tests asks it.
fn stub(dir: &Path, tail: &str) -> PathBuf {
    let log = quoted(&dir.join("invocations.log"));
    let program = dir.join("stub-codex");
    std::fs::write(
        &program,
        format!(
            "#!/bin/sh\nprintf '%s' \"$*\" | tr '\\n' ' ' >> {log}\nprintf '\\n' >> {log}\n{tail}"
        ),
    )
    .expect("writes the stub");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755))
            .expect("makes the stub executable");
    }
    program
}

/// one path, safe to drop into a `/bin/sh` script.
///
/// Single quotes, and a refusal rather than an escape for a path containing
/// one: nothing here produces such a path, and a quoting scheme with a clever
/// case is how a stub silently becomes a syntax error again.
fn quoted(path: &Path) -> String {
    let text = path.to_str().expect("a scratch path is utf-8");
    assert!(
        !text.contains('\''),
        "a scratch path may not be quoted: {text}"
    );
    format!("'{text}'")
}

/// every invocation the stub recorded, in order.
fn invocations(dir: &Path) -> Vec<String> {
    std::fs::read_to_string(dir.join("invocations.log"))
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

/// a plane over a scratch directory, plus its event receiver.
async fn plane(dir: &Path, attachments: PathBuf) -> (Deliveries, mpsc::Receiver<wire::Event>) {
    plane_with(dir, attachments, PathBuf::from("codex")).await
}

async fn plane_with(
    dir: &Path,
    attachments: PathBuf,
    codex: PathBuf,
) -> (Deliveries, mpsc::Receiver<wire::Event>) {
    let (events, rx) = mpsc::channel(64);
    let plane = Deliveries::open(
        &dir.join("outbox"),
        NETWORK,
        attachments,
        dir.join("registry"),
        codex,
        None,
        events,
    )
    .await
    .expect("the plane opens");
    (plane, rx)
}

/// wait for the next delivery receipt. Synchronized on the event, never on a
/// sleep: the lane is a task, so its work lands when it lands.
async fn next_delivery(rx: &mut mpsc::Receiver<wire::Event>) -> (State, Option<String>) {
    loop {
        let event = rx.recv().await.expect("the event lane stays open");
        if let wire::Event::MsgDelivery { state, reason, .. } = event {
            return (state, reason);
        }
    }
}

fn bind(generation: u64) -> wire::Bind {
    wire::Bind {
        conversation: "conv-1".to_string(),
        participant: "p-recipient".to_string(),
        generation,
        device: "laptop-a".to_string(),
    }
}

fn deliver(seq: u64, binding_generation: u64) -> Box<wire::Deliver> {
    Box::new(wire::Deliver {
        conversation: "conv-1".to_string(),
        participant: "p-recipient".to_string(),
        seq,
        binding_generation,
        message_id: wire::MessageId {
            generation: 2,
            sequence: seq,
        },
        sender: "p-sender".to_string(),
        kind: wire::Kind::Question,
        task: None,
        reply_to: None,
        body: "does the review cover the migration?".to_string(),
        references: Vec::new(),
        expires_at: 2_000,
        network_now: 1_000,
        urgent: false,
    })
}

/// drain the events emitted so far, without waiting for more.
fn drained(rx: &mut mpsc::Receiver<wire::Event>) -> Vec<wire::Event> {
    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }
    events
}

#[tokio::test]
async fn binding_a_named_device_reports_what_that_session_can_do() {
    let dir = scratch("bind");
    let attachments = attach(
        &dir,
        "laptop-a",
        Target::CodexThread {
            thread_id: "thread-abc".to_string(),
        },
    );
    let (plane, mut rx) = plane(&dir, attachments).await;

    plane.dispatch(Messaging::Bind(bind(1))).await;

    let events = drained(&mut rx);
    let [
        wire::Event::MsgBound {
            generation,
            capabilities,
            ..
        },
    ] = events.as_slice()
    else {
        panic!("a bind answers with exactly one bound event: {events:?}");
    };
    assert_eq!(*generation, 1);
    // queueing into an existing codex session reports acceptance (a non-zero
    // exit is a real refusal) but cannot steer a running turn.
    assert!(capabilities.reports_acceptance);
    assert!(!capabilities.steers_active_turn);
    assert_eq!(plane.bound(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Two devices cannot both hold one participant's input. The one arriving with
/// a generation the current holder has already reached loses — which is
/// exactly the shape of a disconnected device coming back with its old number.
#[tokio::test]
async fn a_stale_generation_cannot_take_a_binding_that_moved_on() {
    let dir = scratch("stale-bind");
    let attachments = attach(
        &dir,
        "laptop-a",
        Target::CodexThread {
            thread_id: "thread-abc".to_string(),
        },
    );
    let (plane, mut rx) = plane(&dir, attachments).await;

    plane.dispatch(Messaging::Bind(bind(4))).await;
    let _ = drained(&mut rx);

    // the returning device, still believing it holds generation 3 — and the
    // equal-generation replay, which is the same claim by another name.
    for stale in [3, 4] {
        plane.dispatch(Messaging::Bind(bind(stale))).await;
        let events = drained(&mut rx);
        let [wire::Event::MsgBindRefused { reason, .. }] = events.as_slice() else {
            panic!("generation {stale} must be refused: {events:?}");
        };
        assert_eq!(*reason, BindRefusal::StaleGeneration);
    }

    // and the legitimate replacement still succeeds.
    plane.dispatch(Messaging::Bind(bind(5))).await;
    assert!(matches!(
        drained(&mut rx).as_slice(),
        [wire::Event::MsgBound { generation: 5, .. }]
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A device returning from a disconnect must not be able to release the
/// attachment that replaced it.
#[tokio::test]
async fn a_stale_generation_cannot_release_the_binding_that_replaced_it() {
    let dir = scratch("stale-unbind");
    let attachments = attach(
        &dir,
        "laptop-a",
        Target::CodexThread {
            thread_id: "thread-abc".to_string(),
        },
    );
    let (plane, mut rx) = plane(&dir, attachments).await;
    plane.dispatch(Messaging::Bind(bind(4))).await;
    let _ = drained(&mut rx);

    plane
        .dispatch(Messaging::Unbind {
            conversation: "conv-1".to_string(),
            participant: "p-recipient".to_string(),
            generation: 3,
        })
        .await;
    assert_eq!(plane.bound(), 1, "a stale unbind must not detach anything");

    plane
        .dispatch(Messaging::Unbind {
            conversation: "conv-1".to_string(),
            participant: "p-recipient".to_string(),
            generation: 4,
        })
        .await;
    assert_eq!(plane.bound(), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test]
async fn a_device_this_daemon_does_not_attach_is_refused_by_name() {
    let dir = scratch("unknown-device");
    let attachments = attach(
        &dir,
        "some-other-laptop",
        Target::CodexThread {
            thread_id: "thread-abc".to_string(),
        },
    );
    let (plane, mut rx) = plane(&dir, attachments).await;

    plane.dispatch(Messaging::Bind(bind(1))).await;
    let events = drained(&mut rx);
    let [wire::Event::MsgBindRefused { reason, .. }] = events.as_slice() else {
        panic!("an unattached device must be refused: {events:?}");
    };
    assert_eq!(*reason, BindRefusal::UnknownDevice);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The reconnect case, from this plane's side: a binding is not torn down by
/// anything the link does, so a delivery that arrives after the node has been
/// away still finds it. A pty session dies with its link; a binding must not,
/// because the provider session it names is not this daemon's process.
#[tokio::test]
async fn a_binding_outlives_the_link_that_created_it() {
    let dir = scratch("survives-link");
    let codex = stub_codex(&dir, 0);
    let attachments = attach(
        &dir,
        "laptop-a",
        Target::CodexThread {
            thread_id: "thread-abc".to_string(),
        },
    );
    let (plane, mut rx) = plane_with(&dir, attachments, codex).await;
    plane.dispatch(Messaging::Bind(bind(1))).await;
    let _ = drained(&mut rx);

    // the node's link drops and is redialed. What that does to the TERMINAL
    // plane is end every session; what it must do to this one is nothing at
    // all — the provider session a binding names is not this daemon's process,
    // was not started by it, and is exactly what a reconnecting node expects
    // to still be attached.
    //
    // The daemon's own teardown is the check that says so: `Sessions` has
    // `close_all`, and this plane deliberately has no equivalent, so there is
    // nothing a disconnect COULD call. See
    // `nothing_on_this_plane_can_be_torn_down_by_a_disconnect`.
    assert_eq!(
        plane.bound(),
        1,
        "a binding must survive the link that carried its bind"
    );

    // and the message that arrives after the reconnect goes to the SAME
    // binding — no re-bind, and no replacement session spawned for it.
    plane.dispatch(Messaging::Deliver(deliver(7, 1))).await;
    assert_eq!(next_delivery(&mut rx).await.0, State::Queued);
    assert_eq!(next_delivery(&mut rx).await.0, State::AdapterAccepted);
    assert_eq!(invocations(&dir).len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The invariant the reconnect case rests on, as a shape assertion: this plane
/// exposes no bulk teardown, so a disconnect has nothing to call on it.
///
/// A lint test rather than a comment because the mistake is so easy and so
/// quiet — a `close_all` added here "for symmetry" with the terminal plane
/// would silently detach every personal session on every node restart, and no
/// behavioural test would fail until someone reconnected mid-conversation.
#[test]
fn nothing_on_this_plane_can_be_torn_down_by_a_disconnect() {
    let source = include_str!("mod.rs");
    for teardown in ["fn close_all", "fn clear", "fn detach_all", "fn drop_all"] {
        assert!(
            !source.contains(teardown),
            "`{teardown}` on the delivery plane would let a dropped link detach \
             personal sessions this daemon does not own"
        );
    }
}

/// A delivery naming a generation this device does not hold is not ours to
/// perform: the attachment it was aimed at has been replaced, so performing it
/// would put a stale target's traffic into the current session.
#[tokio::test]
async fn a_delivery_for_a_replaced_binding_is_not_accepted() {
    let dir = scratch("stale-delivery");
    let attachments = attach(
        &dir,
        "laptop-a",
        Target::CodexThread {
            thread_id: "thread-abc".to_string(),
        },
    );
    let (plane, mut rx) = plane(&dir, attachments).await;
    plane.dispatch(Messaging::Bind(bind(5))).await;
    let _ = drained(&mut rx);

    plane.dispatch(Messaging::Deliver(deliver(7, 4))).await;
    assert!(
        drained(&mut rx).is_empty(),
        "a stale-generation delivery must not be taken into the outbox"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// With no binding at all the message is simply not ours yet. Nothing is
/// claimed, and no session is created to receive it.
#[tokio::test]
async fn a_delivery_with_no_binding_is_left_for_whoever_binds() {
    let dir = scratch("unbound-delivery");
    let attachments = attach(
        &dir,
        "laptop-a",
        Target::CodexThread {
            thread_id: "thread-abc".to_string(),
        },
    );
    let (plane, mut rx) = plane(&dir, attachments).await;

    plane.dispatch(Messaging::Deliver(deliver(7, 1))).await;
    assert!(drained(&mut rx).is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Expiry is decided against the AGREED clock the frame carries. This daemon
/// has no opinion about what time it is — and a message already past its
/// deadline never reaches a provider at all.
#[tokio::test]
async fn an_expired_message_is_never_offered_to_a_provider() {
    let dir = scratch("expired");
    let attachments = attach(
        &dir,
        "laptop-a",
        // an executable that does not exist: if this were ever run, the
        // outcome would be `queue_cli_unavailable` instead of `deadline_passed`
        // and this test would say so.
        Target::CodexThread {
            thread_id: "thread-abc".to_string(),
        },
    );
    let (plane, mut rx) = plane(&dir, attachments).await;
    plane.dispatch(Messaging::Bind(bind(1))).await;
    let _ = drained(&mut rx);

    let mut expired = deliver(7, 1);
    expired.network_now = 3_000;
    expired.expires_at = 2_000;
    plane.dispatch(Messaging::Deliver(expired)).await;

    // waited for, not drained: the expiry is decided on the binding's lane, so
    // the second receipt is the lane having got to it.
    let reported = [next_delivery(&mut rx).await, next_delivery(&mut rx).await];
    assert_eq!(
        reported,
        [
            (State::Queued, None),
            (State::Expired, Some("deadline_passed".to_string())),
        ],
        "an expired message is taken into the outbox and then expired, never offered"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Queue ownership is durable BEFORE it is acknowledged. The journal must
/// already name the item by the time the node is told `Queued` — otherwise a
/// crash right there loses a message the network believes this daemon holds.
#[tokio::test]
async fn ownership_is_on_disk_before_it_is_acknowledged() {
    let dir = scratch("durable-first");
    let attachments = attach(
        &dir,
        "laptop-a",
        Target::CodexThread {
            thread_id: "thread-abc".to_string(),
        },
    );
    let (plane, mut rx) = plane(&dir, attachments).await;
    plane.dispatch(Messaging::Bind(bind(1))).await;
    let _ = drained(&mut rx);

    plane.dispatch(Messaging::Deliver(deliver(7, 1))).await;
    assert_eq!(next_delivery(&mut rx).await.0, State::Queued);
    // the attempt happens on the lane, so its receipt is what says the lane
    // has been there — reading the file before that would be reading a race.
    next_delivery(&mut rx).await;

    let journal =
        std::fs::read_to_string(dir.join("outbox").join("outbox.jsonl")).expect("the journal");
    assert!(
        journal.contains(r#""r":"queued""#),
        "ownership must be recorded before it is claimed: {journal}"
    );
    // and the attempt is recorded too — the crash boundary depends on that
    // line existing before the provider is touched.
    assert!(
        journal.contains(r#""r":"attempting""#),
        "the attempt must be recorded before the provider is offered: {journal}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Recovery REPORTS, it does not re-drive. An item left mid-attempt by a crash
/// comes back as `DeliveryUnknown` and is announced as such — never quietly
/// re-offered to a model that may already have acted on it.
#[tokio::test]
async fn recovery_reports_an_unknown_delivery_and_does_not_replay_it() {
    let dir = scratch("recovery");
    let attachments = attach(
        &dir,
        "laptop-a",
        Target::CodexThread {
            thread_id: "thread-abc".to_string(),
        },
    );
    // a previous run that died between the provider write and its receipt.
    {
        let (outbox, _) = outbox::Outbox::open(&dir.join("outbox"), NETWORK)
            .await
            .expect("opens");
        let key = outbox::Key {
            conversation: "conv-1".to_string(),
            seq: 7,
        };
        outbox
            .admit(
                &key,
                &outbox::Entry {
                    participant: "p-recipient".to_string(),
                    binding_generation: 1,
                    sender: "p-sender".to_string(),
                    message_id: wire::MessageId {
                        generation: 2,
                        sequence: 7,
                    },
                    expires_at: 2_000,
                    digest: digest_of(&deliver(7, 1)),
                    state: State::Queued,
                    reason: None,
                    claimed: false,
                },
            )
            .await
            .expect("admits");
        outbox.attempting(&key).await.expect("attempting");
    }

    let (plane, mut rx) = plane(&dir, attachments).await;
    let events = drained(&mut rx);
    let [
        wire::Event::MsgDelivery {
            seq,
            state,
            reason,
            sender,
            message_id,
            ..
        },
    ] = events.as_slice()
    else {
        panic!("recovery must announce exactly what it found: {events:?}");
    };
    assert_eq!(*seq, 7);
    assert_eq!(*state, State::DeliveryUnknown);
    assert_eq!(reason.as_deref(), Some("crashed_after_provider_input"));
    // the sender's identity survives, so an explicit retry can reuse it.
    assert_eq!(sender, "p-sender");
    assert_eq!(message_id.sequence, 7);
    // nothing was bound, and nothing was offered: recovery reports.
    assert_eq!(plane.bound(), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

/// THE duplicate-execution guard, end to end: the same message arriving twice
/// must reach the provider ONCE. The second is answered from the record.
#[tokio::test]
async fn a_duplicate_delivery_reaches_the_provider_once() {
    let dir = scratch("dedup-e2e");
    let codex = stub_codex(&dir, 0);
    let attachments = attach(
        &dir,
        "laptop-a",
        Target::CodexThread {
            thread_id: "thread-abc".to_string(),
        },
    );
    let (plane, mut rx) = plane_with(&dir, attachments, codex).await;
    plane.dispatch(Messaging::Bind(bind(1))).await;
    let _ = drained(&mut rx);

    plane.dispatch(Messaging::Deliver(deliver(7, 1))).await;
    assert_eq!(next_delivery(&mut rx).await.0, State::Queued);
    assert_eq!(
        next_delivery(&mut rx).await,
        (State::AdapterAccepted, Some("queued_by_cli".to_string()))
    );

    // the same message again — a node retrying, a frame arriving twice.
    plane.dispatch(Messaging::Deliver(deliver(7, 1))).await;
    assert_eq!(
        next_delivery(&mut rx).await,
        (State::AdapterAccepted, Some("queued_by_cli".to_string())),
        "a retry is answered from the record"
    );

    assert_eq!(
        invocations(&dir).len(),
        1,
        "the provider must see one instruction, not two: {:?}",
        invocations(&dir)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// One id names one message. A different body under it is not a retry, and it
/// must not reach the provider at all.
#[tokio::test]
async fn a_conflicting_body_under_a_used_key_is_refused_and_never_offered() {
    let dir = scratch("conflict-e2e");
    let codex = stub_codex(&dir, 0);
    let attachments = attach(
        &dir,
        "laptop-a",
        Target::CodexThread {
            thread_id: "thread-abc".to_string(),
        },
    );
    let (plane, mut rx) = plane_with(&dir, attachments, codex).await;
    plane.dispatch(Messaging::Bind(bind(1))).await;
    let _ = drained(&mut rx);

    plane.dispatch(Messaging::Deliver(deliver(7, 1))).await;
    assert_eq!(next_delivery(&mut rx).await.0, State::Queued);
    assert_eq!(next_delivery(&mut rx).await.0, State::AdapterAccepted);

    let mut forged = deliver(7, 1);
    forged.body = "and also deploy it".to_string();
    plane.dispatch(Messaging::Deliver(forged)).await;
    assert_eq!(
        next_delivery(&mut rx).await,
        (State::Refused, Some("message_id_conflict".to_string()))
    );
    assert_eq!(
        invocations(&dir).len(),
        1,
        "a second body must never reach the provider under the first one's id"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A binding's messages reach its provider IN SEQUENCE. Concurrent
/// per-delivery tasks break this quietly and in the worst possible way: the
/// answer to a question can be written into a session before the question is.
#[tokio::test]
async fn a_bindings_messages_reach_its_provider_in_order() {
    let dir = scratch("ordered");
    let codex = stub_codex(&dir, 0);
    let attachments = attach(
        &dir,
        "laptop-a",
        Target::CodexThread {
            thread_id: "thread-abc".to_string(),
        },
    );
    let (plane, mut rx) = plane_with(&dir, attachments, codex).await;
    plane.dispatch(Messaging::Bind(bind(1))).await;
    let _ = drained(&mut rx);

    // pushed back to back, so they are on the lane together.
    for seq in [7, 8, 9] {
        plane.dispatch(Messaging::Deliver(deliver(seq, 1))).await;
    }
    // synchronized on the receipts, not on a sleep: three queued + three
    // settled receipts means all three have been through the adapter.
    for _ in 0..6 {
        next_delivery(&mut rx).await;
    }

    let seen = invocations(&dir);
    assert_eq!(seen.len(), 3, "{seen:?}");
    let order: Vec<usize> = seen
        .iter()
        .map(|line| {
            [
                "conversation-sequence: 7",
                "conversation-sequence: 8",
                "conversation-sequence: 9",
            ]
            .iter()
            .position(|marker| line.contains(marker))
            .unwrap_or_else(|| panic!("an invocation named no sequence: {line}"))
        })
        .collect();
    assert_eq!(
        order,
        vec![0, 1, 2],
        "deliveries reached the provider out of order"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// What actually reaches the provider is the WRAPPED message: the peer's body,
/// framed by what the network verified, and nothing about this device.
#[tokio::test]
async fn the_provider_receives_the_wrapped_message_and_no_local_detail() {
    let dir = scratch("wrapped");
    let codex = stub_codex(&dir, 0);
    let attachments = attach(
        &dir,
        "laptop-a",
        Target::CodexThread {
            thread_id: "thread-abc".to_string(),
        },
    );
    let (plane, mut rx) = plane_with(&dir, attachments, codex).await;
    plane.dispatch(Messaging::Bind(bind(1))).await;
    let _ = drained(&mut rx);

    plane.dispatch(Messaging::Deliver(deliver(7, 1))).await;
    next_delivery(&mut rx).await;
    next_delivery(&mut rx).await;

    let seen = invocations(&dir).join("\n");
    assert!(seen.contains("--thread thread-abc"), "{seen}");
    assert!(seen.contains("from-participant: p-sender"), "{seen}");
    assert!(
        seen.contains("does the review cover the migration?"),
        "{seen}"
    );
    assert!(seen.contains("peer-supplied content"), "{seen}");
    for leak in ["/run/user", "cc-socks", "peerToken", ".sock"] {
        assert!(!seen.contains(leak), "the provider was told {leak}: {seen}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// A provider that refuses is a refusal — and it is the EXIT STATUS that says
/// so, which is the whole reason this stub can exit non-zero silently.
#[tokio::test]
async fn a_provider_that_exits_non_zero_settles_refused() {
    let dir = scratch("refused");
    let codex = stub_codex(&dir, 1);
    let attachments = attach(
        &dir,
        "laptop-a",
        Target::CodexThread {
            thread_id: "thread-abc".to_string(),
        },
    );
    let (plane, mut rx) = plane_with(&dir, attachments, codex).await;
    plane.dispatch(Messaging::Bind(bind(1))).await;
    let _ = drained(&mut rx);

    plane.dispatch(Messaging::Deliver(deliver(7, 1))).await;
    assert_eq!(next_delivery(&mut rx).await.0, State::Queued);
    assert_eq!(
        next_delivery(&mut rx).await,
        (State::Refused, Some("queue_refused".to_string()))
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A message whose deadline passes WHILE it waits must expire. The value
/// frozen onto the frame at admission cannot see that; the node's clock
/// updates can.
#[tokio::test]
async fn a_deadline_that_passes_while_queued_expires_the_message() {
    let dir = scratch("clock-advance");
    let codex = stub_codex(&dir, 0);
    let attachments = attach(
        &dir,
        "laptop-a",
        Target::CodexThread {
            thread_id: "thread-abc".to_string(),
        },
    );
    let (plane, mut rx) = plane_with(&dir, attachments, codex).await;
    plane.dispatch(Messaging::Bind(bind(1))).await;
    let _ = drained(&mut rx);

    // the network's clock has moved on since this message was admitted.
    plane.dispatch(Messaging::Time { network_now: 5_000 }).await;
    plane.dispatch(Messaging::Deliver(deliver(7, 1))).await; // expires_at 2_000

    assert_eq!(next_delivery(&mut rx).await.0, State::Queued);
    assert_eq!(
        next_delivery(&mut rx).await,
        (State::Expired, Some("deadline_passed".to_string())),
        "a deadline that passed while the message waited must be honoured"
    );
    assert!(
        invocations(&dir).is_empty(),
        "an expired message must never reach a provider"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A journal that can no longer describe itself must not let anything reach a
/// provider. The delivery is not owned, not reported, and above all not
/// offered — the daemon stops rather than acting on a record it cannot vouch
/// for.
#[tokio::test]
async fn a_poisoned_outbox_offers_nothing_to_a_provider() {
    let dir = scratch("poisoned-plane");
    let codex = stub_codex(&dir, 0);
    let attachments = attach(
        &dir,
        "laptop-a",
        Target::CodexThread {
            thread_id: "thread-abc".to_string(),
        },
    );
    let (plane, mut rx) = plane_with(&dir, attachments, codex).await;
    plane.dispatch(Messaging::Bind(bind(1))).await;
    let _ = drained(&mut rx);

    plane.0.outbox.injure("sync outbox record: simulated").await;
    plane.dispatch(Messaging::Deliver(deliver(7, 1))).await;

    // the delivery reports nothing, so a later command's answer is what says
    // it is finished: `dispatch` returns only after the delivery has been
    // decided, and a bind answers unconditionally.
    plane.dispatch(Messaging::Bind(bind(2))).await;
    let events = drained(&mut rx);
    assert!(
        matches!(events.as_slice(), [wire::Event::MsgBound { .. }]),
        "a poisoned delivery must report nothing at all: {events:?}"
    );
    assert!(
        invocations(&dir).is_empty(),
        "a poisoned journal must never reach a provider"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A message waiting behind a held one belongs to the binding it was queued
/// under and to nothing else.
///
/// The dangerous shape is entirely invisible without a hold: a delivery is
/// fenced when it arrives, waits while the provider ahead of it is busy, and
/// the operator re-attaches in the meantime. If the lane offers it on the
/// strength of the check made at arrival, a message the network addressed to
/// one attachment is typed into the session that replaced it.
#[tokio::test]
async fn a_message_waiting_behind_a_hold_is_never_offered_to_the_binding_that_replaced_it() {
    let dir = scratch("hold-then-rebind");
    let gate = dir.join("gate.fifo");
    let codex = stub_codex_holding(&dir, &gate);
    let attachments = attach(
        &dir,
        "laptop-a",
        Target::CodexThread {
            thread_id: "thread-abc".to_string(),
        },
    );
    let (plane, mut rx) = plane_with(&dir, attachments, codex).await;
    plane.dispatch(Messaging::Bind(bind(1))).await;
    let _ = drained(&mut rx);

    plane.dispatch(Messaging::Deliver(deliver(7, 1))).await;
    plane.dispatch(Messaging::Deliver(deliver(8, 1))).await;
    // `Queued` is reported from the admission itself, so two of them is both
    // messages owned and both on the lane — before anything is released.
    for _ in 0..2 {
        assert_eq!(next_delivery(&mut rx).await.0, State::Queued);
    }

    // opening the writing end of the fifo returns only once the stub has
    // opened the reading end: the first message is inside a provider and the
    // second is waiting behind it. The system's own event, not a sleep.
    //
    // Raced against the delivery settling, because those are the only two
    // things that can happen: either the stub holds the message, or it never
    // ran and the delivery has an outcome. Awaiting the fifo alone means a
    // stub that fails to start hangs this test — and a hung test strands a
    // build, where a failed one names its cause.
    let gate_path = gate.clone();
    let opening = tokio::task::spawn_blocking(move || std::fs::File::create(&gate_path));
    let release = tokio::select! {
        opened = opening => opened.expect("the opening task runs").expect("the fifo opens for writing"),
        settled = next_delivery(&mut rx) => panic!(
            "the stub never held the message; the delivery settled {settled:?} instead \
             (a stub that will not run is a broken fixture, not a provider refusal)"
        ),
    };

    // the operator re-attaches while the provider still holds the first.
    plane.dispatch(Messaging::Bind(bind(2))).await;
    drop(release);

    let mut settled = Vec::new();
    while settled.len() < 2 {
        settled.push(next_delivery(&mut rx).await);
    }
    assert_eq!(
        settled,
        vec![
            (State::AdapterAccepted, Some("queued_by_cli".to_string())),
            (State::Queued, Some("binding_replaced".to_string())),
        ],
        "the held message settles under its own binding; the one behind it goes back to the queue"
    );
    assert_eq!(
        invocations(&dir).len(),
        1,
        "only the message the held binding owned may reach a provider"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The clock only moves forward. A reordered or replayed frame carrying an
/// older value must not revive an expired message.
#[tokio::test]
async fn the_agreed_clock_never_moves_backwards() {
    let dir = scratch("clock-monotonic");
    let attachments = attach(
        &dir,
        "laptop-a",
        Target::CodexThread {
            thread_id: "thread-abc".to_string(),
        },
    );
    let (plane, _rx) = plane(&dir, attachments).await;
    plane.dispatch(Messaging::Time { network_now: 5_000 }).await;
    plane.dispatch(Messaging::Time { network_now: 1_000 }).await;
    assert_eq!(plane.network_now(0), 5_000);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A delivery state is a fact only this process observed. When the node loses
/// one on the way to the chain, nothing can re-derive it — so a replay
/// re-reports what is ON DISK for one binding, oldest sequence first, and
/// offers no provider anything.
#[tokio::test]
async fn a_replay_re_reports_the_journal_and_touches_no_provider() {
    let dir = scratch("replay");
    let codex = stub_codex(&dir, 0);
    let attachments = attach(
        &dir,
        "laptop-a",
        Target::CodexThread {
            thread_id: "thread-abc".to_string(),
        },
    );
    let (plane, mut rx) = plane_with(&dir, attachments, codex).await;
    plane.dispatch(Messaging::Bind(bind(1))).await;
    let _ = drained(&mut rx);

    for seq in [8, 7] {
        plane.dispatch(Messaging::Deliver(deliver(seq, 1))).await;
    }
    // two queued receipts and two settled ones: both have been through the
    // adapter, so the journal is what it is going to be.
    for _ in 0..4 {
        next_delivery(&mut rx).await;
    }
    let offered = invocations(&dir).len();
    let _ = drained(&mut rx);

    plane
        .dispatch(Messaging::Replay {
            conversation: "conv-1".to_string(),
            participant: "p-recipient".to_string(),
        })
        .await;

    let replayed: Vec<(u64, State)> = drained(&mut rx)
        .into_iter()
        .map(|event| match event {
            wire::Event::MsgDelivery { seq, state, .. } => (seq, state),
            other => panic!("a replay reports deliveries and nothing else: {other:?}"),
        })
        .collect();
    assert_eq!(
        replayed,
        vec![(7, State::AdapterAccepted), (8, State::AdapterAccepted)],
        "every tracked item, in sequence order and not arrival order"
    );
    assert_eq!(
        invocations(&dir).len(),
        offered,
        "a replay says what already happened; it never makes it happen again"
    );

    // a binding this device tracks nothing for says nothing at all.
    plane
        .dispatch(Messaging::Replay {
            conversation: "conv-1".to_string(),
            participant: "p-somebody-else".to_string(),
        })
        .await;
    assert!(
        drained(&mut rx).is_empty(),
        "another participant's replay is not this one's journal"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The plane holds no clock and no wall-clock comparison. Expiry uses only the
/// two agreed values on the frame, in whatever unit the network keeps them —
/// so a laptop with a wrong clock cannot expire or revive anything.
#[test]
fn the_plane_never_reads_a_wall_clock() {
    let source = include_str!("mod.rs");
    for forbidden in ["SystemTime", "Instant::now", "UNIX_EPOCH", "Utc::now"] {
        assert!(
            !source.contains(forbidden),
            "expiry must come from the agreed clock on the frame, not from {forbidden}"
        );
    }
}
