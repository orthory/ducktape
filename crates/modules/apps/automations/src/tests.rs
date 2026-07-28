//! the unit-test suite for the automations module: a CaptureCtx harness over
//! `execute` (probes included) atop a `MemStore`-backed module, and the
//! store-backed staging/root semantics.

use super::*;

#[test]
fn retired_tagged_trigger_shape_rejects_loudly() {
    // the pre-flag-day wire wrapped the trigger in a single-variant enum tag;
    // an all-Option flat struct would otherwise parse it as an all-None
    // fire-on-everything rule. deny_unknown_fields turns it into a loud error.
    let old = serde_json::json!({
        "add_rule": {
            "rule_id": "r1",
            "trigger": {"message_posted": {"channel_id": "general"}},
            "action": {"post_message": {"channel_id": "general", "text": "hi"}}
        }
    });
    let err = decode_msg(&serde_json::to_vec(&old).unwrap());
    assert!(err.is_err(), "retired tagged trigger shape must not decode");
}

use std::collections::{BTreeMap, BTreeSet};

use crate::{AutomationsReply, decode_reply, encode_msg, encode_query};
use chat::{
    Block, Channel, Mark, MessageHead, MessageView, PostPolicy, Span,
    decode_msg as chat_decode_msg, decode_query as chat_decode_query,
    encode_event as chat_encode_event, encode_reply as chat_encode_reply,
};
use futures::executor::block_on;
use inbox::{InboxMsg, decode_msg as inbox_decode_msg};
use sdk::{Env, Event};
use sdk_testkit::{MemStore, TestCtx};
use tasks::{
    Task, TaskStatus, decode_task_msg as tasks_decode_msg, encode_task_reply as tasks_encode_reply,
};

const CHAT: &str = "chat";
const TASKS: &str = "tasks";
const INBOX: &str = "inbox";
const ME: &str = "automations";
/// the default admin submitter, and therefore the owner of every rule these
/// tests create: only an authenticated EXTERNAL origin may own a rule.
const OWNER: &[u8] = b"owner";
/// a second submitter, who owns nothing.
const STRANGER: &[u8] = b"stranger";

/// a minimal `Ctx` capturing emitted msgs and serving canned chat
/// transcripts / channels and a task list — enough to unit-test `execute`
/// (including the pre-emit probes) in isolation.
struct CaptureCtx {
    env: Env,
    /// channel -> messages with contiguous seqs starting at 1. transcript
    /// channels also count as existing for the channel probe.
    transcripts: BTreeMap<String, Vec<MessageView>>,
    /// channels the chat probe reports as existing.
    channels: BTreeSet<String>,
    /// the task list served to the tasks probe.
    tasks: Vec<Task>,
    msgs: Vec<Msg>,
    /// when set, every query returns an error.
    fail_query: bool,
}

impl CaptureCtx {
    fn new() -> Self {
        Self {
            env: Env {
                height: 7,
                consensus_time: 42,
                // an admin ctx by default: rule CRUD is owner-bound, so the
                // baseline origin is an authenticated submitter. hook-arm
                // tests swap it with `with_chat_origin`.
                origin: Origin::External(OWNER.to_vec()),
                me: ME.into(),
            },
            transcripts: BTreeMap::new(),
            channels: BTreeSet::new(),
            tasks: Vec::new(),
            msgs: Vec::new(),
            fail_query: false,
        }
    }
    fn with_origin(mut self, origin: Origin) -> Self {
        self.env.origin = origin;
        self
    }
    fn with_chat_origin(self) -> Self {
        self.with_origin(Origin::Module(CHAT.into()))
    }
    fn with_transcript(mut self, channel: &str, messages: Vec<MessageView>) -> Self {
        self.channels.insert(channel.into());
        self.transcripts.insert(channel.into(), messages);
        self
    }
    fn with_channel(mut self, channel: &str) -> Self {
        self.channels.insert(channel.into());
        self
    }
    fn with_task(mut self, task_id: &str) -> Self {
        self.tasks.push(Task {
            id: task_id.into(),
            title: task_id.into(),
            status: TaskStatus::Open,
            created_at: 0,
            updated_at: 0,
        });
        self
    }
    fn failing_query(mut self) -> Self {
        self.fail_query = true;
        self
    }
    fn chat_msgs(&self) -> Vec<ChatMsg> {
        self.msgs
            .iter()
            .filter(|m| m.target == CHAT)
            .map(|m| chat_decode_msg(&m.payload).expect("chat msg"))
            .collect()
    }
    fn task_msgs(&self) -> Vec<TaskMsg> {
        self.msgs
            .iter()
            .filter(|m| m.target == TASKS)
            .map(|m| tasks_decode_msg(&m.payload).expect("task msg"))
            .collect()
    }
    fn inbox_msgs(&self) -> Vec<InboxMsg> {
        self.msgs
            .iter()
            .filter(|m| m.target == INBOX)
            .map(|m| inbox_decode_msg(&m.payload).expect("inbox msg"))
            .collect()
    }
}

#[async_trait::async_trait(?Send)]
impl Ctx for CaptureCtx {
    fn env(&self) -> &Env {
        &self.env
    }
    fn module_root(&self, _target: &str) -> Option<StateRoot> {
        None
    }
    async fn query(&self, target: &str, req: &[u8]) -> Result<Vec<u8>, Error> {
        if self.fail_query {
            return Err(Error::Module("query failed".into()));
        }
        match target {
            CHAT => match chat_decode_query(req).map_err(Error::Module)? {
                ChatQuery::MessagesRange {
                    channel_id,
                    from_seq,
                    limit,
                } => {
                    let transcript = self.transcripts.get(&channel_id).ok_or_else(|| {
                        Error::Module(format!("unknown channel: {channel_id}"))
                    })?;
                    let head = transcript.len() as u64;
                    let from = from_seq.max(1);
                    let mut window = Vec::new();
                    if limit > 0 && from <= head {
                        let to = head.min(from + limit - 1);
                        window = transcript[(from - 1) as usize..to as usize].to_vec();
                    }
                    Ok(chat_encode_reply(&ChatReply::Messages(window)))
                }
                ChatQuery::Channel { channel_id } => {
                    let channel = self.channels.contains(&channel_id).then(|| Channel {
                        id: channel_id.clone(),
                        name: channel_id,
                        created_at: 0,
                        head_seq: 0,
                        post_policy: PostPolicy::Open,
                        hooks: Vec::new(),
                        pinned: Vec::new(),
                        huddle: Vec::new(),
                        owner: None,
                        archived: false,
                    });
                    Ok(chat_encode_reply(&ChatReply::Channel(channel)))
                }
                ChatQuery::Message { message_id } => {
                    Ok(chat_encode_reply(&ChatReply::Message(
                        self.transcripts
                            .values()
                            .flatten()
                            .find(|view| view.head.message_id == message_id)
                            .cloned(),
                    )))
                }
            },
            TASKS => Ok(tasks_encode_reply(&TaskReply::Tasks(self.tasks.clone()))),
            other => Err(Error::UnknownModule(other.into())),
        }
    }
    fn emit_msg(&mut self, msg: Msg) {
        self.msgs.push(msg);
    }
    fn emit_event(&mut self, _ev: Event) {}
}

// ---- fixtures -----------------------------------------------------------

fn module() -> Automations {
    Automations::new(ME, Box::new(MemStore::new()), CHAT, TASKS, INBOX)
}

fn user(byte: u8) -> AuthorRef {
    AuthorRef::User(vec![byte; 4])
}

fn post_trigger(channel: Option<&str>, text_contains: Option<&str>) -> Trigger {
    Trigger {
        channel_id: channel.map(Into::into),
        mention: None,
        text_contains: text_contains.map(Into::into),
    }
}

fn post_action(channel: &str, template: &str) -> Action {
    Action::PostMessage {
        channel_id: channel.into(),
        template: template.into(),
    }
}

fn task_action(prefix: &str, title: &str) -> Action {
    Action::CreateTask {
        task_id_prefix: prefix.into(),
        title_template: title.into(),
    }
}

fn inbox_action(member: &str, kind: &str, body: &str) -> Action {
    Action::DeliverInbox {
        member_template: member.into(),
        kind: kind.into(),
        body_template: body.into(),
    }
}

fn admin(m: &AutomationsMsg) -> Msg {
    Msg {
        target: ME.into(),
        payload: encode_msg(m),
    }
}

fn create(rule_id: &str, trigger: Trigger, action: Action) -> Msg {
    admin(&AutomationsMsg::CreateRule {
        rule_id: rule_id.into(),
        trigger,
        action,
    })
}

/// a hook event as chat delivers it: raw ChatEvent bytes.
fn posted(channel: &str, seq: u64, author: AuthorRef, mentions: Vec<AuthorRef>) -> Msg {
    Msg {
        target: ME.into(),
        payload: chat_encode_event(&ChatEvent::MessagePosted {
            channel_id: channel.into(),
            seq,
            thread_root: None,
            author,
            mentions,
        }),
    }
}

fn message(channel: &str, seq: u64, author: AuthorRef, blocks: Vec<Block>) -> MessageView {
    MessageView {
        channel_id: channel.into(),
        seq,
        head: MessageHead {
            message_id: format!("{channel}-m{seq}"),
            author,
            blocks,
            created_at: 0,
            rev: 0,
            edited_at: None,
            base_rev: None,
            deleted: false,
            thread: None,
            reply_count: 0,
            last_reply_seq: None,
        },
    }
}

fn list_rules(m: &Automations) -> Vec<Rule> {
    match decode_reply(
        &block_on(m.query(&encode_query(&AutomationsQuery::ListRules))).expect("query"),
    )
    .expect("reply")
    {
        AutomationsReply::Rules(rules) => rules,
        other => panic!("expected Rules, got {other:?}"),
    }
}

fn get_rule(m: &Automations, rule_id: &str) -> Option<Rule> {
    match decode_reply(
        &block_on(m.query(&encode_query(&AutomationsQuery::GetRule {
            rule_id: rule_id.into(),
        })))
        .expect("query"),
    )
    .expect("reply")
    {
        AutomationsReply::Rule(rule) => rule,
        other => panic!("expected Rule, got {other:?}"),
    }
}

fn history(m: &Automations, rule_id: &str, limit: u64) -> Vec<RunRecord> {
    match decode_reply(
        &block_on(m.query(&encode_query(&AutomationsQuery::RunHistory {
            rule_id: rule_id.into(),
            limit,
        })))
        .expect("query"),
    )
    .expect("reply")
    {
        AutomationsReply::History(records) => records,
        other => panic!("expected History, got {other:?}"),
    }
}

fn exec(m: &mut Automations, ctx: &mut CaptureCtx, msg: &Msg) -> Result<(), Error> {
    block_on(m.execute(ctx, msg))
}

// ---- rule CRUD ----------------------------------------------------------

#[test]
fn create_list_and_commit_rules() {
    let mut m = module();
    let mut ctx = CaptureCtx::new();
    exec(
        &mut m,
        &mut ctx,
        &create("r-b", post_trigger(None, None), task_action("t", "T")),
    )
    .expect("create b");
    exec(
        &mut m,
        &mut ctx,
        &create("r-a", post_trigger(None, None), task_action("t", "T")),
    )
    .expect("create a");

    // staged reads see both, in rule_id order; committed root has not moved.
    let root0 = m.root();
    let ids: Vec<String> = list_rules(&m).into_iter().map(|r| r.rule_id).collect();
    assert_eq!(ids, ["r-a", "r-b"], "list order is deterministic");
    assert_eq!(
        m.root(),
        root0,
        "staged writes do not move the committed root"
    );

    block_on(m.commit_block()).expect("commit");
    assert_ne!(m.root(), root0, "commit moves the root");
    assert!(get_rule(&m, "r-a").expect("r-a").enabled);
    assert_eq!(get_rule(&m, "r-a").expect("r-a").created_at, 42);
}

#[test]
fn duplicate_rule_id_is_rejected() {
    let mut m = module();
    let mut ctx = CaptureCtx::new();
    exec(
        &mut m,
        &mut ctx,
        &create("dup", post_trigger(None, None), task_action("t", "T")),
    )
    .expect("create");
    block_on(m.commit_block()).expect("commit");
    let err = exec(
        &mut m,
        &mut ctx,
        &create("dup", post_trigger(None, None), task_action("t", "T")),
    )
    .expect_err("duplicate must reject");
    assert!(matches!(err, Error::Module(msg) if msg.contains("already exists")));
}

#[test]
fn set_enabled_and_delete() {
    let mut m = module();
    let mut ctx = CaptureCtx::new();
    exec(
        &mut m,
        &mut ctx,
        &create("r", post_trigger(None, None), task_action("t", "T")),
    )
    .expect("create");
    block_on(m.commit_block()).expect("commit");

    exec(
        &mut m,
        &mut ctx,
        &admin(&AutomationsMsg::SetEnabled {
            rule_id: "r".into(),
            enabled: false,
        }),
    )
    .expect("disable");
    block_on(m.commit_block()).expect("commit");
    assert!(!get_rule(&m, "r").expect("r").enabled);

    exec(
        &mut m,
        &mut ctx,
        &admin(&AutomationsMsg::DeleteRule {
            rule_id: "r".into(),
        }),
    )
    .expect("delete");
    block_on(m.commit_block()).expect("commit");
    assert!(get_rule(&m, "r").is_none());
    assert!(list_rules(&m).is_empty());
}

#[test]
fn set_enabled_unknown_rule_rejected() {
    let mut m = module();
    let mut ctx = CaptureCtx::new();
    let err = exec(
        &mut m,
        &mut ctx,
        &admin(&AutomationsMsg::SetEnabled {
            rule_id: "ghost".into(),
            enabled: true,
        }),
    )
    .expect_err("unknown rule");
    assert!(matches!(err, Error::Module(msg) if msg.contains("unknown rule")));
}

#[test]
fn caps_are_enforced_at_execute() {
    let mut m = module();
    let mut ctx = CaptureCtx::new();

    // empty rule_id.
    assert!(
        exec(
            &mut m,
            &mut ctx,
            &create("", post_trigger(None, None), task_action("t", "T")),
        )
        .is_err()
    );

    // oversize template.
    let huge = "x".repeat(MAX_TEMPLATE_BYTES + 1);
    assert!(
        exec(
            &mut m,
            &mut ctx,
            &create("big", post_trigger(None, None), post_action("c", &huge)),
        )
        .is_err()
    );

    // oversize rule_id.
    let long_id = "r".repeat(MAX_ID_BYTES + 1);
    assert!(
        exec(
            &mut m,
            &mut ctx,
            &create(&long_id, post_trigger(None, None), task_action("t", "T")),
        )
        .is_err()
    );

    // empty action channel_id.
    assert!(
        exec(
            &mut m,
            &mut ctx,
            &create("x", post_trigger(None, None), post_action("", "hi")),
        )
        .is_err()
    );

    // empty inbox kind.
    assert!(
        exec(
            &mut m,
            &mut ctx,
            &create(
                "inbox-empty-kind",
                post_trigger(None, None),
                inbox_action("alice", "", "body")
            ),
        )
        .is_err()
    );

    // oversized inbox kind.
    assert!(
        exec(
            &mut m,
            &mut ctx,
            &create(
                "inbox-big-kind",
                post_trigger(None, None),
                inbox_action("alice", &"k".repeat(65), "body")
            ),
        )
        .is_err()
    );

    assert!(list_rules(&m).is_empty(), "no rejected rule was staged");
}

// ---- origin + author gating --------------------------------------------

#[test]
fn hook_event_from_non_chat_origin_is_rejected() {
    let mut m = module();
    // an explicit HookEvent wrapper from an external submitter is a spoof.
    let mut ext = CaptureCtx::new().with_origin(Origin::External(vec![9; 4]));
    let err = exec(
        &mut m,
        &mut ext,
        &Msg {
            target: ME.into(),
            payload: encode_msg(&AutomationsMsg::HookEvent(vec![1, 2, 3])),
        },
    )
    .expect_err("hook from non-chat origin must reject");
    assert!(matches!(err, Error::Module(msg) if msg.contains("chat module")));

    // raw ChatEvent bytes from a non-chat origin fail to decode as an
    // AutomationsMsg — also rejected.
    assert!(exec(&mut m, &mut ext, &posted("general", 1, user(1), Vec::new()),).is_err());
}

#[test]
fn a_rule_records_its_creator_and_only_the_creator_administers_it() {
    let mut m = module();
    let mut owner = CaptureCtx::new();
    exec(
        &mut m,
        &mut owner,
        &create("r", post_trigger(None, None), task_action("t", "T")),
    )
    .expect("create");
    block_on(m.commit_block()).expect("commit");
    assert_eq!(
        get_rule(&m, "r").expect("r").owner,
        OWNER,
        "the submitter of CreateRule is the rule's owner"
    );

    // a stranger may neither disable nor delete it. a rule is a STANDING
    // capability — an ungated SetEnabled is a kill switch on someone else's
    // automation, and an ungated DeleteRule removes it outright.
    let mut stranger = CaptureCtx::new().with_origin(Origin::External(STRANGER.to_vec()));
    for op in [
        AutomationsMsg::SetEnabled {
            rule_id: "r".into(),
            enabled: false,
        },
        AutomationsMsg::DeleteRule {
            rule_id: "r".into(),
        },
    ] {
        let err = exec(&mut m, &mut stranger, &admin(&op)).expect_err("stranger must be refused");
        assert!(
            matches!(&err, Error::Module(msg) if msg.contains("only the owner")),
            "a non-owner must be refused: {err:?}"
        );
        block_on(m.abort_block()).expect("abort");
    }
    assert!(get_rule(&m, "r").expect("r").enabled, "nothing landed");

    // the owner performs both.
    exec(
        &mut m,
        &mut owner,
        &admin(&AutomationsMsg::SetEnabled {
            rule_id: "r".into(),
            enabled: false,
        }),
    )
    .expect("owner disables");
    exec(
        &mut m,
        &mut owner,
        &admin(&AutomationsMsg::DeleteRule {
            rule_id: "r".into(),
        }),
    )
    .expect("owner deletes");
    block_on(m.commit_block()).expect("commit");
    assert!(get_rule(&m, "r").is_none());
    assert!(list_rules(&m).is_empty());
}

#[test]
fn a_stranger_cannot_walk_past_the_gate_on_a_no_op_set_enabled() {
    // SetEnabled to the value a rule ALREADY holds stages nothing, so the
    // owner check must come BEFORE that short-circuit or the gate is
    // bypassable — and a stranger must not learn the rule's state from which
    // refusal comes back.
    let mut m = module();
    let mut owner = CaptureCtx::new();
    exec(
        &mut m,
        &mut owner,
        &create("r", post_trigger(None, None), task_action("t", "T")),
    )
    .expect("create");
    block_on(m.commit_block()).expect("commit");

    let mut stranger = CaptureCtx::new().with_origin(Origin::External(STRANGER.to_vec()));
    let err = exec(
        &mut m,
        &mut stranger,
        &admin(&AutomationsMsg::SetEnabled {
            rule_id: "r".into(),
            enabled: true,
        }),
    )
    .expect_err("an idempotent op is still an op");
    assert!(matches!(&err, Error::Module(msg) if msg.contains("only the owner")));
}

#[test]
fn an_ownerless_rule_is_unrepresentable() {
    // only an authenticated external submitter may own a rule, so every other
    // origin is refused at CreateRule — there is no shape in which a rule
    // exists without a principal answerable for it.
    let mut m = module();
    for (origin, refusal) in [
        (Origin::System, "system origin"),
        (Origin::Module("automations".into()), "module origin"),
        (Origin::Module("governance".into()), "module origin"),
        (Origin::External(Vec::new()), "non-empty submitter id"),
    ] {
        let mut ctx = CaptureCtx::new().with_origin(origin.clone());
        let err = exec(
            &mut m,
            &mut ctx,
            &create("r", post_trigger(None, None), task_action("t", "T")),
        )
        .expect_err("an unownable origin must be refused");
        assert!(
            matches!(&err, Error::Module(msg) if msg.contains(refusal)),
            "{origin:?} must be refused with {refusal}: {err:?}"
        );
        block_on(m.abort_block()).expect("abort");
    }
    assert!(list_rules(&m).is_empty(), "no rule was staged");
}

#[test]
fn creating_a_rule_is_gated_but_firing_one_is_not() {
    // the two principals are different and must stay so: a rule is CREATED by
    // its owner's external origin, and RUN under `Origin::Module("automations")`.
    // the hook lane is routed before the owner gate, so a chat event still
    // fires the rule and its action still leaves as a module-authority
    // follow-up — while the module's own origin cannot mint a rule.
    let mut m = module();
    let mut owner = CaptureCtx::new();
    exec(
        &mut m,
        &mut owner,
        &create("r", post_trigger(None, None), task_action("auto", "T")),
    )
    .expect("create");
    block_on(m.commit_block()).expect("commit");

    let mut chat_ctx = CaptureCtx::new().with_chat_origin();
    exec(
        &mut m,
        &mut chat_ctx,
        &posted("general", 1, user(1), Vec::new()),
    )
    .expect("hook events never carry a submitter, and never need one");
    assert_eq!(
        chat_ctx.task_msgs(),
        vec![TaskMsg::CreateTask {
            task_id: "auto-general-1".into(),
            title: "T".into(),
        }],
        "the owner-gated rule still fires under module authority"
    );
    assert_eq!(get_rule(&m, "r").expect("r").owner, OWNER);
}

#[test]
fn only_user_authored_posts_fire() {
    let mut m = module();
    let mut ctx = CaptureCtx::new();
    exec(
        &mut m,
        &mut ctx,
        &create("r", post_trigger(None, None), task_action("t", "T")),
    )
    .expect("create");
    block_on(m.commit_block()).expect("commit");

    // a module-authored post (e.g. our own follow-up) never fires.
    let mut chat_ctx = CaptureCtx::new().with_chat_origin();
    exec(
        &mut m,
        &mut chat_ctx,
        &posted(
            "general",
            1,
            AuthorRef::Module("automations".into()),
            Vec::new(),
        ),
    )
    .expect("no-fail arm");
    assert!(chat_ctx.msgs.is_empty(), "module posts must not trigger");
    block_on(m.commit_block()).expect("commit");
    assert_eq!(get_rule(&m, "r").expect("r").fire_count, 0);
    assert!(history(&m, "r", 16).is_empty());
}

// ---- matching + action emission ----------------------------------------

#[test]
fn create_task_action_emits_deterministic_task_id() {
    let mut m = module();
    let mut ctx = CaptureCtx::new();
    exec(
        &mut m,
        &mut ctx,
        &create(
            "r",
            post_trigger(Some("general"), None),
            task_action("todo", "from {channel} #{seq}"),
        ),
    )
    .expect("create");
    block_on(m.commit_block()).expect("commit");

    let mut chat_ctx = CaptureCtx::new().with_chat_origin();
    exec(
        &mut m,
        &mut chat_ctx,
        &posted("general", 5, user(1), Vec::new()),
    )
    .expect("fire");
    let tasks = chat_ctx.task_msgs();
    assert_eq!(tasks.len(), 1);
    let TaskMsg::CreateTask { task_id, title } = &tasks[0] else {
        panic!("expected CreateTask");
    };
    assert_eq!(task_id, "todo-general-5", "deterministic task id");
    assert_eq!(title, "from general #5", "substituted title");

    block_on(m.commit_block()).expect("commit");
    assert_eq!(get_rule(&m, "r").expect("r").fire_count, 1);
    let recs = history(&m, "r", 16);
    assert_eq!(recs.len(), 1);
    assert!(recs[0].action_ok);
    assert_eq!(recs[0].seq, 5);
    assert_eq!(recs[0].height, 7);
}

#[test]
fn post_message_action_emits_deterministic_message_id() {
    let mut m = module();
    let mut ctx = CaptureCtx::new();
    exec(
        &mut m,
        &mut ctx,
        &create(
            "greet",
            post_trigger(Some("general"), None),
            post_action("announce", "welcome from {channel}/{seq}"),
        ),
    )
    .expect("create");
    block_on(m.commit_block()).expect("commit");

    let mut chat_ctx = CaptureCtx::new().with_chat_origin().with_channel("announce");
    exec(
        &mut m,
        &mut chat_ctx,
        &posted("general", 3, user(2), Vec::new()),
    )
    .expect("fire");
    let posts = chat_ctx.chat_msgs();
    assert_eq!(posts.len(), 1);
    let ChatMsg::PostMessage {
        channel_id,
        message_id,
        blocks,
        ..
    } = &posts[0]
    else {
        panic!("expected PostMessage");
    };
    assert_eq!(channel_id, "announce", "posts to the action channel");
    assert_eq!(
        message_id, "auto-greet-general-3",
        "deterministic message id"
    );
    assert_eq!(blocks, &vec![Block::paragraph("welcome from general/3")]);
}

/// the same PostMessage fire, but the live chat probes (Channel existence +
/// Message-id freshness) are served through the shared `sdk_testkit::TestCtx`
/// `on_query` seam instead of the crate-local `CaptureCtx` — the sibling-read
/// capability the hand-rolled doubles lacked, driving the module's REAL
/// `execute` against a programmed chat response.
#[test]
fn post_message_fire_reads_chat_via_testkit_on_query() {
    let mut m = module();
    let mut create_ctx = CaptureCtx::new();
    exec(
        &mut m,
        &mut create_ctx,
        &create(
            "greet",
            post_trigger(Some("general"), None),
            post_action("announce", "welcome from {channel}/{seq}"),
        ),
    )
    .expect("create");
    block_on(m.commit_block()).expect("commit");

    // chat delivers the hook event (origin = the chat module), and the shared
    // TestCtx answers automations' two chat probes: "announce" exists, and the
    // composed message id is still free.
    let mut ctx = TestCtx::with_env(Env {
        height: 7,
        consensus_time: 42,
        origin: Origin::Module(CHAT.into()),
        me: ME.into(),
    })
    .on_query(CHAT, |req| match chat_decode_query(req).map_err(Error::Module)? {
        ChatQuery::Channel { channel_id } => Ok(chat_encode_reply(&ChatReply::Channel(Some(Channel {
            id: channel_id.clone(),
            name: channel_id,
            created_at: 0,
            head_seq: 0,
            post_policy: PostPolicy::Open,
            hooks: Vec::new(),
            pinned: Vec::new(),
            huddle: Vec::new(),
            owner: None,
            archived: false,
        })))),
        ChatQuery::Message { .. } => Ok(chat_encode_reply(&ChatReply::Message(None))),
        _ => Err(Error::QueryUnsupported),
    });

    block_on(m.execute(&mut ctx, &posted("general", 3, user(2), Vec::new()))).expect("fire");

    let posts: Vec<ChatMsg> = ctx
        .msgs()
        .iter()
        .filter(|msg| msg.target == CHAT)
        .map(|msg| chat_decode_msg(&msg.payload).expect("chat msg"))
        .collect();
    assert_eq!(posts.len(), 1, "the served chat probe let the post through");
    let ChatMsg::PostMessage {
        channel_id,
        message_id,
        ..
    } = &posts[0]
    else {
        panic!("expected PostMessage");
    };
    assert_eq!(channel_id, "announce");
    assert_eq!(message_id, "auto-greet-general-3");
}

#[test]
fn channel_filter_gates_matching() {
    let mut m = module();
    let mut ctx = CaptureCtx::new();
    exec(
        &mut m,
        &mut ctx,
        &create("r", post_trigger(Some("ops"), None), task_action("t", "T")),
    )
    .expect("create");
    block_on(m.commit_block()).expect("commit");

    // wrong channel: no fire.
    let mut wrong = CaptureCtx::new().with_chat_origin();
    exec(
        &mut m,
        &mut wrong,
        &posted("general", 1, user(1), Vec::new()),
    )
    .expect("no fire");
    assert!(wrong.msgs.is_empty());

    // right channel: fires.
    let mut right = CaptureCtx::new().with_chat_origin();
    exec(&mut m, &mut right, &posted("ops", 1, user(1), Vec::new())).expect("fire");
    assert_eq!(right.task_msgs().len(), 1);
}

#[test]
fn mention_filter_matches_agent_display() {
    let mut m = module();
    let mut ctx = CaptureCtx::new();
    exec(
        &mut m,
        &mut ctx,
        &create(
            "r",
            Trigger {
                channel_id: None,
                mention: Some("helper".into()),
                text_contains: None,
            },
            task_action("t", "T"),
        ),
    )
    .expect("create");
    block_on(m.commit_block()).expect("commit");

    let helper = AuthorRef::Agent {
        module: "agent".into(),
        agent_id: "helper".into(),
    };
    // mention present -> fire.
    let mut hit = CaptureCtx::new().with_chat_origin();
    exec(
        &mut m,
        &mut hit,
        &posted("general", 1, user(1), vec![helper.clone()]),
    )
    .expect("fire");
    assert_eq!(hit.task_msgs().len(), 1);

    // no matching mention -> no fire.
    let mut miss = CaptureCtx::new().with_chat_origin();
    exec(
        &mut m,
        &mut miss,
        &posted("general", 2, user(1), Vec::new()),
    )
    .expect("no fire");
    assert!(miss.msgs.is_empty());
}

#[test]
fn text_contains_filter_fetches_message_once() {
    let mut m = module();
    let mut ctx = CaptureCtx::new();
    exec(
        &mut m,
        &mut ctx,
        &create(
            "r",
            post_trigger(Some("general"), Some("deploy")),
            task_action("t", "seen: {text}"),
        ),
    )
    .expect("create");
    block_on(m.commit_block()).expect("commit");

    let msg_hit = message(
        "general",
        1,
        user(1),
        vec![Block::paragraph("please deploy now")],
    );
    let mut hit = CaptureCtx::new()
        .with_chat_origin()
        .with_transcript("general", vec![msg_hit]);
    exec(&mut m, &mut hit, &posted("general", 1, user(1), Vec::new())).expect("fire");
    let tasks = hit.task_msgs();
    assert_eq!(tasks.len(), 1);
    let TaskMsg::CreateTask { title, .. } = &tasks[0] else {
        panic!("expected CreateTask");
    };
    assert_eq!(title, "seen: please deploy now", "{{text}} substituted");

    // text without the substring -> no fire.
    let msg_miss = message(
        "general",
        1,
        user(1),
        vec![Block::paragraph("just chatting")],
    );
    let mut miss = CaptureCtx::new()
        .with_chat_origin()
        .with_transcript("general", vec![msg_miss]);
    exec(
        &mut m,
        &mut miss,
        &posted("general", 1, user(1), Vec::new()),
    )
    .expect("no fire");
    assert!(miss.msgs.is_empty());
}

#[test]
fn chat_trigger_can_deliver_inbox_with_chat_placeholders() {
    let mut m = module();
    let mut ctx = CaptureCtx::new();
    exec(
        &mut m,
        &mut ctx,
        &create(
            "notify-chat",
            post_trigger(Some("general"), None),
            inbox_action(
                "{mention}",
                "chat",
                "channel={channel} seq={seq} author={author} text={text} mention={mention}",
            ),
        ),
    )
    .expect("create");
    block_on(m.commit_block()).expect("commit");

    let mentioned = AuthorRef::Agent {
        module: "agent".into(),
        agent_id: "helper".into(),
    };
    let msg = message(
        "general",
        1,
        user(3),
        vec![Block::paragraph("please review")],
    );
    let mut chat_ctx = CaptureCtx::new()
        .with_chat_origin()
        .with_transcript("general", vec![msg]);
    exec(
        &mut m,
        &mut chat_ctx,
        &posted("general", 1, user(3), vec![mentioned]),
    )
    .expect("fire");

    let delivered = chat_ctx.inbox_msgs();
    assert_eq!(delivered.len(), 1);
    let InboxMsg::Deliver { member, kind, body } = &delivered[0] else {
        panic!("expected Deliver");
    };
    assert_eq!(member, "agent/helper", "member uses first mention display");
    assert_eq!(kind, "chat");
    assert_eq!(
        body,
        "channel=general seq=1 author=user:03030303 text=please review mention=agent/helper"
    );
}

#[test]
fn failed_text_fetch_is_recorded_not_guessed_empty() {
    let mut m = module();
    let mut ctx = CaptureCtx::new();
    exec(
        &mut m,
        &mut ctx,
        &create(
            "r",
            post_trigger(Some("general"), Some("deploy")),
            task_action("t", "T"),
        ),
    )
    .expect("create");
    block_on(m.commit_block()).expect("commit");

    // the fetch fails -> the text-needing rule cannot be evaluated: a
    // recorded failure (never empty-text guessing), no emit, and crucially
    // the block is NOT aborted.
    let mut ctx = CaptureCtx::new().failing_query().with_chat_origin();
    exec(&mut m, &mut ctx, &posted("general", 1, user(1), Vec::new()))
        .expect("no-fail arm survives a failed fetch");
    assert!(ctx.msgs.is_empty());
    block_on(m.commit_block()).expect("commit");
    assert_eq!(get_rule(&m, "r").expect("r").fire_count, 0);
    let recs = history(&m, "r", 4);
    assert_eq!(recs.len(), 1);
    assert!(!recs[0].action_ok);
    assert_eq!(recs[0].detail, "text fetch failed");
}

#[test]
fn legitimately_empty_body_is_valid_text() {
    let mut m = module();
    let mut ctx = CaptureCtx::new();
    // an empty text_contains filter matches an empty body; {text}
    // substitutes as empty. only a FAILED fetch is a failure.
    exec(
        &mut m,
        &mut ctx,
        &create(
            "r",
            post_trigger(Some("general"), Some("")),
            task_action("t", "seen [{text}]"),
        ),
    )
    .expect("create");
    block_on(m.commit_block()).expect("commit");

    let empty_body = message("general", 1, user(1), vec![Block::paragraph("")]);
    let mut chat_ctx = CaptureCtx::new()
        .with_chat_origin()
        .with_transcript("general", vec![empty_body]);
    exec(
        &mut m,
        &mut chat_ctx,
        &posted("general", 1, user(1), Vec::new()),
    )
    .expect("fire");
    let tasks = chat_ctx.task_msgs();
    assert_eq!(tasks.len(), 1, "an empty body is valid content");
    let TaskMsg::CreateTask { title, .. } = &tasks[0] else {
        panic!("expected CreateTask");
    };
    assert_eq!(title, "seen []");
}

// ---- malformed action + budget -----------------------------------------

#[test]
fn empty_template_records_action_ok_false_without_failing() {
    let mut m = module();
    let mut ctx = CaptureCtx::new();
    exec(
        &mut m,
        &mut ctx,
        &create("r", post_trigger(None, None), post_action("c", "")),
    )
    .expect("create");
    block_on(m.commit_block()).expect("commit");

    let mut chat_ctx = CaptureCtx::new().with_chat_origin();
    exec(
        &mut m,
        &mut chat_ctx,
        &posted("general", 1, user(1), Vec::new()),
    )
    .expect("no block failure");
    assert!(chat_ctx.msgs.is_empty(), "no action emitted");

    block_on(m.commit_block()).expect("commit");
    assert_eq!(
        get_rule(&m, "r").expect("r").fire_count,
        0,
        "no successful fire"
    );
    let recs = history(&m, "r", 16);
    assert_eq!(recs.len(), 1);
    assert!(!recs[0].action_ok);
    assert!(recs[0].detail.contains("empty message"));
}

#[test]
fn inbox_member_over_cap_records_action_ok_false_without_emitting() {
    let mut m = module();
    let mut ctx = CaptureCtx::new();
    exec(
        &mut m,
        &mut ctx,
        &create(
            "member-cap",
            post_trigger(None, None),
            inbox_action("{channel}", "chat", "body"),
        ),
    )
    .expect("create");
    block_on(m.commit_block()).expect("commit");

    // chat does not bound channel-id length, so a `{channel}` member can be
    // substituted past the inbox member cap.
    let long_channel = "c".repeat(MAX_MEMBER_BYTES + 1);
    let mut chat_ctx = CaptureCtx::new().with_chat_origin();
    exec(
        &mut m,
        &mut chat_ctx,
        &posted(&long_channel, 1, user(1), Vec::new()),
    )
    .expect("fire records failure");
    assert!(chat_ctx.msgs.is_empty());
    block_on(m.commit_block()).expect("commit");

    let recs = history(&m, "member-cap", 16);
    assert_eq!(recs.len(), 1);
    assert!(!recs[0].action_ok);
    assert!(recs[0].detail.contains("inbox member exceeds cap"));
    assert_eq!(get_rule(&m, "member-cap").expect("rule").fire_count, 0);
}

#[test]
fn inbox_body_over_cap_records_action_ok_false_without_emitting() {
    let mut m = module();
    let mut ctx = CaptureCtx::new();
    exec(
        &mut m,
        &mut ctx,
        &create(
            "body-cap",
            post_trigger(None, None),
            inbox_action("alice", "chat", "{channel}"),
        ),
    )
    .expect("create");
    block_on(m.commit_block()).expect("commit");

    // an unbounded `{channel}` body substitutes past the inbox body cap.
    let long_channel = "c".repeat(INBOX_MAX_BODY_BYTES + 1);
    let mut chat_ctx = CaptureCtx::new().with_chat_origin();
    exec(
        &mut m,
        &mut chat_ctx,
        &posted(&long_channel, 1, user(1), Vec::new()),
    )
    .expect("fire records failure");
    assert!(chat_ctx.msgs.is_empty());
    block_on(m.commit_block()).expect("commit");

    let recs = history(&m, "body-cap", 16);
    assert_eq!(recs.len(), 1);
    assert!(!recs[0].action_ok);
    assert!(recs[0].detail.contains("inbox body exceeds cap"));
    assert_eq!(get_rule(&m, "body-cap").expect("rule").fire_count, 0);
}

#[test]
fn action_budget_caps_emissions_per_event() {
    let mut m = module();
    let mut ctx = CaptureCtx::new();
    // create more matching rules than the budget allows.
    let n = MAX_ACTIONS_PER_EVENT + 3;
    for i in 0..n {
        exec(
            &mut m,
            &mut ctx,
            &create(
                &format!("r{i:02}"),
                post_trigger(None, None),
                task_action("t", "T"),
            ),
        )
        .expect("create");
    }
    block_on(m.commit_block()).expect("commit");

    let mut chat_ctx = CaptureCtx::new().with_chat_origin();
    exec(
        &mut m,
        &mut chat_ctx,
        &posted("general", 1, user(1), Vec::new()),
    )
    .expect("fire");
    assert_eq!(
        chat_ctx.task_msgs().len(),
        MAX_ACTIONS_PER_EVENT,
        "exactly the budget is emitted"
    );
    block_on(m.commit_block()).expect("commit");

    // the first MAX rules (ascending) fired; the rest are budget-skipped.
    let fired = (0..MAX_ACTIONS_PER_EVENT)
        .filter(|i| get_rule(&m, &format!("r{i:02}")).expect("rule").fire_count == 1)
        .count();
    assert_eq!(fired, MAX_ACTIONS_PER_EVENT);
    let skipped = get_rule(&m, &format!("r{:02}", n - 1)).expect("last rule");
    assert_eq!(skipped.fire_count, 0);
    let recs = history(&m, &format!("r{:02}", n - 1), 4);
    assert_eq!(recs.len(), 1);
    assert!(!recs[0].action_ok);
    assert_eq!(recs[0].detail, "action budget exceeded");
}

#[test]
fn disabled_rules_do_not_fire() {
    let mut m = module();
    let mut ctx = CaptureCtx::new();
    exec(
        &mut m,
        &mut ctx,
        &create("r", post_trigger(None, None), task_action("t", "T")),
    )
    .expect("create");
    exec(
        &mut m,
        &mut ctx,
        &admin(&AutomationsMsg::SetEnabled {
            rule_id: "r".into(),
            enabled: false,
        }),
    )
    .expect("disable");
    block_on(m.commit_block()).expect("commit");

    let mut chat_ctx = CaptureCtx::new().with_chat_origin();
    exec(
        &mut m,
        &mut chat_ctx,
        &posted("general", 1, user(1), Vec::new()),
    )
    .expect("no fire");
    assert!(chat_ctx.msgs.is_empty());
}

// ---- pre-emit probes + guards -------------------------------------------

#[test]
fn missing_target_channel_is_recorded_not_emitted() {
    let mut m = module();
    let mut ctx = CaptureCtx::new();
    exec(
        &mut m,
        &mut ctx,
        &create("r", post_trigger(None, None), post_action("ghost", "hi")),
    )
    .expect("create");
    block_on(m.commit_block()).expect("commit");

    // "ghost" is not a known channel: the probe records, never emits.
    let mut chat_ctx = CaptureCtx::new().with_chat_origin();
    exec(
        &mut m,
        &mut chat_ctx,
        &posted("general", 1, user(1), Vec::new()),
    )
    .expect("no-fail arm");
    assert!(chat_ctx.msgs.is_empty(), "no post to a missing channel");
    block_on(m.commit_block()).expect("commit");
    let recs = history(&m, "r", 4);
    assert_eq!(recs.len(), 1);
    assert!(!recs[0].action_ok);
    assert!(recs[0].detail.contains("does not exist"));
    assert_eq!(get_rule(&m, "r").expect("r").fire_count, 0);
}

#[test]
fn squatted_message_id_is_caught_by_probe() {
    let mut m = module();
    let mut ctx = CaptureCtx::new();
    exec(
        &mut m,
        &mut ctx,
        &create("r", post_trigger(None, None), post_action("general", "hi")),
    )
    .expect("create");
    block_on(m.commit_block()).expect("commit");

    // the deterministic id for the seq-1 fire is already taken: a user
    // pre-posted it (id squatting). the probe records, never emits — the
    // emit would abort the posting block at chat's duplicate-id check.
    let mut squatted = message("general", 1, user(9), vec![Block::paragraph("squat")]);
    squatted.head.message_id = "auto-r-general-1".into();
    let mut chat_ctx = CaptureCtx::new()
        .with_chat_origin()
        .with_transcript("general", vec![squatted]);
    exec(
        &mut m,
        &mut chat_ctx,
        &posted("general", 1, user(1), Vec::new()),
    )
    .expect("no-fail arm");
    assert!(chat_ctx.msgs.is_empty(), "no emit against a squatted id");
    block_on(m.commit_block()).expect("commit");
    let recs = history(&m, "r", 4);
    assert_eq!(recs.len(), 1);
    assert!(!recs[0].action_ok);
    assert!(recs[0].detail.contains("already taken"));
}

#[test]
fn task_id_collision_is_caught_by_probe() {
    let mut m = module();
    let mut ctx = CaptureCtx::new();
    exec(
        &mut m,
        &mut ctx,
        &create("r", post_trigger(None, None), task_action("auto", "T")),
    )
    .expect("create");
    block_on(m.commit_block()).expect("commit");

    let mut chat_ctx = CaptureCtx::new().with_chat_origin().with_task("auto-general-5");
    exec(
        &mut m,
        &mut chat_ctx,
        &posted("general", 5, user(1), Vec::new()),
    )
    .expect("no-fail arm");
    assert!(chat_ctx.msgs.is_empty(), "no emit against a taken task id");
    block_on(m.commit_block()).expect("commit");
    let recs = history(&m, "r", 4);
    assert_eq!(recs.len(), 1);
    assert!(!recs[0].action_ok);
    assert!(recs[0].detail.contains("already exists"));
}

#[test]
fn oversized_composed_id_is_recorded_not_emitted() {
    let mut m = module();
    let mut ctx = CaptureCtx::new();
    exec(
        &mut m,
        &mut ctx,
        &create("r", post_trigger(None, None), task_action("auto", "T")),
    )
    .expect("create");
    block_on(m.commit_block()).expect("commit");

    // an event channel long enough to push the composed id over the cap.
    let long_channel = "c".repeat(MAX_ID_BYTES);
    let mut chat_ctx = CaptureCtx::new().with_chat_origin();
    exec(
        &mut m,
        &mut chat_ctx,
        &posted(&long_channel, 1, user(1), Vec::new()),
    )
    .expect("no-fail arm");
    assert!(chat_ctx.msgs.is_empty(), "no emit with an oversized id");
    block_on(m.commit_block()).expect("commit");
    let recs = history(&m, "r", 4);
    assert_eq!(recs.len(), 1);
    assert!(!recs[0].action_ok);
    assert_eq!(recs[0].detail, "composed id exceeds cap");
}

#[test]
fn fire_count_saturates_at_u64_max() {
    // craft a committed state whose rule already sits at u64::MAX — staged
    // directly through the in-crate store seam (no admin op can produce this
    // count in a test's lifetime) — then fire: the count must saturate, not
    // wrap.
    let mut m = module();
    let mut ctx = CaptureCtx::new();
    exec(
        &mut m,
        &mut ctx,
        &create("r", post_trigger(None, None), task_action("auto", "T")),
    )
    .expect("create");
    block_on(m.commit_block()).expect("commit rule");
    let mut rule = block_on(m.rule("r")).expect("load").expect("r");
    rule.fire_count = u64::MAX;
    m.store(rule_key("r"), &rule);
    block_on(m.commit_block()).expect("commit crafted count");

    let mut chat_ctx = CaptureCtx::new().with_chat_origin();
    exec(
        &mut m,
        &mut chat_ctx,
        &posted("general", 1, user(1), Vec::new()),
    )
    .expect("fire");
    assert_eq!(chat_ctx.task_msgs().len(), 1, "the action still emits");
    block_on(m.commit_block()).expect("commit");
    assert_eq!(
        get_rule(&m, "r").expect("r").fire_count,
        u64::MAX,
        "fire_count saturates instead of wrapping"
    );
}

// ---- substitution -------------------------------------------------------

#[test]
fn substitution_covers_all_placeholders_single_pass() {
    // {author} for a user renders the hex pubkey; unknown tokens stay literal.
    let author = display_author(&AuthorRef::User(vec![0xab, 0xcd]));
    let out = substitute(
        "c={channel} s={seq} a={author} t={text} u={unknown}",
        "general",
        9,
        &author,
        "hello",
    );
    assert_eq!(
        out,
        format!("c=general s=9 a={author} t=hello u={{unknown}}")
    );
    assert_eq!(author, "user:abcd");
}

#[test]
fn substitution_does_not_rescan_substituted_values() {
    // a channel value that itself contains a placeholder token is not
    // re-substituted (single pass).
    let out = substitute("{channel}-{seq}", "{seq}", 4, "a", "t");
    assert_eq!(out, "{seq}-4");
}

#[test]
fn blocks_text_concatenates_text_bearing_blocks() {
    let blocks = vec![
        Block::Paragraph(vec![
            Span::plain("hello "),
            Span {
                text: "world".into(),
                marks: vec![Mark::Bold],
            },
        ]),
        Block::Divider,
        Block::Code {
            lang: None,
            text: "code line".into(),
        },
    ];
    assert_eq!(blocks_text(&blocks), "hello world\ncode line");
}

// ---- staging semantics --------------------------------------------------

#[test]
fn abort_discards_staged_rules_and_history() {
    let mut m = module();
    let mut ctx = CaptureCtx::new();
    exec(
        &mut m,
        &mut ctx,
        &create("r", post_trigger(None, None), task_action("t", "T")),
    )
    .expect("create");
    block_on(m.commit_block()).expect("commit");
    let root1 = m.root();

    // stage a fire then abort.
    let mut chat_ctx = CaptureCtx::new().with_chat_origin();
    exec(
        &mut m,
        &mut chat_ctx,
        &posted("general", 1, user(1), Vec::new()),
    )
    .expect("fire");
    assert_eq!(
        m.root(),
        root1,
        "staged fire does not move the committed root"
    );
    block_on(m.abort_block()).expect("abort");
    assert_eq!(m.root(), root1, "abort is byte-identical");
    assert_eq!(get_rule(&m, "r").expect("r").fire_count, 0);
    assert!(history(&m, "r", 16).is_empty());
}

// ---- store-backed state semantics ---------------------------------------

/// two instances replaying the same ops (rule CRUD + a hooked fire) produce
/// identical roots — the determinism the shared store root relies on.
#[test]
fn two_instances_replaying_the_same_ops_produce_identical_roots() {
    let run = || {
        let mut m = module();
        let mut ctx = CaptureCtx::new();
        exec(
            &mut m,
            &mut ctx,
            &create(
                "r1",
                post_trigger(Some("general"), None),
                post_action("general", "hi {seq}"),
            ),
        )
        .expect("create r1");
        exec(
            &mut m,
            &mut ctx,
            &create(
                "r2",
                post_trigger(None, Some("x")),
                task_action("p", "t {text}"),
            ),
        )
        .expect("create r2");
        exec(
            &mut m,
            &mut ctx,
            &create(
                "r3",
                Trigger {
                    channel_id: None,
                    mention: Some("ops".into()),
                    text_contains: None,
                },
                inbox_action("{author}", "notify", "posted {channel}@{seq}"),
            ),
        )
        .expect("create r3");
        block_on(m.commit_block()).expect("commit rules");

        // fire r1 to populate the run-history ring (the transcript provides
        // the channel for the probe and text for r2's filter, which does not
        // match).
        let mut chat_ctx = CaptureCtx::new().with_chat_origin().with_transcript(
            "general",
            vec![message(
                "general",
                1,
                user(1),
                vec![Block::paragraph("hello")],
            )],
        );
        exec(
            &mut m,
            &mut chat_ctx,
            &posted("general", 1, user(1), Vec::new()),
        )
        .expect("fire");
        block_on(m.commit_block()).expect("commit fire");
        m
    };
    let (a, b) = (run(), run());
    assert_eq!(a.root(), b.root());
    assert_ne!(a.root(), module().root(), "state moved the root");
    assert_eq!(list_rules(&a), list_rules(&b));
    assert_eq!(history(&a, "r1", 16), history(&b, "r1", 16));
}

/// the module rides the store's resolver sync lane — the manifest capture
/// path branches on this handle, so automations joins/restores like the
/// other qmdb modules (never as checkpoint snapshot bytes).
#[test]
fn state_sync_handle_is_resolver_backed() {
    let m = module();
    match m.state_sync_handle().unwrap() {
        StateSyncHandle::ResolverBacked { backend, .. } => assert_eq!(backend, "qmdb"),
        other => panic!("unexpected handle: {other:?}"),
    }
}

#[test]
fn run_records_with_oversized_event_fields_still_commit() {
    // chat does not bound an event channel id with THIS module's caps, so a
    // matching rule can stage a run record whose channel_id exceeds them
    // (here via the composed-id guard record). the NO-FAIL arm never rejects
    // a run record — it must record and commit.
    let mut m = module();
    let mut ctx = CaptureCtx::new();
    exec(
        &mut m,
        &mut ctx,
        &create("r", post_trigger(None, None), task_action("t", "T")),
    )
    .expect("create");
    block_on(m.commit_block()).expect("commit rule");
    let root_before = m.root();

    let long_channel = "c".repeat(MAX_ID_BYTES * 2);
    let mut chat_ctx = CaptureCtx::new().with_chat_origin();
    exec(
        &mut m,
        &mut chat_ctx,
        &posted(&long_channel, 1, user(1), Vec::new()),
    )
    .expect("fire");
    block_on(m.commit_block()).expect("commit fire");
    let records = history(&m, "r", 4);
    assert_eq!(records.len(), 1, "the match was recorded");
    assert_eq!(records[0].channel_id, long_channel);
    assert_ne!(m.root(), root_before, "the record moved the root");
}

/// the run-history ring trims to [`MAX_RUN_HISTORY`]: the cursor places each
/// append and point-deletes the oldest past the cap, so only the newest
/// records survive.
#[test]
fn run_history_ring_drops_the_oldest_past_the_cap() {
    let mut m = module();
    let mut ctx = CaptureCtx::new();
    exec(
        &mut m,
        &mut ctx,
        &create("r", post_trigger(None, None), task_action("t", "T")),
    )
    .expect("create");
    block_on(m.commit_block()).expect("commit rule");

    let overflow = 2u64;
    let total = MAX_RUN_HISTORY as u64 + overflow;
    for seq in 1..=total {
        let mut chat_ctx = CaptureCtx::new().with_chat_origin().with_channel("general");
        exec(
            &mut m,
            &mut chat_ctx,
            &posted("general", seq, user(1), Vec::new()),
        )
        .expect("fire");
    }
    block_on(m.commit_block()).expect("commit fires");

    let records = history(&m, "r", MAX_RUN_HISTORY as u64);
    assert_eq!(records.len(), MAX_RUN_HISTORY, "the ring holds exactly the cap");
    assert_eq!(
        records.first().expect("oldest").seq,
        overflow + 1,
        "the oldest records were dropped"
    );
    assert_eq!(records.last().expect("newest").seq, total);
}
