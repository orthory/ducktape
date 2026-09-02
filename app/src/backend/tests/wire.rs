use super::*;

/// A SEARCH COSTS ITS SLOWEST SOURCE, NOT THEIR SUM. `search_workspace` awaited
/// six independent sources one after another, and nothing in it reads what
/// another leg produced. Warm that is worth nothing — every leg answers in a
/// few milliseconds. COLD it is the whole cost: a module's first touch measured
/// 10-54 s against this app's 30 s client ceiling, so serial is several
/// ceilings end to end and fanned out is one.
///
/// THE OVERLAP IS THE GUARANTEE, SO THE OVERLAP IS WHAT IS PINNED — observed
/// from outside the process by a node that answers nothing until all six legs
/// are in flight together. The pin this replaced greped the join's text for six
/// names, which folding two legs into one `async { a.await; b.await }` inside
/// the join defeats while staying green.
///
/// The row order is asserted in the same run: a fan-out that silently reordered
/// the results would be a different defect.
///
/// This does NOT contradict the `join_all` ban in backend/document.rs: that one
/// guards the WRITE chain, where an op built on the block before it must land
/// after it.
#[tokio::test(flavor = "current_thread")]
async fn a_workspace_search_reaches_its_six_sources_together() {
    let watch: std::sync::Arc<Mutex<FanOutWatch>> = Default::default();
    let rpc = node_that_answers_only_a_full_fan_out(9, &[], watch.clone()).await;

    let results = search_workspace(rpc, "needle".into()).await;

    assert_eq!(
        watch.lock().expect("stub watch").overlapped,
        [
            "chat", "files", "forge", "pages", "runs", "runs", "tasks", "tasks", "tasks"
        ],
        "every round trip a workspace search opens with must be in flight at \
         once — anything missing here waited on another request's reply. The \
         repeats are the legs that read more than one thing: tasks walks three \
         status pages, runs reads pending and recent."
    );
    // Every lane answered, so nothing is held back.
    assert_eq!(results.partial, "");
    // And the rows land in the order the screen shows them. The tasks lane is
    // three status pages behind one source, hence the run-length squash.
    let mut order: Vec<String> = results.hits.iter().map(|hit| hit.kind.clone()).collect();
    order.dedup();
    assert_eq!(order, ["page", "code", "file", "task", "run"]);
    // The page row heads with its PAGE, which is the second wave's whole job —
    // and the reason `list_pages` is a lane of its own rather than a substring
    // collision with the search it follows. Served the search's reply instead,
    // the title lookup fails and every page hit falls back to "Untitled".
    let page = results
        .hits
        .iter()
        .find(|hit| hit.kind == "page")
        .expect("the pages lane answered");
    assert_eq!(page.title, "The needle page");
}

/// A SOURCE THAT DID NOT ANSWER IS NOT A SOURCE WITH NOTHING TO SAY. All six
/// legs failed silently — `if let Ok(..)` on two, `return Vec::new()` on the
/// rest — and the node's per-module cold start runs tens of seconds against a
/// 30 s client ceiling, so a timeout was the ordinary case, not the exotic one.
/// A search that reached the node and lost three of its six sources still
/// rendered a confident count, a full chip strip reading 0 for kinds it never
/// read, and — when the survivors were empty — "Nothing matched that query in
/// this workspace". Three lies off one timeout, in the app that spent the night
/// learning to say nothing rather than something false.
///
/// EVERY LEG, NOT THE ONE I FIXED FIRST. The round-2 version refused the forge
/// lane alone, and reverting the silence report on chat, files or tasks — two
/// of them the `return Vec::new()` swallowers — kept the suite green. The
/// defect was class-wide, so the pin walks the class: each source in turn is
/// the one that does not answer.
#[tokio::test(flavor = "current_thread")]
async fn a_search_that_lost_a_source_says_which_one() {
    /// The six sources, each with the name the screen must call it by and the
    /// hit kind it contributes. One table: a seventh source added to
    /// `search_workspace` with no silence report has to be added here to pass,
    /// and then fails.
    const SOURCES: [(&str, &str, &str); 6] = [
        ("chat", "Messages", "message"),
        ("pages", "Pages", "page"),
        ("forge", "Code", "code"),
        ("files", "Files", "file"),
        ("tasks", "Tasks", "task"),
        ("runs", "Runs", "run"),
    ];

    for (leg, label, silent_kind) in SOURCES {
        let leg_alone: &'static [&'static str] = match leg {
            "chat" => &["chat"],
            "pages" => &["pages"],
            "forge" => &["forge"],
            "files" => &["files"],
            "tasks" => &["tasks"],
            _ => &["runs"],
        };
        let rpc = node_that_answers_only_a_full_fan_out(9, leg_alone, Default::default()).await;

        let results = search_workspace(rpc, "needle".into()).await;

        assert_eq!(
            results.partial,
            format!("{label} did not answer — these results are incomplete."),
            "the screen must name the source it did not read"
        );
        // The chip strip's contract is "a count of 0 means nothing matched,
        // never no loader", so the source that never ran keeps no chip at all.
        let chips: Vec<&str> = results
            .kinds
            .iter()
            .map(|kind| kind.kind.as_str())
            .collect();
        let answered: Vec<&str> = SOURCES
            .iter()
            .map(|(_, _, kind)| *kind)
            .filter(|kind| *kind != silent_kind)
            .collect();
        assert_eq!(chips, answered, "{label} was refused, so it keeps no chip");
        // And the answer that did arrive is untouched, in screen order —
        // degrading the survivors would be the opposite mistake. The chat lane
        // carries no rows on purpose (see `SEARCH_LANES`); every other source
        // contributes exactly one.
        let mut rows: Vec<&str> = results.hits.iter().map(|hit| hit.kind.as_str()).collect();
        rows.dedup();
        let carried: Vec<&str> = ["page", "code", "file", "task", "run"]
            .into_iter()
            .filter(|kind| *kind != silent_kind)
            .collect();
        assert_eq!(
            rows, carried,
            "with {label} silent the other sources still land, in screen order"
        );
    }

    // CARDINALITY. Every case above refuses exactly ONE source, and a filter
    // keyed on `silent.first()` instead of `silent.contains(..)` passes all six
    // — the reviewer changed that one token and the suite stayed green while a
    // second silent source got a chip reading 0, against the strip's own "a
    // count of 0 means nothing matched, never no loader". Two at once is the
    // case the PR body's own headline scenario describes.
    let rpc =
        node_that_answers_only_a_full_fan_out(9, &["chat", "pages"], Default::default()).await;
    let results = search_workspace(rpc, "needle".into()).await;
    assert_eq!(
        results.partial, "Messages, Pages did not answer — these results are incomplete.",
        "both silent sources are named, in screen order"
    );
    let chips: Vec<&str> = results
        .kinds
        .iter()
        .map(|kind| kind.kind.as_str())
        .collect();
    assert_eq!(
        chips,
        ["code", "file", "task", "run"],
        "NEITHER refused source keeps a chip — not just the first one"
    );
}

/// The key file's own reading, WITHOUT its password — what the launch window
/// and the identity cache both resolve through. A plaintext or garbled file is
/// not "a key we could not open", it is not a key.
#[test]
fn the_identity_reads_without_the_password_and_a_non_v1_file_does_not() {
    let directory = tempfile::tempdir().unwrap();
    let key = directory.path().join("user.key");
    let (_, minted) = keystore::userkey::mint_user_key(&key, "password-123").unwrap();

    let read = keystore::userkey::read_user_key_file(&key).expect("an encrypted v1 file parses");
    assert_eq!(read.pubkey, minted.public_key().as_ref());

    std::fs::write(&key, "plaintext-key").unwrap();
    assert!(keystore::userkey::read_user_key_file(&key).is_err());
    let prefix = keystore::userkey::USER_KEY_ENCRYPTED_PREFIX;
    std::fs::write(&key, format!("{prefix}not-base64!!")).unwrap();
    assert!(keystore::userkey::read_user_key_file(&key).is_err());
}

/// THE session property, now that the key opens in THIS process: ONE argon2id
/// pass, then a real signed frame per write — each carrying its OWN payload
/// and verifying under this device's identity.
///
/// The stub-child version of this test could only ever check that request and
/// answer lines stayed paired on a pipe. `decode_frame` checks the signature,
/// which is the property that actually matters: a frame the node accepts as
/// authored by this key. A mispaired payload here would mean the app submits
/// one operation's bytes under another's receipt.
#[tokio::test(flavor = "current_thread")]
async fn one_unlock_signs_every_request_of_the_session() {
    let directory = tempfile::tempdir().unwrap();
    let key = directory.path().join("user.key");
    let (_, minted) = keystore::userkey::mint_user_key(&key, "password-123").unwrap();

    let signer = super::rpc::Signer::unlock(key.clone(), Zeroizing::new("password-123".into()))
        .await
        .expect("the minted key opens under the password that sealed it");
    for op in 0..5u8 {
        let frame = signer.sign("chat", op as u64, &[op, op, op]);
        let (origin, message) = node::decode_frame(&frame).expect("the frame verifies");
        assert_eq!(message.target, "chat");
        assert_eq!(
            message.payload,
            vec![op, op, op],
            "request {op} got another's payload"
        );
        let sdk::Origin::External(author) = origin else {
            panic!("a user-signed frame is external authorship");
        };
        assert_eq!(author, minted.public_key().as_ref());
    }

    // A wrong password is refused, and an empty one names the locked state
    // rather than reporting a bad key.
    assert!(
        super::rpc::Signer::unlock(key.clone(), Zeroizing::new("wrong-password".into()))
            .await
            .is_err()
    );
    // `.map(drop)` because a `Signer` deliberately has no `Debug` — the whole
    // point of the type is that the opened key does not get printed anywhere.
    let locked = super::rpc::Signer::unlock(key, Zeroizing::new(String::new()))
        .await
        .map(drop)
        .expect_err("an empty password cannot open anything");
    assert!(locked.contains("locked"), "{locked}");
}

/// THE FAN-OUT SET, READ THROUGH A REAL NODE — the poll a live call session
/// runs once a second, and the read the whole huddle rides on. Everything
/// downstream of it is exact: the hub parses each entry with `from_hex_32` and
/// admits that peer's media by the key it gets, so a roster row that is not 64
/// lowercase hex characters of NODE key is a call that stays silent with
/// nothing to see anywhere.
///
/// It also pins the vocabulary that made the LIVE pill unreachable once
/// already: `HuddleEntry.user` is the kernel's BARE user id, and a comparison
/// against any other spelling of it marks nobody as you — which here would
/// mean fanning this device's own media at itself and never at the peer.
#[tokio::test(flavor = "current_thread")]
async fn a_huddles_roster_names_the_node_keys_its_media_is_admitted_by() {
    let storage = tempfile::tempdir().unwrap();
    let sim = simnode::boot(
        storage.path(),
        "127.0.0.1:0".parse().unwrap(),
        simnode::SimOpts {
            auto: true,
            ..Default::default()
        },
    )
    .unwrap();
    let rpc = RpcClient::new(&format!("http://{}", sim.addr())).unwrap();
    let (me, peer) = (
        ed25519::PrivateKey::from_seed(11),
        ed25519::PrivateKey::from_seed(12),
    );
    // Two people, two nodes — the huddle's roster is (user, node) pairs, and
    // it is the NODE half the media plane speaks.
    let (my_node, peer_node) = ([0xa1u8; 32], [0xb2u8; 32]);

    submit_test(
        &rpc,
        &me,
        1,
        "chat",
        chat::encode_msg(&ChatMsg::CreateChannel {
            channel_id: "eng".into(),
            name: "Engineering".into(),
            post_policy: PostPolicy::Open,
        }),
    )
    .await;
    submit_test(
        &rpc,
        &me,
        2,
        "chat",
        chat::encode_msg(&ChatMsg::JoinHuddle {
            channel_id: "eng".into(),
            node: my_node.to_vec(),
        }),
    )
    .await;
    submit_test(
        &rpc,
        &peer,
        1,
        "chat",
        chat::encode_msg(&ChatMsg::JoinHuddle {
            channel_id: "eng".into(),
            node: peer_node.to_vec(),
        }),
    )
    .await;

    let mine = me.public_key().as_ref().to_vec();
    let (_channel, roster) = load_channel_facts(&rpc, "eng", Some(&mine))
        .await
        .expect("the huddle's channel reads back");
    assert_eq!(roster.len(), 2, "both people are on the roster");
    assert_eq!(
        roster.iter().filter(|row| row.is_you).count(),
        1,
        "exactly one row is this device's — the id vocabulary has to match"
    );

    let nodes = huddle_recipient_nodes(roster);
    assert_eq!(
        nodes,
        vec![hex_encode(&peer_node)],
        "the fan-out is the OTHER node's key: ours in it would aim this \
         device's media at itself, and the peer's missing from it is the \
         silence this whole poll exists to end"
    );
    let admissible = nodes[0].len() == 64 && nodes[0].chars().all(|c| c.is_ascii_hexdigit());
    assert!(
        admissible,
        "the hub parses a recipient with `from_hex_32`; anything else is \
         dropped and the peer is never admitted: {}",
        nodes[0]
    );
    // `shutdown`, not a drop: the handle's last executor reference cannot be
    // dropped on this async thread (see `SimHandle::shutdown`).
    sim.shutdown();
}

#[test]
fn post_commit_hydration_errors_are_not_retryable() {
    let error = committed_error("read failed".into());
    assert!(error.committed);
    assert_eq!(error.message, "read failed");
}

#[tokio::test(flavor = "current_thread")]
async fn chat_and_pages_round_trip_over_signed_frames() {
    let storage = tempfile::tempdir().unwrap();
    let sim = simnode::boot(
        storage.path(),
        "127.0.0.1:0".parse().unwrap(),
        simnode::SimOpts {
            auto: true,
            ..Default::default()
        },
    )
    .unwrap();
    let origin = format!("http://{}", sim.addr());
    let rpc = RpcClient::new(&origin).unwrap();
    let signer = ed25519::PrivateKey::from_seed(7);

    submit_test(
        &rpc,
        &signer,
        1,
        "chat",
        chat::encode_msg(&ChatMsg::CreateChannel {
            channel_id: "general".into(),
            name: "General".into(),
            post_policy: PostPolicy::Open,
        }),
    )
    .await;
    submit_test(
        &rpc,
        &signer,
        2,
        "chat",
        chat::encode_msg(&ChatMsg::PostMessage {
            channel_id: "general".into(),
            message_id: "hello-1".into(),
            blocks: vec![chat::Block::paragraph("hello from the app")],
            thread: None,
            as_agent: None,
        }),
    )
    .await;
    submit_test(
        &rpc,
        &signer,
        3,
        "pages",
        pages::encode_msg(&PageMsg::CreatePage {
            page_id: "welcome".into(),
            title: "Welcome".into(),
        }),
    )
    .await;
    submit_test(
        &rpc,
        &signer,
        4,
        "pages",
        pages::encode_msg(&PageMsg::InsertBlock {
            parent: "welcome".into(),
            after: None,
            block: NewBlock {
                id: "intro".into(),
                kind: BlockKind::Paragraph,
                text: "A signed page block".into(),
                marks: Vec::new(),
            },
        }),
    )
    .await;

    let chat = load_chat_data(&rpc, Some("general")).await.unwrap();
    assert_eq!(chat.channels[0].name, "General");
    assert_eq!(chat.messages[0].body, "hello from the app");
    let pages = load_pages_data(&rpc, Some("welcome")).await.unwrap();
    assert_eq!(pages.active_page_title, "Welcome");
    assert_eq!(pages.blocks[0].text, "A signed page block");

    let origin = rpc.origin().to_string();
    let loaded_page = load_page(origin.clone(), "welcome".into()).await.unwrap();
    assert_eq!(loaded_page.active_page, "welcome");
    assert_eq!(loaded_page.blocks[0].text, "A signed page block");

    // A SAVE AGAINST A PAGE THE INDEX DOES NOT HOLD MUST REFUSE, NOT RETARGET.
    // `load_pages_data` answers a missing id with `pages.first()` — here that is
    // `welcome`, a real page full of real blocks. Without the guard in
    // `save_page_document` this call plans one buffer against another page's
    // blocks and emits removes against ITS ids. It refuses before any write, so
    // the password never has to be real.
    //
    // THE TITLE MUST MATCH THE PAGE IT WOULD FALL BACK TO, or this test proves
    // nothing: a differing title makes the title write fire first, and it dies
    // `BlockNotFound` on the id that does not exist. That accident is the only
    // thing standing between today's code and the corruption — and it does not
    // happen when the titles agree, which two untitled pages always do. With
    // the title matched, the body plan is `remove every line`.
    let stray = save_page_document(
        origin.clone(),
        String::new(),
        "no-such-page".into(),
        "Welcome\n".into(),
        "Welcome\n".into(),
    )
    .await;
    // ASSERT THE REASON, NOT JUST THE FAILURE. An unsigned save fails anyway —
    // at the signer, several steps after the plan was already built against the
    // wrong page's blocks. Only the message separates "refused before planning"
    // from "planned the damage, then could not sign it".
    let refusal = stray.expect_err("a save must not retarget to another page");
    assert_eq!(
        refusal.message, "page was not found",
        "the save must refuse on the page it cannot find, before it plans or signs anything"
    );
    let after = load_pages_data(&rpc, Some("welcome")).await.unwrap();
    assert_eq!(
        after.blocks[0].text, "A signed page block",
        "the refused save must not have touched the page it fell back to"
    );
    let workspace = connect(origin.clone(), 0, 0).await.unwrap();
    let mut live = live_events(origin.clone());
    let ready = next_change(&mut live).await;
    assert_eq!(ready.kind, crate::LiveKind::Ready);
    drop(ready);
    submit_test(
        &rpc,
        &signer,
        5,
        "chat",
        chat::encode_msg(&ChatMsg::PostMessage {
            channel_id: "general".into(),
            message_id: "hello-2".into(),
            blocks: vec![chat::Block::paragraph("arrived on the next block")],
            thread: None,
            as_agent: None,
        }),
    )
    .await;
    let changed = next_change(&mut live).await;
    assert_eq!(
        changed.kind,
        crate::LiveKind::Chat,
        "a chat op folds into a chat delta"
    );
    assert_eq!(changed.chat.len(), 1);
    let ChatDelta::Posted {
        channel_id,
        seq,
        message,
    } = &changed.chat[0]
    else {
        panic!("a post must publish a Posted payload")
    };
    assert_eq!(channel_id, "general");
    assert_eq!(
        *seq, 2,
        "the delta carries the module-assigned sequence from the feed stamp"
    );
    assert_eq!(message.body, "arrived on the next block");
    assert!(
        !changed.load_chat && !changed.load_pages,
        "a folded chat delta requires no reload"
    );
    assert!(changed.height > workspace.height);
    let base_height = changed.height;
    // Production drops this payload when the generated LiveUpdated reducer
    // returns. This direct stream fixture is that consumer, so release its
    // one-in-flight permit before asking the stream for later blocks.
    drop(changed);
    submit_test(
        &rpc,
        &signer,
        6,
        "chat",
        chat::encode_msg(&ChatMsg::PostMessage {
            channel_id: "general".into(),
            message_id: "reply-1".into(),
            blocks: vec![chat::Block::paragraph("a threaded reply")],
            thread: Some(1),
            as_agent: None,
        }),
    )
    .await;
    submit_test(
        &rpc,
        &signer,
        7,
        "chat",
        chat::encode_msg(&ChatMsg::EditMessage {
            channel_id: "general".into(),
            seq: 1,
            blocks: vec![chat::Block::paragraph("hello, edited")],
            base_rev: Some(0),
        }),
    )
    .await;
    submit_test(
        &rpc,
        &signer,
        8,
        "chat",
        chat::encode_msg(&ChatMsg::AddReaction {
            channel_id: "general".into(),
            seq: 1,
            emoji: "👍".into(),
        }),
    )
    .await;

    wait_for_block(&mut live, base_height + 3).await;
    let chat = load_chat_data(&rpc, Some("general")).await.unwrap();
    assert_eq!(chat.active_channel_name, "General");
    assert_eq!(chat.messages[0].body, "hello, edited");
    assert!(chat.messages[0].edited);
    assert_eq!(chat.messages[0].reply_count, 1);
    assert_eq!(chat.messages[0].reactions[0].emoji, "👍");
    let thread = load_thread_data(&rpc, "general", 1).await.unwrap();
    assert_eq!(thread.messages.len(), 2);
    assert_eq!(thread.messages[1].body, "a threaded reply");
    let hit = load_chat_hit(origin.clone(), "general".into(), 1, 3, 7)
        .await
        .unwrap();
    // ONE ROW BACK, NOT A PRE-CLICK LIST SNAPSHOT. Search navigation reads only
    // the selected channel row; carrying a list back would revert deltas the
    // live stream folded during the round trip (`upsert_channel_rows`).
    assert_eq!(
        hit.channels
            .iter()
            .map(|row| row.id.as_str())
            .collect::<Vec<_>>(),
        vec!["general"]
    );
    assert_eq!(hit.generation, 7);
    assert_eq!(hit.selected_message_seq, 1);
    assert_eq!(hit.active_thread_seq, 1);
    assert_eq!(hit.thread_target_seq, 3);
    assert_eq!(hit.thread_messages[1].body, "a threaded reply");
    submit_test(
        &rpc,
        &signer,
        9,
        "pages",
        pages::encode_msg(&PageMsg::InsertBlock {
            parent: "welcome".into(),
            after: Some("intro".into()),
            block: NewBlock {
                id: "heading".into(),
                kind: BlockKind::Heading2,
                text: "Nested work".into(),
                marks: Vec::new(),
            },
        }),
    )
    .await;
    submit_test(
        &rpc,
        &signer,
        10,
        "pages",
        pages::encode_msg(&PageMsg::InsertBlock {
            parent: "heading".into(),
            after: None,
            block: NewBlock {
                id: "todo".into(),
                kind: BlockKind::Todo,
                text: "Ship the editor".into(),
                marks: Vec::new(),
            },
        }),
    )
    .await;
    submit_test(
        &rpc,
        &signer,
        11,
        "pages",
        pages::encode_msg(&PageMsg::SetChecked {
            block_id: "todo".into(),
            checked: true,
        }),
    )
    .await;
    submit_test(
        &rpc,
        &signer,
        12,
        "pages",
        pages::encode_msg(&PageMsg::InsertBlock {
            parent: "welcome".into(),
            after: Some("heading".into()),
            block: NewBlock {
                id: "child".into(),
                kind: BlockKind::Page,
                text: "Child page".into(),
                marks: Vec::new(),
            },
        }),
    )
    .await;

    wait_for_block(&mut live, base_height + 7).await;
    let pages = load_pages_data(&rpc, Some("welcome")).await.unwrap();
    assert_eq!(pages.pages[0].id, "welcome");
    assert_eq!(pages.pages[1].id, "child");
    assert_eq!(pages.pages[1].prefix, "  ");
    assert_eq!(pages.blocks[2].id, "todo");
    assert_eq!(pages.blocks[2].prefix, "  ");
    assert!(pages.blocks[2].checked);

    submit_test(
        &rpc,
        &signer,
        13,
        "pages",
        pages::encode_msg(&PageMsg::AddComment {
            thread_id: "thread-live".into(),
            comment_id: "comment-live".into(),
            target: "intro".into(),
            text: "temporary".into(),
            anchor: None,
            mentions: Vec::new(),
            as_agent: None,
        }),
    )
    .await;
    wait_for_block(&mut live, base_height + 8).await;
    let threads = load_page_threads(origin.clone(), "welcome".into(), 1)
        .await
        .unwrap();
    assert!(
        threads
            .threads
            .iter()
            .any(|thread| thread.id == "thread-live"),
        "the live comment's thread is on the page rail"
    );
    submit_test(
        &rpc,
        &signer,
        14,
        "pages",
        pages::encode_msg(&PageMsg::DeleteComment {
            comment_id: "comment-live".into(),
        }),
    )
    .await;
    wait_for_block(&mut live, base_height + 9).await;
    let threads = load_page_threads(origin.clone(), "welcome".into(), 2)
        .await
        .unwrap();
    assert!(
        !threads
            .threads
            .iter()
            .any(|thread| thread.id == "thread-live"),
        "the deleted comment's thread is gone from the page rail"
    );

    let refreshed = live_resync_load(
        origin,
        "general".into(),
        "welcome".into(),
        "both".into(),
        false,
        7,
        3,
        0,
    )
    .await
    .unwrap();
    assert_eq!(refreshed.generation, 7);
    assert_eq!(
        refreshed.fold_serial, 3,
        "the reply echoes the fold serial the request snapshotted (#1041)"
    );
    assert!(refreshed.chat_loaded && refreshed.pages_loaded);
    assert_eq!(refreshed.messages[1].body, "arrived on the next block");
    assert_eq!(refreshed.active_page, "welcome");
    sim.shutdown();
}

#[test]
fn hydration_retry_is_capped() {
    assert_eq!(retry_delay(1), Duration::from_secs(1));
    assert_eq!(retry_delay(3), Duration::from_secs(4));
    assert_eq!(retry_delay(99), Duration::from_secs(16));
}

/// A `runs` OP IS A SIGNAL, NOT A FOLD. Nothing on screen draws a run row: the
/// fact that module feeds is `AgentRow.live`, joined into the AGENTS
/// projection out of another module's state (`agents_with_a_run_in_flight`).
/// So there is nothing local to fold into, and the only useful shape is a
/// plane update naming `runs` — which the handler answers by refetching that
/// projection, the Forge seat's live dot with it.
#[tokio::test(flavor = "current_thread")]
async fn a_runs_op_asks_the_agents_projection_to_refetch() {
    let update = folded_update(
        "",
        "runs",
        ducktape_rpc::StreamOp {
            height: 7,
            seq: 0,
            time: 0,
            origin: ducktape_rpc::StreamOrigin {
                kind: ducktape_rpc::StreamOriginKind::Module,
                id: Some("runs".into()),
            },
            payload: Some(serde_json::json!({"claim_job": {"run_id": "run-1"}})),
            payload_hex: None,
            assigned: None,
            assigned_hex: None,
        },
    )
    .await
    .expect("a runs op is visible to the shell");
    assert_eq!(update.kind, crate::LiveKind::Plane);
    assert_eq!(update.module, "runs", "the module IS the whole payload");
    assert_eq!(update.height, 7);
    assert!(
        !update.load_chat && !update.load_pages,
        "the signal buys the agents projection, not a chat or pages slice"
    );
}

/// A PLANE WITH NO SUBSCRIPTION IS A DEAD ARM, and a silent one. `folded_update`
/// can only route an op the stream was asked to deliver, so the subscribe list
/// and its match arms are one contract kept in two places. `runs` is the case
/// that proved it: `AgentRow.live` is read from that module, the Forge seat
/// draws a live dot off the joined row, and nothing ever said the module
/// changed — so the dot stayed dark for the length of a run. The EXACT list is
/// the pin, because a topic dropped here fails nothing else.
#[test]
fn the_live_stream_subscribes_to_every_plane_the_console_reads() {
    const LIVE: &str = include_str!("../live.rs");
    let list = LIVE
        .split_once("rpc.module_events(")
        .expect("the subscribe call")
        .1
        .split_once("],")
        .expect("the topic list")
        .0;
    let topics: Vec<&str> = list
        .lines()
        .filter_map(|line| {
            line.trim()
                .strip_prefix('"')?
                .split_once("\".to_string(),")
                .map(|(topic, _)| topic)
        })
        .collect();
    assert_eq!(
        topics,
        [
            "chat",
            "pages",
            "inbox",
            "forge",
            "valset",
            "governance",
            "identity",
            "agent",
            "runs",
            "files",
        ]
    );
}

/// TWO MODULES, ONE PROJECTION — every quadrant. `agent` commits an agent's
/// registration and `runs` commits its liveness, so this is the only plane
/// predicate that answers for a pair; `plane_live_hit`'s single `want` cannot
/// express it and the Ice checker will not let the handler spell the pair
/// inline. A non-plane kind must stay false whatever module it names, or a
/// chat fold's module string starts issuing agent queries.
#[test]
fn the_agents_plane_hit_answers_for_both_of_its_modules_and_nothing_else() {
    for (kind, module, want) in [
        (crate::LiveKind::Plane, "agent", true),
        (crate::LiveKind::Plane, "runs", true),
        (crate::LiveKind::Plane, "valset", false),
        (crate::LiveKind::Chat, "agent", false),
        (crate::LiveKind::Chat, "runs", false),
        (crate::LiveKind::Resync, "runs", false),
    ] {
        assert_eq!(
            agents_plane_hit(kind, module.into()),
            want,
            "{kind:?} / {module}"
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn the_live_subscription_waits_for_the_ui_to_drop_its_publication() {
    let gate = Arc::new(tokio::sync::Semaphore::new(1));
    let permit = gate
        .clone()
        .acquire_owned()
        .await
        .expect("the gate is open");
    let update = LiveUpdate {
        permit: LivePermit::held(permit),
        ..LiveUpdate::default()
    };
    assert!(update.permit.is_held());
    assert!(gate.clone().try_acquire_owned().is_err());

    let generated_message_clone = update.clone();
    drop(update);
    assert!(
        gate.clone().try_acquire_owned().is_err(),
        "every clone must leave the generated update before the stream resumes"
    );
    drop(generated_message_clone);
    assert!(gate.try_acquire_owned().is_ok());
}

/// A TIP MOVES THE HEAD AND MUST FETCH NOTHING.
///
/// The heartbeat rides every block, and an idle chain nop-fills once per
/// `BLOCK_TIME` (`bin/node/src/constants.rs`) — so anything this update
/// triggers runs at ~1 Hz forever, on a chain where nothing happened. A load
/// hung off it would be a poll wearing a consensus costume, and `/v1/query` is
/// checkpoint-gated (`backend/live.rs`), so that poll would also be the thing
/// that hands a healthy node's console "error sending request".
///
/// `assert_no_polling` cannot see this: it greps `lifecycle.ice` for lines
/// starting with `every ` and a load reached through a live update is invisible
/// to it. So the guard is here, on the value itself.
#[test]
fn a_tip_carries_the_head_and_loads_nothing() {
    let tip = live_update(crate::LiveKind::Tip, "Live · block 41", 41);
    assert_eq!(tip.height, 41, "the head is the tip's entire payload");
    assert!(
        !tip.load_chat && !tip.load_pages,
        "a tip must not trigger a load — that is a 1 Hz poll on an idle chain"
    );
    assert!(
        !tip.debounce,
        "there is nothing to coalesce: a tip fetches nothing"
    );
    assert!(
        tip.module.is_empty(),
        "a heartbeat is not a topic, so it names no module"
    );
}

#[test]
fn a_refused_key_password_reaches_the_screen_as_a_sentence() {
    // Verbatim what the key tool hands the app on a mistyped unlock password —
    // the mapping keys on the CLI's own `WRONG_PASSWORD_ERR` text.
    let refused =
        user_error("ducktape user key unlock refused: FATAL: corrupt or wrong password".into());
    assert_eq!(
        refused,
        "That password did not open this device's key. Check it and try again."
    );
    // A module's own sentence still flows through untouched.
    assert_eq!(user_error("post is empty".into()), "post is empty");
}

/// A HEIGHT IS RECORDED WHERE IT IS LEARNED, and there are exactly two places
/// this client ever learns one: the receipt of a write it signed, and an op its
/// live stream delivered. The stream half is driven end to end below
/// (`an_op_the_stream_delivered_is_waited_out_by_the_reload_behind_it`); the
/// write half cannot be, because reaching the recording means signing a real
/// frame with a real key, so it is pinned as the source shape it is — one call
/// on the receipt, at the funnel every module's writes already pass through.
///
/// Deleting it does not fail a read: it makes every read AFTER a write stop
/// waiting, which is a stale document nobody notices until a line duplicates.
#[test]
fn a_signed_write_records_the_block_that_took_it() {
    const RPC: &str = include_str!("../rpc.rs");
    let body = |name: &str| {
        RPC.split(&format!("pub(crate) async fn {name}("))
            .nth(1)
            .unwrap_or_else(|| panic!("{name} is declared"))
            .split("\n/// ")
            .next()
            .unwrap_or_else(|| panic!("{name} body"))
    };
    // a write this device signs and one a passkey/wallet signed in the
    // browser share ONE submit funnel, so the receipt is recorded once.
    let signed_write = body("signed_write");
    assert!(
        signed_write.contains("submit_raw_frame("),
        "signed_write submits through the raw-frame funnel"
    );
    let funnel = body("submit_raw_frame");
    let submit = funnel
        .find("submit_frame(")
        .expect("the funnel submits the frame");
    let record = funnel
        .find("note_module_block(")
        .expect("the funnel records the block its write landed in");
    assert!(
        submit < record,
        "the height is recorded from the RECEIPT, so there is nothing to \
         record until the node has answered with one"
    );
}
