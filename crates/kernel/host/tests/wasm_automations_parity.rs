//! the adapter-port equivalence proof for the automations cutover: the
//! `automations` guest component (the NATIVE `automations` crate compiled to
//! wasm behind `guest-adapter`) and the native `Automations` module answer
//! the SAME op sequence with IDENTICAL query replies, and their roots move in
//! lockstep. the roots THEMSELVES differ — the port persists the native
//! canonical snapshot as one host-KV value — and this proof pins that
//! representation difference from genesis
//! (automations' empty encoding carries TWO zero counts, so unlike saga/agent
//! its genesis root already differs from the empty host-KV store's).
//!
//! automations is a chat-HOOK SUBSCRIBER: both hosts carry the REAL native
//! siblings under the production ids (`bin/node/src/host_state.rs` —
//! chat over its own qmdb store, tasks, inbox) and drive actual chat posts
//! through the hook fan-out, so on the wasm side the whole intake runs in the
//! guest: the origin gate, the loop-prevention author check, the text fetch
//! and the pre-emit PROBES (channel-exists / message-id-unused /
//! task-id-unused — host-routed `query-module` reads resolved through
//! memoized replay against the live siblings), and the follow-up emissions
//! that land on chat/tasks/inbox IN THE SAME BLOCK as the triggering post.
//! the NO-FAIL contract crosses the seam unchanged: a probe rejection stages
//! a RunRecord instead of aborting the posting user's block.

use automations::{
    Action, Automations, AutomationsMsg, AutomationsQuery, AutomationsReply, MAX_FILTER_BYTES,
    MAX_ID_BYTES, MAX_TEMPLATE_BYTES, Trigger, decode_reply, encode_msg, encode_query,
};
use chat::{Block, Chat, ChatMsg, PostPolicy, encode_msg as chat_encode_msg};
use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use host::{BlockContext, Host, MemberOutcome, SubmitError};
use inbox::Inbox;
use sdk::{Error, Msg, Origin, StateRoot};
use statesync::qmdb::QmdbStore;
use tasks::{
    TaskMsg, TaskQuery, TaskReply, Tasks, decode_task_reply as tasks_decode_reply,
    encode_task_msg as tasks_encode_msg, encode_task_query as tasks_encode_query,
};
use wasm_host::WasmModule;

/// GENERATED artifact — built from the `automations` module's guest port by
/// guest-builder (`make wasm-modules`); committed so this proof is self-contained.
const AUTOMATIONS_WASM: &[u8] = include_bytes!("fixtures/automations.component.wasm");

fn wasm_automations() -> WasmModule {
    WasmModule::from_bytes("automations", AUTOMATIONS_WASM).expect("load component")
}

/// the production wiring, verbatim (`bin/node/src/host_state.rs`).
fn native_automations() -> Automations {
    Automations::new("automations", "chat", "tasks", "inbox")
}

/// a 32-byte submitter key (the ordered lane hands modules verified ed25519
/// ids; the parity claim only needs them distinct and non-empty).
fn key(tag: u8) -> Vec<u8> {
    vec![tag; 32]
}

/// the shared native sibling set: chat over a REAL qmdb store (the module the
/// hook events and probes run against), tasks, inbox.
async fn siblings(
    context: &deterministic::Context,
    label: &'static str,
) -> Vec<Box<dyn sdk::Module>> {
    let store = QmdbStore::init(context.child(label), "chat").await;
    vec![
        Box::new(Chat::new("chat", Box::new(store))),
        Box::new(Tasks::new("tasks")),
        Box::new(Inbox::new("inbox")),
    ]
}

async fn native_host(context: &deterministic::Context) -> Host {
    let mut modules = siblings(context, "native_chat").await;
    modules.push(Box::new(native_automations()));
    Host::genesis(modules).expect("genesis")
}

async fn wasm_host_(context: &deterministic::Context) -> Host {
    let mut modules = siblings(context, "wasm_chat").await;
    modules.push(Box::new(wasm_automations()));
    Host::genesis(modules).expect("genesis")
}

fn block(height: u64, origin: Origin) -> BlockContext {
    BlockContext {
        height,
        consensus_time: 1_000 + height,
        origin,
    }
}

fn auto_op(m: &AutomationsMsg) -> Msg {
    Msg {
        target: "automations".into(),
        payload: encode_msg(m),
    }
}

fn create_rule(rule_id: &str, trigger: Trigger, action: Action) -> Msg {
    auto_op(&AutomationsMsg::CreateRule {
        rule_id: rule_id.into(),
        trigger,
        action,
    })
}

fn on_general(text_contains: Option<&str>) -> Trigger {
    Trigger {
        channel_id: Some("general".into()),
        mention: None,
        text_contains: text_contains.map(Into::into),
    }
}

fn post(channel: &str, id: &str, text: &str) -> Msg {
    Msg {
        target: "chat".into(),
        payload: chat_encode_msg(&ChatMsg::PostMessage {
            channel_id: channel.into(),
            message_id: id.into(),
            blocks: vec![Block::paragraph(text)],
            thread: None,
            as_agent: None,
        }),
    }
}

const RULE_IDS: [&str; 4] = ["r-ghost", "r-inbox", "r-post", "r-task"];

/// the read matrix: the rule listing (fire counts ride in it), per-rule gets
/// (present and absent), and each rule's run history.
async fn replies(h: &Host) -> Vec<Vec<u8>> {
    let mut queries = vec![encode_query(&AutomationsQuery::ListRules)];
    for id in RULE_IDS {
        queries.push(encode_query(&AutomationsQuery::GetRule {
            rule_id: id.into(),
        }));
        queries.push(encode_query(&AutomationsQuery::RunHistory {
            rule_id: id.into(),
            limit: 64,
        }));
    }
    queries.push(encode_query(&AutomationsQuery::GetRule {
        rule_id: "absent".into(),
    }));
    let mut out = Vec::new();
    for q in &queries {
        out.push(h.query("automations", q).await.expect("query"));
    }
    out
}

fn root_of(h: &Host) -> StateRoot {
    h.module_root("automations")
        .expect("automations registered")
}

const SIBLING_IDS: [&str; 3] = ["chat", "tasks", "inbox"];

/// submit one ACCEPTED op to both hosts: identical replies, per-sibling
/// cross-host agreement (the follow-up lanes — a diverging or missing
/// PostMessage/CreateTask/Deliver diverges the sibling roots), lockstep
/// automations-root movement.
async fn roundtrip(
    native: &mut Host,
    wasm: &mut Host,
    height: u64,
    origin: Origin,
    m: Msg,
    moves: bool,
) {
    let (n_before, w_before) = (root_of(native), root_of(wasm));
    native
        .submit_at(block(height, origin.clone()), m.clone())
        .await
        .expect("native submit");
    wasm.submit_at(block(height, origin), m)
        .await
        .expect("wasm submit");
    assert_eq!(
        replies(native).await,
        replies(wasm).await,
        "replies diverge after block {height}"
    );
    for sibling in SIBLING_IDS {
        assert_eq!(
            native.module_root(sibling),
            wasm.module_root(sibling),
            "the {sibling} sibling diverged at {height}"
        );
    }
    if moves {
        assert_ne!(root_of(native), n_before, "native root stuck at {height}");
        assert_ne!(root_of(wasm), w_before, "wasm root stuck at {height}");
    } else {
        assert_eq!(root_of(native), n_before, "native root moved at {height}");
        assert_eq!(root_of(wasm), w_before, "wasm root moved at {height}");
    }
    // the roots themselves always differ (the pinned representation difference).
    assert_ne!(root_of(native), root_of(wasm));
}

/// submit one REJECTED op to both hosts: reasons carry the same needle, and
/// both automations roots (and all siblings) are byte-identical to pre-block.
async fn reject_roundtrip(
    native: &mut Host,
    wasm: &mut Host,
    height: u64,
    origin: Origin,
    m: Msg,
    needle: &str,
) {
    let (n_before, w_before) = (root_of(native), root_of(wasm));
    let n_err = native
        .submit_at(block(height, origin.clone()), m.clone())
        .await
        .expect_err("native must reject");
    let w_err = wasm
        .submit_at(block(height, origin), m)
        .await
        .expect_err("wasm must reject");
    let SubmitError::Rejected(Error::Module(n_msg)) = n_err else {
        panic!("native rejection shape: {n_err:?}");
    };
    let SubmitError::Rejected(Error::Module(w_msg)) = w_err else {
        panic!("wasm rejection shape: {w_err:?}");
    };
    assert!(n_msg.contains(needle), "native reason: {n_msg}");
    assert!(
        w_msg.contains(needle),
        "wasm reason must carry the native reason: {w_msg}"
    );
    assert_eq!(root_of(native), n_before, "native root moved on reject");
    assert_eq!(root_of(wasm), w_before, "wasm root moved on reject");
    for sibling in SIBLING_IDS {
        assert_eq!(native.module_root(sibling), wasm.module_root(sibling));
    }
    assert_eq!(replies(native).await, replies(wasm).await);
}

/// decoded task list off one host (cross-host equality is separately pinned
/// via the sibling roots; this is for spot checks).
async fn task_ids(h: &Host) -> Vec<String> {
    let reply = h
        .query("tasks", &tasks_encode_query(&TaskQuery::List))
        .await
        .expect("tasks query");
    let TaskReply::Tasks(tasks) = tasks_decode_reply(&reply).expect("decode");
    tasks.into_iter().map(|t| t.id).collect()
}

#[test]
fn same_ops_same_replies_follow_ups_land_and_probes_downgrade() {
    deterministic::Runner::default().start(|context| async move {
        let mut native = native_host(&context).await;
        let mut wasm = wasm_host_(&context).await;
        let user = Origin::External(key(0xA1));
        let ops = Origin::External(key(0xB2));

        // the representation difference is visible from GENESIS here: automations' empty
        // canonical encoding is TWO zero counts (rules + history), the wasm
        // port's empty host-KV store is ONE — different preimages, different
        // roots (contrast saga/agent, whose single-count empty encodings
        // coincide with the empty store until the first write).
        assert_ne!(root_of(&native), StateRoot::ZERO);
        assert_ne!(
            root_of(&native),
            root_of(&wasm),
            "genesis roots must differ — the port uses a distinct root representation"
        );
        // the inbox sibling's empty root, for the delivery-landed claims below
        // (the inbox has no read surface — its module root is the whole
        // observable committed state).
        let inbox_genesis = native.module_root("inbox").expect("inbox registered");

        // ---- the shared world: a hooked channel. sibling-only blocks leave
        // automations alone on both runtimes.
        let (n0, w0) = (root_of(&native), root_of(&wasm));
        for host in [&mut native, &mut wasm] {
            host.submit_at(
                block(1, user.clone()),
                Msg {
                    target: "chat".into(),
                    payload: chat_encode_msg(&ChatMsg::CreateChannel {
                        channel_id: "general".into(),
                        name: "General".into(),
                        post_policy: PostPolicy::Open,
                    }),
                },
            )
            .await
            .expect("create channel");
            host.submit_at(
                block(2, user.clone()),
                Msg {
                    target: "chat".into(),
                    payload: chat_encode_msg(&ChatMsg::RegisterHook {
                        channel_id: "general".into(),
                        module_id: "automations".into(),
                    }),
                },
            )
            .await
            .expect("register hook");
        }
        assert_eq!(root_of(&native), n0, "sibling blocks hold the native root");
        assert_eq!(root_of(&wasm), w0, "sibling blocks hold the wasm root");

        // ---- rule CRUD: every action family, one rule each. r-ghost targets
        // a channel that does not exist — its every fire must DOWNGRADE to a
        // RunRecord through the probe layer, never abort a posting block.
        roundtrip(
            &mut native,
            &mut wasm,
            3,
            ops.clone(),
            create_rule(
                "r-post",
                on_general(Some("duck")),
                Action::PostMessage {
                    channel_id: "general".into(),
                    template: "echo {author}: {text}".into(),
                },
            ),
            true,
        )
        .await;
        roundtrip(
            &mut native,
            &mut wasm,
            4,
            ops.clone(),
            create_rule(
                "r-task",
                on_general(None),
                Action::CreateTask {
                    task_id_prefix: "job".into(),
                    title_template: "follow up on {channel}#{seq}".into(),
                },
            ),
            true,
        )
        .await;
        roundtrip(
            &mut native,
            &mut wasm,
            5,
            ops.clone(),
            create_rule(
                "r-inbox",
                on_general(None),
                Action::DeliverInbox {
                    member_template: "ops-{channel}".into(),
                    kind: "note".into(),
                    body_template: "{author} said: {text}".into(),
                },
            ),
            true,
        )
        .await;
        roundtrip(
            &mut native,
            &mut wasm,
            6,
            ops.clone(),
            create_rule(
                "r-ghost",
                on_general(None),
                Action::PostMessage {
                    channel_id: "missing".into(),
                    template: "into the void".into(),
                },
            ),
            true,
        )
        .await;

        // ---- the first user post (seq 1): the hook fans the event into the
        // guest, where the whole intake runs — r-ghost downgrades (channel
        // probe), r-inbox delivers, r-post passes both chat probes and posts
        // (its module-authored reply is seq 2, whose OWN hook event must be
        // loop-prevented), r-task creates job-general-1. everything commits
        // in the posting block, identically.
        roundtrip(
            &mut native,
            &mut wasm,
            7,
            user.clone(),
            post("general", "m1", "the duck says hi"),
            true,
        )
        .await;
        assert_eq!(task_ids(&wasm).await, vec!["job-general-1".to_string()]);
        // r-inbox's Deliver landed ATOMICALLY in the posting block: the inbox
        // sibling root moved off empty (cross-host agreement on it is pinned
        // inside `roundtrip` via SIBLING_IDS).
        assert_ne!(
            native.module_root("inbox").expect("inbox registered"),
            inbox_genesis,
            "the hooked post delivered nothing to the inbox"
        );

        // ---- a post WITHOUT the text filter's needle: r-post skips (its
        // fire_count must not bump), the unfiltered rules still fire.
        roundtrip(
            &mut native,
            &mut wasm,
            8,
            user.clone(),
            post("general", "m2", "no waterfowl here"),
            true,
        )
        .await;

        // ---- SetEnabled false: a disabled rule never engages.
        roundtrip(
            &mut native,
            &mut wasm,
            9,
            ops.clone(),
            auto_op(&AutomationsMsg::SetEnabled {
                rule_id: "r-task".into(),
                enabled: false,
            }),
            true,
        )
        .await;
        roundtrip(
            &mut native,
            &mut wasm,
            10,
            user.clone(),
            post("general", "m3", "duck again"),
            true,
        )
        .await;
        assert_eq!(
            task_ids(&wasm).await,
            vec!["job-general-1".to_string(), "job-general-3".to_string()],
            "the disabled window skipped exactly the middle post"
        );

        // ---- id squatting: pre-create the task id the NEXT post composes
        // (m3 was seq 4 + r-post's reply seq 5 → the next user post is seq 6),
        // re-enable r-task, and post — the task probe must downgrade the fire
        // to a RunRecord on both runtimes instead of aborting the block.
        for host in [&mut native, &mut wasm] {
            host.submit_at(
                block(11, ops.clone()),
                Msg {
                    target: "tasks".into(),
                    payload: tasks_encode_msg(&TaskMsg::CreateTask {
                        task_id: "job-general-6".into(),
                        title: "squatted".into(),
                    }),
                },
            )
            .await
            .expect("squat the composed id");
        }
        roundtrip(
            &mut native,
            &mut wasm,
            12,
            ops.clone(),
            auto_op(&AutomationsMsg::SetEnabled {
                rule_id: "r-task".into(),
                enabled: true,
            }),
            true,
        )
        .await;
        roundtrip(
            &mut native,
            &mut wasm,
            13,
            user.clone(),
            post("general", "m4", "quiet post"),
            true,
        )
        .await;
        // no NEW task landed (the squat is the only "job-general-6").
        assert_eq!(
            task_ids(&wasm).await.len(),
            3,
            "the probe downgraded the fire"
        );

        // ---- DeleteRule: the tombstone hides the rule from reads.
        roundtrip(
            &mut native,
            &mut wasm,
            14,
            ops.clone(),
            auto_op(&AutomationsMsg::DeleteRule {
                rule_id: "r-ghost".into(),
            }),
            true,
        )
        .await;
        let reply = wasm
            .query(
                "automations",
                &encode_query(&AutomationsQuery::GetRule {
                    rule_id: "r-ghost".into(),
                }),
            )
            .await
            .expect("get");
        assert_eq!(
            decode_reply(&reply).expect("decode"),
            AutomationsReply::Rule(None)
        );

        // the inbox lane really landed, and landed IDENTICALLY: the sibling
        // inbox roots moved off empty and agree across the hosts (the inbox
        // has no read surface — its decoded views are the index guest's job;
        // the module root IS its whole observable committed state).
        assert_eq!(
            native.module_root("inbox"),
            wasm.module_root("inbox"),
            "the inbox deliveries diverged between the runtimes"
        );
        assert_ne!(
            native.module_root("inbox").expect("inbox registered"),
            inbox_genesis,
            "the hooked posts delivered nothing to the inbox"
        );

        // queries are read-only on the wasm side too.
        let settled = root_of(&wasm);
        let _ = replies(&wasm).await;
        assert_eq!(root_of(&wasm), settled, "a query moved the wasm root");
    });
}

#[test]
fn rejections_match_and_leave_no_trace() {
    deterministic::Runner::default().start(|context| async move {
        let mut native = native_host(&context).await;
        let mut wasm = wasm_host_(&context).await;
        let ops = Origin::External(key(0xB2));

        for host in [&mut native, &mut wasm] {
            host.submit_at(
                block(1, ops.clone()),
                create_rule(
                    "r-post",
                    on_general(None),
                    Action::PostMessage {
                        channel_id: "general".into(),
                        template: "x".into(),
                    },
                ),
            )
            .await
            .expect("seed rule");
        }

        // every distinct refusal family: id/filter/template caps, action
        // shapes, duplicates, unknowns, the forged-hook origin gate, and the
        // decode seam. each rejected block leaves BOTH roots byte-identical.
        let rejects: Vec<(Msg, &str)> = vec![
            (
                create_rule(
                    "",
                    on_general(None),
                    Action::PostMessage {
                        channel_id: "c".into(),
                        template: "t".into(),
                    },
                ),
                "rule_id must not be empty",
            ),
            (
                create_rule(
                    &"r".repeat(MAX_ID_BYTES + 1),
                    on_general(None),
                    Action::PostMessage {
                        channel_id: "c".into(),
                        template: "t".into(),
                    },
                ),
                "rule_id exceeds",
            ),
            (
                create_rule(
                    "r1",
                    Trigger {
                        channel_id: Some(String::new()),
                        mention: None,
                        text_contains: None,
                    },
                    Action::PostMessage {
                        channel_id: "c".into(),
                        template: "t".into(),
                    },
                ),
                "trigger channel_id must not be empty",
            ),
            (
                create_rule(
                    "r1",
                    Trigger {
                        channel_id: None,
                        mention: Some("m".repeat(MAX_FILTER_BYTES + 1)),
                        text_contains: None,
                    },
                    Action::PostMessage {
                        channel_id: "c".into(),
                        template: "t".into(),
                    },
                ),
                "trigger mention exceeds",
            ),
            (
                create_rule(
                    "r1",
                    on_general(None),
                    Action::PostMessage {
                        channel_id: String::new(),
                        template: "t".into(),
                    },
                ),
                "action channel_id must not be empty",
            ),
            (
                create_rule(
                    "r1",
                    on_general(None),
                    Action::PostMessage {
                        channel_id: "c".into(),
                        template: "t".repeat(MAX_TEMPLATE_BYTES + 1),
                    },
                ),
                "action template exceeds",
            ),
            (
                create_rule(
                    "r1",
                    on_general(None),
                    Action::CreateTask {
                        task_id_prefix: String::new(),
                        title_template: "t".into(),
                    },
                ),
                "action task_id_prefix must not be empty",
            ),
            (
                create_rule(
                    "r1",
                    on_general(None),
                    Action::DeliverInbox {
                        member_template: "m".into(),
                        kind: String::new(),
                        body_template: "b".into(),
                    },
                ),
                "action kind must not be empty",
            ),
            (
                create_rule(
                    "r-post",
                    on_general(None),
                    Action::PostMessage {
                        channel_id: "c".into(),
                        template: "t".into(),
                    },
                ),
                "rule already exists",
            ),
            (
                auto_op(&AutomationsMsg::SetEnabled {
                    rule_id: "ghost".into(),
                    enabled: true,
                }),
                "unknown rule",
            ),
            (
                auto_op(&AutomationsMsg::DeleteRule {
                    rule_id: "ghost".into(),
                }),
                "unknown rule",
            ),
            // the spoof gate: a SUBMITTED HookEvent (any non-chat origin)
            // rejects — only chat's own follow-ups wear the chat origin.
            (
                auto_op(&AutomationsMsg::HookEvent(b"forged".to_vec())),
                "hook events must originate from the chat module",
            ),
            (
                Msg {
                    target: "automations".into(),
                    payload: b"definitely-not-json".to_vec(),
                },
                "expected value",
            ),
        ];

        for (height, (m, needle)) in rejects.into_iter().enumerate() {
            let height = height as u64 + 2;
            reject_roundtrip(&mut native, &mut wasm, height, ops.clone(), m, needle).await;
        }
    });
}

#[test]
fn multi_dispatch_block_reads_prior_writes_and_isolates_rejections() {
    deterministic::Runner::default().start(|context| async move {
        let mut native = native_host(&context).await;
        let mut wasm = wasm_host_(&context).await;
        let ops = Origin::External(key(0xB2));

        // ONE block, three ops: the disable reads the rule STAGED by the
        // first dispatch, and the delete reads the disable's staged record —
        // on the wasm side each later dispatch reloads the prior dispatch's
        // staged `__state` (the read-your-writes seam the adapter relies on).
        let batch = vec![
            (
                ops.clone(),
                create_rule(
                    "x",
                    on_general(None),
                    Action::CreateTask {
                        task_id_prefix: "p".into(),
                        title_template: "t".into(),
                    },
                ),
            ),
            (
                ops.clone(),
                auto_op(&AutomationsMsg::SetEnabled {
                    rule_id: "x".into(),
                    enabled: false,
                }),
            ),
            (
                ops.clone(),
                auto_op(&AutomationsMsg::DeleteRule {
                    rule_id: "x".into(),
                }),
            ),
        ];
        let n_out = native
            .submit_block(block(1, ops.clone()), batch.clone())
            .await
            .expect("native block");
        let w_out = wasm
            .submit_block(block(1, ops.clone()), batch)
            .await
            .expect("wasm block");
        for out in [&n_out, &w_out] {
            assert!(
                out.members
                    .iter()
                    .all(|m| matches!(m, MemberOutcome::Applied { .. })),
                "all members must apply: {:?}",
                out.members
            );
        }
        assert_eq!(replies(&native).await, replies(&wasm).await);
        // created, disabled, and deleted within one block: nothing remains.
        let reply = wasm
            .query(
                "automations",
                &encode_query(&AutomationsQuery::GetRule {
                    rule_id: "x".into(),
                }),
            )
            .await
            .expect("get");
        assert_eq!(
            decode_reply(&reply).expect("decode"),
            AutomationsReply::Rule(None)
        );

        // ONE block where the middle member rejects AGAINST THE STAGE (a
        // duplicate of the rule staged by the first member — read-your-writes
        // decides the rejection), while the third member disables that same
        // staged rule: [Applied, Rejected, Applied].
        let (n_before, w_before) = (root_of(&native), root_of(&wasm));
        let batch = vec![
            (
                ops.clone(),
                create_rule(
                    "y",
                    on_general(None),
                    Action::CreateTask {
                        task_id_prefix: "p".into(),
                        title_template: "one".into(),
                    },
                ),
            ),
            (
                ops.clone(),
                create_rule(
                    "y",
                    on_general(None),
                    Action::CreateTask {
                        task_id_prefix: "p".into(),
                        title_template: "two".into(),
                    },
                ),
            ),
            (
                ops.clone(),
                auto_op(&AutomationsMsg::SetEnabled {
                    rule_id: "y".into(),
                    enabled: false,
                }),
            ),
        ];
        let n_out = native
            .submit_block(block(2, ops.clone()), batch.clone())
            .await
            .expect("native block");
        let w_out = wasm
            .submit_block(block(2, ops.clone()), batch)
            .await
            .expect("wasm block");
        for out in [&n_out, &w_out] {
            assert!(matches!(out.members[0], MemberOutcome::Applied { .. }));
            assert!(
                matches!(out.members[1], MemberOutcome::Rejected { .. }),
                "the duplicate must reject against the SAME-BLOCK stage: {:?}",
                out.members
            );
            assert!(matches!(out.members[2], MemberOutcome::Applied { .. }));
        }
        assert_ne!(root_of(&native), n_before);
        assert_ne!(root_of(&wasm), w_before);
        assert_eq!(replies(&native).await, replies(&wasm).await);
    });
}
