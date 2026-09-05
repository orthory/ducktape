//! state-sync round-trip: a joiner reconstructs a byte-identical qmdb root by
//! pulling a source store's operation range through commonware's qmdb sync,
//! then wraps a fresh `Automations` around the injected store — the same
//! discriminating property chat, pages, and agent prove, over the rules +
//! run-history layout.
//!
//! the source creates rules, DISABLES one (record overwrite), DELETES one
//! (roster overwrite + record delete), and fires the hook arm twice (rule
//! fire-count overwrite, run-cursor overwrites, seq-keyed run records — one
//! carrying an event channel id far over this module's own id caps, which the
//! op log must ship verbatim), so a naive "export live records and re-apply
//! sorted" could never reproduce the log. only a real sync that ships the
//! ACTUAL proven op range lands on the same root.
//!
//! the roster and the run cursor (the two consensus-consumed aggregate
//! records) are ordinary store state under reserved keys, so they sync like
//! any record — the joiner answers the full listing and the run history
//! exactly like the source.

use automations::{
    Action, Automations, AutomationsMsg, AutomationsQuery, AutomationsReply, MAX_ID_BYTES, Trigger,
    decode_reply, encode_msg, encode_query,
};
use chat::{
    AuthorRef, ChannelAccess, ChatEvent, ChatReply, encode_event as chat_encode_event,
    encode_reply as chat_encode_reply,
};
use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use sdk::{Env, MerkleStore as _, Module, Msg, Origin, StateRoot};
use sdk_testkit::TestCtx;
use statesync::qmdb::QmdbStore;

/// drives `execute` with a controllable env. the only sibling read the ops
/// below make is the fire path's owner-standing probe (the fired rule is an
/// otherwise probe-free `DeliverInbox` with static templates), answered here
/// as an open channel does — admitted.
fn ctx(height: u64, origin: Origin) -> TestCtx {
    TestCtx::with_env(Env {
        height,
        consensus_time: height,
        origin,
        me: "automations".into(),
    })
    .on_query("chat", |_req| {
        Ok(chat_encode_reply(&ChatReply::Access(ChannelAccess {
            may_read: true,
            may_post: true,
        })))
    })
}

fn operator() -> Origin {
    Origin::External(vec![9; 32])
}

fn admin(m: &AutomationsMsg) -> Msg {
    Msg {
        target: "automations".into(),
        payload: encode_msg(m),
    }
}

fn create(rule_id: &str, channel: Option<&str>, action: Action) -> AutomationsMsg {
    AutomationsMsg::CreateRule {
        rule_id: rule_id.into(),
        trigger: Trigger {
            channel_id: channel.map(Into::into),
            mention: None,
            text_contains: None,
        },
        action,
    }
}

/// a hook event as chat delivers it: raw ChatEvent bytes under the chat origin.
fn posted(channel: &str, seq: u64) -> Msg {
    Msg {
        target: "automations".into(),
        payload: chat_encode_event(&ChatEvent::MessagePosted {
            channel_id: channel.into(),
            seq,
            thread_root: None,
            author: AuthorRef::User(vec![1; 32]),
            mentions: Vec::new(),
        }),
    }
}

// drive one op through the REAL module path: execute + commit_block (one op
// per block-height), so the committed op log is what a validator produces.
async fn apply_commit(m: &mut Automations, height: u64, origin: Origin, op: Msg) {
    m.execute(&mut ctx(height, origin), &op).await.unwrap();
    m.commit_block().await.unwrap();
}

async fn query_reply(m: &Automations, q: &AutomationsQuery) -> AutomationsReply {
    decode_reply(&m.query(&encode_query(q)).await.unwrap()).unwrap()
}

/// the read matrix compared source-vs-joiner: the listing, the deleted rule's
/// point read, and the firing rule's run history.
const QUERIES: [&str; 3] = ["rules", "gamma", "history"];

async fn replies(m: &Automations) -> Vec<AutomationsReply> {
    let queries = [
        AutomationsQuery::ListRules,
        AutomationsQuery::GetRule {
            rule_id: "gamma".into(),
        },
        AutomationsQuery::RunHistory {
            rule_id: "alpha".into(),
            limit: 16,
        },
    ];
    let mut out = Vec::new();
    for q in &queries {
        out.push(query_reply(m, q).await);
    }
    out
}

#[test]
fn synced_store_reconstructs_source_root_rules_and_history() {
    deterministic::Runner::default().start(|context| async move {
        // SOURCE: build the rule book + run ring through the real op path —
        // a disable (overwrite), a delete (roster overwrite + record delete),
        // and two fires (fire-count/cursor overwrites, run-record appends),
        // so the op log carries more than inserts.
        let mut src = Automations::new(
            "automations",
            Box::new(QmdbStore::init(context.child("src"), "src").await),
            "chat",
            "tasks",
            "inbox",
        );
        apply_commit(
            &mut src,
            1,
            operator(),
            admin(&create(
                "alpha",
                None,
                Action::DeliverInbox {
                    member_template: "ops".into(),
                    kind: "note".into(),
                    body_template: "a post landed".into(),
                },
            )),
        )
        .await;
        apply_commit(
            &mut src,
            2,
            operator(),
            admin(&create(
                "beta",
                Some("general"),
                Action::PostMessage {
                    channel_id: "general".into(),
                    template: "echo".into(),
                },
            )),
        )
        .await;
        apply_commit(
            &mut src,
            3,
            operator(),
            admin(&create(
                "gamma",
                Some("general"),
                Action::CreateTask {
                    task_id_prefix: "job".into(),
                    title_template: "t".into(),
                },
            )),
        )
        .await;
        apply_commit(
            &mut src,
            4,
            operator(),
            admin(&AutomationsMsg::SetEnabled {
                rule_id: "beta".into(),
                enabled: false,
            }),
        )
        .await;
        apply_commit(
            &mut src,
            5,
            operator(),
            admin(&AutomationsMsg::DeleteRule {
                rule_id: "gamma".into(),
            }),
        )
        .await;
        // two hook fires under the chat origin: only the wildcard alpha is
        // enabled and probe-free. the second event's channel id is far over
        // this module's own id caps — the run record must ship through the op
        // log verbatim (chat, not this module, bounds event channel ids).
        let chat_origin = Origin::Module("chat".into());
        apply_commit(&mut src, 6, chat_origin.clone(), posted("general", 1)).await;
        let long_channel = "c".repeat(MAX_ID_BYTES * 2);
        apply_commit(&mut src, 7, chat_origin, posted(&long_channel, 1)).await;

        let src_root: StateRoot = src.root();
        assert_ne!(src_root, StateRoot::ZERO, "source must have a real root");
        // capture the source's answers to compare against the joiner's.
        let src_replies = replies(&src).await;
        let AutomationsReply::History(src_history) = &src_replies[2] else {
            panic!("expected history");
        };
        assert_eq!(src_history.len(), 2, "both fires were recorded");
        assert_eq!(src_history[1].channel_id, long_channel);

        // the module consumed its store, so REOPEN the committed partitions
        // as a bare store for the handoff (drop first — one owner at a time).
        drop(src);
        let src_store = QmdbStore::init(context.child("src_serve"), "src").await;
        assert_eq!(
            src_store.root(),
            src_root,
            "reopened store must recover the committed root"
        );

        // describe the target (root + op range), THEN hand the source off as
        // the sync resolver (consumes it — order matters).
        let target = src_store.sync_boundary_target().await;
        let resolver = src_store.into_resolver();

        // JOINER: reconstruct on a FRESH context + namespace by pulling from
        // the resolver, then wrap the module around the injected store — the
        // exact shape a joining host uses. no ops are applied in application
        // order on this side.
        let store = QmdbStore::sync_from(context.child("dst"), "dst", target, resolver)
            .await
            .expect("sync_from");
        let synced = Automations::new("automations", Box::new(store), "chat", "tasks", "inbox");

        // THE PROPERTY: identical qmdb root — the root-hash linkage a joiner
        // needs at the boundary height.
        assert_eq!(
            synced.root(),
            src_root,
            "synced store root must equal the source root"
        );

        // roster, records, cursor, and run ring synced together: the joiner
        // answers every read exactly like the source.
        let synced_replies = replies(&synced).await;
        for (name, (a, b)) in QUERIES.iter().zip(src_replies.iter().zip(&synced_replies)) {
            assert_eq!(a, b, "the {name} reply diverged");
        }
        let AutomationsReply::Rules(rules) = &synced_replies[0] else {
            panic!("expected the listing");
        };
        assert_eq!(rules.len(), 2, "gamma's delete survived the sync");
        assert_eq!(rules[0].rule_id, "alpha");
        assert_eq!(rules[0].fire_count, 2, "the fire-count overwrites landed");
        assert_eq!(rules[1].rule_id, "beta");
        assert!(!rules[1].enabled, "the disable overwrite landed");
        assert_eq!(
            synced_replies[1],
            AutomationsReply::Rule(None),
            "the deleted rule stays deleted on the joiner"
        );
    });
}
