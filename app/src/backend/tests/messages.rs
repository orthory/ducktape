use super::*;

#[test]
fn a_dm_id_is_pair_derived_and_cannot_be_forged() {
    // the viewer's key, and a peer's ACCOUNT NUMBER as the directory keys it.
    let a = "aa".repeat(32);
    let b = "42".to_string();
    assert_eq!(
        dm_channel_id(a.clone(), b.clone()),
        dm_channel_id(b.clone(), a.clone()),
        "both sides derive the same channel"
    );
    // the sidebar's own filter: the pair's derived channel drops out of
    // CHANNELS, an ordinary room stays, and an unknown viewer filters
    // nothing rather than guessing.
    let channel = |id: &str| ChatChannel {
        id: id.into(),
        name: id.into(),
        archived: false,
        members_only: false,
        huddle_count: 0,
        head_seq: 0,
    };
    let peers = vec![DmPeer {
        key: b.clone(),
        name: "b".into(),
        initials: "B".into(),
        is_agent: false,
        channel_id: dm_channel_id(a.clone(), b.clone()),
    }];
    let listing = vec![
        channel(&dm_channel_id(a.clone(), b.clone())),
        channel("general"),
    ];
    let rooms = chat_sidebar_rooms(listing.clone(), peers.clone(), Vec::new());
    assert_eq!(rooms.len(), 1);
    assert_eq!(rooms[0].channel.id, "general");
    // AND AN EMPTY `channel_id` CLAIMS NOTHING. A peer row whose load resolved
    // no account number of ours carries none, and it must not swallow every
    // channel whose id happens to be empty — but the DM does NOT fall back
    // into the room list either: a derived two-party id is never a CHANNELS
    // row, whoever's it is (`another_members_dm_is_not_a_channel_of_mine`).
    let unresolved = vec![DmPeer {
        channel_id: String::new(),
        ..peers[0].clone()
    }];
    let without_the_directory = chat_sidebar_rooms(listing, unresolved, Vec::new());
    assert_eq!(
        without_the_directory
            .iter()
            .map(|row| row.channel.id.as_str())
            .collect::<Vec<_>>(),
        ["general"]
    );

    // the id the app mints is the id chat will accept from a USER author:
    // ':' is reserved for module origins and '/' is refused outright, so a
    // minted id carrying either is a DM that can never be created.
    // `chat::client`'s own test runs the id through that rule directly.
    let id = dm_channel_id(a, b);
    assert!(
        !id.contains(':'),
        "a user-authored channel id may not carry ':'"
    );
    assert!(!id.contains('/'), "a channel id may not carry '/'");
    assert!(id.starts_with("dm-") && id.len() == 67);
}

#[test]
fn the_post_gate_names_why_a_viewer_cannot_post() {
    let members = vec![ChatMember {
        key: "beef".into(),
        label: "b".into(),
    }];
    assert_eq!(post_gate(false, false, Vec::new(), "cafe".into()), "");
    assert_eq!(
        post_gate(true, false, members.clone(), "beef".into()),
        "channel_archived"
    );
    assert_eq!(
        post_gate(false, true, members.clone(), "cafe".into()),
        "members_only"
    );
    assert_eq!(post_gate(false, true, members.clone(), "beef".into()), "");

    // A seat is the ACCOUNT's: the viewer's passkey holds the seat, and her
    // device key is bound to the same account, so the device may post too.
    seed_names(NameDirectory::new(BTreeMap::from([
        (
            "beef".to_string(),
            BoundAccount {
                number: 7,
                name: "b".into(),
            },
        ),
        (
            "b00f".to_string(),
            BoundAccount {
                number: 7,
                name: "b".into(),
            },
        ),
        (
            "cafe".to_string(),
            BoundAccount {
                number: 8,
                name: "c".into(),
            },
        ),
    ])));
    assert_eq!(
        post_gate(false, true, members.clone(), "b00f".into()),
        "members_only",
        "a sibling key does not inherit a historical key seat"
    );
    let members = vec![ChatMember {
        key: "acct:7".into(),
        label: "b".into(),
    }];
    assert_eq!(post_gate(false, true, members.clone(), "b00f".into()), "");
    assert_eq!(
        post_gate(false, true, members, "cafe".into()),
        "members_only"
    );
    seed_names(NameDirectory::default());
}

/// A SEARCH HIT SAYS WHICH ROOM IT IS IN, ONCE. The hit's `meta` was
/// `#{seq}` — the message's sequence number, rendered exactly like a channel,
/// because every channel in this app is written `# General`. So a palette row
/// read `#1` and the reader could not tell whether that was a room, a position,
/// or which of four channels the message actually lived in.
///
/// Three surfaces render `hit.meta` — the palette, the chat sidebar and the
/// Explorer — and only the Explorer composed the channel in, at its own call
/// site. The room now lives in `meta` itself, so all three agree and the
/// Explorer stops composing (which would have printed it twice).
#[test]
fn a_search_hit_names_its_room_exactly_once() {
    const CHAT: &str = include_str!("../chat.rs");
    let hit = CHAT
        .split("ChatSearchHit {")
        .nth(1)
        .expect("the search hit mapping")
        .split("})")
        .next()
        .expect("mapping body");
    assert!(
        hit.contains(r#"meta: format!("{} · #{}", hit.channel_id, hit.seq)"#),
        "the room comes first, then the sequence"
    );

    const SEARCH: &str = include_str!("../search.rs");
    let message_arm = SEARCH
        .split("kind: \"message\".into(),")
        .nth(1)
        .expect("the message hit arm")
        .split("});")
        .next()
        .expect("arm body");
    assert!(
        message_arm.contains("meta: hit.meta,"),
        "the Explorer carries the meta through"
    );
    assert!(
        !message_arm.contains("hit.channel_id, hit.meta"),
        "composing the channel again is what printed it twice"
    );
}

/// AN UNREAD HEIGHT SAYS SO. The Node overview must not print `h 0` before a
/// status document lands — a measured zero for a chain sitting at ~398,000.
///
/// `height_label` already had the vocabulary: a negative height is `h —`. The
/// field simply defaulted to 0, which is a reading rather than the absence of
/// one, so the state default is now the sentinel the label understands.
#[test]
fn an_unread_block_height_is_not_reported_as_zero() {
    assert_eq!(height_label(-1), "h —", "the no-reading sentinel");
    assert_ne!(
        height_label(0),
        "h —",
        "zero is a real height and must keep reading as one"
    );

    // The state default is what Node shows before any node fact lands.
    // This is the RENDERER's contract and it is unchanged: `0` still reads as a
    // real height here. What changed is upstream — `served_height` decides that
    // a `0` on the wire was never a measurement, so no zero reaches this label
    // as a head. See `a_resyncing_replica_has_no_head_to_print_a_checkpoint_against`.
    const STATE: &str = include_str!("../../ui/state/node.ice");
    assert!(
        STATE.contains("node_height:i64 = -1"),
        "an unread height must default to the sentinel, not to a measured zero"
    );
}

/// A DISPLAY NAME MUST NOT BE FORMATTED TWICE. `search_chat` already runs the
/// wire author through `author_display`, so an Explorer hit arrives holding
/// "alice", "user 48cedb0d…" or "quackbot". The Explorer then ran `author_name`
/// over that a SECOND time; none of those strings carries a `user:`/`agent:`
/// prefix to split, so every one fell through to the `_` arm and every message
/// hit in workspace search was attributed to "system".
///
/// Driven: the same message reads `user 48cedb0d…` in the timeline and `system`
/// in Explorer search.
#[test]
fn a_search_hits_author_is_not_reformatted_into_system() {
    // What `search_chat` hands the Explorer, for each kind of author.
    for displayed in ["alice", "user 48cedb0d…", "quackbot", "chat"] {
        assert_eq!(
            author_name(displayed),
            "system",
            "a second pass over a display name loses it — this is why the hit \
             must carry `hit.author` through untouched"
        );
    }

    // And the first pass is the one that is correct.
    assert_eq!(
        author_display("user:48cedb0d131f", &NameDirectory::default()),
        "user 48cedb0d…"
    );
    let program = identity::AccountView {
        number: 7,
        name: "quackbot".into(),
        control: identity::Control::Program {
            controller: 1,
            executor: "agent".into(),
            generation: 0,
            standing: identity::ProgramStanding::Active,
        },
        keys: Vec::new(),
        avatar: None,
        bio: None,
        updated_at: 0,
    };
    assert_eq!(
        author_display("acct:7", &NameDirectory::from_accounts(&[program])),
        "quackbot"
    );

    // The call site itself, pinned: the message arm must carry the author
    // through, never re-format it. Without this the assertions above hold
    // while the Explorer goes on printing "system".
    const SEARCH: &str = include_str!("../search.rs");
    let message_arm = SEARCH
        .split("kind: \"message\".into(),")
        .nth(1)
        .expect("the message hit arm")
        .split("});")
        .next()
        .expect("arm body");
    assert!(
        message_arm.contains("title: hit.author,"),
        "the message hit carries the display name it was handed"
    );
    assert!(
        !message_arm.contains("author_name("),
        "re-formatting it is what produced `system`"
    );
}

#[test]
fn optimistic_reaction_survives_the_canonical_replay() {
    let mut message = optimistic_message(Vec::new(), "hello".into(), "message-a".into());
    message[0].pending = false;
    message[0].seq = 7;
    let reactor = "user:aa11".to_string();

    let tapped =
        ::chat::client::optimistic_reaction(message, 7, "👍".into(), true, reactor.clone());
    assert_eq!(tapped[0].reactions.len(), 1);
    assert_eq!(tapped[0].reactions[0].emoji, "👍");
    assert_eq!(tapped[0].reactions[0].count, 1);
    assert!(tapped[0].reactions[0].reacted_by_me);

    // The settled delta replays the SAME reactor handle — set semantics keep
    // the count at 1 instead of doubling the optimistic chip.
    let replayed = ::chat::client::merge_message_reaction(tapped, 7, "👍", true, &reactor, true);
    assert_eq!(replayed[0].reactions.len(), 1);
    assert_eq!(replayed[0].reactions[0].count, 1);
    assert!(replayed[0].reactions[0].reacted_by_me);

    // The optimistic remove folds the chip away entirely.
    let removed =
        ::chat::client::optimistic_reaction(replayed, 7, "👍".into(), false, "user:aa11".into());
    assert!(removed[0].reactions.is_empty());
}

#[test]
fn concurrent_optimistic_messages_settle_independently() {
    let pending = optimistic_message(
        optimistic_message(Vec::new(), "first".into(), "message-a".into()),
        "second".into(),
        "message-b".into(),
    );
    assert_eq!(
        pending
            .iter()
            .map(|message| message.id.as_str())
            .collect::<Vec<_>>(),
        ["message-a", "message-b"]
    );

    let canonical = |id: &str, seq: i64, body: &str| ChatMessage {
        id: id.into(),
        view_key: seq,
        seq,
        author: "You".into(),
        meta: format!("#{seq}"),
        body: body.into(),
        blocks: paragraph_blocks(body),
        pending: false,
        rev: 0,
        edited: false,
        deleted: false,
        reply_count: 0,
        thread_seq: 0,
        show_author: true,
        initial: "Y".into(),
        avatar_kind: "human".into(),
        height: 0,
        time: 0,
        reactions: Vec::new(),
        render_rev: 0,
    };
    let after_second = merge_message_send_result(
        vec![canonical("message-b", 1, "second")],
        pending,
        "general".into(),
        "general".into(),
    );
    assert_eq!(after_second.len(), 2);
    assert!(!after_second[0].pending);
    assert_eq!(after_second[1].id, "message-a");
    assert!(after_second[1].pending);

    let settled = merge_message_send_result(
        vec![
            canonical("message-b", 1, "second"),
            canonical("message-a", 2, "first"),
        ],
        after_second,
        "general".into(),
        "general".into(),
    );
    assert_eq!(
        settled
            .iter()
            .map(|message| message.id.as_str())
            .collect::<Vec<_>>(),
        ["message-b", "message-a"]
    );
    assert!(settled.iter().all(|message| !message.pending));

    let after_stale_response = merge_message_send_result(
        vec![canonical("message-b", 1, "second")],
        settled,
        "general".into(),
        "general".into(),
    );
    assert_eq!(
        after_stale_response
            .iter()
            .map(|message| message.id.as_str())
            .collect::<Vec<_>>(),
        ["message-b", "message-a"]
    );
}

#[test]
fn message_groups_collapse_consecutive_authors() {
    let msg = |seq: i64, author: &str, deleted: bool| ChatMessage {
        id: format!("m{seq}"),
        view_key: seq,
        seq,
        author: author.into(),
        meta: format!("#{seq}"),
        body: "body".into(),
        blocks: paragraph_blocks("body"),
        pending: false,
        rev: 0,
        edited: false,
        deleted,
        reply_count: 0,
        thread_seq: 0,
        show_author: false,
        initial: "A".into(),
        avatar_kind: "human".into(),
        height: 0,
        time: 0,
        reactions: Vec::new(),
        render_rev: 0,
    };
    let mut messages = vec![
        msg(1, "alice", false),
        msg(2, "alice", false),
        msg(3, "bob", false),
        msg(4, "bob", true),
        msg(5, "bob", false),
    ];
    mark_message_groups(&mut messages);
    let shown: Vec<bool> = messages.iter().map(|message| message.show_author).collect();
    // 1 opens the list; 2 shares alice -> continuation; 3 switches to bob -> header;
    // 4 is deleted -> header; 5 follows a deleted message -> header.
    assert_eq!(shown, vec![true, false, true, true, true]);
}

#[test]
fn history_pagination_prepends_older_and_flags_more() {
    let msg = |seq: i64| ChatMessage {
        id: format!("m{seq}"),
        view_key: seq,
        seq,
        author: "alice".into(),
        meta: format!("#{seq}"),
        body: "body".into(),
        blocks: paragraph_blocks("body"),
        pending: false,
        rev: 0,
        edited: false,
        deleted: false,
        reply_count: 0,
        thread_seq: 0,
        show_author: false,
        initial: "A".into(),
        avatar_kind: "human".into(),
        height: 0,
        time: 0,
        reactions: Vec::new(),
        render_rev: 0,
    };
    let loaded = vec![msg(3), msg(4), msg(5)];
    assert_eq!(oldest_message_seq(loaded.clone()), 3);
    // prepend an older page whose last item (seq 3) duplicates the current head.
    let merged = prepend_history(loaded, vec![msg(1), msg(2), msg(3)]);
    assert_eq!(
        merged.iter().map(|message| message.seq).collect::<Vec<_>>(),
        vec![1, 2, 3, 4, 5]
    );
    assert_eq!(oldest_message_seq(merged), 1);
}

/// The composer's grammar loop, closed over a real node: the SAME parser
/// the rich composer previews (`parse_message_with_mentions`) builds the
/// committed blocks, and the spans read back off the node still carry the
/// marks. If the preview grammar and the renderer grammar ever drift, one
/// of the two ends of this test moves.
#[tokio::test(flavor = "current_thread")]
async fn composer_markdown_round_trips_rich_spans() {
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
    let signer = ed25519::PrivateKey::from_seed(11);

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
            message_id: "styled-1".into(),
            blocks: parse_message_with_mentions(
                "say **hi** to _all_",
                &MentionCandidates::default(),
            ),
            thread: None,
        }),
    )
    .await;

    let chat = load_chat_data(&rpc, Some("general")).await.unwrap();
    let message = &chat.messages[0];
    let block = &message.blocks[0];
    assert!(block.rich, "marked text lands as a rich paragraph");
    assert!(
        block.spans.iter().any(|span| span.bold == "hi"),
        "the bold run survives the round trip"
    );
    assert!(
        block.spans.iter().any(|span| span.italic == "all"),
        "the italic run survives the round trip"
    );
    sim.shutdown();
}

#[tokio::test(flavor = "current_thread")]
async fn timeline_pages_past_thread_only_traffic() {
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
    let signer = ed25519::PrivateKey::from_seed(8);

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
            message_id: "root".into(),
            blocks: vec![chat::Block::paragraph("root stays visible")],
            thread: None,
        }),
    )
    .await;
    for index in 0_u64..257 {
        submit_test(
            &rpc,
            &signer,
            index + 3,
            "chat",
            chat::encode_msg(&ChatMsg::PostMessage {
                channel_id: "general".into(),
                message_id: format!("reply-{index}"),
                blocks: vec![chat::Block::paragraph(format!("reply {index}"))],
                thread: Some(1),
            }),
        )
        .await;
    }

    let chat = load_chat_data(&rpc, Some("general")).await.unwrap();
    assert_eq!(chat.messages.len(), 1);
    assert_eq!(chat.messages[0].body, "root stays visible");
    assert!(
        !chat.has_older_history,
        "257 replies after the only root do not consume a root page or invent history"
    );
    let first = load_thread_data(&rpc, "general", 1).await.unwrap();
    assert_eq!(first.messages.len(), 257);
    assert_eq!(first.next_reply_seq, 257);
    assert!(first.has_more);
    let last = load_thread_page(origin.clone(), "general".into(), 1, first.next_reply_seq, 9)
        .await
        .unwrap();
    assert_eq!(last.messages.len(), 1);
    assert_eq!(last.messages[0].body, "reply 256");
    assert_eq!(last.next_reply_seq, 0);
    assert!(!last.has_more);
    let sparse = load_thread(origin, "general".into(), 1, 258, 10)
        .await
        .unwrap();
    assert_eq!(sparse.target_seq, 258);
    assert_eq!(sparse.next_reply_seq, 0);
    assert_eq!(sparse.messages.len(), 2);
    assert_eq!(sparse.messages[1].body, "reply 256");
    sim.shutdown();
}

/// A cold start used to open on `channels.first()` — wire order is by ID, so
/// the demo workspace landed on an empty `channel-1786073…` and the console
/// said "No messages yet" with three populated rooms listed under it.
#[test]
fn a_cold_start_lands_on_a_room_with_something_in_it() {
    let channel = |id: &str, head: i64, archived: bool| ChatChannel {
        id: id.into(),
        name: id.into(),
        archived,
        members_only: false,
        huddle_count: 0,
        head_seq: head,
    };
    let landing = |channels: &[ChatChannel]| {
        landing_channel(channels)
            .map(|channel| channel.id.clone())
            .unwrap_or_default()
    };

    // The demo's own shape: the empty room sorts first by ID.
    let demo = vec![
        channel("channel-1786073", 0, false),
        channel("engineering", 46, false),
        channel("general", 9, false),
    ];
    assert_eq!(landing(&demo), "engineering");

    // An archived room is not a landing even when it is the only one with
    // traffic — you cannot post into it.
    let archived_history = vec![channel("archive", 500, true), channel("general", 0, false)];
    assert_eq!(landing(&archived_history), "general");

    // Every room empty, and every room archived: still land somewhere.
    assert_eq!(
        landing(&[channel("a", 0, false), channel("b", 0, false)]),
        "a"
    );
    assert_eq!(
        landing(&[channel("a", 0, true), channel("b", 5, true)]),
        "a"
    );
    assert_eq!(landing(&[]), "");

    // The chooser is only worth anything if the loader routes through it.
    const LOAD: &str = include_str!("../load.rs");
    assert!(
        LOAD.contains(".or_else(|| landing_channel(&channels).map(|channel| channel.id.clone()))"),
        "load_chat_data falls back through the chooser, not through .first()"
    );
}

#[test]
fn client_local_unread_tracking_seeds_marks_and_places_the_divider() {
    let channel = |id: &str, head: i64| ChatChannel {
        id: id.into(),
        name: id.into(),
        archived: false,
        members_only: false,
        huddle_count: 0,
        head_seq: head,
    };
    let read = |channel: &str, seq: i64| ChannelRead {
        channel: channel.into(),
        seq,
    };
    let message = |seq: i64, pending: bool| ChatMessage {
        id: format!("m{seq}"),
        view_key: if pending { -1 } else { seq },
        seq: if pending { -1 } else { seq },
        author: "u".into(),
        meta: String::new(),
        body: String::new(),
        blocks: Vec::new(),
        pending,
        rev: 0,
        edited: false,
        deleted: false,
        reply_count: 0,
        thread_seq: 0,
        show_author: true,
        initial: "U".into(),
        avatar_kind: "human".into(),
        height: 0,
        time: 0,
        reactions: Vec::new(),
        render_rev: 0,
    };

    let reads = vec![read("general", 100), read("random", 30)];
    let channels = vec![channel("general", 100), channel("random", 50)];

    // channel_last_read / channel_head_seq: lookup, 0 when absent.
    assert_eq!(channel_last_read(reads.clone(), "random".into()), 30);
    assert_eq!(channel_last_read(reads.clone(), "missing".into()), 0);
    assert_eq!(channel_head_seq(channels.clone(), "random".into()), 50);
    assert_eq!(channel_head_seq(channels.clone(), "missing".into()), 0);

    // mark_channel_read upserts to the max, adds absent, ignores empty id.
    let marked = mark_channel_read(reads.clone(), "random".into(), 50);
    assert_eq!(channel_last_read(marked.clone(), "random".into()), 50);
    let lowered = mark_channel_read(marked, "random".into(), 40);
    assert_eq!(channel_last_read(lowered, "random".into()), 50);
    let added = mark_channel_read(reads.clone(), "new".into(), 7);
    assert_eq!(channel_last_read(added, "new".into()), 7);
    assert_eq!(
        mark_channel_read(reads.clone(), String::new(), 9).len(),
        reads.len()
    );

    // Every prepared row carries its own unread scalar. Both sections resolve
    // it once when source state moves, never from a list-taking view call.
    let rooms = chat_sidebar_rooms(channels.clone(), Vec::new(), reads.clone());
    assert!(!rooms[0].unread);
    assert!(rooms[1].unread);
    let dm = DmPeer {
        key: "peer".into(),
        name: "Peer".into(),
        initials: "P".into(),
        is_agent: false,
        channel_id: "random".into(),
    };
    let dms = chat_sidebar_dms(channels.clone(), vec![dm], reads.clone());
    assert!(dms[0].unread);
    assert!(!chat_sidebar_rooms(vec![channel("random", 30)], Vec::new(), reads.clone())[0].unread);

    // initial_channel_reads: seed absent channels to head, preserve existing.
    let seeded = initial_channel_reads(channels.clone(), vec![read("random", 30)]);
    assert_eq!(channel_last_read(seeded.clone(), "random".into()), 30);
    assert_eq!(channel_last_read(seeded.clone(), "general".into()), 100);
    assert!(!chat_sidebar_rooms(vec![channel("general", 100)], Vec::new(), seeded)[0].unread);

    // first_unread_seq: first message past the boundary; pending (seq -1)
    // never anchors it; 0 when caught up.
    let messages = vec![
        message(31, false),
        message(40, false),
        message(50, false),
        message(0, true),
    ];
    assert_eq!(first_unread_seq(messages.clone(), 30), 31);
    assert_eq!(first_unread_seq(messages.clone(), 45), 50);
    assert_eq!(first_unread_seq(messages.clone(), 50), 0);
    assert_eq!(first_unread_seq(messages, 0), 0);

    // frozen_unread_boundary: same channel is left untouched; a change
    // re-freezes at the arrived channel's last-read, or 0 when caught up.
    assert_eq!(
        frozen_unread_boundary(
            reads.clone(),
            channels.clone(),
            "random".into(),
            "random".into(),
            30
        ),
        30
    );
    assert_eq!(
        frozen_unread_boundary(
            reads.clone(),
            channels.clone(),
            "general".into(),
            "random".into(),
            999
        ),
        30
    );
    let caught_up = vec![read("general", 100), read("random", 50)];
    assert_eq!(
        frozen_unread_boundary(caught_up, channels, "general".into(), "random".into(), 999),
        0
    );
}

/// The fairness cap counts websocket work, including chat ops that deliberately
/// fold to no UI delta (hook registration). Counting only `batch.chat.len()`
/// lets an always-ready invisible run monopolise one stream poll forever.
#[test]
fn the_live_chat_batch_budget_counts_invisible_frames() {
    const LIVE: &str = include_str!("../live.rs");
    let collector = LIVE
        .split_once("async fn collect_ready_chat_updates(")
        .expect("the ready-chat collector")
        .1
        .split_once("/// The complete chat-owned result")
        .expect("the collector body")
        .0;
    assert!(collector.contains("let mut consumed = batch.chat.len();"));
    assert!(collector.contains("while consumed < LIVE_CHAT_BATCH_LIMIT"));
    assert!(collector.contains("consumed += 1;"));
    assert!(
        !collector.contains("while batch.chat.len() < LIVE_CHAT_BATCH_LIMIT"),
        "invisible chat frames must consume the publication's fairness budget"
    );
    let outer = LIVE
        .split_once("let mut skipped_ready_frames = 0usize;")
        .expect("the outer stream fairness budget")
        .1
        .split_once("async fn collect_ready_chat_updates(")
        .expect("the ready-chat collector boundary")
        .0;
    assert!(outer.contains("skipped_ready_frames += 1;"));
    assert!(outer.contains("tokio::task::yield_now().await;"));
}

/// EVERY CHAT READ RIDES THE VIEW LANE. `load_channel_row` is awaited INSIDE
/// the live stream's decoder fold, so a `/v1/query` there hands the node's
/// single select loop the fold of every subscriber: a channel row cost up to
/// the node's whole checkpoint write (issue #1018). `ChatViewQuery::Channel`
/// returns the identical `ChannelInfo` off an MVCC snapshot, off-loop.
///
/// Pinned as a source shape for the same reason `connect`'s cause is: the
/// difference between the two lanes is which HTTP route the round trip takes,
/// and both answer the same rows against a live node — a behavioural assertion
/// cannot tell them apart.
#[test]
fn chat_reads_never_cross_the_dispatch_query_lane() {
    // `load_channel_row` is the fold's caller; `load_channel_facts` is the read
    // itself, shared with the channel-switch window loader.
    const LIVE: &str = include_str!("../live.rs");
    const LOAD: &str = include_str!("../load.rs");
    let load_channel_row = LIVE
        .split("pub(crate) async fn load_channel_row(")
        .nth(1)
        .expect("load_channel_row is declared")
        .split("\npub ")
        .next()
        .expect("load_channel_row body");
    assert!(
        load_channel_row.contains("load_channel_facts("),
        "the channel row goes through the shared index-view read"
    );
    let load_channel_facts = LOAD
        .split("pub(crate) async fn load_channel_facts(")
        .nth(1)
        .expect("load_channel_facts is declared")
        .split("\npub ")
        .next()
        .expect("load_channel_facts body");
    assert!(
        load_channel_facts.contains("ChatViewQuery::Channel {"),
        "the channel row reads the index view arm"
    );
    // AND NEITHER DOES THE DIRECTORY IT NAMES THE ROSTER WITH. The filling
    // read (`read_accounts`, behind `refresh_names`) is an identity
    // `/v1/query`, so reaching for it here would put the very round trip this
    // test bans back inside the fold — one indirection further away, where the
    // `.query(` sweep below cannot see it. The reader handed in carries the
    // directory as last read.
    for body in [load_channel_row, load_channel_facts] {
        assert!(
            !body.contains("read_accounts(") && !body.contains("refresh_names("),
            "the fold's channel read takes the directory it already has"
        );
    }
    for body in [load_channel_row, load_channel_facts] {
        assert!(
            !body.contains(".query("),
            "a chat read on /v1/query pays the node's checkpoint tax"
        );
    }

    // The whole crate, not just this function: `ChatQuery`/`ChatReply` are the
    // dispatch-lane types, and `backend/mod.rs` is the one `use` every backend
    // module inherits. An import reappearing IS a chat read crawling back onto
    // the select loop.
    // The `use` LINES, not the file: the comment above them names the banned
    // types to say why they are banned, and a sweep over raw source cannot
    // tell a symbol from the prose about it.
    const MOD: &str = include_str!("../mod.rs");
    let imports: Vec<&str> = MOD
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("use "))
        .collect();
    assert!(
        !imports
            .iter()
            .any(|line| line.contains("ChatQuery") || line.contains("ChatReply")),
        "no backend module may reach for chat's dispatch query type: {imports:?}"
    );
    assert!(
        imports.iter().any(|line| line.contains("ChatViewQuery")),
        "the view lane's types are the ones the backend imports"
    );
}

/// Timeline hydration and history paging each issue one root-index request.
/// This source shape is load-bearing: putting a loop back here turns a long
/// reply-only suffix into channel-open latency again even though rows render
/// in a bounded window.
#[test]
fn timeline_pages_are_one_root_view_call_without_a_message_walk() {
    const LOAD: &str = include_str!("../load.rs");
    let query_roots = LOAD
        .split("async fn query_roots(")
        .nth(1)
        .expect("query_roots is declared")
        .split("\npub(crate) async fn load_messages(")
        .next()
        .expect("query_roots body");
    assert_eq!(query_roots.matches(".view(").count(), 1);
    assert!(!query_roots.contains("loop {") && !query_roots.contains("while "));

    let load_messages = LOAD
        .split("pub(crate) async fn load_messages(")
        .nth(1)
        .expect("load_messages is declared")
        .split("\n}")
        .next()
        .expect("load_messages body");
    assert_eq!(load_messages.matches("query_roots(").count(), 1);
    assert!(!load_messages.contains(".view("));

    let load_older = LOAD
        .split("pub async fn load_older_messages(")
        .nth(1)
        .expect("load_older_messages is declared")
        .split("\npub(crate) async fn load_messages_around(")
        .next()
        .expect("load_older_messages body");
    assert_eq!(load_older.matches("query_roots(").count(), 1);
    assert!(!load_older.contains("loop {") && !load_older.contains("while "));

    assert!(!LOAD.contains("walk_roots_back"));
    assert!(!LOAD.contains("ChatViewQuery::MessagesLatest"));
    assert!(!LOAD.contains("ChatViewQuery::MessagesRange"));
}

/// A NAME REGISTERED ON A NETWORK IS THE NAME ITS MESSAGES CARRY.
///
/// A chat row stamps `user:{hex}` and nothing else — a key is what signed the
/// frame — while the name that key registered lives in the identity module. The
/// two were never joined: a freshly joined resident read a DM whose every
/// message was attributed to `user bf431c5d…`, with the same account rendered
/// "orthory" in the DIRECT list one pane to the left.
///
/// The directory is built from the identity roster `read_accounts` pages,
/// and EVERY key of an account answers to that account's name — a person with a
/// laptop and a phone signs with two keys and is one name in the timeline.
#[test]
fn every_key_of_an_account_renders_as_that_accounts_name() {
    let key = |byte: u8| identity::KeyView {
        scheme: identity::KeyScheme::Ed25519,
        pubkey: vec![byte; 32],
        label: None,
        added_at: 0,
    };
    let account = |number: u64, name: &str, keys: Vec<identity::KeyView>| identity::AccountView {
        number,
        name: name.into(),
        control: identity::Control::Keys,
        keys,
        avatar: None,
        bio: None,
        updated_at: 0,
    };
    let names = directory_of(&[
        account(1, "eddy", vec![key(0x56)]),
        // two devices, one person
        account(2, "orthory", vec![key(0x03), key(0xbf)]),
    ]);

    let handle = |byte: u8| format!("user:{}", hex_encode(&[byte; 32]));
    assert_eq!(author_display(&handle(0xbf), &names), "orthory");
    assert_eq!(
        author_display(&handle(0x03), &names),
        "orthory",
        "the second device is the same person, not a second one"
    );
    assert_eq!(author_display(&handle(0x56), &names), "eddy");
    // A key on no account is still honestly its short hex; nothing is invented.
    assert_eq!(
        author_display(&handle(0x11), &names),
        format!("user {}", short_label(&hex_encode(&[0x11u8; 32])))
    );
    // And a cold directory (a resident whose identity module cannot answer yet)
    // degrades to exactly that, for everyone.
    assert!(directory_of(&[]).is_empty());
}

// ============================================================================
// THE COPY RANGE. Every decision the two handler bodies apply is here, so this
// is where the feature is actually pinned: which rows a range covers, what
// comes out of it, and where a press leaves it.
// ============================================================================

fn message(seq: i64, author: &str, body: &str) -> ChatMessage {
    ChatMessage {
        id: format!("m{seq}"),
        view_key: seq,
        seq,
        author: author.into(),
        meta: String::new(),
        body: body.into(),
        blocks: Vec::new(),
        pending: false,
        rev: 0,
        edited: false,
        deleted: false,
        reply_count: 0,
        thread_seq: 0,
        show_author: true,
        initial: author[..1].to_uppercase(),
        avatar_kind: "person".into(),
        height: 0,
        time: 0,
        reactions: Vec::new(),
        render_rev: 0,
    }
}

fn room() -> Vec<ChatMessage> {
    vec![
        message(1, "ana", "first"),
        message(2, "bo", "second"),
        message(3, "ana", "third"),
        message(4, "bo", "fourth"),
    ]
}

/// A RANGE IS ITS TWO ENDS, IN EITHER ORDER. Dragging up a channel is as
/// ordinary as dragging down it, and the reader's anchor is as often the newest
/// row as the oldest — so `anchor` is not required to be the smaller seq.
#[test]
fn a_copy_range_covers_its_ends_whichever_way_round_they_are() {
    let rows = room();
    assert_eq!(copy_range_count(&rows, 2, 3), 2, "downwards");
    assert_eq!(copy_range_count(&rows, 3, 2), 2, "and upwards");
    assert_eq!(copy_range_count(&rows, 2, 2), 1, "one message is a range");
    assert_eq!(copy_range_count(&rows, 0, 0), 0, "and none is not");
    // An end that is no longer on screen is not an end. A range whose anchor
    // was deleted or paged out covers nothing rather than silently widening to
    // whatever is left.
    assert_eq!(copy_range_count(&rows, 9, 9), 0, "an end nobody holds");
}

/// WHAT COMES OUT IS WHAT YOU COULD READ. Oldest first regardless of which end
/// was clicked, one entry per message, blank-line separated so a multi-line
/// body survives the paste.
#[test]
fn the_copied_text_reads_in_timeline_order() {
    let rows = room();
    assert_eq!(
        copy_range_text(&rows, 3, 1),
        "ana: first\n\nbo: second\n\nana: third",
        "clicked bottom-up, pasted top-down"
    );
    assert_eq!(copy_range_text(&rows, 2, 2), "bo: second");
}

/// A TOMBSTONE IS NOT A LINE. A deleted row inside the range contributes
/// nothing — there is no body to lift, and a placeholder would be a line the
/// reader never wrote. The count follows the text, so the toast cannot claim
/// more than reached the clipboard.
#[test]
fn a_deleted_row_inside_the_range_contributes_nothing() {
    let mut rows = room();
    rows[1].deleted = true;
    rows[1].body = String::new();
    assert_eq!(copy_range_text(&rows, 1, 3), "ana: first\n\nana: third");
    assert_eq!(copy_range_toast(&rows, 1, 1), "Message copied");
}

/// SHIFT KEEPS THE ANCHOR, A PLAIN CLICK MOVES IT. This is the whole gesture.
#[test]
fn shift_extends_and_a_plain_click_starts_over() {
    use crate::CopySurface::{Nowhere, Thread, Timeline};
    let started = copy_range_after_press(0, Nowhere, 2, Timeline, false);
    assert_eq!(
        (started.anchor, started.head),
        (2, 2),
        "a click is a range of one"
    );

    let widened = copy_range_after_press(2, Timeline, 5, Timeline, true);
    assert_eq!(
        (widened.anchor, widened.head),
        (2, 5),
        "⇧ moves the far end"
    );

    let restarted = copy_range_after_press(2, Timeline, 5, Timeline, false);
    assert_eq!(
        (restarted.anchor, restarted.head),
        (5, 5),
        "no ⇧ starts over"
    );

    // ⇧ with nothing open is a plain click: there is no anchor to keep.
    let nothing_to_extend = copy_range_after_press(0, Nowhere, 5, Timeline, true);
    assert_eq!((nothing_to_extend.anchor, nothing_to_extend.head), (5, 5));

    // AND A RANGE NEVER SPANS THE TWO SURFACES. A ⇧-click in the rail while a
    // range is open in the stream starts a fresh one in the rail, because the
    // rows between them are not a run of anything.
    let crossed = copy_range_after_press(2, Timeline, 7, Thread, true);
    assert_eq!((crossed.anchor, crossed.head), (7, 7));
    assert_eq!(crossed.surface, Thread);
}

/// A ROW LIGHTS UP ONLY FOR A RANGE DRAWN WHERE IT LIVES. A reply and a
/// timeline row draw their seqs from the SAME channel sequence, so without the
/// surface a reply would tint inside a range whose copy never included it.
#[test]
fn the_surface_keeps_a_reply_out_of_the_streams_range() {
    use crate::CopySurface::{Thread, Timeline};
    assert!(seq_in_copy_range(3, 2, 5, Timeline, Timeline));
    assert!(
        !seq_in_copy_range(3, 2, 5, Timeline, Thread),
        "a reply in the rail"
    );
    assert!(
        !seq_in_copy_range(6, 2, 5, Timeline, Timeline),
        "past the end"
    );
    assert!(
        !seq_in_copy_range(3, 0, 0, Timeline, Timeline),
        "no range at all"
    );
}

/// THE PLATE IS ONE ANSWER, AND IT IS ORDERED. The row you are ON outranks a
/// row that merely sits in a range; a deleted row wears neither, because a
/// tint would say there is something there to lift.
#[test]
fn the_row_plate_ranks_selection_over_range_and_skips_a_tombstone() {
    use crate::RowPlate::{Plain, Ranged, Selected};
    assert_eq!(message_plate(false, false, false), Plain);
    assert_eq!(message_plate(false, false, true), Ranged);
    assert_eq!(message_plate(false, true, true), Selected);
    assert_eq!(
        message_plate(true, true, true),
        Plain,
        "a tombstone tints for nothing"
    );
}

/// THE CHORD LIFTS THE ROWS THE BAR COUNTED. The surface picks the list, so
/// ⌘C in a rail-drawn range never reaches into the stream behind it.
#[test]
fn the_surface_picks_the_list_the_copy_reads() {
    use crate::CopySurface::{Nowhere, Thread, Timeline};
    let timeline = room();
    let thread = vec![message(7, "cy", "a reply")];
    assert_eq!(copy_range_rows(&timeline, &thread, Timeline).len(), 4);
    assert_eq!(copy_range_rows(&timeline, &thread, Thread).len(), 1);
    assert!(copy_range_rows(&timeline, &thread, Nowhere).is_empty());
}

/// A PENDING ROW IS NOT AN END OF ANYTHING. A message still in flight carries a
/// negative seq, and a range with one at either end covers no rows: the bar
/// holding the only Clear button would vanish while the ⌘C route, armed on the
/// anchor, stayed armed with nothing able to disarm it. Pressing one ends the
/// range instead of opening an unclearable one.
#[test]
fn a_press_on_a_pending_row_ends_the_range_rather_than_arming_a_dead_one() {
    use crate::CopySurface::{Nowhere, Timeline};
    let cleared = copy_range_after_press(0, Nowhere, -1, Timeline, false);
    assert_eq!(
        (cleared.anchor, cleared.head),
        (0, 0),
        "a plain click on one"
    );
    assert_eq!(cleared.surface, Nowhere);

    let dropped = copy_range_after_press(2, Timeline, -3, Timeline, true);
    assert_eq!(
        (dropped.anchor, dropped.head),
        (0, 0),
        "and ⇧-clicking one drops the range it would otherwise have widened"
    );
    assert_eq!(dropped.surface, Nowhere);
    assert_eq!(
        copy_range_count(&room(), dropped.anchor, dropped.head),
        0,
        "which is the count the bar was already showing"
    );
}
