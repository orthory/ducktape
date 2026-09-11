use super::*;
use crate::OP_CHAT_POST_MESSAGE;
use pages::PageMsg;

// ---- the agent session lane --------------------------------------------------
// an ephemeral key, bound to a live run by the node that HOLDS ITS LEASE, is
// the only origin that may write mid-run. every refusal here is a LOUD Err
// (unlike the settle path's pages degrade): the agent submitted this op and is
// waiting on the answer.

/// the node executing the run — the committed lease-holder, and therefore the
/// only origin that may open its session.
const ASSIGNEE: [u8; 32] = [0xab; 32];
/// the ephemeral session key the assignee mints for one run.
const SESSION_KEY: [u8; 32] = [0xcd; 32];
const CHILD_SESSION_KEY: [u8; 32] = [0xde; 32];

/// a pages-wired module with one in-flight run for "bot", plus the registry
/// and the run id.
fn awaiting_session_run() -> (RunsModule, Registry, String) {
    let registry = registry(&["bot"]);
    let mut m = configured(&registry).with_pages_module("pages");
    request_post(&mut m, &registry, 2, &[]);
    commit(&mut m);
    (m, registry, run_id_for("general", 2, "bot"))
}

/// a ctx for a session op: `origin` submits, and the run's lease is held by
/// ASSIGNEE. carries the transcript (the channel exists) and the page "p1".
fn session_ctx(registry: &Registry, run_id: &str, origin: Origin) -> CaptureCtx {
    CaptureCtx::new()
        .at(5)
        .with_origin(origin)
        .with_registry(registry)
        .with_transcript("general", transcript(2))
        .with_page("p1", page_blocks("p1", "Spec"))
        .with_lease_holder(run_id, &ASSIGNEE)
}

fn open(run_id: &str, key: &[u8]) -> Msg {
    admin(&RunsMsg::OpenAgentSession {
        attempt: 0,
        run_id: run_id.into(),
        session_key: key.to_vec(),
    })
}

fn open_attempt(run_id: &str, attempt: u32, key: &[u8]) -> Msg {
    admin(&RunsMsg::OpenAgentSession {
        run_id: run_id.into(),
        attempt,
        session_key: key.to_vec(),
    })
}

/// one live action under a fresh request id — every call is new work.
fn act(run_id: &str, action: ActionEnvelope) -> Msg {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    act_as(run_id, &format!("auto-{n}"), action)
}

/// one live action under the caller's request id — the idempotency key.
fn act_as(run_id: &str, request_id: &str, action: ActionEnvelope) -> Msg {
    admin(&RunsMsg::AgentAction {
        run_id: run_id.into(),
        request_id: request_id.into(),
        action,
    })
}

fn delegate(run_id: &str, request_id: &str, agent_id: &str, instruction: &str) -> Msg {
    act_as(run_id, request_id, agent_call(agent_id, instruction))
}

fn comment(target: &str) -> ActionEnvelope {
    page_comment(target, "looks good")
}

fn sessions(m: &RunsModule) -> Vec<AgentSession> {
    let reply = block_on(m.query(&encode_query(&RunsQuery::AgentSessions))).unwrap();
    match runs_decode_reply(&reply).unwrap() {
        RunsReply::AgentSessions(sessions) => sessions,
        other => panic!("unexpected reply: {other:?}"),
    }
}

fn delegations(m: &RunsModule, caller_run_id: &str) -> Vec<DelegationView> {
    let reply = block_on(m.query(&encode_query(&RunsQuery::Delegations {
        caller_run_id: caller_run_id.into(),
    })))
    .unwrap();
    match runs_decode_reply(&reply).unwrap() {
        RunsReply::Delegations(delegations) => delegations,
        other => panic!("unexpected reply: {other:?}"),
    }
}

/// a module whose run already carries a committed, freshly-opened session.
fn with_open_session() -> (RunsModule, Registry, String) {
    let (mut m, registry, run_id) = awaiting_session_run();
    let mut ctx = session_ctx(&registry, &run_id, Origin::External(ASSIGNEE.to_vec()));
    exec(&mut m, &mut ctx, &open(&run_id, &SESSION_KEY)).unwrap();
    commit(&mut m);
    (m, registry, run_id)
}

fn with_open_delegating_session() -> (RunsModule, Registry, String) {
    let registry = registry(&["bot", "worker", "reviewer"]);
    let mut m = configured(&registry);
    request_post(&mut m, &registry, 2, &[]);
    commit(&mut m);
    let run_id = run_id_for("general", 2, "bot");
    let mut ctx = session_ctx(&registry, &run_id, Origin::External(ASSIGNEE.to_vec()));
    exec(&mut m, &mut ctx, &open(&run_id, &SESSION_KEY)).unwrap();
    commit(&mut m);
    (m, registry, run_id)
}

// ---- opening: the lease IS the authorization ---------------------------------

#[test]
fn a_live_session_calls_a_peer_and_collects_its_result_without_a_parent_record() {
    let (mut m, registry, caller_run) = with_open_delegating_session();
    let mut ctx = session_ctx(
        &registry,
        &caller_run,
        Origin::External(SESSION_KEY.to_vec()),
    );
    exec(
        &mut m,
        &mut ctx,
        &delegate(&caller_run, "parser", "worker", "Implement the parser."),
    )
    .unwrap();
    assert_eq!(ctx.dispatch_msgs().len(), 1);
    let calls = delegations(&m, &caller_run);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].status, DelegationStatus::Pending);
    assert_eq!(calls[0].callee_agent_id, "worker");
    assert_eq!(calls[0].root_run_id, caller_run);
    let callee_run = calls[0].callee_run_id.clone();
    commit(&mut m);

    let mut joiner = module();
    joiner.install(&m.snapshot(), m.root()).unwrap();
    assert_eq!(
        delegations(&joiner, &caller_run),
        delegations(&m, &caller_run),
        "the live call edge and scoped result lane round-trip through state sync"
    );
    assert!(get_pending(&joiner, &callee_run).is_some());

    let mut result_ctx = CaptureCtx::new()
        .at(8)
        .with_dispatch_origin()
        .with_registry(&registry)
        .with_transcript("general", transcript(2));
    exec(
        &mut m,
        &mut result_ctx,
        &result_event(
            &callee_run,
            Ok(runner_wrapper("Worker result", serde_json::json!({}))),
        ),
    )
    .unwrap();
    assert!(
        result_ctx.chat_msgs().is_empty(),
        "a callee returns to its caller, not to the user's chat thread"
    );
    commit(&mut m);
    let calls = delegations(&m, &caller_run);
    assert_eq!(calls[0].status, DelegationStatus::Delivered);
    assert_eq!(
        calls[0].result.as_ref().unwrap().reply_blocks[0].text,
        "Worker result"
    );
    assert!(
        get_pending(&m, &caller_run).is_some(),
        "the caller stays live"
    );

    let mut settle = CaptureCtx::new()
        .at(9)
        .with_dispatch_origin()
        .with_registry(&registry)
        .with_transcript("general", transcript(2));
    exec(
        &mut m,
        &mut settle,
        &result_event(
            &caller_run,
            Ok(runner_wrapper("Done", serde_json::json!({}))),
        ),
    )
    .unwrap();
    commit(&mut m);
    assert!(
        delegations(&m, &caller_run).is_empty(),
        "root settlement removes its ephemeral call tree"
    );
}

#[test]
fn delegate_run_truncates_references_at_the_dispatch_wide_sibling_budget() {
    let (mut m, registry, caller_run) = with_open_delegating_session();
    m = m.with_files_module("files").with_pages_module("pages");
    let page_limit = usize::from(pages::MAX_PAGE_QUERY_LIMIT);
    let mut ctx = session_ctx(
        &registry,
        &caller_run,
        Origin::External(SESSION_KEY.to_vec()),
    )
    .with_transcript(
        "general",
        vec![
            message(1, "start"),
            message(
                2,
                "[Plan](duck://page/plan) [notes](duck://files/shared/attachments/u/notes.md)",
            ),
        ],
    )
    .with_page(
        "plan",
        page_with_block_count(page_limit * MAX_SIBLING_QUERY_READS + 1, ""),
    )
    .with_file(
        "/shared/attachments/u/notes.md",
        b"must not cross the budget",
    );

    exec(
        &mut m,
        &mut ctx,
        &delegate(&caller_run, "bounded", "worker", "read the references"),
    )
    .unwrap();

    assert_eq!(ctx.distinct_query_count(), MAX_SIBLING_QUERY_READS);
    let DispatchMsg::Dispatch { payload, .. } = &ctx.dispatch_msgs()[0] else {
        panic!("expected delegated dispatch");
    };
    let payload: serde_json::Value = serde_json::from_slice(payload).unwrap();
    let context = payload["context"].as_str().unwrap();
    assert!(
        context.contains("[page context truncated at bounded read limit]"),
        "{context}"
    );
    assert!(
        context.contains("[attachment context truncated at bounded read limit]"),
        "{context}"
    );
}

#[test]
fn call_ids_are_idempotent_and_completed_calls_release_the_root_slot() {
    let (mut m, registry, caller_run) = with_open_delegating_session();
    let call = delegate(&caller_run, "one", "worker", "work");
    let mut first = session_ctx(
        &registry,
        &caller_run,
        Origin::External(SESSION_KEY.to_vec()),
    );
    exec(&mut m, &mut first, &call).unwrap();
    commit(&mut m);
    assert_eq!(sessions(&m)[0].actions, 1);

    let mut replay = session_ctx(
        &registry,
        &caller_run,
        Origin::External(SESSION_KEY.to_vec()),
    );
    exec(&mut m, &mut replay, &call).unwrap();
    assert!(replay.dispatch_msgs().is_empty(), "same request is a no-op");
    assert_eq!(sessions(&m)[0].actions, 1, "a replay spends nothing");

    // fill the tree to its live-call bound; the next call is refused by name.
    for index in 2..=MAX_DELEGATIONS_PER_RUN {
        let mut ctx = session_ctx(
            &registry,
            &caller_run,
            Origin::External(SESSION_KEY.to_vec()),
        );
        exec(
            &mut m,
            &mut ctx,
            &delegate(&caller_run, &format!("live-{index}"), "worker", "work"),
        )
        .unwrap();
        commit(&mut m);
    }
    let mut full = session_ctx(
        &registry,
        &caller_run,
        Origin::External(SESSION_KEY.to_vec()),
    );
    let err = exec(
        &mut m,
        &mut full,
        &delegate(&caller_run, "two", "reviewer", "review"),
    )
    .unwrap_err();
    assert!(
        matches!(err, Error::Module(ref reason) if reason.contains("concurrency limit")),
        "{err:?}"
    );

    // a completed call releases its slot: more than the live bound may be
    // admitted sequentially, and the completed result history must still
    // round-trip while the root is live.
    for index in 1..=MAX_DELEGATIONS_PER_RUN {
        let callee_run = delegations(&m, &caller_run)
            .into_iter()
            .find(|call| call.status == DelegationStatus::Pending)
            .unwrap()
            .callee_run_id;
        let mut result_ctx = CaptureCtx::new()
            .at(8)
            .with_dispatch_origin()
            .with_registry(&registry)
            .with_transcript("general", transcript(2));
        exec(
            &mut m,
            &mut result_ctx,
            &result_event(
                &callee_run,
                Ok(runner_wrapper("done", serde_json::json!({}))),
            ),
        )
        .unwrap();
        commit(&mut m);

        let mut next = session_ctx(
            &registry,
            &caller_run,
            Origin::External(SESSION_KEY.to_vec()),
        );
        exec(
            &mut m,
            &mut next,
            &delegate(&caller_run, &format!("next-{index}"), "worker", "work"),
        )
        .unwrap();
        assert_eq!(next.dispatch_msgs().len(), 1);
        commit(&mut m);
    }
    // every admitted call spent one session action, and so did the refused
    // one: its action was staged before the execution met the bound.
    assert_eq!(
        sessions(&m)[0].actions,
        (2 * MAX_DELEGATIONS_PER_RUN + 1) as u32
    );
    let mut joiner = module();
    joiner.install(&m.snapshot(), m.root()).unwrap();
}

#[test]
fn a_caller_exit_cancels_and_prunes_its_recursive_subtree() {
    let (mut m, registry, root_run) = with_open_delegating_session();
    let mut root_ctx = session_ctx(&registry, &root_run, Origin::External(SESSION_KEY.to_vec()));
    exec(
        &mut m,
        &mut root_ctx,
        &delegate(&root_run, "worker-call", "worker", "work"),
    )
    .unwrap();
    commit(&mut m);
    let worker_run = delegations(&m, &root_run)[0].callee_run_id.clone();

    let mut open_ctx = session_ctx(&registry, &worker_run, Origin::External(ASSIGNEE.to_vec()));
    exec(
        &mut m,
        &mut open_ctx,
        &open(&worker_run, &CHILD_SESSION_KEY),
    )
    .unwrap();
    commit(&mut m);

    let mut worker_ctx = session_ctx(
        &registry,
        &worker_run,
        Origin::External(CHILD_SESSION_KEY.to_vec()),
    );
    exec(
        &mut m,
        &mut worker_ctx,
        &delegate(&worker_run, "review-call", "reviewer", "review"),
    )
    .unwrap();
    commit(&mut m);
    let reviewer_run = delegations(&m, &worker_run)[0].callee_run_id.clone();
    assert!(get_pending(&m, &reviewer_run).is_some());

    let mut settle = CaptureCtx::new()
        .at(9)
        .with_dispatch_origin()
        .with_registry(&registry)
        .with_transcript("general", transcript(2));
    exec(
        &mut m,
        &mut settle,
        &result_event(
            &worker_run,
            Ok(runner_wrapper("worker done", serde_json::json!({}))),
        ),
    )
    .unwrap();
    assert!(matches!(
        settle.dispatch_msgs().as_slice(),
        [DispatchMsg::CancelDispatch { dispatch_id }]
            if dispatch_id == &dispatch_id_for(&reviewer_run)
    ));
    assert!(
        get_pending(&m, &reviewer_run).is_none(),
        "the cancelled descendant is removed immediately"
    );
    assert_eq!(
        delegations(&m, &worker_run)[0].status,
        DelegationStatus::Cancelled
    );
    assert_eq!(
        delegations(&m, &root_run)[0].status,
        DelegationStatus::Delivered
    );
    commit(&mut m);

    let mut joiner = module();
    joiner.install(&m.snapshot(), m.root()).unwrap();
    assert_eq!(joiner.root(), m.root());
}

#[test]
fn the_lease_holder_binds_a_session_and_a_stranger_cannot() {
    let (mut m, registry, run_id) = awaiting_session_run();

    // THE CORE AUTHORIZATION TEST: a node that does not hold the run's lease
    // may not open its session — not the owner, not another validator, nobody.
    for (origin, what) in [
        (user(9), "the agent's owner"),
        (Origin::External(vec![0xff; 32]), "another node"),
        // a module id the origin router does not claim, so the op really
        // reaches this arm (a collaborator's id would route to its intake).
        (Origin::Module("pages".into()), "a module"),
        (Origin::System, "the system origin"),
    ] {
        let mut ctx = session_ctx(&registry, &run_id, origin);
        let err = exec(&mut m, &mut ctx, &open(&run_id, &SESSION_KEY)).unwrap_err();
        assert!(
            matches!(&err, Error::Module(reason) if reason.contains("lease") || reason.contains("executing a run")),
            "{what} must not open a session: {err:?}"
        );
        assert!(sessions(&m).is_empty(), "{what} staged nothing");
        abort(&mut m);
    }

    // the assignee — the node the dispatch plane really handed the work to.
    let mut ctx = session_ctx(&registry, &run_id, Origin::External(ASSIGNEE.to_vec()));
    exec(&mut m, &mut ctx, &open(&run_id, &SESSION_KEY)).unwrap();
    commit(&mut m);

    let bound = sessions(&m);
    assert_eq!(bound.len(), 1);
    assert_eq!(bound[0].run_id, run_id);
    assert_eq!(bound[0].session_key, SESSION_KEY.to_vec());
    assert_eq!(
        bound[0].agent_id, "bot",
        "the agent id comes from the run's committed entry, never the payload"
    );
    assert_eq!(bound[0].opened_at, 5, "the consensus counter of the block");
    assert_eq!(bound[0].actions, 0, "a fresh session has spent nothing");
}

/// a session ctx like [`session_ctx`] but WITHOUT a committed lease wired — the
/// caller adds the dispatch/saga answer the lease lookup should resolve.
fn leaseless_ctx(registry: &Registry, origin: Origin) -> CaptureCtx {
    CaptureCtx::new()
        .at(5)
        .with_origin(origin)
        .with_registry(registry)
        .with_transcript("general", transcript(2))
        .with_page("p1", page_blocks("p1", "Spec"))
}

#[test]
fn opening_is_refused_when_the_run_has_no_dispatch_record() {
    let (mut m, registry, run_id) = awaiting_session_run();
    // the run is in flight in runs' own state, but dispatch holds no record —
    // the lease lookup has nothing to resolve.
    let mut ctx = leaseless_ctx(&registry, Origin::External(ASSIGNEE.to_vec()));
    let err = exec(&mut m, &mut ctx, &open(&run_id, &SESSION_KEY)).unwrap_err();
    assert!(
        matches!(&err, Error::Module(reason) if reason.contains("dispatch record")),
        "a run with no dispatch record cannot open a session: {err:?}"
    );
    assert!(sessions(&m).is_empty(), "a refused open stages nothing");
}

#[test]
fn opening_is_refused_when_the_dispatch_is_already_delivered() {
    let (mut m, registry, run_id) = awaiting_session_run();
    // a terminal (Delivered) dispatch runs nowhere — no live lease to hold.
    let mut ctx = leaseless_ctx(&registry, Origin::External(ASSIGNEE.to_vec()))
        .with_taken_dispatch(&dispatch_id_for(&run_id));
    let err = exec(&mut m, &mut ctx, &open(&run_id, &SESSION_KEY)).unwrap_err();
    assert!(
        matches!(&err, Error::Module(reason) if reason.contains("lease")),
        "a delivered run holds no execution lease: {err:?}"
    );
    assert!(sessions(&m).is_empty(), "a refused open stages nothing");
}

#[test]
fn opening_is_refused_when_the_saga_holds_no_lease() {
    let (mut m, registry, run_id) = awaiting_session_run();
    // the dispatch still awaits its saga, but the saga carries no committed
    // lease (its assignee is `None`) — nobody is executing this run.
    let mut ctx = leaseless_ctx(&registry, Origin::External(ASSIGNEE.to_vec()))
        .with_awaiting_but_no_lease(&run_id);
    let err = exec(&mut m, &mut ctx, &open(&run_id, &SESSION_KEY)).unwrap_err();
    assert!(
        matches!(&err, Error::Module(reason) if reason.contains("lease")),
        "an unassigned saga holds no execution lease: {err:?}"
    );
    assert!(sessions(&m).is_empty(), "a refused open stages nothing");
}

#[test]
fn a_session_key_of_the_wrong_length_is_refused() {
    let (mut m, registry, run_id) = awaiting_session_run();
    for key in [vec![], vec![7u8; 31], vec![7u8; 33]] {
        let mut ctx = session_ctx(&registry, &run_id, Origin::External(ASSIGNEE.to_vec()));
        let err = exec(&mut m, &mut ctx, &open(&run_id, &key)).unwrap_err();
        assert!(
            matches!(&err, Error::Module(reason) if reason.contains("32 bytes")),
            "a {}-byte key must be refused: {err:?}",
            key.len()
        );
        assert!(sessions(&m).is_empty());
        abort(&mut m);
    }
}

#[test]
fn opening_on_a_settled_or_unknown_run_is_refused() {
    let (mut m, registry, run_id) = awaiting_session_run();
    // the run settles: its entry prunes, so there is nothing left to bind to.
    let mut ctx = CaptureCtx::new()
        .at(6)
        .with_dispatch_origin()
        .with_registry(&registry)
        .with_transcript("general", transcript(2));
    exec(&mut m, &mut ctx, &result_event(&run_id, Err("boom".into()))).unwrap();
    commit(&mut m);

    for (target, what) in [
        (run_id.as_str(), "a settled run"),
        ("chat\x1fgeneral\x1f9\x1fghost", "an unknown run"),
    ] {
        let mut ctx = session_ctx(&registry, target, Origin::External(ASSIGNEE.to_vec()));
        let err = exec(&mut m, &mut ctx, &open(target, &SESSION_KEY)).unwrap_err();
        assert!(
            matches!(&err, Error::Module(reason) if reason.contains("not in flight")),
            "{what} must be refused: {err:?}"
        );
        assert!(sessions(&m).is_empty());
        abort(&mut m);
    }
}

#[test]
fn a_second_open_cannot_replace_a_live_session() {
    // a squatted re-open would revoke the key the agent is CURRENTLY acting
    // under and inherit its remaining budget. first binding wins.
    let (mut m, registry, run_id) = with_open_session();
    let mut ctx = session_ctx(&registry, &run_id, Origin::External(ASSIGNEE.to_vec()));
    let err = exec(&mut m, &mut ctx, &open(&run_id, &[0xee; 32])).unwrap_err();
    assert!(
        matches!(&err, Error::Module(reason) if reason.contains("already has an open agent session")),
        "{err:?}"
    );
    assert_eq!(
        sessions(&m)[0].session_key,
        SESSION_KEY.to_vec(),
        "the live key stands"
    );
}

// ---- acting: the bound key IS the ACL ----------------------------------------

#[test]
fn only_the_bound_session_key_may_act() {
    // THE CORE ACL TEST. a frame's origin is its VERIFIED public key, so this
    // comparison is authorship consensus can trust — and nobody else's key
    // passes it, not the owner's, not even the assignee's own node key.
    let (mut m, registry, run_id) = with_open_session();
    for (origin, what) in [
        (Origin::External(ASSIGNEE.to_vec()), "the executing node"),
        (user(9), "the agent's owner"),
        (Origin::External(vec![0xee; 32]), "an unrelated key"),
        (Origin::Module("pages".into()), "a module"),
        (Origin::System, "the system origin"),
    ] {
        let mut ctx = session_ctx(&registry, &run_id, origin);
        let err = exec(&mut m, &mut ctx, &act(&run_id, comment("b-p"))).unwrap_err();
        assert!(
            matches!(&err, Error::Module(reason) if reason.contains("session key")),
            "{what} must not act through the session: {err:?}"
        );
        assert!(ctx.page_msgs().is_empty(), "{what} emitted nothing");
        assert_eq!(sessions(&m)[0].actions, 0, "{what} spent no budget");
        abort(&mut m);
    }

    // the bound key acts.
    let mut ctx = session_ctx(&registry, &run_id, Origin::External(SESSION_KEY.to_vec()));
    exec(&mut m, &mut ctx, &act(&run_id, comment("b-p"))).unwrap();
    commit(&mut m);
    assert_eq!(ctx.page_msgs().len(), 1);
    assert_eq!(
        sessions(&m)[0].actions,
        1,
        "the applied action spent budget"
    );
}

#[test]
fn an_action_prepares_one_program_comment_proposal() {
    // This validator probe exposes the prepared payload; the real-host
    // programmable_model tests prove the Program call and canonical author.
    let (mut m, registry, run_id) = with_open_session();
    let mut ctx = session_ctx(&registry, &run_id, Origin::External(SESSION_KEY.to_vec()));
    exec(&mut m, &mut ctx, &act(&run_id, comment("b-p"))).unwrap();

    let msgs = ctx.page_msgs();
    assert_eq!(msgs.len(), 1, "exactly one pages follow-up");
    let PageMsg::AddComment {
        thread_id,
        comment_id,
        target,
        text,
        ..
    } = &msgs[0]
    else {
        panic!("expected AddComment, got {:?}", msgs[0]);
    };

    assert_eq!(target, "b-p");
    assert_eq!(text, "looks good");
    // the session lane's id space is DISJOINT from the settle path's: the
    // session numbers by its committed action counter under an `s` prefix, so a
    // mid-run comment can never squat the id the final response's nth action
    // mints (which would silently degrade that action on delivery).
    let rid = dispatch_id_for(&run_id);
    assert_eq!(*thread_id, format!("agent/{rid}/thread/s0"));
    assert_eq!(*comment_id, format!("agent/{rid}/comment/s0"));
    assert!(pages::id_is_index_safe(thread_id) && pages::id_is_index_safe(comment_id));
}

#[test]
fn minted_ids_are_deterministic_in_the_committed_action_counter() {
    // the same (run_id, counter) mints byte-identical ids on every replaying
    // validator: no host randomness, no wall clock — the counter is committed
    // state, and it is the ONLY thing that advances the id.
    let ids = |m: &mut RunsModule, registry: &Registry, run_id: &str| {
        let mut ctx = session_ctx(registry, run_id, Origin::External(SESSION_KEY.to_vec()));
        exec(m, &mut ctx, &act(run_id, comment("b-p"))).unwrap();
        commit(m);
        match ctx.page_msgs().remove(0) {
            PageMsg::AddComment {
                thread_id,
                comment_id,
                ..
            } => (thread_id, comment_id),
            other => panic!("expected AddComment, got {other:?}"),
        }
    };
    let replay = || {
        let (mut m, registry, run_id) = with_open_session();
        let first = ids(&mut m, &registry, &run_id);
        let second = ids(&mut m, &registry, &run_id);
        (first, second)
    };
    let (left, right) = (replay(), replay());
    assert_eq!(left, right, "same run, same counters => identical ids");
    let ((thread0, _), (thread1, _)) = &left;
    assert_ne!(thread0, thread1, "the counter advances the id");
    assert!(thread1.ends_with("/s1"), "the second action is slot s1");
}

#[test]
fn the_action_budget_bounds_a_session() {
    let (mut m, registry, run_id) = with_open_session();
    for i in 0..MAX_ACTIONS_PER_SESSION {
        let mut ctx = session_ctx(&registry, &run_id, Origin::External(SESSION_KEY.to_vec()));
        exec(&mut m, &mut ctx, &act(&run_id, comment("b-p"))).unwrap();
        commit(&mut m);
        assert_eq!(sessions(&m)[0].actions, i + 1);
    }
    let mut ctx = session_ctx(&registry, &run_id, Origin::External(SESSION_KEY.to_vec()));
    let err = exec(&mut m, &mut ctx, &act(&run_id, comment("b-p"))).unwrap_err();
    assert!(
        matches!(&err, Error::Module(reason) if reason.contains("spent its budget")),
        "{err:?}"
    );
    assert!(ctx.page_msgs().is_empty());
    assert_eq!(sessions(&m)[0].actions, MAX_ACTIONS_PER_SESSION);
}

// ---- chat.post_message: speaking into any channel ----------------------------

#[test]
fn post_message_prepares_a_post_for_the_program_to_execute() {
    let post = post_message("general", "still working on it", None);
    let (mut m, registry, run_id) = with_open_session();
    let mut ctx = session_ctx(&registry, &run_id, Origin::External(SESSION_KEY.to_vec()));
    exec(&mut m, &mut ctx, &act(&run_id, post)).unwrap();
    commit(&mut m);

    let msgs = ctx.chat_msgs();
    assert_eq!(msgs.len(), 1);
    let ChatMsg::PostMessage {
        channel_id,
        message_id,
        blocks,
        thread,
    } = &msgs[0]
    else {
        panic!("expected PostMessage, got {:?}", msgs[0]);
    };
    assert_eq!(channel_id, "general");

    assert_eq!(*thread, None);
    assert_eq!(blocks, &vec![Block::paragraph("still working on it")]);
    assert_eq!(
        *message_id,
        post_message_id(&run_id, "s0"),
        "deterministic, and never the run's ONE reply id"
    );
    assert_ne!(*message_id, reply_message_id(&run_id));
    assert_eq!(sessions(&m)[0].actions, 1);
}

#[test]
fn a_submit_carries_any_module_message_verbatim_to_the_module_it_names() {
    // the floor under the typed catalog: whatever a member may submit to a
    // module, the run may. the bytes reach the module untouched, under the
    // program account, and only that module judges them — runs probes
    // nothing and refuses nothing here.
    let message =
        serde_json::json!({"create_task": {"task_id": "t-submit", "title": "via submit"}});
    let submit = envelope(
        crate::OP_SUBMIT,
        Some(serde_json::json!({"module": "tasks"})),
        message.clone(),
    );
    let (mut m, registry, run_id) = with_open_session();
    let mut ctx = session_ctx(&registry, &run_id, Origin::External(SESSION_KEY.to_vec()));
    exec(&mut m, &mut ctx, &act_as(&run_id, "via-submit", submit)).unwrap();
    commit(&mut m);

    let to_tasks: Vec<&Msg> = ctx.msgs.iter().filter(|m| m.target == "tasks").collect();
    assert_eq!(to_tasks.len(), 1, "{:?}", ctx.msgs);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&to_tasks[0].payload).unwrap(),
        message,
        "the module message is carried verbatim"
    );
    assert_eq!(sessions(&m)[0].actions, 1);
    // the receipt names the module and the message, and nothing else: the
    // module's own verdict is what the run reads back from that module.
    let receipt = block_on(m.action_request(&crate::action_request_id(&run_id, "via-submit")))
        .unwrap()
        .expect("receipt");
    assert_eq!(receipt.view.operation, crate::OP_SUBMIT);
    assert_eq!(
        receipt.view.result,
        serde_json::json!({"module": "tasks", "message": "create_task"})
    );

    // a bare string is a message with no fields (a unit variant on the wire),
    // and the module named need not be one runs is wired to at all.
    let (mut m, registry, run_id) = with_open_session();
    let mut ctx = session_ctx(&registry, &run_id, Origin::External(SESSION_KEY.to_vec()));
    let bare = envelope(
        crate::OP_SUBMIT,
        Some(serde_json::json!({"module": "inventory"})),
        serde_json::json!("rebalance"),
    );
    exec(&mut m, &mut ctx, &act(&run_id, bare)).unwrap();
    let to_inventory: Vec<&Msg> = ctx
        .msgs
        .iter()
        .filter(|m| m.target == "inventory")
        .collect();
    assert_eq!(to_inventory.len(), 1, "{:?}", ctx.msgs);
    assert_eq!(to_inventory[0].payload, br#""rebalance""#);
}

#[test]
fn post_message_probes_everything_chat_would_reject() {
    // the no-fail rule binds the EMISSION: an unknown channel, a squatted id, or
    // a ghost thread root would each make chat reject the follow-up. each is
    // caught here, before the op exists.
    let post = |channel: &str, text: &str, thread: Option<u64>| post_message(channel, text, thread);
    for (action, needle) in [
        (post("ghost", "hi", None), "unknown channel"),
        (
            post("general", "hi", Some(99)),
            "thread root does not exist",
        ),
        (post("general", "  ", None), "non-empty text"),
    ] {
        let (mut m, registry, run_id) = with_open_session();
        let mut ctx = session_ctx(&registry, &run_id, Origin::External(SESSION_KEY.to_vec()));
        let err = exec(&mut m, &mut ctx, &act(&run_id, action)).unwrap_err();
        assert!(
            matches!(&err, Error::Module(reason) if reason.contains(needle)),
            "expected {needle:?}, got {err:?}"
        );
        assert!(ctx.chat_msgs().is_empty(), "nothing is emitted");
        assert_eq!(sessions(&m)[0].actions, 0, "a refusal spends no budget");
    }

    // a squatted message id: ids are client-chosen, so anyone could take the
    // one this action mints. chat would reject the duplicate — caught here.
    let (mut m, registry, run_id) = with_open_session();
    let squatted = post_message_id(&run_id, "s0");
    let mut ctx = session_ctx(&registry, &run_id, Origin::External(SESSION_KEY.to_vec()))
        .with_transcript(
            "general",
            vec![MessageView {
                head: MessageHead {
                    message_id: squatted,
                    ..message(1, "squat").head
                },
                ..message(1, "squat")
            }],
        );
    let err = exec(&mut m, &mut ctx, &act(&run_id, post("general", "hi", None))).unwrap_err();
    assert!(
        matches!(&err, Error::Module(reason) if reason.contains("already taken")),
        "{err:?}"
    );
    assert!(ctx.chat_msgs().is_empty());
}

// ---- the close-out: a session never outlives its run -------------------------

#[test]
fn the_session_prunes_on_every_settle_path() {
    // delivery, worker failure, and cancellation ALL arrive as the one
    // ResultEvent (a cancel routes through the dispatch plane, whose
    // Err("cancelled") delivery lands in the same intake), so this is every
    // path by which a run's entry prunes — and the session goes with it, in the
    // same block. an agent's key stops being an authority the moment its run
    // stops existing.
    for (outcome, what) in [
        (Ok(response(&["done"], vec![])), "delivery"),
        (Err("worker exploded".to_string()), "failure"),
        (Err("cancelled".to_string()), "cancellation"),
    ] {
        let (mut m, registry, run_id) = with_open_session();
        assert_eq!(sessions(&m).len(), 1, "{what}: a session is open");

        let mut ctx = CaptureCtx::new()
            .at(9)
            .with_dispatch_origin()
            .with_registry(&registry)
            .with_transcript("general", transcript(2));
        exec(&mut m, &mut ctx, &result_event(&run_id, outcome)).unwrap();
        commit(&mut m);

        assert!(sessions(&m).is_empty(), "{what}: the session is pruned");
        assert_eq!(get_pending(&m, &run_id), None, "{what}: the run is gone");

        // and the key is dead: a further action is refused.
        let mut ctx = session_ctx(&registry, &run_id, Origin::External(SESSION_KEY.to_vec()));
        let err = exec(&mut m, &mut ctx, &act(&run_id, comment("b-p"))).unwrap_err();
        assert!(
            matches!(&err, Error::Module(reason) if reason.contains("run is not in flight")),
            "{what}: a settled run's key must not act: {err:?}"
        );
    }
}

#[test]
fn an_aborted_block_binds_no_session() {
    let (mut m, registry, run_id) = awaiting_session_run();
    let mut ctx = session_ctx(&registry, &run_id, Origin::External(ASSIGNEE.to_vec()));
    exec(&mut m, &mut ctx, &open(&run_id, &SESSION_KEY)).unwrap();
    assert_eq!(sessions(&m).len(), 1, "staged: read-your-writes");
    abort(&mut m);
    assert!(
        sessions(&m).is_empty(),
        "an aborted block leaves no session"
    );
}

// ---- committed state: the root-hash and the snapshot ---------------------------

#[test]
fn a_session_moves_the_root_and_round_trips_through_a_snapshot() {
    // the session registry IS the mid-run ACL, so it is committed state: every
    // validator must hold the same one, and a joiner must receive it.
    let (mut m, registry, run_id) = awaiting_session_run();
    let before = m.root();

    let mut ctx = session_ctx(&registry, &run_id, Origin::External(ASSIGNEE.to_vec()));
    exec(&mut m, &mut ctx, &open(&run_id, &SESSION_KEY)).unwrap();
    commit(&mut m);
    let opened = m.root();
    assert_ne!(opened, before, "opening a session moves the root-hash");

    // spending an action moves it again — the counter is the id salt, so a
    // validator that replayed a different count would mint different ids.
    let mut ctx = session_ctx(&registry, &run_id, Origin::External(SESSION_KEY.to_vec()));
    exec(&mut m, &mut ctx, &act(&run_id, comment("b-p"))).unwrap();
    commit(&mut m);
    let spent = m.root();
    assert_ne!(spent, opened, "spending an action moves the root-hash");

    // the snapshot carries the session, and the joiner re-derives the root from
    // the decoded bytes — the consensus-agreed root, never the peer, is the
    // trust anchor.
    let mut joiner = module().with_pages_module("pages");
    joiner.install(&m.snapshot(), spent).unwrap();
    assert_eq!(joiner.root(), spent, "installed root equals the source");
    assert_eq!(sessions(&joiner), sessions(&m), "the session round-trips");
    let session = &sessions(&joiner)[0];
    assert_eq!(session.session_key, SESSION_KEY.to_vec());
    assert_eq!(session.actions, 1);

    // and the joiner honours it: the bound key still acts, a stranger still
    // cannot.
    let mut ctx = session_ctx(&registry, &run_id, Origin::External(SESSION_KEY.to_vec()));
    exec(&mut joiner, &mut ctx, &act(&run_id, comment("b-p"))).unwrap();
    assert_eq!(ctx.page_msgs().len(), 1, "the installed session is live");
}

#[test]
fn a_forged_snapshot_session_is_rejected_by_the_decoder() {
    // a session may never outlive its run, and its key is a fixed-width ed25519
    // key — so a snapshot violating either is not one any honest node could have
    // produced. the decoder refuses it before the root check even runs.
    let (m, ..) = with_open_session();

    // an orphaned session: the same session, but the pending section is empty.
    let orphaned = crate::state::encode_committed(
        &m.receipts.snapshot(),
        m.next_action_item,
        &BTreeMap::new(),
        &m.sessions,
        &m.delegations,
        &m.models,
    );
    let err = module().install(&orphaned, StateRoot::ZERO).unwrap_err();
    assert!(
        matches!(&err, Error::Module(reason) if reason.contains("names no in-flight run")),
        "{err:?}"
    );

    // a short key: the ACL would compare against something that is not a key.
    let stunted = m
        .sessions
        .iter()
        .map(|(run_id, s)| {
            let s = AgentSession {
                session_key: vec![1, 2, 3],
                ..s.clone()
            };
            (run_id.clone(), s)
        })
        .collect();
    let forged = crate::state::encode_committed(
        &m.receipts.snapshot(),
        m.next_action_item,
        &m.pending,
        &stunted,
        &m.delegations,
        &m.models,
    );
    let err = module().install(&forged, StateRoot::ZERO).unwrap_err();
    assert!(
        matches!(&err, Error::Module(reason) if reason.contains("32-byte ed25519 key")),
        "{err:?}"
    );
}

/// the node the lease MOVED to (a `ReassignRun`, or a saga expiry re-leasing
/// the run on its own).
const NEW_ASSIGNEE: [u8; 32] = [0x11; 32];

#[test]
fn a_moved_lease_strands_the_old_session_and_lets_the_new_holder_open_one() {
    const NEW_SESSION_KEY: [u8; 32] = [0x22; 32];
    let (mut m, registry, run_id) = with_open_session();
    let reassigned = |origin: Origin| {
        CaptureCtx::new()
            .at(6)
            .with_origin(origin)
            .with_registry(&registry)
            .with_transcript("general", transcript(2))
            .with_lease_holder(&run_id, &NEW_ASSIGNEE)
    };
    let post = post_message("general", "still here", None);

    // the ex-holder's key is still bound, and still refused: its authority was
    // the lease, and the lease left.
    let mut ctx = reassigned(Origin::External(SESSION_KEY.to_vec()));
    let err = exec(&mut m, &mut ctx, &act(&run_id, post.clone())).unwrap_err();
    assert!(
        matches!(&err, Error::Module(reason) if reason.contains("lease has moved")),
        "{err:?}"
    );
    assert!(ctx.chat_msgs().is_empty(), "{:?}", ctx.chat_msgs());
    assert_eq!(sessions(&m)[0].session_key, SESSION_KEY.to_vec());

    // the node actually executing the run now opens its own session,
    // REPLACING the stranded one.
    let mut ctx = reassigned(Origin::External(NEW_ASSIGNEE.to_vec()));
    exec(&mut m, &mut ctx, &open(&run_id, &NEW_SESSION_KEY)).unwrap();
    commit(&mut m);
    let live = sessions(&m);
    assert_eq!(live.len(), 1);
    assert_eq!(live[0].session_key, NEW_SESSION_KEY.to_vec());
    assert_eq!(live[0].lease.holder, NEW_ASSIGNEE.to_vec());

    // The same binding may be delivered twice without resetting its counters.
    let mut ctx = reassigned(Origin::External(NEW_ASSIGNEE.to_vec()));
    exec(&mut m, &mut ctx, &open(&run_id, &NEW_SESSION_KEY)).unwrap();
    assert_eq!(sessions(&m), live);
}

#[test]
fn a_moved_lease_stops_the_old_session_from_delegating() {
    let (mut m, registry, run_id) = with_open_delegating_session();
    let mut ctx = session_ctx(&registry, &run_id, Origin::External(SESSION_KEY.to_vec()))
        .with_lease_holder(&run_id, &NEW_ASSIGNEE);
    let err = exec(
        &mut m,
        &mut ctx,
        &delegate(&run_id, "parser", "worker", "Implement the parser."),
    )
    .unwrap_err();
    assert!(
        matches!(&err, Error::Module(reason) if reason.contains("lease has moved")),
        "{err:?}"
    );
    assert!(ctx.dispatch_msgs().is_empty());
    assert!(delegations(&m, &run_id).is_empty());
}

#[test]
fn live_replies_resolve_the_original_thread_without_naming_it() {
    for (parent, expected_root) in [(None, 3), (Some(1), 1)] {
        let registry = registry(&["bot"]);
        let mut m = configured(&registry);
        let mut messages = transcript(2);
        messages.push(message_in(
            "general",
            3,
            Party::Key(vec![1; 32]),
            "work here",
            parent,
        ));
        let mut ctx = CaptureCtx::new()
            .at(3)
            .with_program_origin()
            .with_registry(&registry)
            .with_transcript("general", messages.clone());
        exec(&mut m, &mut ctx, &engagement("general", 3, vec![])).unwrap();
        commit(&mut m);
        let run = run_id_for("general", 3, "bot");
        let mut ctx = session_ctx(&registry, &run, Origin::External(ASSIGNEE.to_vec()));
        exec(&mut m, &mut ctx, &open(&run, &SESSION_KEY)).unwrap();
        commit(&mut m);
        let mut ctx = session_ctx(&registry, &run, Origin::External(SESSION_KEY.to_vec()))
            .with_transcript("general", messages);
        exec(&mut m, &mut ctx, &act(&run, reply("Working on it"))).unwrap();
        assert_eq!(
            ctx.chat_msgs(),
            vec![ChatMsg::PostMessage {
                channel_id: "general".into(),
                message_id: post_message_id(&run, "s0"),
                thread: Some(expected_root),
                blocks: vec![Block::paragraph("Working on it")],
            }]
        );
    }
}

/// THE ACKNOWLEDGEMENT: a live reaction lands on the anchor message without
/// naming it, its removal is the mirror op, and the receipt names the
/// message it marked.
#[test]
fn live_reactions_mark_the_anchor_message_without_naming_it() {
    let (mut m, registry, run) = with_open_session();
    let mut ctx = session_ctx(&registry, &run, Origin::External(SESSION_KEY.to_vec()));
    exec(&mut m, &mut ctx, &act_as(&run, "ack", react("👀"))).unwrap();
    assert_eq!(
        ctx.chat_msgs(),
        vec![ChatMsg::AddReaction {
            channel_id: "general".into(),
            seq: 2,
            emoji: "👀".into(),
        }]
    );
    commit(&mut m);
    let mut ctx = session_ctx(&registry, &run, Origin::External(SESSION_KEY.to_vec()));
    exec(&mut m, &mut ctx, &act_as(&run, "done", unreact("👀"))).unwrap();
    assert_eq!(
        ctx.chat_msgs(),
        vec![ChatMsg::RemoveReaction {
            channel_id: "general".into(),
            seq: 2,
            emoji: "👀".into(),
        }]
    );
    commit(&mut m);
    let receipt = block_on(m.action_request(&crate::action_request_id(&run, "ack")))
        .unwrap()
        .expect("receipt");
    assert_eq!(receipt.view.operation, crate::OP_REACT);
    assert_eq!(
        receipt.view.result,
        serde_json::json!({"channel_id": "general", "seq": 2, "emoji": "👀"})
    );
    assert_eq!(sessions(&m)[0].actions, 2);
}

#[test]
fn a_reaction_needs_a_chat_source_and_a_bounded_emoji() {
    for (emoji, expected) in [
        ("", "requires an emoji"),
        ("🦆🦆🦆🦆🦆🦆🦆🦆🦆🦆🦆🦆🦆🦆🦆🦆🦆", "chat's cap"),
    ] {
        let (mut m, registry, run) = with_open_session();
        let mut ctx = session_ctx(&registry, &run, Origin::External(SESSION_KEY.to_vec()));
        let error = exec(&mut m, &mut ctx, &act(&run, react(emoji))).unwrap_err();
        assert!(
            matches!(error, Error::Module(ref reason) if reason.contains(expected)),
            "{error:?}"
        );
        assert_eq!(sessions(&m)[0].actions, 0);
    }
    {
        let (mut m, registry, run) = with_open_session();
        let entry = m.pending.get_mut(&dispatch_id_for(&run)).unwrap();
        entry.channel_id.clear();
        entry.anchor_seq = 0;
        let mut ctx = session_ctx(&registry, &run, Origin::External(SESSION_KEY.to_vec()));
        let error = exec(&mut m, &mut ctx, &act(&run, react("👀"))).unwrap_err();
        assert!(
            matches!(error, Error::Module(ref reason) if reason.contains("no reply destination")),
            "{error:?}"
        );
    }
}

#[test]
fn live_reply_requires_a_nonempty_chat_response() {
    {
        let (mut m, registry, run) = with_open_session();
        let mut ctx = session_ctx(&registry, &run, Origin::External(SESSION_KEY.to_vec()));
        let error = exec(&mut m, &mut ctx, &act(&run, reply("  "))).unwrap_err();
        assert!(
            matches!(error, Error::Module(ref reason) if reason.contains("non-empty text")),
            "{error:?}"
        );
        assert_eq!(sessions(&m)[0].actions, 0);
    }
    {
        let (mut m, registry, run) = with_open_session();
        let entry = m.pending.get_mut(&dispatch_id_for(&run)).unwrap();
        entry.channel_id.clear();
        entry.anchor_seq = 0;
        let mut ctx = session_ctx(&registry, &run, Origin::External(SESSION_KEY.to_vec()));
        let error = exec(&mut m, &mut ctx, &act(&run, reply("hello"))).unwrap_err();
        assert!(
            matches!(error, Error::Module(ref reason) if reason.contains("no reply destination")),
            "{error:?}"
        );
    }
}

#[test]
fn same_node_retries_rotate_keys_and_preserve_the_run_budget_and_snapshot() {
    let (mut m, registry, run) = with_open_session();
    let mut ctx = session_ctx(&registry, &run, Origin::External(SESSION_KEY.to_vec()));
    exec(&mut m, &mut ctx, &act(&run, comment("b-p"))).unwrap();
    commit(&mut m);
    let retried =
        |origin| session_ctx(&registry, &run, origin).with_lease_attempt(&run, &ASSIGNEE, 1);
    let mut old = retried(Origin::External(SESSION_KEY.to_vec()));
    assert!(exec(&mut m, &mut old, &act(&run, comment("b-p"))).is_err());
    let mut holder = retried(Origin::External(ASSIGNEE.to_vec()));
    assert!(
        exec(&mut m, &mut holder, &open(&run, &CHILD_SESSION_KEY)).is_err(),
        "a stale attempt cannot bind under a fresh lease"
    );
    exec(
        &mut m,
        &mut holder,
        &open_attempt(&run, 1, &CHILD_SESSION_KEY),
    )
    .unwrap();
    commit(&mut m);
    assert_eq!(sessions(&m)[0].actions, 1);
    assert_eq!(sessions(&m)[0].lease.attempt, 1);
    assert!(exec(&mut m, &mut old, &act(&run, comment("b-p"))).is_err());
    let root = m.root();
    let mut joiner = module().with_pages_module("pages");
    joiner.install(&m.snapshot(), root).unwrap();
    assert_eq!(sessions(&joiner), sessions(&m));
    for _ in 1..MAX_ACTIONS_PER_SESSION {
        let mut ctx = retried(Origin::External(CHILD_SESSION_KEY.to_vec()));
        exec(&mut joiner, &mut ctx, &act(&run, comment("b-p"))).unwrap();
        commit(&mut joiner);
    }
    exec(
        &mut joiner,
        &mut holder,
        &open_attempt(&run, 1, &CHILD_SESSION_KEY),
    )
    .unwrap();
    let mut ctx = retried(Origin::External(CHILD_SESSION_KEY.to_vec()));
    let error = exec(&mut joiner, &mut ctx, &act(&run, comment("b-p"))).unwrap_err();
    assert!(
        matches!(error, Error::Module(ref reason) if reason.contains("spent its budget")),
        "{error:?}"
    );
    assert_eq!(sessions(&joiner)[0].actions, MAX_ACTIONS_PER_SESSION);
}

#[test]
fn returning_to_a_previous_holder_does_not_revive_its_old_key() {
    let (mut m, registry, run) = with_open_session();
    for (holder, attempt, key) in [
        (NEW_ASSIGNEE, 1, CHILD_SESSION_KEY),
        (ASSIGNEE, 2, [0x55; 32]),
    ] {
        let mut ctx = session_ctx(&registry, &run, Origin::External(holder.to_vec()))
            .with_lease_attempt(&run, &holder, attempt);
        exec(&mut m, &mut ctx, &open_attempt(&run, attempt, &key)).unwrap();
        commit(&mut m);
        ctx.env.origin = Origin::External(SESSION_KEY.to_vec());
        assert!(exec(&mut m, &mut ctx, &act(&run, reply("stale"))).is_err());
    }
}

#[test]
fn terminal_saga_fences_the_key_before_dispatch_records_completion() {
    let (mut m, registry, run) = with_open_session();
    let mut ctx = session_ctx(&registry, &run, Origin::External(SESSION_KEY.to_vec()))
        .with_saga_assignee(
            &crate::sink::saga_id_for_dispatch("runs", &dispatch_id_for(&run)),
            &ASSIGNEE,
        );
    let error = exec(&mut m, &mut ctx, &act(&run, reply("too late"))).unwrap_err();
    assert!(
        matches!(error, Error::Module(ref reason) if reason.contains("no execution lease")),
        "{error:?}"
    );
    ctx.env.origin = Origin::External(ASSIGNEE.to_vec());
    assert!(exec(&mut m, &mut ctx, &open(&run, &SESSION_KEY)).is_err());
    assert_eq!(sessions(&m)[0].actions, 0);
}

#[test]
fn a_settled_batch_counts_page_comments_it_already_staged() {
    // the thread holds (cap - 1) COMMITTED comments; the response carries two
    // comments to it. the committed-only probe is blind to the first comment,
    // so the same-block accounting is what degrades the second instead of
    // letting pages abort the delivery block.
    let (mut m, registry, run) = with_open_session();
    let mut thread = dummy_thread_view("review");
    thread.thread.target = "b-p".into();
    thread.thread.comment_ids = (0..pages::MAX_COMMENTS_PER_THREAD - 1)
        .map(|i| format!("comment-{i}"))
        .collect();
    let mut ctx = CaptureCtx::new()
        .at(9)
        .with_dispatch_origin()
        .with_registry(&registry)
        .with_transcript("general", transcript(2))
        .with_page("p1", page_blocks("p1", "Spec"))
        .with_page_thread(thread);
    let comment = page_thread_comment("review", "hello");
    exec(
        &mut m,
        &mut ctx,
        &result_event(&run, Ok(response(&[], vec![comment.clone(), comment]))),
    )
    .unwrap();
    assert_eq!(ctx.page_msgs().len(), 1, "{:?}", ctx.page_msgs());
    assert!(
        ctx.notes().iter().any(|n| n.contains("thread is full")),
        "{:?}",
        ctx.notes()
    );
}

#[test]
fn envelopes_are_decoded_against_the_catalog_in_the_module() {
    for (action, expected) in [
        (
            envelope(
                OP_CHAT_POST_MESSAGE,
                Some(serde_json::json!({"channel_id": "general", "author": 1})),
                text_content("hello"),
            ),
            "chat.post_message target",
        ),
        (
            envelope(
                crate::OP_JOBS_COMMENT,
                Some(serde_json::json!({"thread_id": "wrong"})),
                text_content("hello"),
            ),
            "jobs.comment target",
        ),
        (
            envelope("unknown", None, text_content("hello")),
            "not a catalog operation",
        ),
        (
            envelope(
                crate::OP_REPLY,
                Some(serde_json::json!({"channel_id": "general"})),
                text_content("hello"),
            ),
            "takes no target",
        ),
        (
            envelope(crate::OP_REPLY, None, serde_json::json!({"text": "hello"})),
            "reply input",
        ),
    ] {
        let (mut m, registry, run) = with_open_session();
        let mut ctx = session_ctx(&registry, &run, Origin::External(SESSION_KEY.to_vec()));
        let error = exec(&mut m, &mut ctx, &act(&run, action)).unwrap_err();
        assert!(format!("{error}").contains(expected), "{error}");
        assert_eq!(sessions(&m)[0].actions, 0);
        assert!(
            ctx.chat_msgs().is_empty() && ctx.page_msgs().is_empty() && ctx.job_msgs().is_empty()
        );
    }
}

#[test]
fn default_job_replies_require_the_original_claim_but_explicit_posts_choose_the_job() {
    let (m, registry, run) = with_open_session();
    let mut entry = m.pending_entry(&dispatch_id_for(&run)).unwrap().clone();
    entry.job_id = Some("job-1".into());
    entry.job_claim_height = 3;
    for (claim_height, action, accepted) in [
        (3, reply("progress"), true),
        (4, reply("progress"), false),
        (4, job_comment("job-1", "progress"), true),
    ] {
        let ctx = session_ctx(&registry, &run, Origin::External(SESSION_KEY.to_vec()))
            .with_claimed_job("job-1", claim_height);
        let response = AgentResponse {
            reply_blocks: Vec::new(),
            actions: vec![action],
            commit_message: None,
        };
        let result = block_on(m.validate_response(&ctx, &run, &entry, Lane::Session(0), response));
        assert_eq!(result.is_ok(), accepted, "{result:?}");
        if let Err(reason) = result {
            assert!(reason.contains("original job claim"), "{reason}");
        }
        assert!(ctx.job_msgs().is_empty(), "validation must not emit");
    }
}

#[test]
fn a_callee_result_cannot_stage_a_module_update() {
    // both settle lanes are the catalog's final lane, but only the run's own
    // final response binds a forge output a module update can pin; a callee's
    // result is refused by name instead of validating an update nobody emits.
    let (m, registry, run) = with_open_session();
    let entry = m.pending_entry(&dispatch_id_for(&run)).unwrap().clone();
    let update = envelope(
        crate::OP_MODULES_UPDATE,
        None,
        serde_json::json!({
            "module_id": "hello",
            "artifact": "hello.module",
            "code_hash": "a".repeat(64),
            "after": 50,
        }),
    );
    let ctx = session_ctx(&registry, &run, Origin::External(SESSION_KEY.to_vec()));
    let response = AgentResponse {
        reply_blocks: Vec::new(),
        actions: vec![update],
        commit_message: None,
    };
    let reason = block_on(m.validate_response(&ctx, &run, &entry, Lane::DelegatedSettle, response))
        .unwrap_err();
    assert!(reason.contains("run's own final response"), "{reason}");
}

/// A live page post mints its page under the session's action slot and the
/// receipt names it, so the agent can link the page it just published.
#[test]
fn a_live_page_post_mints_its_page_under_the_action_slot() {
    let (mut m, registry, run) = with_open_session();
    let mut ctx = session_ctx(&registry, &run, Origin::External(SESSION_KEY.to_vec()));
    let content = serde_json::json!([{"type": "text", "text": "notes"}]);
    exec(
        &mut m,
        &mut ctx,
        &act_as(&run, "pub", page_post("Notes", content)),
    )
    .unwrap();
    let page_id = format!("agent/{}/page/s0", dispatch_id_for(&run));
    let msgs = ctx.page_msgs();
    assert!(
        matches!(&msgs[..], [PageMsg::CreatePage { page_id: id, title, blocks }] if *id == page_id && title == "Notes" && blocks.len() == 1),
        "{msgs:?}"
    );
    commit(&mut m);
    let receipt = block_on(m.action_request(&crate::action_request_id(&run, "pub")))
        .unwrap()
        .expect("receipt");
    assert_eq!(receipt.view.operation, crate::OP_PAGES_POST);
    assert_eq!(
        receipt.view.result,
        serde_json::json!({"page_id": page_id, "title": "Notes"})
    );
}

// ---- the run journal ------------------------------------------------------------

#[test]
fn every_lifecycle_op_stamps_the_facts_it_committed_and_nothing_else() {
    let registry = registry(&["bot"]);
    let mut m = configured(&registry);
    let ctx = request_post(&mut m, &registry, 2, &[]);
    let run = run_id_for("general", 2, "bot");
    assert_eq!(
        ctx.journal(),
        vec![RunEvent {
            run_id: run.clone(),
            fact: RunFact::Dispatched {
                agent_id: "bot".into(),
                channel_id: "general".into(),
                anchor_seq: 2,
                job_id: None,
                delegation_id: None,
                requester: Origin::Program(registry["bot"].account),
            },
        }]
    );
    commit(&mut m);

    let mut ctx = session_ctx(&registry, &run, Origin::External(ASSIGNEE.to_vec()));
    exec(&mut m, &mut ctx, &open(&run, &SESSION_KEY)).unwrap();
    assert_eq!(
        ctx.journal(),
        vec![RunEvent {
            run_id: run.clone(),
            fact: RunFact::SessionOpened {
                attempt: 0,
                holder: crate::hex(&ASSIGNEE),
            },
        }]
    );
    commit(&mut m);
    // the idempotent re-open moves nothing, so it stamps nothing
    let mut ctx = session_ctx(&registry, &run, Origin::External(ASSIGNEE.to_vec()));
    exec(&mut m, &mut ctx, &open(&run, &SESSION_KEY)).unwrap();
    assert!(ctx.journal().is_empty());

    let mut ctx = session_ctx(&registry, &run, Origin::External(SESSION_KEY.to_vec()));
    exec(&mut m, &mut ctx, &act_as(&run, "ack", react("👀"))).unwrap();
    assert_eq!(
        ctx.journal(),
        vec![RunEvent {
            run_id: run.clone(),
            fact: RunFact::Acted {
                request_id: crate::action_request_id(&run, "ack"),
                lane: LaneKind::Live,
                operation: crate::OP_REACT.into(),
                result: serde_json::json!({"channel_id": "general", "seq": 2, "emoji": "👀"}),
            },
        }]
    );
    commit(&mut m);

    let mut ctx = CaptureCtx::new()
        .at(8)
        .with_dispatch_origin()
        .with_registry(&registry)
        .with_transcript("general", transcript(2));
    exec(
        &mut m,
        &mut ctx,
        &result_event(&run, Err("worker  exploded".into())),
    )
    .unwrap();
    // the failure reply is an action the run staged, on the final lane: it
    // enters the journal after the settle fact that produced it.
    assert_eq!(
        ctx.journal(),
        vec![
            RunEvent {
                run_id: run.clone(),
                fact: RunFact::Settled {
                    outcome: RunOutcome::Failed,
                    reason: Some("worker exploded".into()),
                    degraded: false,
                    executing_node: "unknown".into(),
                    output_ref: None,
                    pr: None,
                },
            },
            RunEvent {
                run_id: run.clone(),
                fact: RunFact::Acted {
                    request_id: format!("result/{}/0", dispatch_id_for(&run)),
                    lane: LaneKind::Final,
                    operation: crate::OP_REPLY.into(),
                    result: serde_json::json!({
                        "destination": {"channel_id": "general", "kind": "chat", "thread": 2},
                        "id": crate::reply_message_id(&run),
                    }),
                },
            },
        ]
    );
    commit(&mut m);
    assert_eq!(recent_runs(&m)[0].outcome, RunOutcome::Failed);
}
