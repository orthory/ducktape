use super::*;

// ── comments (folded into the pages module) ──

fn add(thread: &str, comment: &str, target: &str, text: &str) -> PageMsg {
    PageMsg::AddComment {
        thread_id: thread.into(),
        comment_id: comment.into(),
        target: target.into(),
        text: text.into(),
        anchor: None,
        mentions: Vec::new(),
        as_agent: None,
    }
}

fn add_as_agent(thread: &str, comment: &str, target: &str, agent: &str) -> PageMsg {
    PageMsg::AddComment {
        thread_id: thread.into(),
        comment_id: comment.into(),
        target: target.into(),
        text: "agent says".into(),
        anchor: None,
        mentions: Vec::new(),
        as_agent: Some(agent.into()),
    }
}

#[test]
fn exact_comment_anchor_rebases_with_target_text() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        seed_page(&mut p, "p1").await;
        let mut anchored = add("t1", "m1", "b1", "on the selection");
        if let PageMsg::AddComment { anchor, .. } = &mut anchored {
            *anchor = Some(RelativeAnchor { start: 0, end: 99 });
        }
        apply_err_as(&mut p, &anchored, user("alice"), "invalid text range").await;
        if let PageMsg::AddComment { anchor, .. } = &mut anchored {
            *anchor = Some(RelativeAnchor { start: 0, end: 2 });
        }
        apply_commit_as(&mut p, &anchored, user("alice")).await;
        apply_commit(
            &mut p,
            &PageMsg::UpdateText {
                block_id: "b1".into(),
                text: "++b1".into(),
                marks: None,
            },
        )
        .await;
        assert_eq!(
            query_thread(&p, "t1").await.unwrap().thread.anchor,
            Some(RelativeAnchor { start: 2, end: 4 })
        );
        apply_commit(
            &mut p,
            &PageMsg::MoveCommentThread {
                thread_id: "t1".into(),
                target: "b2".into(),
                anchor: Some(RelativeAnchor { start: 0, end: 2 }),
            },
        )
        .await;
        let moved_from = query_target_threads(&p, "b1", 0, 0).await;
        let moved_to = query_target_threads(&p, "b2", 0, 0).await;
        assert!(moved_from.threads.is_empty());
        assert_eq!(moved_to.threads[0].target, "b2");
        assert_eq!(
            moved_to.threads[0].anchor,
            Some(RelativeAnchor { start: 0, end: 2 })
        );
    });
}

#[test]
fn add_comment_reports_structured_agent_mentions_to_tagging() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages").with_tagging("tagging");
        let mut op = add("t1", "c1", "page-1", "@qa-luna please review");
        let PageMsg::AddComment { mentions, .. } = &mut op else {
            unreachable!()
        };
        mentions.push(AuthorRef::Agent {
            module: "runs".into(),
            agent_id: "qa-luna".into(),
        });
        let mut ctx = ctx_as(user("eddy"));
        p.execute(&mut ctx, &msg(&op)).await.unwrap();
        assert_eq!(ctx.msgs().len(), 1);
        assert_eq!(ctx.msgs()[0].target, "tagging");
        let tagging::TaggingMsg::Tag(event) = tagging::decode_msg(&ctx.msgs()[0].payload).unwrap()
        else {
            panic!("expected tag event")
        };
        assert_eq!(event.container, "t1");
        assert_eq!(event.content_seq, 1);
        assert_eq!(event.author, tagging::Author::User(b"eddy".to_vec()));
        assert_eq!(
            event.tags,
            vec![tagging::EntityRef {
                module: "runs".into(),
                entity: "qa-luna".into(),
            }]
        );
    });
}

#[test]
fn edit_comment_reports_only_supplied_new_agent_mentions_to_tagging() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages").with_tagging("tagging");
        apply_commit_as(&mut p, &add("t1", "c1", "page-1", "draft"), user("eddy")).await;
        let edit = PageMsg::EditComment {
            comment_id: "c1".into(),
            text: "@qa-luna please review".into(),
            mentions: vec![AuthorRef::Agent {
                module: "runs".into(),
                agent_id: "qa-luna".into(),
            }],
        };
        let mut ctx = ctx_as(user("eddy"));
        p.execute(&mut ctx, &msg(&edit)).await.unwrap();

        let tagging::TaggingMsg::Tag(event) = tagging::decode_msg(&ctx.msgs()[0].payload).unwrap()
        else {
            panic!("expected tag event")
        };
        assert_eq!(event.container, "t1");
        assert_eq!(event.content_seq, 1);
        assert_eq!(
            event.tags,
            vec![tagging::EntityRef {
                module: "runs".into(),
                entity: "qa-luna".into(),
            }]
        );
    });
}

#[test]
fn add_comment_rejects_over_length_ids_before_staging() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        let long_thread = "t".repeat(MAX_THREAD_ID_BYTES + 1);
        apply_err_as(
            &mut p,
            &add(&long_thread, "m1", "b1", "hi"),
            user("alice"),
            "id or target too large",
        )
        .await;
        let long_comment = "m".repeat(MAX_COMMENT_ID_BYTES + 1);
        apply_err_as(
            &mut p,
            &add("t1", &long_comment, "b1", "hi"),
            user("alice"),
            "id or target too large",
        )
        .await;
        let long_target = "b".repeat(MAX_COMMENT_TARGET_BYTES + 1);
        apply_err_as(
            &mut p,
            &add("t1", "m1", &long_target, "hi"),
            user("alice"),
            "id or target too large",
        )
        .await;
        // nothing staged — an id at exactly the cap still lands.
        apply_commit_as(
            &mut p,
            &add(&"t".repeat(MAX_THREAD_ID_BYTES), "m1", "b1", "hi"),
            user("alice"),
        )
        .await;
    });
}

#[test]
fn add_comment_rejects_oversized_origins() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        for (comment, origin) in [
            (
                "m1",
                sdk::Origin::External(vec![b'x'; MAX_COMMENT_AUTHOR_BYTES + 1]),
            ),
            (
                "m2",
                sdk::Origin::Module("m".repeat(MAX_COMMENT_AUTHOR_BYTES + 1)),
            ),
        ] {
            apply_err_as(
                &mut p,
                &add("t1", comment, "b1", "hi"),
                origin,
                "comment author is too large",
            )
            .await;
        }
    });
}

/// pin the ESCAPE vector: an id carrying a serde_json-escaping char (`"`,
/// `\`, or a control char < 0x20) is rejected at admission even under the
/// length cap — otherwise its escaped serialization (2–6 B/char) could still
/// overflow a derived block and abort it. covers thread_id, comment_id, and
/// target.
#[test]
fn add_comment_rejects_escaping_char_ids() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        for (t, c, tg) in [
            ("th\u{1}read", "m1", "b1"), // control char in thread_id
            ("t1", "com\"ment", "b1"),   // quote in comment_id
            ("t1", "m1", "tar\\get"),    // backslash in target
        ] {
            apply_err_as(
                &mut p,
                &add(t, c, tg, "hi"),
                user("alice"),
                "id or target too large",
            )
            .await;
        }
    });
}

/// the R4 invariant: at the count caps, ids bounded to their length caps keep
/// the DERIVED shared blocks under MAX_BLOCK_LEN, so an AddComment append can
/// never abort a block on size. because admission ALSO rejects escaping chars
/// (see `add_comment_rejects_escaping_char_ids`), every admitted id
/// serializes 1:1, so a max-BYTE-length ASCII id is the true worst case — a
/// non-ASCII UTF-8 id of the same `len()` serializes to the same byte count.
#[test]
fn bounded_ids_keep_the_derived_blocks_under_max_block_len() {
    // the per-target thread index is a Vec<thread_id> of up to
    // MAX_THREADS_PER_TARGET entries.
    let tid = "t".repeat(MAX_THREAD_ID_BYTES);
    let index: Vec<String> = (0..MAX_THREADS_PER_TARGET).map(|_| tid.clone()).collect();
    let index_bytes = serde_json::to_vec(&index).unwrap().len();
    assert!(
        index_bytes < MAX_BLOCK_LEN,
        "full target index {index_bytes} >= {MAX_BLOCK_LEN}"
    );

    // a thread record holds comment_ids: Vec<comment_id> up to
    // MAX_COMMENTS_PER_THREAD, plus its own (bounded) id/target fields.
    let thread = Thread {
        id: "t".repeat(MAX_THREAD_ID_BYTES),
        target: "b".repeat(MAX_COMMENT_TARGET_BYTES),
        opener: AuthorRef::System,
        created_at: 0,
        anchor: None,
        resolved: false,
        resolved_by: None,
        comment_ids: (0..MAX_COMMENTS_PER_THREAD)
            .map(|_| "m".repeat(MAX_COMMENT_ID_BYTES))
            .collect(),
    };
    let thread_bytes = serde_json::to_vec(&thread).unwrap().len();
    assert!(
        thread_bytes < MAX_BLOCK_LEN,
        "full thread block {thread_bytes} >= {MAX_BLOCK_LEN}"
    );
}

#[test]
fn a_full_comment_thread_keeps_the_block_removal_escape_path() {
    deterministic::Runner::default().start(|_context| async move {
        assert_eq!(MAX_COMMENTS_PER_THREAD + 2, MAX_TRAVERSAL_WORK);
        let mut p = Pages::new("pages", Box::new(sdk_testkit::MemStore::new()));
        let comment_ids: Vec<_> = (0..MAX_COMMENTS_PER_THREAD)
            .map(|index| format!("comment-{index}"))
            .collect();
        p.store_block(&Block {
            id: "target".into(),
            parent: None,
            page: "target".into(),
            kind: BlockKind::Page,
            text: String::new(),
            marks: Vec::new(),
            checked: false,
            children: Vec::new(),
        })
        .unwrap();
        p.stage_index(&BTreeMap::from([("target".into(), None)]))
            .unwrap();
        p.stage(
            "\0ct:thread",
            serde_json::to_vec(&Thread {
                id: "thread".into(),
                target: "target".into(),
                opener: AuthorRef::System,
                created_at: 0,
                anchor: None,
                resolved: false,
                resolved_by: None,
                comment_ids: comment_ids.clone(),
            })
            .unwrap(),
        )
        .unwrap();
        p.stage("\0ci:target", serde_json::to_vec(&vec!["thread"]).unwrap())
            .unwrap();
        for id in &comment_ids {
            p.stage(
                &format!("\0cc:{id}"),
                serde_json::to_vec(&Comment {
                    id: id.clone(),
                    thread_id: "thread".into(),
                    author: AuthorRef::System,
                    text: String::new(),
                    created_at: 0,
                    edited_at: None,
                    deleted: false,
                })
                .unwrap(),
            )
            .unwrap();
        }
        p.commit_block().await.unwrap();

        p.apply(
            PageMsg::RemoveBlock {
                block_id: "target".into(),
            },
            &Origin::System,
            0,
        )
        .await
        .unwrap();

        assert!(p.load_block("target").await.unwrap().is_none());
        assert!(p.load_thread("thread").await.unwrap().is_none());
        assert!(p.load_comment(&comment_ids[0]).await.unwrap().is_none());
        assert!(p.load_index().await.unwrap().is_empty());
    });
}

#[test]
fn as_agent_refines_a_module_origin_into_an_agent_author() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        apply_commit_as(
            &mut p,
            &add_as_agent("t1", "m1", "b1", "bot"),
            sdk::Origin::Module("runs".into()),
        )
        .await;
        let view = query_thread(&p, "t1").await.unwrap();
        let agent = AuthorRef::Agent {
            module: "runs".into(),
            agent_id: "bot".into(),
        };
        assert_eq!(view.thread.opener, agent, "the opener is the agent");
        assert_eq!(
            view.comments[0].comment.author, agent,
            "the comment author too"
        );
    });
}

#[test]
fn as_agent_requires_a_module_origin_and_a_non_empty_id() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        apply_err_as(
            &mut p,
            &add_as_agent("t1", "m1", "b1", "bot"),
            user("alice"),
            "as_agent requires a module origin",
        )
        .await;
        apply_err_as(
            &mut p,
            &add_as_agent("t1", "m1", "b1", ""),
            sdk::Origin::Module("runs".into()),
            "empty as_agent",
        )
        .await;
        apply_err_as(
            &mut p,
            &add_as_agent(
                "t1",
                "m1",
                "b1",
                &"a".repeat(MAX_COMMENT_AGENT_ID_BYTES + 1),
            ),
            sdk::Origin::Module("runs".into()),
            "as_agent is too large",
        )
        .await;
    });
}

#[test]
fn get_comment_serves_the_record_tombstones_included() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        assert_eq!(query_comment(&p, "m1").await, None, "absent id is None");
        apply_commit_as(&mut p, &add("t1", "m1", "b1", "x"), user("alice")).await;
        assert!(query_comment(&p, "m1").await.is_some());
        // a tombstoned comment KEEPS its record — the probe must still see it
        // (AddComment rejects the id even when deleted).
        apply_commit_as(&mut p, &add("t1", "m2", "b1", "y"), user("alice")).await;
        apply_commit_as(
            &mut p,
            &PageMsg::DeleteComment {
                comment_id: "m1".into(),
            },
            user("alice"),
        )
        .await;
        let m1 = query_comment(&p, "m1").await.unwrap();
        assert!(m1.deleted, "the tombstone is served, not hidden");
    });
}

#[test]
fn comment_add_opens_then_appends_and_batches_by_target() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        apply_commit_as(&mut p, &add("t1", "m1", "b1", "first"), user("alice")).await;
        apply_commit_as(&mut p, &add("t1", "m2", "b1", "second"), user("bob")).await;
        apply_commit_as(&mut p, &add("t2", "m3", "b1", "other"), user("alice")).await;
        apply_commit_as(&mut p, &add("t3", "m4", "b2", "elsewhere"), user("alice")).await;

        let b1 = query_target_threads(&p, "b1", 0, 0).await;
        let b2 = query_target_threads(&p, "b2", 0, 0).await;
        assert_eq!(b1.threads.len(), 2);
        let t1 = b1.threads.iter().find(|thread| thread.id == "t1").unwrap();
        assert_eq!(t1.opener, AuthorRef::User(b"alice".to_vec()));
        assert_eq!(t1.comment_count, 2);
        let comments = query_thread(&p, "t1").await.unwrap();
        assert_eq!(
            comments
                .comments
                .iter()
                .map(|item| item.comment.text.as_str())
                .collect::<Vec<_>>(),
            ["first", "second"]
        );
        assert_eq!(
            comments.comments[1].comment.author,
            AuthorRef::User(b"bob".to_vec())
        );
        assert_eq!(b2.threads.len(), 1);
    });
}

#[test]
fn comment_queries_page_targets_and_skip_tombstones_with_stable_ordinals() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        for index in 0..=MAX_THREAD_PAGE_LIMIT {
            let thread = format!("t{index:02}");
            let comment = format!("c{index:02}");
            apply_commit_as(
                &mut p,
                &add(&thread, &comment, "b1", &thread),
                user("alice"),
            )
            .await;
        }
        for index in 1..=MAX_COMMENT_PAGE_LIMIT {
            let comment = format!("m{index:02}");
            apply_commit_as(&mut p, &add("t00", &comment, "b1", &comment), user("alice")).await;
        }
        apply_commit_as(
            &mut p,
            &PageMsg::DeleteComment {
                comment_id: "m03".into(),
            },
            user("alice"),
        )
        .await;

        let first_threads = query_target_threads(&p, "b1", 0, u16::MAX).await;
        assert_eq!(first_threads.total, u32::from(MAX_THREAD_PAGE_LIMIT) + 1);
        assert_eq!(
            first_threads.threads.len(),
            usize::from(MAX_THREAD_PAGE_LIMIT),
            "the target limit clamps"
        );
        assert_eq!(first_threads.threads[0].id, "t00");
        assert_eq!(
            first_threads.threads[0].comment_count,
            u32::from(MAX_COMMENT_PAGE_LIMIT) + 1
        );
        assert_eq!(
            first_threads.next_from,
            Some(u32::from(MAX_THREAD_PAGE_LIMIT))
        );
        let final_threads =
            query_target_threads(&p, "b1", first_threads.next_from.unwrap(), u16::MAX).await;
        assert_eq!(final_threads.threads.len(), 1);
        assert_eq!(final_threads.threads[0].id, "t64");
        assert_eq!(final_threads.next_from, None);

        let first_comments = query_thread_comments(&p, "t00", 0, u16::MAX).await.unwrap();
        assert_eq!(
            first_comments
                .comments
                .iter()
                .map(|item| item.ordinal)
                .collect::<Vec<_>>(),
            [1, 2, 3, 5, 6, 7, 8],
            "the deleted middle comment stays a gap in stable ordinals"
        );
        assert_eq!(
            first_comments.next_from,
            Some(u32::from(MAX_COMMENT_PAGE_LIMIT)),
            "the comment limit clamps by lifetime slots"
        );
        let final_comments =
            query_thread_comments(&p, "t00", first_comments.next_from.unwrap(), u16::MAX)
                .await
                .unwrap();
        assert_eq!(
            final_comments
                .comments
                .iter()
                .map(|item| item.ordinal)
                .collect::<Vec<_>>(),
            [9]
        );
        assert_eq!(final_comments.next_from, None);
    });
}

#[test]
fn comment_queries_reject_ids_that_cannot_exist() {
    deterministic::Runner::default().start(|context| async move {
        let p = pages_on!(context, "pages");
        let target = "x".repeat(MAX_COMMENT_TARGET_BYTES + 1);
        let target_query = PageQuery::ThreadsForTarget {
            target,
            from: 0,
            limit: 0,
        };
        assert_eq!(
            p.query(&encode_query(&target_query))
                .await
                .unwrap_err()
                .to_string(),
            "Module(comment id or target too large)"
        );

        let thread_query = PageQuery::CommentsForThread {
            thread_id: "x".repeat(MAX_THREAD_ID_BYTES + 1),
            from: 0,
            limit: 0,
        };
        assert_eq!(
            p.query(&encode_query(&thread_query))
                .await
                .unwrap_err()
                .to_string(),
            "Module(comment id or target too large)"
        );
    });
}

#[test]
fn comment_append_rejects_target_mismatch_duplicate_and_empty_origin() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        apply_commit_as(&mut p, &add("t1", "m1", "b1", "x"), user("alice")).await;
        apply_err_as(
            &mut p,
            &add("t1", "m2", "b2", "y"),
            user("alice"),
            "target mismatch",
        )
        .await;
        apply_err_as(
            &mut p,
            &add("t1", "m1", "b1", "z"),
            user("alice"),
            "duplicate comment id",
        )
        .await;
        apply_err_as(
            &mut p,
            &add("t9", "m9", "b1", "z"),
            sdk::Origin::External(vec![]),
            "empty origin",
        )
        .await;
    });
}

#[test]
fn comment_edit_and_delete_are_author_only() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        apply_commit_as(&mut p, &add("t1", "m1", "b1", "orig"), user("alice")).await;
        apply_err_as(
            &mut p,
            &PageMsg::EditComment {
                comment_id: "m1".into(),
                text: "hax".into(),
                mentions: Vec::new(),
            },
            user("bob"),
            "not the comment author",
        )
        .await;
        apply_err_as(
            &mut p,
            &PageMsg::DeleteComment {
                comment_id: "m1".into(),
            },
            user("bob"),
            "not the comment author",
        )
        .await;
        apply_commit_as(
            &mut p,
            &PageMsg::EditComment {
                comment_id: "m1".into(),
                text: "edited".into(),
                mentions: Vec::new(),
            },
            user("alice"),
        )
        .await;
        let v = query_thread(&p, "t1").await.unwrap();
        assert_eq!(v.comments[0].comment.text, "edited");
        assert_eq!(v.comments[0].comment.edited_at, Some(7));
    });
}

#[test]
fn comment_deleting_last_live_removes_the_thread() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        apply_commit_as(&mut p, &add("t1", "m1", "b1", "a"), user("alice")).await;
        apply_commit_as(&mut p, &add("t1", "m2", "b1", "b"), user("alice")).await;
        apply_commit_as(
            &mut p,
            &PageMsg::DeleteComment {
                comment_id: "m1".into(),
            },
            user("alice"),
        )
        .await;
        let v = query_thread(&p, "t1").await.unwrap();
        assert_eq!(
            v.comments
                .iter()
                .map(|item| item.comment.text.as_str())
                .collect::<Vec<_>>(),
            ["b"]
        );
        apply_commit_as(
            &mut p,
            &PageMsg::DeleteComment {
                comment_id: "m2".into(),
            },
            user("alice"),
        )
        .await;
        assert!(query_thread(&p, "t1").await.is_none());
        assert!(
            query_target_threads(&p, "b1", 0, 0)
                .await
                .threads
                .is_empty()
        );
    });
}

#[test]
fn comment_resolve_toggles_and_records_resolver() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        apply_commit_as(&mut p, &add("t1", "m1", "b1", "a"), user("alice")).await;
        apply_commit_as(
            &mut p,
            &PageMsg::ResolveThread {
                thread_id: "t1".into(),
                resolved: true,
            },
            user("bob"),
        )
        .await;
        let v = query_thread(&p, "t1").await.unwrap();
        assert!(v.thread.resolved);
        assert_eq!(v.thread.resolved_by, Some(AuthorRef::User(b"bob".to_vec())));
        apply_commit_as(
            &mut p,
            &PageMsg::ResolveThread {
                thread_id: "t1".into(),
                resolved: false,
            },
            user("alice"),
        )
        .await;
        assert_eq!(
            query_thread(&p, "t1").await.unwrap().thread.resolved_by,
            None
        );
        apply_err_as(
            &mut p,
            &PageMsg::ResolveThread {
                thread_id: "ghost".into(),
                resolved: true,
            },
            user("alice"),
            "thread not found",
        )
        .await;
    });
}

#[test]
fn comment_caps_and_reserved_ids_reject() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        let huge = "x".repeat(MAX_COMMENT_TEXT_BYTES + 1);
        apply_err_as(
            &mut p,
            &add("t1", "m1", "b1", &huge),
            user("alice"),
            "comment text too large",
        )
        .await;
        assert!(p.staged.is_empty(), "a rejected comment op stages nothing");
        // a NUL-prefixed id lands in the reserved keyspace — rejected.
        apply_err_as(
            &mut p,
            &add("\u{0}evil", "m1", "b1", "x"),
            user("alice"),
            "reserved block id",
        )
        .await;
    });
}

// comments ride the same qmdb as blocks (reserved NUL-prefixed keys), so a
// block edit and a comment op never collide, and both compose into root.
#[test]
fn comments_and_blocks_coexist_and_move_the_root() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        seed_page(&mut p, "p1").await;
        let before = p.root();
        apply_commit_as(&mut p, &add("t1", "m1", "b1", "note on b1"), user("alice")).await;
        assert_ne!(p.root(), before, "a comment write moves the root");
        // the block tree is untouched by the comment.
        assert_eq!(
            ids(&get_page(&p, "p1").await.unwrap()),
            ["p1", "b1", "b2", "b3"]
        );
        assert_eq!(query_target_threads(&p, "b1", 0, 0).await.threads.len(), 1);
    });
}

// deleting a block (or page) must purge the comment threads anchored to it,
// so no comment records dangle in the reserved keyspace forever.
#[test]
fn deleting_a_block_purges_its_comment_threads() {
    deterministic::Runner::default().start(|context| async move {
        let mut p = pages_on!(context, "pages");
        seed_page(&mut p, "p1").await; // p1 + b1,b2,b3
        apply_commit_as(&mut p, &add("t1", "m1", "b1", "on b1"), user("alice")).await;
        apply_commit_as(&mut p, &add("t2", "m2", "p1", "on the page"), user("alice")).await;
        assert_eq!(query_target_threads(&p, "b1", 0, 0).await.threads.len(), 1);

        // remove b1 → its thread t1 (+ comment m1 + target index) is gone.
        apply_commit(
            &mut p,
            &PageMsg::RemoveBlock {
                block_id: "b1".into(),
            },
        )
        .await;
        assert!(
            query_target_threads(&p, "b1", 0, 0)
                .await
                .threads
                .is_empty()
        );
        assert!(query_thread(&p, "t1").await.is_none());

        // delete the page → the page-anchored thread t2 is purged too.
        apply_commit(
            &mut p,
            &PageMsg::CreatePage {
                page_id: "p2".into(),
                title: "keep".into(),
            },
        )
        .await;
        apply_commit(
            &mut p,
            &PageMsg::RemoveBlock {
                block_id: "p1".into(),
            },
        )
        .await;
        assert!(query_thread(&p, "t2").await.is_none());
        assert!(
            query_target_threads(&p, "p1", 0, 0)
                .await
                .threads
                .is_empty()
        );
    });
}
