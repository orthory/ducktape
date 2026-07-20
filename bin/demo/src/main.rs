//! a runnable super-app demo: twenty registered modules — including QMDB-backed
//! KV and EVM state, Git/DuckFS substrates, collaboration apps, and system
//! routing modules — dispatched over ONE host, showing
//! the app-hash evolve as typed cross-module ops flow, ending on the
//! agent-collaboration beat: a mention becomes a run and a pending saga in one
//! block.
//!
//! run: `cargo run -p demo`

use statesync::qmdb::QmdbStore;
use agent::AgentModule;
use agent::{
    ACTION_CHAT_POST, ACTION_TASKS_CREATE, AgentMsg, encode_msg as agent_encode_msg,
};
use runs::{RunsModule, run_id_for};
use runs::{
    RunsMsg, RunsQuery, RunsReply, TurnPolicy, decode_reply as runs_decode_reply,
    encode_msg as runs_encode_msg, encode_query as runs_encode_query,
};
use automations::Automations;
use chat::Chat;
use chat::{
    Block as ChatBlock, ChatMsg, ChatQuery, ChatReply, PostPolicy,
    decode_reply as chat_decode_reply, encode_msg as chat_encode_msg,
    encode_query as chat_encode_query,
};
use commonware_codec::DecodeExt as _;
use commonware_cryptography::{Signer as _, ed25519::PrivateKey};
use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use directory::Directory;
use directory::{DirMsg, DirQuery, decode_reply, encode_msg, encode_query};
use gateway::Gateway;
use files::Files;
use forge::Forge;
use forge::{
    ForgeMsg, ForgeQuery, ForgeReply, decode_reply as forge_decode_reply,
    encode_msg as forge_encode_msg, encode_query as forge_encode_query,
};
use greeter::Greeter;
use host::{BlockContext, Host};
use identity::Identity;
use inbox::Inbox;
use inbox::{
    InboxMsg, InboxQuery, InboxReply, decode_reply as inbox_decode_reply,
    encode_msg as inbox_encode_msg, encode_query as inbox_encode_query,
};
use saga::SagaModule;
use saga::{
    SagaQuery, SagaReply, decode_reply as saga_decode_reply, decode_worker_request,
    encode_query as saga_encode_query,
};
use sdk::{Msg, Origin};
use tasks::Tasks;
use valset::Valset;
use valset::{
    ValsetMsg, ValsetQuery, ValsetReply, decode_reply as valset_decode_reply,
    encode_msg as valset_encode_msg, encode_query as valset_encode_query,
};

fn main() {
    // forge's substrate is a real git repo on disk. wipe any prior run's dir so
    // genesis starts from an unborn repo (root == ZERO) and output is reproducible.
    let forge_repo = std::env::temp_dir().join("ducktape-forge-demo");
    let _ = std::fs::remove_dir_all(&forge_repo);
    // duckfs gets the same treatment: a wiped per-run data dir keeps output
    // reproducible (the skeleton runs in-memory; tasks 5/6 use the dir).
    let duckfs_dir = std::env::temp_dir().join("ducktape-duckfs-demo");
    let _ = std::fs::remove_dir_all(&duckfs_dir);

    deterministic::Runner::default().start(|context| async move {
        // genesis: the module registry (would be consensus state on a real chain).
        let kv = kv::Kv::new("kv", Box::new(QmdbStore::init(context.child("kv"), "kv").await));
        let directory = Directory::new("directory");
        let greeter = Greeter::new("greeter");
        let forge = Forge::init("forge", forge_repo.clone())
            .expect("forge init")
            .with_chat("chat");
        let chat = Chat::new("chat", Box::new(QmdbStore::init(context.child("chat"), "chat").await))
            .with_tagging("tagging");
        let valset = Valset::new("valset");
        let saga = SagaModule::new("saga");
        let dispatch = dispatch::DispatchModule::new("dispatch", "saga");
        let tagging = tagging::TaggingModule::new("tagging").with_direct_owner("runs");
        let tasks = Tasks::new("tasks");
        // the deterministic user->nodes binding registry: no valset gating and
        // a fixed demo chain id (the demo has no real network descriptor).
        let identity = Identity::new("identity", None, "demo".into());
        let gateway = Gateway::new("gateway", "identity", None, "demo");
        let inbox = Inbox::new("inbox");
        let files = Files::open("files", duckfs_dir.clone()).expect("duckfs open");
        let agent = AgentModule::new("agent", "saga", Some("runs".into()));
        let runs = RunsModule::new(
            "runs",
            "chat",
            "saga",
            "tagging",
            "dispatch",
            "agent",
            Some("tasks".into()),
            Some("tasks".into()),
        )
        // the duckfs/files module the portable (v3) composer pins its source
        // head from (W2) — mandatory for envelope composition.
        .with_files_module("files");
        let automations = Automations::new("automations", "chat", "tasks", "inbox");
        let mut host = Host::genesis(vec![
            Box::new(kv),
            Box::new(directory),
            Box::new(greeter),
            Box::new(forge),
            Box::new(chat),
            Box::new(valset),
            Box::new(saga),
            Box::new(dispatch),
            Box::new(tagging),
            Box::new(tasks),
            Box::new(identity),
            Box::new(gateway),
            Box::new(inbox),
            Box::new(files),
            Box::new(agent),
            Box::new(runs),
            Box::new(automations),
        ])
        .expect("genesis");

        println!("=== super-app demo — 19 registered modules over one host ===");
        println!("forge repo       : {}", forge_repo.display());
        println!("genesis app-hash : {:?}", host.app_hash());
        println!(
            "genesis forge root (unborn git repo): {:?}",
            host.module_root("forge").unwrap()
        );
        println!(
            "genesis valset root (empty set)     : {:?}",
            host.module_root("valset").unwrap()
        );
        println!(
            "genesis chat root (no channels)     : {:?}",
            host.module_root("chat").unwrap()
        );

        // block 1: a typed Set to the in-memory directory module.
        let out = host
            .submit(Msg {
                target: "directory".into(),
                payload: encode_msg(&DirMsg::Set {
                    key: "name".into(),
                    value: "world".into(),
                }),
            })
            .await
            .expect("submit block 1");
        println!("\n[block 1] directory <- Set(name = world)");
        println!("  app-hash       : {:?}", out.app_hash);

        // block 2: trigger greeter. it QUERIES directory (typed, cross-module),
        // then emits typed follow-up writes to directory + kv — all in one block.
        let out = host
            .submit(Msg {
                target: "greeter".into(),
                payload: b"name".to_vec(),
            })
            .await
            .expect("submit block 2");
        println!("\n[block 2] greeter(name): query directory -> write greeting to directory + kv");
        println!("  app-hash       : {:?}", out.app_hash);

        // block 3: a typed Commit to the GIT-backed forge module. this writes a
        // file + makes a real (deterministic) git commit; forge's root moves to
        // the new HEAD oid, which folds straight into the app-hash.
        let out = host
            .submit(Msg {
                target: "forge".into(),
                payload: forge_encode_msg(&ForgeMsg::Commit {
                    // empty repo slug -> the default repo (single-repo wire).
                    repo: String::new(),
                    path: "README.md".into(),
                    content: "# hello from a git-backed module\n".into(),
                    message: "forge: initial commit".into(),
                }),
            })
            .await
            .expect("submit block 3");
        println!("\n[block 3] forge <- Commit(README.md) — a real git commit");
        println!("  app-hash       : {:?}", out.app_hash);
        println!(
            "  forge root     : {:?}",
            host.module_root("forge").unwrap()
        );

        // read forge's HEAD back out (typed query) — the sha1 git oid hex. this hex is
        // the sha256 PREIMAGE of the forge root: a git commit addressing the app-hash.
        let reply = host
            .query("forge", &forge_encode_query(&ForgeQuery::Head))
            .await
            .expect("query forge");
        if let ForgeReply::Head(Some(oid)) = forge_decode_reply(&reply).unwrap() {
            println!("  forge git HEAD : {oid}");
            println!("  (^ the 40-char sha1 oid is the sha256 preimage of the forge root above)");
        }

        // read the derived greeting back out of the directory (sync typed query).
        let reply = host
            .query(
                "directory",
                &encode_query(&DirQuery::Get {
                    key: "greeting:name".into(),
                }),
            )
            .await
            .expect("query directory");
        println!(
            "\ndirectory[greeting:name] = {:?}",
            decode_reply(&reply).unwrap()
        );

        // and read it back out of the QMDB kv module — a real async cross-module read.
        let kv_reply = host
            .query(
                "kv",
                &kv::encode_query(&kv::KvQuery::Get {
                    key: b"greeting:name".to_vec(),
                }),
            )
            .await
            .expect("query kv");
        if let kv::KvReply::Value(Some(v)) =
            kv::decode_reply(&kv_reply).unwrap()
        {
            println!(
                "kv[greeting:name]        = {:?}",
                String::from_utf8_lossy(&v)
            );
        }

        // block 4: create a chat channel. chat derives authorship from the
        // dispatch origin (no author field in any payload), so the demo submits
        // with an explicit external origin — a seed-derived ed25519 pubkey
        // standing in for a real submitter id. the default empty external
        // origin would be rejected.
        let demo_user = PrivateKey::decode(&[5u8; 32][..])
            .expect("32-byte seed is a valid ed25519 private key")
            .public_key()
            .as_ref()
            .to_vec();
        let as_demo_user = || BlockContext { protocol_version: 0,
            height: 0,
            consensus_time: 0,
            origin: Origin::External(demo_user.clone()),
        };
        let out = host
            .submit_at(
                as_demo_user(),
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
            .expect("submit block 4");
        println!("\n[block 4] chat <- CreateChannel(general) — authorship from origin");
        println!("  chat root      : {:?}", host.module_root("chat").unwrap());
        println!("  app-hash       : {:?}", out.app_hash);

        // block 5: a root message, a thread reply, and a reaction — three ops,
        // three blocks; sequences come from the channel's head_seq counter.
        host.submit_at(
            as_demo_user(),
            Msg {
                target: "chat".into(),
                payload: chat_encode_msg(&ChatMsg::PostMessage {
                    channel_id: "general".into(),
                    message_id: "m1".into(),
                    blocks: vec![ChatBlock::paragraph("what changed?")],
                    thread: None,
                    as_agent: None,
                }),
            },
        )
        .await
        .expect("submit block 5 post");
        host.submit_at(
            as_demo_user(),
            Msg {
                target: "chat".into(),
                payload: chat_encode_msg(&ChatMsg::PostMessage {
                    channel_id: "general".into(),
                    message_id: "m2".into(),
                    blocks: vec![ChatBlock::paragraph(
                        "chat is block-based and origin-authored now",
                    )],
                    thread: Some(1),
                    as_agent: None,
                }),
            },
        )
        .await
        .expect("submit block 5 reply");
        let out = host
            .submit_at(
                as_demo_user(),
                Msg {
                    target: "chat".into(),
                    payload: chat_encode_msg(&ChatMsg::AddReaction {
                        channel_id: "general".into(),
                        seq: 2,
                        emoji: "🦆".into(),
                    }),
                },
            )
            .await
            .expect("submit block 5 reaction");
        let reply = host
            .query(
                "chat",
                &chat_encode_query(&ChatQuery::MessagesLatest {
                    channel_id: "general".into(),
                    limit: 16,
                }),
            )
            .await
            .expect("query chat");
        if let ChatReply::Messages(messages) = chat_decode_reply(&reply).unwrap() {
            println!("\n[block 5] chat <- PostMessage(m1) + thread reply(m2) + AddReaction");
            println!("  message count  : {}", messages.len());
            println!(
                "  m1 replies     : {} (last reply seq: {:?})",
                messages[0].head.reply_count, messages[0].head.last_reply_seq
            );
            println!(
                "  m2 reactions   : {:?}",
                messages[1]
                    .reactions
                    .iter()
                    .map(|r| (r.emoji.as_str(), r.reactors.len()))
                    .collect::<Vec<_>>()
            );
            println!("  chat root      : {:?}", host.module_root("chat").unwrap());
            println!("  app-hash       : {:?}", out.app_hash);
        }

        // block 6: a NEW validator JOINs the permissionless ed25519 valset. derive
        // the key deterministically from a fixed seed (any 32 bytes is a valid
        // ed25519 seed) so the demo is reproducible. the valset root moves off
        // ZERO and folds another module's commitment into the app-hash.
        let seed = [7u8; 32];
        let new_validator = PrivateKey::decode(&seed[..])
            .expect("32-byte seed is a valid ed25519 private key")
            .public_key()
            .as_ref()
            .to_vec();
        // membership ops are governance-gated (external origins are refused);
        // the demo's direct join rides a SYSTEM-origin block — the same trusted
        // orchestration lane genesis seeding uses.
        let out = host
            .submit_at(
                host::BlockContext { protocol_version: 0,
                    height: 0,
                    consensus_time: 0,
                    origin: sdk::Origin::System,
                },
                Msg {
                    target: "valset".into(),
                    payload: valset_encode_msg(&ValsetMsg::Join {
                        key: new_validator.clone(),
                    }),
                },
            )
            .await
            .expect("submit block 6");
        println!("\n[block 6] valset <- Join(ed25519 pubkey) — a new validator joins");
        let reply = host
            .query("valset", &valset_encode_query(&ValsetQuery::Validators))
            .await
            .expect("query valset");
        let ValsetReply::Validators(vs) = valset_decode_reply(&reply).unwrap() else {
            panic!("expected Validators reply");
        };
        println!("  validator count: {} (was 0 at genesis)", vs.len());
        println!(
            "  valset root    : {:?}",
            host.module_root("valset").unwrap()
        );
        println!("  app-hash       : {:?}", out.app_hash);

        // block 7: the agent-collaboration loop (design §3). register an agent
        // (which model+prompt it runs is committed into the app-hash), watch
        // the chat channel under a Mention policy — the watch and chat's hook
        // registration commit atomically — enable the runs module as the
        // jobs-board worker by admin op (not genesis config), then post a message MENTIONING the
        // agent: the very same block carries the post, the tagging plane's
        // engagement delivery, the pending entry, the dispatch, and its saga
        // trigger. the emitted WorkerRequest effect is the off-consensus LLM
        // seam a reactor driver answers as an ordinary oracle op in some
        // later block.
        host.submit_at(
            as_demo_user(),
            Msg {
                target: "agent".into(),
                payload: agent_encode_msg(&AgentMsg::RegisterAgent {
                    agent_id: "quackbot".into(),
                    display_name: "Quackbot".into(),
                    capability: "mock-llm-1".into(),
                    allowed_actions: vec![ACTION_CHAT_POST.into(), ACTION_TASKS_CREATE.into()],
                    recipe_hash: None,
                    caps: None,
                    // the persona is a curated skill now (an `always` one), not a
                    // prompt blob. the demo's mock provider ignores the composed
                    // context anyway, so it registers soul-less.
                    skills: None,
                }),
            },
        )
        .await
        .expect("submit block 7 register");
        host.submit_at(
            as_demo_user(),
            Msg {
                target: "runs".into(),
                payload: runs_encode_msg(&RunsMsg::EnableJobWorker { enabled: true }),
            },
        )
        .await
        .expect("submit block 7 enable jobs worker");
        host.submit_at(
            as_demo_user(),
            Msg {
                target: "runs".into(),
                payload: runs_encode_msg(&RunsMsg::WatchChannel {
                    channel_id: "general".into(),
                    policy: TurnPolicy::Mention,
                }),
            },
        )
        .await
        .expect("submit block 7 watch");
        let out = host
            .submit_at(
                as_demo_user(),
                Msg {
                    target: "chat".into(),
                    payload: chat_encode_msg(&ChatMsg::PostMessage {
                        channel_id: "general".into(),
                        message_id: "m3".into(),
                        blocks: vec![ChatBlock::Paragraph(vec![
                            chat::Span::plain("hey "),
                            chat::Span {
                                text: "@quackbot".into(),
                                marks: vec![chat::Mark::Mention(
                                    chat::AuthorRef::Agent {
                                        module: "runs".into(),
                                        agent_id: "quackbot".into(),
                                    },
                                )],
                            },
                            chat::Span::plain(" can you follow up?"),
                        ])],
                        thread: None,
                        as_agent: None,
                    }),
                },
            )
            .await
            .expect("submit block 7 mention");
        println!(
            "\n[block 7] agent <- Register; runs <- EnableJobWorker(true); runs <- Watch(Mention); chat <- PostMessage(@quackbot)"
        );
        println!(
            "  work orders    : {} WorkerRequest (the off-consensus LLM seam)",
            out.events
                .iter()
                .filter(|e| decode_worker_request(&e.payload).is_ok())
                .count()
        );
        let run_id = run_id_for("general", 3, "quackbot");
        let reply = host
            .query("runs", &runs_encode_query(&RunsQuery::PendingRuns))
            .await
            .expect("query runs pending");
        if let RunsReply::PendingRuns(pending) = runs_decode_reply(&reply).unwrap() {
            for entry in pending.iter().filter(|p| p.run_id == run_id) {
                println!(
                    "  pending run    : {} (dispatch {}, anchored at seq {})",
                    entry.run_id, entry.dispatch_id, entry.anchor_seq
                );
            }
        }
        let reply = host
            .query(
                "dispatch",
                &dispatch::encode_query(&dispatch::DispatchQuery::Dispatch {
                    receiver: "runs".into(),
                    dispatch_id: runs::dispatch_id_for(&run_id),
                }),
            )
            .await
            .expect("query dispatch");
        let dispatch::DispatchReply::Dispatch(Some(dispatch_view)) =
            dispatch::decode_reply(&reply).unwrap()
        else {
            panic!("the run's dispatch must exist");
        };
        let dispatch::DispatchStatus::AwaitingResult { saga_id } =
            dispatch_view.status.clone()
        else {
            panic!("the dispatch awaits its saga");
        };
        let reply = host
            .query(
                "saga",
                &saga_encode_query(&SagaQuery::Get {
                    saga_id: saga_id.clone(),
                }),
            )
            .await
            .expect("query saga");
        let SagaReply::Saga(Some(saga_view)) = saga_decode_reply(&reply).unwrap() else {
            panic!("the run's saga must exist");
        };
        println!(
            "  saga           : {} {:?} (deadline view {:?})",
            saga_id, saga_view.status, saga_view.deadline
        );
        println!(
            "  (post + tag + engagement + run + dispatch + trigger: ONE block — the P2 cascade)"
        );
        println!("  app-hash       : {:?}", out.app_hash);

        // block 8: the INBOX notification queue. modules deliver to a member as
        // a follow-up so the notification commits atomically with its cause; here
        // an external submitter self-delivers a note to show the air-gap-native
        // path (no external push service). the queue holds it as consensus state.
        let out = host
            .submit_at(
                BlockContext { protocol_version: 0,
                    height: 0,
                    consensus_time: 9,
                    // the submitter's id — the inbox derives `source` from this
                    // origin ("ext:" + hex of the external bytes), never from
                    // the payload.
                    origin: Origin::External(b"cli".to_vec()),
                },
                Msg {
                    target: "inbox".into(),
                    payload: inbox_encode_msg(&InboxMsg::Deliver {
                        member: "quackbot".into(),
                        kind: "mention".into(),
                        body: "you were mentioned in #general".into(),
                    }),
                },
            )
            .await
            .expect("submit block 8");
        println!("\n[block 8] inbox <- Deliver(quackbot) — a notification as consensus state");
        let reply = host
            .query(
                "inbox",
                &inbox_encode_query(&InboxQuery::List {
                    member: "quackbot".into(),
                    from_seq: 0,
                    limit: 16,
                }),
            )
            .await
            .expect("query inbox");
        if let InboxReply::Items(items) = inbox_decode_reply(&reply).unwrap() {
            for note in &items {
                println!(
                    "  note           : seq {} [{}] from {:?} — {}",
                    note.seq, note.kind, note.source, note.body
                );
            }
        }
        println!("  app-hash       : {:?}", out.app_hash);

        // every registered module, straight from the registry — the same set
        // (and sorted-id order) the app-hash composes over.
        println!("\nmodule roots:");
        for (id, root) in host.module_roots() {
            println!("  {id:>11} : {root:?}");
        }
        println!("\nfinal app-hash   : {:?}", host.app_hash());
    });
}
