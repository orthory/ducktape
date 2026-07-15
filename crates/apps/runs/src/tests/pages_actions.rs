use super::*;
use agent::{ACTION_PAGES_COMMENT, ACTION_PAGES_SET_CHECKED};
use pages::PageMsg;

fn page_trigger_thread() -> pages::ThreadView {
    pages::ThreadView {
        thread: pages::Thread {
            id: "thread-1".into(),
            target: "b-p".into(),
            opener: pages::AuthorRef::User(vec![4; 32]),
            created_at: 1,
            anchor: None,
            resolved: false,
            resolved_by: None,
            comment_ids: vec!["comment-1".into()],
        },
        comments: vec![pages::Comment {
            id: "comment-1".into(),
            thread_id: "thread-1".into(),
            author: pages::AuthorRef::User(vec![4; 32]),
            text: "@bot review".into(),
            created_at: 1,
            edited_at: None,
            deleted: false,
        }],
    }
}

#[test]
fn pages_triggered_run_replies_in_the_same_comment_thread() {
    let mut registry = registry(&[("bot", &[ACTION_PAGES_COMMENT])]);
    registry.get_mut("bot").unwrap().caps.pages_write = vec!["p1".into()];
    let mut m = module()
        .with_files_module("files")
        .with_pages_module("pages");
    let mut engage_ctx = CaptureCtx::new()
        .with_tagging_origin()
        .with_registry(&registry)
        .with_page("p1", page_blocks("p1", "Spec"))
        .with_page_thread(page_trigger_thread());
    let engagement = Msg {
        target: "runs".into(),
        payload: tagging_encode_event(&EngagementEvent {
            source: "pages".into(),
            container: "thread-1".into(),
            content_seq: 1,
            author: Author::User(vec![4; 32]),
            tags: vec![agent_tag("bot")],
        }),
    };
    exec(&mut m, &mut engage_ctx, &engagement).unwrap();
    commit(&mut m);

    let run_id = page_run_id_for("thread-1", 1, "bot");
    let mut delivery = CaptureCtx::new()
        .at(8)
        .with_dispatch_origin()
        .with_registry(&registry)
        .with_page("p1", page_blocks("p1", "Spec"))
        .with_page_thread(page_trigger_thread());
    exec(
        &mut m,
        &mut delivery,
        &result_event(
            &run_id,
            Ok(runner_wrapper("Reviewed.", serde_json::json!({}))),
        ),
    )
    .unwrap();
    assert!(delivery.chat_msgs().is_empty());
    let replies = delivery.page_msgs();
    assert_eq!(replies.len(), 1);
    let PageMsg::AddComment {
        thread_id,
        target,
        text,
        anchor,
        mentions,
        as_agent,
        ..
    } = &replies[0]
    else {
        panic!("expected page comment reply")
    };
    assert_eq!(thread_id, "thread-1");
    assert_eq!(target, "b-p");
    assert_eq!(text, "Reviewed.");
    assert!(mentions.is_empty());
    assert!(anchor.is_none());
    assert_eq!(as_agent.as_deref(), Some("bot"));
}

// ---- the pages effects lane (M2) ---------------------------------------------
// pages.comment / pages.set_checked applied at the run boundary: grant + cap
// gated, probe-guarded, and — unlike the task lane — degrading PER ACTION.

/// a pages-wired module holding one pending run for "bot" (granted `actions`,
/// pages_write = `caps`), plus the registry and the run id.
fn awaiting_pages_run(actions: &[&str], caps: &[&str]) -> (RunsModule, Registry, String) {
    let mut registry = registry(&[("bot", actions)]);
    registry.get_mut("bot").unwrap().caps.pages_write =
        caps.iter().map(|s| s.to_string()).collect();
    let mut m = watched(TurnPolicy::All, &registry).with_pages_module("pages");
    engage_post(&mut m, &registry, 2, &[]);
    commit(&mut m);
    (m, registry, run_id_for("general", 2, "bot"))
}

/// the delivery ctx: dispatch origin, the registry, the transcript, and the
/// committed page "p1" (root + paragraph "b-p" + todo "b-t").
fn delivery_ctx(registry: &Registry) -> CaptureCtx {
    CaptureCtx::new()
        .at(8)
        .with_dispatch_origin()
        .with_registry(registry)
        .with_transcript("general", transcript(2))
        .with_page("p1", page_blocks("p1", "Spec"))
}

fn comment_effect(target: &str) -> serde_json::Value {
    serde_json::json!({
        "effects": [{"kind": ACTION_PAGES_COMMENT, "target": target, "body": "looks good"}]
    })
}

fn deliver(m: &mut RunsModule, ctx: &mut CaptureCtx, run_id: &str, facets: serde_json::Value) {
    exec(
        m,
        ctx,
        &result_event(run_id, Ok(runner_wrapper("done", facets))),
    )
    .unwrap();
}

/// the run's terminal record after commit — the "run DELIVERS" assertion.
fn assert_delivered(m: &mut RunsModule, run_id: &str) {
    commit(m);
    let record = recent_runs(m)
        .into_iter()
        .find(|r| r.run_id == run_id)
        .expect("a terminal record");
    assert_eq!(record.outcome, RunOutcome::Delivered, "the run delivers");
}

#[test]
fn a_pages_comment_effect_lands_agent_authored_with_deterministic_ids() {
    let (mut m, registry, run_id) =
        awaiting_pages_run(&[ACTION_CHAT_POST, ACTION_PAGES_COMMENT], &["p1"]);
    let mut ctx = delivery_ctx(&registry);
    // the target is a BLOCK id — the cap resolves its owning page "p1".
    deliver(&mut m, &mut ctx, &run_id, comment_effect("b-p"));

    assert_eq!(ctx.chat_msgs().len(), 1, "the reply still posts");
    let msgs = ctx.page_msgs();
    assert_eq!(msgs.len(), 1, "exactly one pages follow-up");
    let PageMsg::AddComment {
        thread_id,
        comment_id,
        target,
        text,
        anchor,
        mentions,
        as_agent,
    } = &msgs[0]
    else {
        panic!("expected AddComment, got {:?}", msgs[0]);
    };
    // ids derive from hash(run_id) + action index — replay-identical, never
    // random, and index-safe (hex, no reserved separators) by construction.
    let rid = dispatch_id_for(&run_id);
    assert_eq!(*thread_id, format!("agent/{rid}/thread/0"));
    assert_eq!(*comment_id, format!("agent/{rid}/comment/0"));
    assert!(pages::id_is_index_safe(thread_id) && pages::id_is_index_safe(comment_id));
    assert_eq!(target, "b-p");
    assert_eq!(text, "looks good");
    assert!(anchor.is_none());
    assert!(mentions.is_empty());
    assert_eq!(
        as_agent.as_deref(),
        Some("bot"),
        "the comment is agent-attributed"
    );
    assert_delivered(&mut m, &run_id);
}

#[test]
fn a_page_root_target_and_a_wildcard_cap_also_pass_the_gate() {
    // the target IS the page id (a root names itself as its page) and the
    // grant is the literal wildcard.
    let (mut m, registry, run_id) =
        awaiting_pages_run(&[ACTION_CHAT_POST, ACTION_PAGES_COMMENT], &["*"]);
    let mut ctx = delivery_ctx(&registry);
    deliver(&mut m, &mut ctx, &run_id, comment_effect("p1"));
    assert_eq!(ctx.page_msgs().len(), 1);
    assert_delivered(&mut m, &run_id);
}

#[test]
fn a_cap_denied_pages_action_degrades_and_the_run_still_delivers() {
    // granted the ACTION but pages_write covers a different page.
    let (mut m, registry, run_id) =
        awaiting_pages_run(&[ACTION_CHAT_POST, ACTION_PAGES_COMMENT], &["other-page"]);
    let mut ctx = delivery_ctx(&registry);
    deliver(&mut m, &mut ctx, &run_id, comment_effect("b-p"));

    assert!(ctx.page_msgs().is_empty(), "the denied comment is dropped");
    assert!(
        ctx.notes()
            .iter()
            .any(|n| n.contains("lacks pages_write for p1")),
        "the deny leaves a breadcrumb: {:?}",
        ctx.notes()
    );
    assert_eq!(ctx.chat_msgs().len(), 1, "the reply still posts");
    assert_delivered(&mut m, &run_id);
}

#[test]
fn an_ungranted_pages_action_degrades_instead_of_failing_the_run() {
    // pages.comment is NOT in allowed_actions — unlike a task action, the
    // grant miss degrades this action alone (decision 6's scoping).
    let (mut m, registry, run_id) = awaiting_pages_run(&[ACTION_CHAT_POST], &["*"]);
    let mut ctx = delivery_ctx(&registry);
    deliver(&mut m, &mut ctx, &run_id, comment_effect("b-p"));

    assert!(ctx.page_msgs().is_empty());
    assert!(
        ctx.notes()
            .iter()
            .any(|n| n.contains("not allowed to pages.comment")),
        "{:?}",
        ctx.notes()
    );
    assert_eq!(ctx.chat_msgs().len(), 1);
    assert_delivered(&mut m, &run_id);
}

#[test]
fn an_unresolvable_target_and_an_empty_body_each_degrade_alone() {
    let (mut m, registry, run_id) = awaiting_pages_run(
        &[
            ACTION_CHAT_POST,
            ACTION_PAGES_COMMENT,
            ACTION_PAGES_SET_CHECKED,
        ],
        &["*"],
    );
    let mut ctx = delivery_ctx(&registry);
    // three actions: a ghost target, an empty body, and one VALID todo flip —
    // the bad ones degrade, the good one still applies.
    deliver(
        &mut m,
        &mut ctx,
        &run_id,
        serde_json::json!({
            "effects": [
                {"kind": ACTION_PAGES_COMMENT, "target": "ghost", "body": "hi"},
                {"kind": ACTION_PAGES_COMMENT, "target": "b-p", "body": ""},
                {"kind": ACTION_PAGES_SET_CHECKED, "block": "b-t", "checked": true},
            ]
        }),
    );

    let msgs = ctx.page_msgs();
    assert_eq!(msgs.len(), 1, "only the valid action applies: {msgs:?}");
    assert!(matches!(
        &msgs[0],
        PageMsg::SetChecked { block_id, checked: true } if block_id == "b-t"
    ));
    let notes = ctx.notes();
    assert!(
        notes
            .iter()
            .any(|n| n.contains("target does not exist: ghost")),
        "{notes:?}"
    );
    assert!(
        notes.iter().any(|n| n.contains("comment body is empty")),
        "{notes:?}"
    );
    assert_delivered(&mut m, &run_id);
}

#[test]
fn set_checked_requires_a_todo_block_and_carries_no_attribution() {
    let (mut m, registry, run_id) =
        awaiting_pages_run(&[ACTION_CHAT_POST, ACTION_PAGES_SET_CHECKED], &["p1"]);
    let mut ctx = delivery_ctx(&registry);
    deliver(
        &mut m,
        &mut ctx,
        &run_id,
        serde_json::json!({
            "effects": [
                {"kind": ACTION_PAGES_SET_CHECKED, "block": "b-p", "checked": true},
                {"kind": ACTION_PAGES_SET_CHECKED, "block": "b-t", "checked": true},
            ]
        }),
    );

    let msgs = ctx.page_msgs();
    // the paragraph flip degrades (pages would reject NotTodo — probed, so
    // the emitted op can never abort the block); the todo flip applies.
    assert_eq!(msgs.len(), 1);
    assert!(
        matches!(&msgs[0], PageMsg::SetChecked { block_id, checked: true } if block_id == "b-t"),
        "SetChecked carries no as_agent — origin-gated only: {:?}",
        msgs[0]
    );
    assert!(
        ctx.notes().iter().any(|n| n.contains("is not a todo")),
        "{:?}",
        ctx.notes()
    );
    assert_delivered(&mut m, &run_id);
}

#[test]
fn squatted_ids_and_a_crowded_target_degrade_the_comment() {
    // anyone can mint pages ids, so the deterministic thread/comment ids are
    // squattable and the target's thread list is cappable — each probe must
    // catch its case (an emitted op pages rejects would abort the block).
    let (_, registry, run_id) = awaiting_pages_run(&[ACTION_CHAT_POST, ACTION_PAGES_COMMENT], &["*"]);
    let rid = dispatch_id_for(&run_id);
    for (ctx, needle) in [
        (
            delivery_ctx(&registry).with_taken_page_id(&format!("agent/{rid}/thread/0")),
            "thread id already taken",
        ),
        (
            delivery_ctx(&registry).with_taken_page_id(&format!("agent/{rid}/comment/0")),
            "comment id already taken",
        ),
        (
            delivery_ctx(&registry).with_crowded_page_target("b-p"),
            "already holds",
        ),
    ] {
        let mut m2 = {
            let (m2, ..) = awaiting_pages_run(&[ACTION_CHAT_POST, ACTION_PAGES_COMMENT], &["*"]);
            m2
        };
        let mut ctx = ctx;
        deliver(&mut m2, &mut ctx, &run_id, comment_effect("b-p"));
        assert!(ctx.page_msgs().is_empty(), "the squatted case emits nothing");
        assert!(
            ctx.notes().iter().any(|n| n.contains(needle)),
            "expected {needle:?} in {:?}",
            ctx.notes()
        );
        assert_delivered(&mut m2, &run_id);
    }
}

#[test]
fn same_block_thread_cap_degrades_the_overflow_comment_without_aborting() {
    // the target holds (cap - 1) COMMITTED threads; the run emits two
    // comments to it. the committed-only probe is blind to the first
    // comment's staged thread, so without same-run accounting BOTH would
    // emit and the second AddComment would abort the delivery block
    // (TooManyThreads). the accounting makes the second DEGRADE instead.
    let cap = pages::MAX_THREADS_PER_TARGET;
    let (mut m, registry, run_id) =
        awaiting_pages_run(&[ACTION_CHAT_POST, ACTION_PAGES_COMMENT], &["*"]);
    let mut ctx = delivery_ctx(&registry).with_page_target_threads("b-p", cap - 1);
    deliver(
        &mut m,
        &mut ctx,
        &run_id,
        serde_json::json!({
            "effects": [
                {"kind": ACTION_PAGES_COMMENT, "target": "b-p", "body": "first"},
                {"kind": ACTION_PAGES_COMMENT, "target": "b-p", "body": "second"},
            ]
        }),
    );

    let msgs = ctx.page_msgs();
    assert_eq!(msgs.len(), 1, "only the first comment fits: {msgs:?}");
    assert!(matches!(&msgs[0], PageMsg::AddComment { text, .. } if text == "first"));
    assert!(
        ctx.notes().iter().any(|n| n.contains("already holds")),
        "the overflow comment leaves a cap breadcrumb: {:?}",
        ctx.notes()
    );
    assert_eq!(ctx.chat_msgs().len(), 1, "the reply still posts (no abort)");
    assert_delivered(&mut m, &run_id);
}

#[test]
fn a_pathological_channel_still_yields_a_safe_hashed_comment_id() {
    // the run id ALWAYS carries the reserved \x1f separator AND a channel id
    // of any length. an inline-run-id scheme would make the minted comment id
    // both escape-laden (\x1f -> , 6 B) and, for a long channel,
    // over-length — pages would reject it (IdTooLarge) and abort the block.
    // hashing the run id keeps the minted id short, hex, escape-free, so the
    // comment LANDS regardless of the channel — the structural immunization.
    let channel = "c".repeat(400);
    let mut registry = registry(&[("bot", &[ACTION_CHAT_POST, ACTION_PAGES_COMMENT])]);
    registry.get_mut("bot").unwrap().caps.pages_write = vec!["*".into()];
    let mut m = module().with_pages_module("pages");
    let mut ctx = CaptureCtx::new().with_origin(user(9)).with_registry(&registry);
    exec(
        &mut m,
        &mut ctx,
        &admin(&RunsMsg::WatchChannel {
            channel_id: channel.clone(),
            policy: TurnPolicy::All,
        }),
    )
    .unwrap();
    commit(&mut m);
    let mut ctx = CaptureCtx::new()
        .at(2)
        .with_tagging_origin()
        .with_registry(&registry)
        .with_transcript(&channel, transcript(2));
    exec(&mut m, &mut ctx, &engagement(&channel, 2, vec![])).unwrap();
    commit(&mut m);
    let run_id = run_id_for(&channel, 2, "bot");

    let mut ctx = CaptureCtx::new()
        .at(8)
        .with_dispatch_origin()
        .with_registry(&registry)
        .with_transcript(&channel, transcript(2))
        .with_page("p1", page_blocks("p1", "Spec"));
    deliver(&mut m, &mut ctx, &run_id, comment_effect("b-p"));

    let msgs = ctx.page_msgs();
    assert_eq!(msgs.len(), 1, "the comment lands despite the channel: {msgs:?}");
    let PageMsg::AddComment {
        thread_id,
        comment_id,
        ..
    } = &msgs[0]
    else {
        panic!("expected AddComment, got {:?}", msgs[0]);
    };
    assert!(
        pages::id_is_index_safe(thread_id) && thread_id.len() <= pages::MAX_THREAD_ID_BYTES,
        "minted thread id must clear pages' caps: {thread_id}"
    );
    assert!(
        pages::id_is_index_safe(comment_id) && comment_id.len() <= pages::MAX_COMMENT_ID_BYTES,
        "minted comment id must clear pages' caps: {comment_id}"
    );
    assert_eq!(ctx.chat_msgs().len(), 1, "the reply still posts (no abort)");
    assert_delivered(&mut m, &run_id);
}

#[test]
fn an_unwired_pages_module_degrades_to_a_breadcrumb() {
    // the same run on a module WITHOUT with_pages_module: the forge-unwired
    // pattern — breadcrumb, no pages msg, delivery proceeds.
    let registry = {
        let mut r = registry(&[("bot", &[ACTION_CHAT_POST, ACTION_PAGES_COMMENT])]);
        r.get_mut("bot").unwrap().caps.pages_write = vec!["*".into()];
        r
    };
    let mut m = watched(TurnPolicy::All, &registry);
    engage_post(&mut m, &registry, 2, &[]);
    commit(&mut m);
    let run_id = run_id_for("general", 2, "bot");
    let mut ctx = delivery_ctx(&registry);
    deliver(&mut m, &mut ctx, &run_id, comment_effect("b-p"));

    assert!(ctx.page_msgs().is_empty());
    assert!(
        ctx.notes().iter().any(|n| n.contains("no pages module wired")),
        "{:?}",
        ctx.notes()
    );
    assert_eq!(ctx.chat_msgs().len(), 1);
    assert_delivered(&mut m, &run_id);
}

#[test]
fn task_actions_keep_their_all_or_nothing_lane() {
    // a response mixing a VALID pages action with an INVALID task action
    // still fails the whole run — decision 6 scopes the degrade to the two
    // pages actions only; the task lane is untouched.
    let (mut m, registry, run_id) =
        awaiting_pages_run(&[ACTION_CHAT_POST, ACTION_PAGES_COMMENT], &["*"]);
    let mut ctx = delivery_ctx(&registry);
    deliver(
        &mut m,
        &mut ctx,
        &run_id,
        serde_json::json!({
            "effects": [
                {"kind": ACTION_PAGES_COMMENT, "target": "b-p", "body": "hi"},
                // tasks.create was never granted — the strict lane fails the run.
                {"kind": "tasks.create", "task_id": "t9", "title": "nope"},
            ]
        }),
    );
    assert!(ctx.page_msgs().is_empty(), "nothing applies on a failed run");
    assert!(ctx.task_msgs().is_empty());
    commit(&mut m);
    let record = recent_runs(&m)
        .into_iter()
        .find(|r| r.run_id == run_id)
        .expect("a terminal record");
    assert_eq!(record.outcome, RunOutcome::Failed, "the task lane still fails the run");
}
