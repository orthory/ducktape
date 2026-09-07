//! the STORE-BACKED cutover-continuity proof for chat: the chat guest
//! component over `WasmModule::with_store(QmdbStore)` and the native `Chat`
//! over the same store shape are ROOT-CONTINUOUS — the same op sequence
//! commits the IDENTICAL qmdb merkle root after every block (both roots ARE
//! the store's root; qmdb's batch canonicalizes mutations by hashed key, so
//! the native logical-key commit order and the wasm hashed-key drain order
//! produce the same op log), including the byte-identical NO-OP blocks the
//! idempotent reaction ops rely on. this cutover changes the executor, not one
//! committed byte. hook fan-out
//! (`emit-msg` follow-ups) and `RegisterHook`'s registry check — a sibling
//! `module-root` read resolved by the runtime's memoized replay — are pinned
//! against a shared sink module.

use attribution::AttributionModule;
use chat::{
    Block, Chat, ChatMsg, ChatQuery, ChatReply, HUDDLE_JOIN_NS, Mark, MessageHead, Party,
    PostPolicy, Span, decode_reply, encode_msg, encode_query, huddle_join_preimage,
};
use commonware_cryptography::{Signer as _, ed25519};
use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use host::{BlockContext, Host, MemberOutcome, SubmitError};
use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle};
use sha2::Digest as _;
use statesync::qmdb::{QmdbStore, QmdbSyncReq, encode_qmdb_req};
use wasm_host::WasmModule;

/// GENERATED artifact — built from the module crate's guest port by
/// guest-builder (`make wasm-modules`); committed so this proof is self-contained.
const CHAT_WASM: &[u8] = include_bytes!("fixtures/chat.component.wasm");

fn mixed_mentions(first: Party, second: Party, direct: u64, text: &str) -> Vec<Block> {
    vec![
        Block::Paragraph(vec![
            Span::plain(text),
            Span {
                text: " first #topic".into(),
                marks: vec![
                    Mark::Bold,
                    Mark::Mention(first.clone()),
                    Mark::Mention(Party::Account(direct)),
                ],
            },
        ]),
        Block::Quote(vec![Span {
            text: " repeated and second".into(),
            marks: vec![Mark::Mention(first), Mark::Italic, Mark::Mention(second)],
        }]),
        Block::Code {
            lang: Some("text".into()),
            text: "unchanged code".into(),
        },
    ]
}

/// Fill the canonical serialized head exactly to its existing consensus cap.
fn fill_message_head(head: &mut MessageHead) -> String {
    let available = chat::MAX_MESSAGE_HEAD_BYTES - sdk::wire::encode(head).len();
    let text = "x".repeat(available);
    let Block::Paragraph(spans) = &mut head.blocks[0] else {
        panic!("paragraph")
    };
    spans[0].text = text.clone();
    assert_eq!(sdk::wire::encode(head).len(), chat::MAX_MESSAGE_HEAD_BYTES);
    text
}

async fn exercise_message_capacity(host: &mut Host) -> Vec<host::DispatchRecord> {
    let keys: Vec<_> = (1..=3)
        .map(|seed| {
            ed25519::PrivateKey::from_seed(seed)
                .public_key()
                .as_ref()
                .to_vec()
        })
        .collect();
    for (index, key) in keys.iter().enumerate() {
        host.submit_at(
            block(index as u64 + 1, key),
            Msg {
                target: "identity".into(),
                payload: identity::encode_msg(&identity::IdentityMsg::Create {
                    name: format!("account-{index}"),
                    scheme: identity::KeyScheme::Ed25519,
                }),
            },
        )
        .await
        .unwrap();
    }
    let channel = host
        .submit_at(
            block(4, &keys[0]),
            op(&ChatMsg::CreateChannel {
                channel_id: "capacity".into(),
                name: "Capacity".into(),
                post_policy: PostPolicy::Open,
            }),
        )
        .await
        .unwrap();
    let mut indexed = std::collections::BTreeMap::new();
    let mut trace = channel.dispatches;
    for dispatch in trace.iter().filter(|dispatch| dispatch.module == "chat") {
        let row = index_guest::OpRow {
            height: 4,
            seq: 0,
            time: 1_004,
            origin: index_guest::OriginTag::system(),
            payload: dispatch.payload.clone(),
            assigned: dispatch.assigned.clone(),
        };
        let writes = chat::index::fold_op(&row, &indexed).unwrap();
        index_guest::apply_to_map(&mut indexed, writes);
    }
    let mut posted = MessageHead {
        message_id: "long".into(),
        author: Party::Account(1),
        origin: Origin::External(keys[0].clone()),
        content_origin: Origin::External(keys[0].clone()),
        blocks: mixed_mentions(Party::Account(2), Party::Account(3), 2, ""),
        created_at: 1_005,
        rev: 0,
        revision: 1,
        edited_at: None,
        base_rev: None,
        deleted: false,
        thread: None,
        reply_count: 0,
        last_reply_seq: None,
    };
    let post_text = fill_message_head(&mut posted);
    let post = ChatMsg::PostMessage {
        channel_id: "capacity".into(),
        message_id: "long".into(),
        thread: None,
        blocks: mixed_mentions(
            Party::Key(keys[1].clone()),
            Party::Key(keys[2].clone()),
            2,
            &post_text,
        ),
    };
    let mut edited = MessageHead {
        blocks: mixed_mentions(Party::Account(3), Party::Account(2), 1, ""),
        rev: 1,
        revision: 2,
        edited_at: Some(1_006),
        base_rev: Some(0),
        ..posted.clone()
    };
    let edit_text = fill_message_head(&mut edited);
    let edit = ChatMsg::EditMessage {
        channel_id: "capacity".into(),
        seq: 1,
        base_rev: Some(0),
        blocks: mixed_mentions(
            Party::Key(keys[2].clone()),
            Party::Key(keys[1].clone()),
            1,
            &edit_text,
        ),
    };
    for (height, input, expected, accounts) in [
        (5, post, posted, vec![2, 3]),
        (6, edit, edited.clone(), vec![3, 2]),
    ] {
        let outcome = host
            .submit_at(block(height, &keys[0]), op(&input))
            .await
            .unwrap();
        let dispatch = outcome
            .dispatches
            .iter()
            .find(|dispatch| dispatch.module == "chat")
            .unwrap();
        assert!(dispatch.assigned.len() < sdk::MAX_ASSIGNED_BYTES);
        let assigned = chat::decode_assigned(&dispatch.assigned).unwrap();
        let key_mentions = match &assigned {
            chat::ChatAssigned::Posted { key_mentions, .. }
            | chat::ChatAssigned::Edited { key_mentions, .. } => key_mentions,
            other => panic!("unexpected stamp {other:?}"),
        };
        assert_eq!(
            key_mentions, &accounts,
            "repeated keys consume one assignment"
        );
        assert_eq!(assigned.actor(), &Party::Account(1));
        let row = index_guest::OpRow {
            height,
            seq: 0,
            time: 1_000 + height,
            origin: index_guest::OriginTag::system(),
            payload: dispatch.payload.clone(),
            assigned: dispatch.assigned.clone(),
        };
        let writes = chat::index::fold_op(&row, &indexed).unwrap();
        index_guest::apply_to_map(&mut indexed, writes);
        let request = serde_json::to_vec(&chat::index::ChatViewQuery::Message {
            message_id: "long".into(),
        })
        .unwrap();
        let indexed_bytes = chat::index::serve_view(&indexed, &request).unwrap();
        let chat::index::ChatViewReply::Message(Some(projected)) =
            serde_json::from_slice(&indexed_bytes).unwrap()
        else {
            panic!("indexed message")
        };
        let bytes = host
            .query(
                "chat",
                &encode_query(&ChatQuery::Message {
                    message_id: "long".into(),
                }),
            )
            .await
            .unwrap();
        let ChatReply::Message(Some(canonical)) = decode_reply(&bytes).unwrap() else {
            panic!("canonical message")
        };
        assert_eq!(canonical.head, expected);
        assert_eq!(projected.blocks, canonical.head.blocks);
        assert_eq!(projected.author, "acct:1");
        let stamp = serde_json::to_value(&assigned).unwrap();
        let delta = chat::client::delta_from_op(
            &dispatch.payload,
            Some(&stamp),
            "system",
            None,
            chat::client::ChatReader::nobody(),
            height,
        )
        .unwrap()
        .unwrap();
        let message = match delta {
            chat::client::ChatDelta::Posted { message, .. }
            | chat::client::ChatDelta::Edited { message, .. } => message,
            other => panic!("unexpected delta {other:?}"),
        };
        let hydrated = chat::client::chat_message(projected, chat::client::ChatReader::nobody());
        assert_eq!(message.blocks, hydrated.blocks);
        assert_eq!(message.body, hydrated.body);
        trace.extend(outcome.dispatches);
    }
    let before = host.root_hash();
    let mut oversized = edited.blocks;
    let Block::Paragraph(spans) = &mut oversized[0] else {
        panic!("paragraph")
    };
    spans[0].text.push('x');
    let rejected = host
        .submit_at(
            block(7, &keys[0]),
            op(&ChatMsg::EditMessage {
                channel_id: "capacity".into(),
                seq: 1,
                blocks: oversized,
                base_rev: Some(1),
            }),
        )
        .await;
    assert!(matches!(rejected, Err(SubmitError::Rejected(_))));
    assert_eq!(
        host.root_hash(),
        before,
        "the existing 64 KiB bound still rejects atomically"
    );
    trace
}

#[test]
fn native_full_message_capacity_keeps_compact_stamps_and_canonical_mentions() {
    deterministic::Runner::default().start(|context| async move {
        exercise_message_capacity(&mut native_host(&context).await).await;
    });
}

#[test]
fn wasm_full_message_capacity_and_mixed_mention_index_match_native() {
    deterministic::Runner::default().start(|context| async move {
        let mut native = native_host(&context).await;
        let mut wasm = wasm_host_(&context).await;
        let native_trace = exercise_message_capacity(&mut native).await;
        let wasm_trace = exercise_message_capacity(&mut wasm).await;
        assert_eq!(native_trace, wasm_trace);
        assert_eq!(native.module_roots(), wasm.module_roots());
    });
}

const FANOUT_FIRST_ACCOUNT: u64 = 1_000;
const FANOUT_ACCOUNT_COUNT: u64 = 900;

fn fanout_key(number: u64) -> Vec<u8> {
    ed25519::PrivateKey::from_seed(number)
        .public_key()
        .as_ref()
        .to_vec()
}

async fn fanout_identity_store(
    context: &deterministic::Context,
) -> QmdbStore<deterministic::Context> {
    use sdk::MerkleStore as _;
    let mut store = QmdbStore::init(context.child("fanout_identity"), "fanout_identity").await;
    store
        .commit_batch(vec![(
            sdk::store_key(sdk::genesis_config::CONFIG_KEY),
            Some(sdk::genesis_config::encode_config(&[(
                "chain_id", b"fanout",
            )])),
        )])
        .await
        .unwrap();
    let mut module = identity::Identity::new("identity", Box::new(store), "fanout".into());
    for number in 1..FANOUT_FIRST_ACCOUNT + FANOUT_ACCOUNT_COUNT {
        let mut ctx = sdk_testkit::TestCtx::with_env(sdk::Env {
            height: 0,
            consensus_time: 0,
            origin: Origin::External(fanout_key(number)),
            me: "identity".into(),
            cause: sdk::Cause::Direct,
        });
        module
            .execute(
                &mut ctx,
                &Msg {
                    target: "identity".into(),
                    payload: identity::encode_msg(&identity::IdentityMsg::Create {
                        name: format!("person-{number}"),
                        scheme: identity::KeyScheme::Ed25519,
                    }),
                },
            )
            .await
            .unwrap();
    }
    module.commit_block().await.unwrap();
    drop(module);
    QmdbStore::init(context.child("fanout_identity"), "fanout_identity").await
}

async fn fanout_native_host(context: &deterministic::Context) -> Host {
    let mut host = native_host(context).await;
    host.register(Box::new(identity::Identity::new(
        "identity",
        Box::new(fanout_identity_store(context).await),
        "fanout".into(),
    )));
    host
}

async fn fanout_wasm_host(context: &deterministic::Context) -> Host {
    let mut host = wasm_host_(context).await;
    host.register(Box::new(
        WasmModule::with_store(
            "identity",
            include_bytes!("fixtures/identity.component.wasm"),
            Box::new(fanout_identity_store(context).await),
        )
        .unwrap(),
    ));
    host
}

fn fold_chat_dispatches(
    indexed: &mut std::collections::BTreeMap<Vec<u8>, Vec<u8>>,
    dispatches: &[host::DispatchRecord],
    height: u64,
) {
    for dispatch in dispatches
        .iter()
        .filter(|dispatch| dispatch.module == "chat")
    {
        let row = index_guest::OpRow {
            height,
            seq: 0,
            time: 1_000 + height,
            origin: index_guest::OriginTag::system(),
            payload: dispatch.payload.clone(),
            assigned: dispatch.assigned.clone(),
        };
        let writes = chat::index::fold_op(&row, indexed).unwrap();
        index_guest::apply_to_map(indexed, writes);
    }
}

enum FanoutWrite {
    Post,
    Edit,
}

enum FanoutReferences {
    Keys,
    Accounts,
}

async fn exercise_key_mention_fanout(
    host: &mut Host,
    write: FanoutWrite,
    references: FanoutReferences,
) {
    let signer = fanout_key(1);
    let created = host
        .submit_at(
            block(1, &signer),
            op(&ChatMsg::CreateChannel {
                channel_id: "fanout".into(),
                name: "Fanout".into(),
                post_policy: PostPolicy::Open,
            }),
        )
        .await
        .unwrap();
    let mut indexed = std::collections::BTreeMap::new();
    fold_chat_dispatches(&mut indexed, &created.dispatches, 1);
    let accounts: Vec<_> = (FANOUT_FIRST_ACCOUNT..FANOUT_FIRST_ACCOUNT + FANOUT_ACCOUNT_COUNT)
        .rev()
        .collect();
    let blocks = |parties: Vec<Party>| {
        vec![Block::Paragraph(vec![Span {
            text: "mention fanout".into(),
            marks: parties.into_iter().map(Mark::Mention).collect(),
        }])]
    };
    let normalized = blocks(accounts.iter().copied().map(Party::Account).collect());
    let (input, key_mentions) = match references {
        FanoutReferences::Keys => (
            blocks(
                accounts
                    .iter()
                    .map(|number| Party::Key(fanout_key(*number)))
                    .collect(),
            ),
            accounts.clone(),
        ),
        FanoutReferences::Accounts => (normalized.clone(), Vec::new()),
    };
    let mut expected = MessageHead {
        message_id: "fanout-message".into(),
        author: Party::Account(1),
        origin: Origin::External(signer.clone()),
        content_origin: Origin::External(signer.clone()),
        blocks: normalized,
        created_at: 1_002,
        rev: 0,
        revision: 1,
        edited_at: None,
        base_rev: None,
        deleted: false,
        thread: None,
        reply_count: 0,
        last_reply_seq: None,
    };
    let (height, msg, assigned) = match write {
        FanoutWrite::Post => (
            2,
            ChatMsg::PostMessage {
                channel_id: "fanout".into(),
                message_id: "fanout-message".into(),
                blocks: input,
                thread: None,
            },
            chat::ChatAssigned::Posted {
                seq: 1,
                actor: Party::Account(1),
                key_mentions,
            },
        ),
        FanoutWrite::Edit => {
            let baseline = host
                .submit_at(
                    block(2, &signer),
                    op(&post("fanout", "fanout-message", "before", None)),
                )
                .await
                .unwrap();
            fold_chat_dispatches(&mut indexed, &baseline.dispatches, 2);
            expected.rev = 1;
            expected.revision = 2;
            expected.edited_at = Some(1_003);
            expected.base_rev = Some(0);
            (
                3,
                ChatMsg::EditMessage {
                    channel_id: "fanout".into(),
                    seq: 1,
                    blocks: input,
                    base_rev: Some(0),
                },
                chat::ChatAssigned::Edited {
                    rev: 1,
                    actor: Party::Account(1),
                    key_mentions,
                },
            )
        }
    };
    let head_bytes = sdk::wire::encode(&expected).len();
    let payload_bytes = encode_msg(&msg).len();
    let assigned_bytes = chat::encode_assigned(&assigned).len();
    assert!(head_bytes <= chat::MAX_MESSAGE_HEAD_BYTES);
    // Below 1 MiB, leaving the signed node frame's 16 KiB envelope reserve.
    assert!(payload_bytes < 1 << 20);
    match references {
        FanoutReferences::Keys => assert!(assigned_bytes > 4 * 1024),
        FanoutReferences::Accounts => assert!(assigned_bytes < 4 * 1024),
    }
    let result = host.submit_at(block(height, &signer), op(&msg)).await;
    let outcome = result.unwrap_or_else(|error| {
        panic!(
            "{head_bytes}-byte valid head, {payload_bytes}-byte payload, \
             {assigned_bytes}-byte metadata rejected: {error:?}"
        )
    });
    let dispatch = outcome
        .dispatches
        .iter()
        .find(|dispatch| dispatch.module == "chat")
        .unwrap();
    assert_eq!(dispatch.assigned, chat::encode_assigned(&assigned));
    let bytes = host
        .query(
            "chat",
            &encode_query(&ChatQuery::Message {
                message_id: "fanout-message".into(),
            }),
        )
        .await
        .unwrap();
    let ChatReply::Message(Some(canonical)) = decode_reply(&bytes).unwrap() else {
        panic!("canonical fanout message")
    };
    assert_eq!(canonical.head, expected);
    fold_chat_dispatches(&mut indexed, &outcome.dispatches, height);
    let bytes = chat::index::serve_view(
        &indexed,
        &serde_json::to_vec(&chat::index::ChatViewQuery::Message {
            message_id: "fanout-message".into(),
        })
        .unwrap(),
    )
    .unwrap();
    let chat::index::ChatViewReply::Message(Some(projected)) =
        serde_json::from_slice(&bytes).unwrap()
    else {
        panic!("indexed fanout message")
    };
    assert_eq!(projected.blocks, canonical.head.blocks);
    let stamp = serde_json::to_value(&assigned).unwrap();
    let delta = chat::client::delta_from_op(
        &dispatch.payload,
        Some(&stamp),
        "system",
        None,
        chat::client::ChatReader::nobody(),
        height,
    )
    .unwrap()
    .unwrap();
    let message = match delta {
        chat::client::ChatDelta::Posted { message, .. }
        | chat::client::ChatDelta::Edited { message, .. } => message,
        other => panic!("unexpected delta {other:?}"),
    };
    let hydrated = chat::client::chat_message(projected, chat::client::ChatReader::nobody());
    assert_eq!(message.blocks, hydrated.blocks);
    assert_eq!(message.body, hydrated.body);

    // A missing reference after one full batch still refuses the first
    // missing party in payload order, before a later invalid party. A failed
    // replacement leaves both source content and attribution unchanged.
    let mut invalid = accounts
        .iter()
        .take(identity::MAX_QUERY_LIMIT as usize)
        .copied()
        .map(Party::Account)
        .collect::<Vec<_>>();
    invalid.extend([
        Party::Account(9_999),
        Party::Module("invalid-person".into()),
    ]);
    let settled = host.module_roots();
    let error = host
        .submit_at(
            block(height + 1, &signer),
            op(&ChatMsg::EditMessage {
                channel_id: "fanout".into(),
                seq: 1,
                blocks: blocks(invalid),
                base_rev: Some(expected.rev),
            }),
        )
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("a mention names no account: 9999"),
        "{error}"
    );
    assert_eq!(host.module_roots(), settled);
}

#[test]
fn native_many_distinct_key_mentions_post_fit_message_capacity() {
    deterministic::Runner::default().start(|context| async move {
        exercise_key_mention_fanout(
            &mut fanout_native_host(&context).await,
            FanoutWrite::Post,
            FanoutReferences::Keys,
        )
        .await;
    });
}

#[test]
fn native_many_distinct_key_mentions_edit_fit_message_capacity() {
    deterministic::Runner::default().start(|context| async move {
        exercise_key_mention_fanout(
            &mut fanout_native_host(&context).await,
            FanoutWrite::Edit,
            FanoutReferences::Keys,
        )
        .await;
    });
}

#[test]
fn wasm_many_distinct_key_mentions_post_fit_message_capacity() {
    deterministic::Runner::default().start(|context| async move {
        exercise_key_mention_fanout(
            &mut fanout_wasm_host(&context).await,
            FanoutWrite::Post,
            FanoutReferences::Keys,
        )
        .await;
    });
}

#[test]
fn wasm_many_distinct_key_mentions_edit_fit_message_capacity() {
    deterministic::Runner::default().start(|context| async move {
        exercise_key_mention_fanout(
            &mut fanout_wasm_host(&context).await,
            FanoutWrite::Edit,
            FanoutReferences::Keys,
        )
        .await;
    });
}

#[test]
fn native_many_account_mentions_fit_message_capacity() {
    deterministic::Runner::default().start(|context| async move {
        exercise_key_mention_fanout(
            &mut fanout_native_host(&context).await,
            FanoutWrite::Post,
            FanoutReferences::Accounts,
        )
        .await;
    });
}

#[test]
fn wasm_many_account_mentions_fit_message_capacity() {
    deterministic::Runner::default().start(|context| async move {
        exercise_key_mention_fanout(
            &mut fanout_wasm_host(&context).await,
            FanoutWrite::Post,
            FanoutReferences::Accounts,
        )
        .await;
    });
}

/// a 32-byte submitter key (the ordered lane hands modules verified ed25519
/// ids; the parity claim only needs them distinct and non-empty).
fn key(tag: u8) -> Vec<u8> {
    vec![tag; 32]
}

/// a real `JoinHuddle` for `user_bytes`, naming `node` and carrying its proof
/// of possession — the shape `stage_join_huddle` now requires past the
/// node-length gate.
fn join_huddle(channel_id: &str, user_bytes: &[u8], node: &ed25519::PrivateKey) -> ChatMsg {
    let preimage = huddle_join_preimage(channel_id, user_bytes);
    ChatMsg::JoinHuddle {
        channel_id: channel_id.into(),
        node: node.public_key().as_ref().to_vec(),
        node_proof: node.sign(HUDDLE_JOIN_NS, &preimage).as_ref().to_vec(),
    }
}

fn op(m: &ChatMsg) -> Msg {
    Msg {
        target: "chat".into(),
        payload: encode_msg(m),
    }
}

/// one block's agreed context: both runtimes must see the identical env.
fn block(height: u64, who: &[u8]) -> BlockContext {
    block_as(height, Origin::External(who.to_vec()))
}

/// the same, for the origin KINDS an external key cannot express — a
/// module-minted (and therefore UNOWNED) channel is one of them.
fn block_as(height: u64, origin: Origin) -> BlockContext {
    BlockContext {
        height,
        consensus_time: 1_000 + height,
        origin,
    }
}

fn post(channel: &str, id: &str, text: &str, thread: Option<u64>) -> ChatMsg {
    ChatMsg::PostMessage {
        channel_id: channel.into(),
        message_id: id.into(),
        blocks: vec![Block::paragraph(text)],
        thread,
    }
}

/// a hook sink: swallows the `ChatEvent` follow-ups a post fans out and
/// commits to the byte-concatenation of everything it received — so a hook
/// notification that diverged (or went missing) between the runtimes diverges
/// the sink roots. staged/committed split keeps the block boundary honest.
struct HookSink {
    staged: Vec<Vec<u8>>,
    committed: Vec<u8>,
}

impl HookSink {
    fn new() -> Self {
        Self {
            staged: Vec::new(),
            committed: Vec::new(),
        }
    }
}

#[async_trait::async_trait(?Send)]
impl Module for HookSink {
    fn id(&self) -> ModuleId {
        "sink".into()
    }
    fn root(&self) -> StateRoot {
        StateRoot(sha2::Sha256::digest(&self.committed).into())
    }
    async fn execute(&mut self, _ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        self.staged.push(msg.payload.clone());
        Ok(())
    }
    async fn commit_block(&mut self) -> Result<(), Error> {
        for payload in self.staged.drain(..) {
            self.committed.extend_from_slice(&payload);
        }
        Ok(())
    }
    async fn abort_block(&mut self) -> Result<(), Error> {
        self.staged.clear();
        Ok(())
    }
}

async fn native_host(context: &deterministic::Context) -> Host {
    let store = QmdbStore::init(context.child("native_chat"), "chat").await;
    Host::genesis(vec![
        Box::new(
            Chat::new("chat", Box::new(store))
                .with_attribution("attribution")
                .with_identity("identity"),
        ),
        // the production tag-report target, kept NATIVE in both hosts for
        // isolation: this proof is about the chat cutover, and an identical
        // native attribution on both sides absorbs the emitted follow-ups
        // identically.
        Box::new(identity::Identity::new(
            "identity",
            Box::new(sdk_testkit::MemStore::new()),
            "parity".into(),
        )),
        Box::new(AttributionModule::new(
            "attribution",
            Box::new(sdk_testkit::MemStore::new()),
        )),
        Box::new(HookSink::new()),
    ])
    .expect("genesis")
}

async fn wasm_host_(context: &deterministic::Context) -> Host {
    let store = QmdbStore::init(context.child("wasm_chat"), "chat").await;
    Host::genesis(vec![
        Box::new(
            // NOTE: no `.with_attribution`/`.with_identity` here — the guest
            // compiles the exact production builder chain
            // (`Chat::new(..).with_attribution(..).with_identity(..)`) in.
            WasmModule::with_store("chat", CHAT_WASM, Box::new(store)).expect("load component"),
        ),
        Box::new(identity::Identity::new(
            "identity",
            Box::new(sdk_testkit::MemStore::new()),
            "parity".into(),
        )),
        Box::new(AttributionModule::new(
            "attribution",
            Box::new(sdk_testkit::MemStore::new()),
        )),
        Box::new(HookSink::new()),
    ])
    .expect("genesis")
}

/// the read matrix — the three kept dispatch queries — over one existing
/// channel (`channel` — a range read against an absent channel REJECTS, and a
/// native rejection and its wit-wrapped rendering are legitimately different
/// strings, so the byte-equal matrix only probes channels both hosts hold)
/// plus the global id lookups. the absent CHANNEL record and the absent
/// message id answer a comparable `None`.
async fn replies(h: &Host, channel: &str, message_id: &str) -> Vec<Vec<u8>> {
    let queries = [
        encode_query(&ChatQuery::Channel {
            channel_id: channel.into(),
        }),
        encode_query(&ChatQuery::Channel {
            channel_id: "absent".into(),
        }),
        encode_query(&ChatQuery::MessagesRange {
            channel_id: channel.into(),
            from_seq: 1,
            limit: 16,
        }),
        encode_query(&ChatQuery::Message {
            message_id: message_id.into(),
        }),
        encode_query(&ChatQuery::Message {
            message_id: "ghost".into(),
        }),
    ];
    let mut out = Vec::new();
    for q in &queries {
        out.push(h.query("chat", q).await.expect("query"));
    }
    out
}

/// chat + attribution + sink: the whole observable state of one host.
fn roots(h: &Host) -> (StateRoot, StateRoot, StateRoot) {
    (
        h.module_root("chat").expect("chat registered"),
        h.module_root("attribution")
            .expect("attribution registered"),
        h.module_root("sink").expect("sink registered"),
    )
}

#[test]
fn same_ops_identical_roots_block_by_block() {
    deterministic::Runner::default().start(|context| async move {
        let mut native = native_host(&context).await;
        let mut wasm = wasm_host_(&context).await;
        let (alice, bob, carol) = (key(0xA1), key(0xB2), key(0xC3));

        // ROOT CONTINUITY from block zero: both sides commit to the SAME
        // (empty) qmdb store — equal roots.
        assert_eq!(roots(&native), roots(&wasm), "genesis roots diverge");
        assert!(native.block_durable_ids().contains("chat"));
        assert!(wasm.block_durable_ids().contains("chat"));

        // every op family, one block each. `moves` = false marks the
        // idempotent no-op blocks whose op log must stay UNTOUCHED — the
        // native module stages nothing, so the wasm side must commit nothing.
        let ops: Vec<(Vec<u8>, ChatMsg, bool)> = vec![
            (
                alice.clone(),
                ChatMsg::CreateChannel {
                    channel_id: "general".into(),
                    name: "General".into(),
                    post_policy: PostPolicy::Open,
                },
                true,
            ),
            (
                alice.clone(),
                post("general", "m1", "hello world", None),
                true,
            ),
            // a thread reply: bumps the root's summary + the thread index.
            (bob.clone(), post("general", "m2", "hi!", Some(1)), true),
            (
                alice.clone(),
                ChatMsg::EditMessage {
                    channel_id: "general".into(),
                    seq: 1,
                    blocks: vec![Block::paragraph("hello world, edited")],
                    base_rev: Some(0),
                },
                true,
            ),
            (
                bob.clone(),
                ChatMsg::AddReaction {
                    channel_id: "general".into(),
                    seq: 1,
                    emoji: "👍".into(),
                },
                true,
            ),
            // the IDEMPOTENT duplicate: stages nothing on the native side, so
            // the store op log — and the root — must stay byte-identical on
            // the wasm side too (the empty-batch skip parity).
            (
                bob.clone(),
                ChatMsg::AddReaction {
                    channel_id: "general".into(),
                    seq: 1,
                    emoji: "👍".into(),
                },
                false,
            ),
            (
                carol.clone(),
                ChatMsg::AddReaction {
                    channel_id: "general".into(),
                    seq: 1,
                    emoji: "🎉".into(),
                },
                true,
            ),
            // exact remove of the last 🎉 reactor: deletes the reaction record
            // AND rewrites the emoji index (a staged DELETE riding the batch).
            (
                carol.clone(),
                ChatMsg::RemoveReaction {
                    channel_id: "general".into(),
                    seq: 1,
                    emoji: "🎉".into(),
                },
                true,
            ),
            // removing an absent reaction: deterministic no-op block.
            (
                carol.clone(),
                ChatMsg::RemoveReaction {
                    channel_id: "general".into(),
                    seq: 1,
                    emoji: "🎉".into(),
                },
                false,
            ),
            // membership + a members-only channel gate.
            (
                alice.clone(),
                ChatMsg::CreateChannel {
                    channel_id: "private".into(),
                    name: "Private".into(),
                    post_policy: PostPolicy::MembersOnly,
                },
                true,
            ),
            (
                alice.clone(),
                ChatMsg::SetMembership {
                    channel_id: "private".into(),
                    party: chat::Party::Key(bob.clone()),
                    member: true,
                },
                true,
            ),
            (
                bob.clone(),
                post("private", "m3", "members only", None),
                true,
            ),
            // RegisterHook's registry check is `ctx.module_root("sink")` — in
            // the wasm guest that is a SIBLING read resolved by memoized
            // replay before the hook is staged.
            (
                alice.clone(),
                ChatMsg::RegisterHook {
                    channel_id: "general".into(),
                    module_id: "sink".into(),
                },
                true,
            ),
            // this post fans out: a ChatEvent follow-up to the sink (pinned by
            // the sink root) plus the attribution report, all in the same block.
            (
                alice.clone(),
                post("general", "m4", "hook this", None),
                true,
            ),
            (
                alice.clone(),
                ChatMsg::DeleteMessage {
                    channel_id: "general".into(),
                    // m4 is general's THIRD sequence (m3 lives in "private").
                    seq: 3,
                },
                true,
            ),
            (
                bob.clone(),
                join_huddle("general", &bob, &ed25519::PrivateKey::from_seed(0x11)),
                true,
            ),
            // re-joining with the same node key: stages nothing.
            (
                bob.clone(),
                join_huddle("general", &bob, &ed25519::PrivateKey::from_seed(0x11)),
                false,
            ),
            (
                alice.clone(),
                ChatMsg::SweepHuddle {
                    channel_id: "general".into(),
                    party: chat::Party::Key(bob.clone()),
                },
                true,
            ),
            // alice, not bob: hook (un)registration is channel-admin authority
            // and alice owns "general".
            (
                alice.clone(),
                ChatMsg::UnregisterHook {
                    channel_id: "general".into(),
                    module_id: "sink".into(),
                },
                true,
            ),
        ];

        for (height, (who, msg, moves)) in ops.into_iter().enumerate() {
            let height = height as u64 + 1;
            let before = roots(&native);
            native
                .submit_at(block(height, &who), op(&msg))
                .await
                .expect("native submit");
            wasm.submit_at(block(height, &who), op(&msg))
                .await
                .expect("wasm submit");

            // THE claim: identical roots after every block boundary.
            assert_eq!(
                roots(&native),
                roots(&wasm),
                "roots diverge after block {height}"
            );
            let chat_root = native.module_root("chat").expect("chat");
            if moves {
                assert_ne!(chat_root, before.0, "chat root stuck at {height}");
            } else {
                // the idempotent no-op: NOTHING staged, NOTHING committed, so
                // the op log (and the root) is byte-identical to a single
                // application — on both runtimes.
                assert_eq!(
                    chat_root, before.0,
                    "no-op block moved the root at {height}"
                );
            }
            assert_eq!(
                replies(&native, "general", "m1").await,
                replies(&wasm, "general", "m1").await,
                "replies diverge after block {height}"
            );
        }

        // the hook actually fired and matched: the sink saw at least one
        // ChatEvent (its root left the empty-state hash) — identically.
        assert_ne!(
            native.module_root("sink"),
            Some(StateRoot(sha2::Sha256::digest([]).into())),
            "the hook fan-out never reached the sink"
        );

        // identical resolver sync surface: same pinned target, same
        // proof-carrying serve bytes — a joiner cannot tell which executor
        // produced the store.
        let n_target = native
            .resolver_sync_target("chat")
            .await
            .expect("native target");
        let w_target = wasm
            .resolver_sync_target("chat")
            .await
            .expect("wasm target");
        assert_eq!(n_target, w_target, "resolver sync targets diverge");
        let req = encode_qmdb_req(&QmdbSyncReq::Ops {
            op_count: n_target.op_count,
            start_loc: n_target.start,
            max_ops: 64,
            include_pinned: true,
        });
        assert_eq!(
            native.serve_sync("chat", &req).await.expect("native serve"),
            wasm.serve_sync("chat", &req).await.expect("wasm serve"),
            "sync serve bytes diverge"
        );

        // queries are read-only on the wasm side too.
        let settled = roots(&wasm);
        let _ = replies(&wasm, "general", "m1").await;
        assert_eq!(roots(&wasm), settled, "a query moved a root");
    });
}

#[test]
fn sync_handle_matches_native() {
    deterministic::Runner::default().start(|context| async move {
        let native = Chat::new(
            "chat",
            Box::new(QmdbStore::init(context.child("rev_native"), "chat").await),
        )
        .with_attribution("attribution");
        let wasm = WasmModule::with_store(
            "chat",
            CHAT_WASM,
            Box::new(QmdbStore::init(context.child("rev_wasm"), "chat").await),
        )
        .expect("load component");

        let n_handle = native.state_sync_handle().expect("native handle");
        let w_handle = wasm.state_sync_handle().expect("wasm handle");
        assert_eq!(n_handle, w_handle, "sync handles diverge");
        assert!(
            matches!(w_handle, StateSyncHandle::ResolverBacked { ref backend, .. } if backend == "qmdb"),
            "store-backed tenant must stay resolver-backed: {w_handle:?}"
        );
    });
}

#[test]
fn rejections_match_and_leave_no_trace() {
    deterministic::Runner::default().start(|context| async move {
        let mut native = native_host(&context).await;
        let mut wasm = wasm_host_(&context).await;
        let (alice, carol) = (key(0xA1), key(0xC3));

        for host in [&mut native, &mut wasm] {
            host.submit_at(
                block(1, &alice),
                op(&ChatMsg::CreateChannel {
                    channel_id: "general".into(),
                    name: "General".into(),
                    post_policy: PostPolicy::Open,
                }),
            )
            .await
            .expect("create");
            host.submit_at(
                block(2, &alice),
                op(&ChatMsg::CreateChannel {
                    channel_id: "private".into(),
                    name: "Private".into(),
                    post_policy: PostPolicy::MembersOnly,
                }),
            )
            .await
            .expect("create");
            host.submit_at(block(3, &alice), op(&post("general", "m1", "hello", None)))
                .await
                .expect("post");
            // a MODULE-minted channel — the `forge:<repo>:<n>` shape — which
            // is UNOWNED by construction: the principal that minted it is a
            // module, and no user is it.
            host.submit_at(
                block_as(4, Origin::Module("sink".into())),
                op(&ChatMsg::CreateChannel {
                    channel_id: "sink:room".into(),
                    name: "Sink Room".into(),
                    post_policy: PostPolicy::Open,
                }),
            )
            .await
            .expect("module-minted channel");
        }

        // the rejection matrix: distinct refusal families. each rejected block
        // must leave BOTH roots byte-identical (staged writes discarded).
        let rejects: Vec<(Vec<u8>, ChatMsg, &str)> = vec![
            (
                alice.clone(),
                ChatMsg::CreateChannel {
                    channel_id: "general".into(),
                    name: "Duplicate".into(),
                    post_policy: PostPolicy::Open,
                },
                "already exists",
            ),
            (
                alice.clone(),
                post("general", "m1", "duplicate id", None),
                "already exists",
            ),
            (
                alice.clone(),
                post("ghost", "mx", "no channel", None),
                "unknown channel",
            ),
            // reserved module namespace: an external user may not mint ids
            // containing ':'.
            (
                alice.clone(),
                ChatMsg::CreateChannel {
                    channel_id: "forge:sneaky".into(),
                    name: "Sneak".into(),
                    post_policy: PostPolicy::Open,
                },
                "reserved for modules",
            ),
            // the pre-consensus empty external origin never authenticates.
            (
                Vec::new(),
                post("general", "anon", "anonymous", None),
                "non-empty submitter id",
            ),
            // only the stored author may edit.
            (
                carol.clone(),
                ChatMsg::EditMessage {
                    channel_id: "general".into(),
                    seq: 1,
                    blocks: vec![Block::paragraph("hijack")],
                    base_rev: None,
                },
                "only the author",
            ),
            // members-only gate: carol never joined.
            (
                carol.clone(),
                post("private", "m9", "let me in", None),
                "members-only",
            ),
            // hooking an unregistered module fails the registry check — the
            // sibling module-root read answers `None` on both runtimes.
            (
                alice.clone(),
                ChatMsg::RegisterHook {
                    channel_id: "general".into(),
                    module_id: "ghost-module".into(),
                },
                "unknown hook module",
            ),
            // CHANNEL-ADMIN AUTHORITY, proven in the compiled component and
            // not just natively: the gate reads `env().origin`, the one
            // authorization input that crosses the WIT boundary, so a gate
            // keyed on it is exactly the kind that can compile, review as
            // correct, and be inert inside the guest.
            //
            // alice owns "general"; carol writes neither its roster nor its
            // hook list. the roster IS `PostPolicy::MembersOnly`'s admission
            // list, so an ungated `SetMembership` let carol admit HERSELF and
            // post straight through the only admission rule chat has.
            (
                carol.clone(),
                ChatMsg::SetMembership {
                    channel_id: "general".into(),
                    party: chat::Party::Key(carol.clone()),
                    member: true,
                },
                "only the owner",
            ),
            // a hook is a standing subscription to everything posted there.
            (
                carol.clone(),
                ChatMsg::RegisterHook {
                    channel_id: "general".into(),
                    module_id: "sink".into(),
                },
                "only the owner",
            ),
            // and the sharper half: an ungated unregister is a one-message off
            // switch for every automation on the channel.
            (
                carol.clone(),
                ChatMsg::UnregisterHook {
                    channel_id: "general".into(),
                    module_id: "sink".into(),
                },
                "only the owner",
            ),
            // an UNOWNED channel admits NO user — not even alice, who
            // administers channels of her own. the module that minted it is
            // its principal, and `check_channel_admin` fails closed rather
            // than letting the `None` owner fall through.
            (
                alice.clone(),
                ChatMsg::SetMembership {
                    channel_id: "sink:room".into(),
                    party: chat::Party::Key(alice.clone()),
                    member: true,
                },
                "is unowned",
            ),
        ];

        for (height, (who, msg, needle)) in rejects.into_iter().enumerate() {
            let height = height as u64 + 5;
            let before = roots(&native);
            assert_eq!(before, roots(&wasm));

            let n_err = native
                .submit_at(block(height, &who), op(&msg))
                .await
                .expect_err("native must reject");
            let w_err = wasm
                .submit_at(block(height, &who), op(&msg))
                .await
                .expect_err("wasm must reject");

            // both reject DETERMINISTICALLY with the native module's reason.
            // the wasm runtime wraps the reason in its wit-error rendering, so
            // the parity claim is containment, not string equality.
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

            // abort invariance: roots byte-identical to pre-block, still equal.
            assert_eq!(roots(&native), before, "native root moved on reject");
            assert_eq!(roots(&wasm), before, "wasm root moved on reject");
            assert_eq!(
                replies(&native, "general", "m1").await,
                replies(&wasm, "general", "m1").await
            );
        }
    });
}

#[test]
fn multi_dispatch_block_reads_prior_writes_and_isolates_rejections() {
    deterministic::Runner::default().start(|context| async move {
        let mut native = native_host(&context).await;
        let mut wasm = wasm_host_(&context).await;
        let (alice, carol) = (key(0xA1), key(0xC3));

        // ONE block, three dispatches: the post reads the channel CREATED one
        // dispatch earlier (staged, not committed — its head_seq counter too),
        // and the reaction reads the staged message head. on the wasm side
        // dispatch N+1's reads come from the OUTER staged overlay — the
        // guest's inner pending died with dispatch N — which is exactly the
        // native pending-persists-across-dispatches view.
        let batch = vec![
            (
                Origin::External(alice.clone()),
                op(&ChatMsg::CreateChannel {
                    channel_id: "room".into(),
                    name: "Room".into(),
                    post_policy: PostPolicy::Open,
                }),
            ),
            (
                Origin::External(alice.clone()),
                op(&post("room", "r1", "first in room", None)),
            ),
            (
                Origin::External(alice.clone()),
                op(&ChatMsg::AddReaction {
                    channel_id: "room".into(),
                    seq: 1,
                    emoji: "🚀".into(),
                }),
            ),
        ];
        let n_out = native
            .submit_block(block(1, &alice), batch.clone())
            .await
            .expect("native block");
        let w_out = wasm
            .submit_block(block(1, &alice), batch)
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
        assert_eq!(roots(&native), roots(&wasm));
        assert_eq!(
            replies(&native, "room", "r1").await,
            replies(&wasm, "room", "r1").await
        );

        // ONE block where the SECOND member rejects: the runtime aborts the
        // staged overlay and replays the accepted member — committed state
        // must equal the accepted subset alone, on both runtimes.
        let before = roots(&native);
        let batch = vec![
            (
                Origin::External(alice.clone()),
                op(&post("room", "r2", "accepted", None)),
            ),
            (
                Origin::External(carol.clone()),
                op(&ChatMsg::EditMessage {
                    channel_id: "room".into(),
                    seq: 1,
                    blocks: vec![Block::paragraph("hijack")],
                    base_rev: None,
                }),
            ),
        ];
        let n_out = native
            .submit_block(block(2, &alice), batch.clone())
            .await
            .expect("native block");
        let w_out = wasm
            .submit_block(block(2, &alice), batch)
            .await
            .expect("wasm block");
        for out in [&n_out, &w_out] {
            assert!(matches!(out.members[0], MemberOutcome::Applied { .. }));
            assert!(matches!(out.members[1], MemberOutcome::Rejected { .. }));
        }
        assert_ne!(roots(&native), before, "accepted member must land");
        assert_eq!(roots(&native), roots(&wasm));
        assert_eq!(
            replies(&native, "room", "r1").await,
            replies(&wasm, "room", "r1").await
        );
        for host in [&native, &wasm] {
            let reply = host
                .query(
                    "chat",
                    &encode_query(&ChatQuery::Message {
                        message_id: "r2".into(),
                    }),
                )
                .await
                .expect("query");
            let ChatReply::Message(Some(view)) = decode_reply(&reply).expect("decode") else {
                panic!("r2 must exist");
            };
            assert_eq!(view.seq, 2);
            assert!(
                !view.head.deleted && view.head.rev == 0,
                "rejected member must leave no trace"
            );
        }
    });
}
