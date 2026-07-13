use super::*;
use crate::facets::{WireSink, decode_run_result_v1};
use crate::response::{
    FAILURE_EXCERPT_BYTES, agent_response_from_text, failure_excerpt, parse_strict_response,
};
use crate::{decode_reply as runs_decode_reply, encode_msg, encode_query};
use agent::{
    ACTION_TASKS_CREATE, ACTION_TASKS_UPDATE_STATUS,
    encode_event as agent_encode_event, encode_reply as agent_encode_reply,
};
use chat::{AuthorRef, Channel, MessageHead, decode_msg as chat_decode_msg};
use dispatch::{
    DispatchStatus, DispatchView, decode_msg as dispatch_decode_msg,
    encode_reply as dispatch_encode_reply, encode_result_event,
};
use duckfs_core::{
    decode_msg as files_decode_msg, decode_query as files_decode_query,
    encode_reply as files_encode_reply,
};
use futures::executor::block_on;
use jobs::{
    Claim as JobClaim, Job, encode_event as jobs_encode_event, encode_reply as jobs_encode_reply,
};
use sdk::{Effect, Env};
use tagging::{Author, encode_event as tagging_encode_event};
use tasks::{Task, decode_msg as tasks_decode_msg, encode_reply as tasks_encode_reply};

/// a canned registry: agent id -> record, served by the ctx's "agent"
/// query arm exactly like the live registry module would answer.
type Registry = BTreeMap<String, AgentRecord>;

/// a minimal `Ctx` that captures emitted msgs/effects/events and serves
/// a canned agent registry, chat transcripts, task lists, job records,
/// and dispatch records — enough to unit-test `execute` in isolation
/// (the host provides the real routing in integration).
struct CaptureCtx {
    env: Env,
    /// agent id -> registry record served by the "agent" arm.
    agents: Registry,
    /// channel -> messages with contiguous seqs starting at 1.
    transcripts: BTreeMap<String, Vec<MessageView>>,
    tasks: Vec<Task>,
    /// dispatch ids the dispatch module already has a record for — the
    /// committed turn-claim layer the module probes.
    taken_dispatches: BTreeSet<String>,
    /// dispatch id -> the node key holding the run's execution lease, served as
    /// `DispatchView.assignee` (the session lane's authorization). a dispatch
    /// listed here is AwaitingResult; one only in `taken_dispatches` is
    /// delivered, and a delivered run runs nowhere (assignee `None`).
    dispatch_assignees: BTreeMap<String, Vec<u8>>,
    /// job_id -> board record served by the jobs arm (finalize guard).
    jobs: BTreeMap<String, Job>,
    /// repo -> born (branch, tip-hex) pairs, served by the "forge"
    /// ListRefs arm (the sink's branch-born probe and the compose lane's
    /// commit pinning).
    forge_refs: BTreeMap<String, Vec<(String, String)>>,
    /// (repo, number) -> tracker item, served by the "forge" GetItem arm
    /// (the compose lane's committed item lookup) and, as summaries, by the
    /// ListItems arm (the sink's duplicate-PR guard).
    forge_items: BTreeMap<(String, u64), forge::ItemDetail>,
    /// saga_id -> the winning attempt's lease holder, served by the "saga"
    /// Get arm as a Done saga (the sink's executing-node attribution).
    saga_assignees: BTreeMap<String, Vec<u8>>,
    /// page_id -> the whole page in preorder, served by the "pages" GetPage
    /// arm (the M2 `[[page:<id>]]` injection lane); the GetBlock arm scans
    /// these pages by block id (the pages-effects target resolution).
    pages: BTreeMap<String, Vec<pages::Block>>,
    /// explicit committed page comment threads used by Pages-triggered runs.
    page_threads: BTreeMap<String, pages::ThreadView>,
    /// thread/comment ids the pages module already holds — the squat
    /// simulation the CommentThread/GetComment freshness probes hit.
    taken_page_ids: BTreeSet<String>,
    /// target -> committed thread count served by the ThreadsForTargets arm
    /// (the capacity probe); absent targets serve zero threads.
    page_target_threads: BTreeMap<String, usize>,
    /// the committed duckfs head served by the "files" Refs arm — the v3
    /// composer's `source_snapshot` pin. `None` = a fresh network (null pin).
    files_head: Option<String>,
    msgs: Vec<Msg>,
    #[allow(dead_code)]
    effects: Vec<Effect>,
    events: Vec<Event>,
}
impl CaptureCtx {
    fn new() -> Self {
        Self {
            env: Env {
                protocol_version: 0,
                height: 0,
                consensus_time: 0,
                origin: Origin::System,
                me: "runs".into(),
            },
            agents: Registry::new(),
            transcripts: BTreeMap::new(),
            tasks: Vec::new(),
            taken_dispatches: BTreeSet::new(),
            dispatch_assignees: BTreeMap::new(),
            jobs: BTreeMap::new(),
            forge_refs: BTreeMap::new(),
            forge_items: BTreeMap::new(),
            saga_assignees: BTreeMap::new(),
            pages: BTreeMap::new(),
            page_threads: BTreeMap::new(),
            taken_page_ids: BTreeSet::new(),
            page_target_threads: BTreeMap::new(),
            files_head: None,
            msgs: Vec::new(),
            effects: Vec::new(),
            events: Vec::new(),
        }
    }
    fn at(mut self, view: u64) -> Self {
        self.env.height = view;
        self.env.consensus_time = view;
        self
    }
    /// register a born branch under `repo` (the sink's branch-born probe;
    /// tip = a fixed zero oid where the tip does not matter).
    fn with_forge_ref(self, repo: &str, branch: &str) -> Self {
        let tip = "00".repeat(20);
        self.with_forge_tip(repo, branch, &tip)
    }
    /// register a born branch with an explicit tip (the compose lane's
    /// commit pinning).
    fn with_forge_tip(mut self, repo: &str, branch: &str, tip: &str) -> Self {
        self.forge_refs
            .entry(repo.into())
            .or_default()
            .push((branch.into(), tip.into()));
        self
    }
    /// register a tracker item served by the "forge" GetItem arm.
    fn with_forge_item(mut self, repo: &str, item: forge::ItemDetail) -> Self {
        self.forge_items
            .insert((repo.into(), item.summary.number), item);
        self
    }
    /// register the node key holding `saga_id`'s winning lease, served by
    /// the "saga" Get arm (the sink's executing-node attribution).
    fn with_saga_assignee(mut self, saga_id: &str, key: &[u8]) -> Self {
        self.saga_assignees.insert(saga_id.into(), key.to_vec());
        self
    }
    /// register a committed page (whole preorder Vec, root first) served by
    /// the "pages" GetPage arm.
    fn with_page(mut self, page_id: &str, blocks: Vec<pages::Block>) -> Self {
        self.pages.insert(page_id.into(), blocks);
        self
    }
    fn with_page_thread(mut self, view: pages::ThreadView) -> Self {
        self.page_threads.insert(view.thread.id.clone(), view);
        self
    }
    /// mark a thread/comment id as already minted in the pages module (the
    /// freshness probes then see it taken).
    fn with_taken_page_id(mut self, id: &str) -> Self {
        self.taken_page_ids.insert(id.into());
        self
    }
    /// mark a target as already holding the thread cap.
    fn with_crowded_page_target(self, target: &str) -> Self {
        self.with_page_target_threads(target, pages::MAX_THREADS_PER_TARGET)
    }
    /// serve `count` committed threads for `target` (the capacity probe).
    fn with_page_target_threads(mut self, target: &str, count: usize) -> Self {
        self.page_target_threads.insert(target.into(), count);
        self
    }
    /// set the committed duckfs head the "files" Refs arm serves (the v3
    /// composer's `source_snapshot`).
    fn with_files_head(mut self, head: &str) -> Self {
        self.files_head = Some(head.into());
        self
    }
    fn with_origin(mut self, origin: Origin) -> Self {
        self.env.origin = origin;
        self
    }
    fn with_tagging_origin(self) -> Self {
        self.with_origin(Origin::Module("tagging".into()))
    }
    fn with_dispatch_origin(self) -> Self {
        self.with_origin(Origin::Module("dispatch".into()))
    }
    fn with_jobs_origin(self) -> Self {
        self.with_origin(Origin::Module("jobs".into()))
    }
    fn with_agent_origin(self) -> Self {
        self.with_origin(Origin::Module("agent".into()))
    }
    fn with_registry(mut self, registry: &Registry) -> Self {
        self.agents = registry.clone();
        self
    }
    fn with_transcript(mut self, channel: &str, messages: Vec<MessageView>) -> Self {
        self.transcripts.insert(channel.into(), messages);
        self
    }
    fn with_task(mut self, id: &str) -> Self {
        self.tasks.push(Task {
            id: id.into(),
            title: id.into(),
            status: TaskStatus::Open,
            created_at: 0,
            updated_at: 0,
        });
        self
    }
    fn with_taken_dispatch(mut self, dispatch_id: &str) -> Self {
        self.taken_dispatches.insert(dispatch_id.into());
        self
    }
    /// serve `key` as the node holding `run_id`'s execution lease — what the
    /// dispatch read facade resolves from saga's committed lease, and the ONLY
    /// origin the session lane lets open a session.
    fn with_lease_holder(mut self, run_id: &str, key: &[u8]) -> Self {
        self.dispatch_assignees
            .insert(dispatch_id_for(run_id), key.to_vec());
        self
    }
    /// a job the board holds as Processing, claimed by "runs" at `height`.
    fn with_claimed_job(mut self, job_id: &str, height: u64) -> Self {
        self.jobs.insert(
            job_id.into(),
            Job {
                job_id: job_id.into(),
                kind: "agent/duck".into(),
                spec: "spec".into(),
                submitter: "ext:01".into(),
                status: JobStatus::Processing,
                attempt: 1,
                claim: Some(JobClaim {
                    worker: "runs".into(),
                    claimed_at_height: height,
                    lease_views: JOB_RUN_LEASE_VIEWS,
                }),
                result: None,
                created_at_height: height,
                updated_at_height: height,
            },
        );
        self
    }
    /// decoded chat msgs emitted this dispatch.
    fn chat_msgs(&self) -> Vec<ChatMsg> {
        self.msgs
            .iter()
            .filter(|m| m.target == "chat")
            .map(|m| chat_decode_msg(&m.payload).expect("chat msg"))
            .collect()
    }
    /// decoded task msgs emitted this dispatch.
    fn task_msgs(&self) -> Vec<TaskMsg> {
        self.msgs
            .iter()
            .filter(|m| m.target == "tasks")
            .map(|m| tasks_decode_msg(&m.payload).expect("task msg"))
            .collect()
    }
    /// decoded jobs msgs emitted this dispatch.
    fn job_msgs(&self) -> Vec<JobsMsg> {
        self.msgs
            .iter()
            .filter(|m| m.target == "jobs")
            .map(|m| jobs::decode_msg(&m.payload).expect("jobs msg"))
            .collect()
    }
    /// decoded dispatch-plane msgs emitted this dispatch.
    fn dispatch_msgs(&self) -> Vec<DispatchMsg> {
        self.msgs
            .iter()
            .filter(|m| m.target == "dispatch")
            .map(|m| dispatch_decode_msg(&m.payload).expect("dispatch msg"))
            .collect()
    }
    /// decoded tagging-plane msgs emitted this dispatch.
    fn tagging_msgs(&self) -> Vec<TaggingMsg> {
        self.msgs
            .iter()
            .filter(|m| m.target == "tagging")
            .map(|m| tagging::decode_msg(&m.payload).expect("tagging msg"))
            .collect()
    }
    /// decoded pages msgs emitted this dispatch.
    fn page_msgs(&self) -> Vec<pages::PageMsg> {
        self.msgs
            .iter()
            .filter(|m| m.target == "pages")
            .map(|m| pages::decode_msg(&m.payload).expect("pages msg"))
            .collect()
    }
    /// decoded files msgs emitted this dispatch.
    fn files_msgs(&self) -> Vec<FilesMsg> {
        self.msgs
            .iter()
            .filter(|m| m.target == "files")
            .map(|m| files_decode_msg(&m.payload).expect("files msg"))
            .collect()
    }
    /// the breadcrumb notes emitted this dispatch, as strings.
    fn notes(&self) -> Vec<String> {
        self.events
            .iter()
            .map(|e| String::from_utf8_lossy(&e.payload).into_owned())
            .collect()
    }
}

/// a minimal committed thread view, as the "pages" CommentThread arm serves a
/// taken thread id.
fn dummy_thread_view(id: &str) -> pages::ThreadView {
    pages::ThreadView {
        thread: pages::Thread {
            id: id.into(),
            target: "elsewhere".into(),
            opener: pages::AuthorRef::System,
            created_at: 0,
            resolved: false,
            resolved_by: None,
            comment_ids: Vec::new(),
        },
        comments: Vec::new(),
    }
}

/// a minimal committed comment, as the "pages" GetComment arm serves a taken
/// comment id.
fn dummy_comment(id: &str) -> pages::Comment {
    pages::Comment {
        id: id.into(),
        thread_id: "elsewhere".into(),
        author: pages::AuthorRef::System,
        text: String::new(),
        created_at: 0,
        edited_at: None,
        deleted: false,
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
        match target {
            "agent" => match agent::decode_query(req).map_err(Error::Module)? {
                AgentQuery::Agent { agent_id } => Ok(agent_encode_reply(&AgentReply::Agent(
                    self.agents.get(&agent_id).cloned(),
                ))),
                AgentQuery::Agents => Ok(agent_encode_reply(&AgentReply::Agents(
                    self.agents.values().cloned().collect(),
                ))),
            },
            "chat" => match chat::decode_query(req).map_err(Error::Module)? {
                ChatQuery::MessagesRange {
                    channel_id,
                    from_seq,
                    limit,
                } => {
                    let transcript = self
                        .transcripts
                        .get(&channel_id)
                        .ok_or_else(|| Error::Module(format!("unknown channel: {channel_id}")))?;
                    let head = transcript.len() as u64;
                    let from = from_seq.max(1);
                    let mut window = Vec::new();
                    if limit > 0 && from <= head {
                        let to = head.min(from + limit - 1);
                        window = transcript[(from - 1) as usize..to as usize].to_vec();
                    }
                    Ok(chat::encode_reply(&ChatReply::Messages(window)))
                }
                ChatQuery::Message { message_id } => Ok(chat::encode_reply(&ChatReply::Message(
                    self.transcripts
                        .values()
                        .flatten()
                        .find(|v| v.head.message_id == message_id)
                        .cloned(),
                ))),
                // a channel EXISTS exactly when it has a transcript — the
                // fixture idiom already in use everywhere (a channel with no
                // messages is `with_transcript(ch, vec![])`).
                ChatQuery::Channel { channel_id } => Ok(chat::encode_reply(&ChatReply::Channel(
                    self.transcripts.get(&channel_id).map(|msgs| Channel {
                        id: channel_id.clone(),
                        name: channel_id,
                        created_at: 0,
                        head_seq: msgs.len() as u64,
                        post_policy: chat::PostPolicy::Open,
                        hooks: Vec::new(),
                        pinned: Vec::new(),
                        huddle: Vec::new(),
                    }),
                ))),
                _ => Err(Error::QueryUnsupported),
            },
            "tasks" => Ok(tasks_encode_reply(&TaskReply::Tasks(self.tasks.clone()))),
            "jobs" => match jobs::decode_query(req).map_err(Error::Module)? {
                JobsQuery::Get { job_id } => Ok(jobs_encode_reply(&JobsReply::Job(
                    self.jobs.get(&job_id).cloned(),
                ))),
                _ => Err(Error::QueryUnsupported),
            },
            "dispatch" => match dispatch::decode_query(req).map_err(Error::Module)? {
                DispatchQuery::Dispatch { dispatch_id, .. } => {
                    // an assigned dispatch is still AwaitingResult (a lease is
                    // held); a merely `taken` one already delivered.
                    let assignee = self.dispatch_assignees.get(&dispatch_id).cloned();
                    let awaiting = assignee.is_some();
                    let view =
                        (awaiting || self.taken_dispatches.contains(&dispatch_id)).then(|| {
                            DispatchView {
                                dispatch_id,
                                recipe_id: "agent/x".into(),
                                receiver: "runs".into(),
                                status: if awaiting {
                                    DispatchStatus::AwaitingResult {
                                        saga_id: "saga-1".into(),
                                    }
                                } else {
                                    DispatchStatus::Delivered
                                },
                                outcome: (!awaiting).then(|| Ok(Vec::new())),
                                assignee,
                                attempt: None,
                                max_attempts: None,
                                lease_expires_at: None,
                                deadline: None,
                                lease_updated_at: None,
                                reassignable: None,
                                created_at: 0,
                                updated_at: 0,
                            }
                        });
                    Ok(dispatch_encode_reply(&DispatchReply::Dispatch(view)))
                }
                _ => Err(Error::QueryUnsupported),
            },
            "files" => match files_decode_query(req).map_err(Error::Module)? {
                FilesQuery::Refs {} => Ok(files_encode_reply(&FilesReply::Refs(
                    duckfs_core::RefsInfo {
                        head: self.files_head.clone(),
                        pins: BTreeMap::new(),
                        window_len: 0,
                    },
                ))),
                _ => Err(Error::QueryUnsupported),
            },
            "forge" => match forge::decode_query(req).map_err(Error::Module)? {
                forge::ForgeQuery::ListRefs { repo } => {
                    let refs = self
                        .forge_refs
                        .get(&repo)
                        .into_iter()
                        .flatten()
                        .map(|(name, tip)| forge::RefHead {
                            name: name.clone(),
                            head: tip.clone(),
                        })
                        .collect();
                    Ok(forge::encode_reply(&forge::ForgeReply::Refs(refs)))
                }
                forge::ForgeQuery::GetItem { repo, number } => {
                    Ok(forge::encode_reply(&forge::ForgeReply::Item(
                        self.forge_items.get(&(repo, number)).cloned().map(Box::new),
                    )))
                }
                forge::ForgeQuery::ListItems { repo } => {
                    // ascending by number — the BTreeMap key order, exactly
                    // like the real tracker's listing.
                    let items = self
                        .forge_items
                        .iter()
                        .filter(|((r, _), _)| *r == repo)
                        .map(|(_, d)| d.summary.clone())
                        .collect();
                    Ok(forge::encode_reply(&forge::ForgeReply::Items(items)))
                }
                _ => Err(Error::QueryUnsupported),
            },
            "pages" => match pages::decode_query(req).map_err(Error::Module)? {
                pages::PageQuery::GetPage { page_id } => Ok(pages::encode_reply(
                    &pages::PageReply::Page(self.pages.get(&page_id).cloned()),
                )),
                pages::PageQuery::GetBlock { block_id } => Ok(pages::encode_reply(
                    &pages::PageReply::Block(
                        self.pages
                            .values()
                            .flatten()
                            .find(|b| b.id == block_id)
                            .cloned(),
                    ),
                )),
                pages::PageQuery::CommentThread { thread_id } => Ok(pages::encode_reply(
                    &pages::PageReply::CommentThread(
                        self.page_threads.get(&thread_id).cloned().or_else(|| {
                            self.taken_page_ids
                                .contains(&thread_id)
                                .then(|| dummy_thread_view(&thread_id))
                        }),
                    ),
                )),
                pages::PageQuery::GetComment { comment_id } => Ok(pages::encode_reply(
                    &pages::PageReply::Comment(
                        self.taken_page_ids
                            .contains(&comment_id)
                            .then(|| dummy_comment(&comment_id)),
                    ),
                )),
                pages::PageQuery::ThreadsForTargets { targets } => Ok(pages::encode_reply(
                    &pages::PageReply::CommentThreads(
                        targets
                            .into_iter()
                            .map(|target| {
                                let count =
                                    self.page_target_threads.get(&target).copied().unwrap_or(0);
                                let threads = (0..count)
                                    .map(|i| dummy_thread_view(&format!("t{i}")))
                                    .collect();
                                pages::TargetThreads { target, threads }
                            })
                            .collect(),
                    ),
                )),
                _ => Err(Error::QueryUnsupported),
            },
            "saga" => match saga::decode_query(req).map_err(Error::Module)? {
                saga::SagaQuery::Get { saga_id } => {
                    // a Done saga still carrying its winning attempt's
                    // lease holder — exactly what the saga module commits.
                    let view = self.saga_assignees.get(&saga_id).map(|key| saga::SagaView {
                        origin: SagaOrigin::Module("dispatch".into()),
                        reply_to: Some("dispatch".into()),
                        reply_payload: Vec::new(),
                        spec: Vec::new(),
                        capability: Some("model-1".into()),
                        status: saga::SagaStatus::Done,
                        attempt: 0,
                        max_attempts: RUN_MAX_ATTEMPTS,
                        assignee: Some(key.clone()),
                        pinned_assignee: None,
                        lease_views: None,
                        lease_expires_at: None,
                        deadline: None,
                        result: Some(Vec::new()),
                        error: None,
                        created_at: 0,
                        updated_at: 0,
                    });
                    Ok(saga::encode_reply(&saga::SagaReply::Saga(view)))
                }
                _ => Err(Error::QueryUnsupported),
            },
            other => Err(Error::UnknownModule(other.into())),
        }
    }
    fn emit_msg(&mut self, msg: Msg) {
        self.msgs.push(msg);
    }
    fn emit_event(&mut self, ev: Event) {
        self.events.push(ev);
    }
    fn request_effect(&mut self, eff: Effect) {
        self.effects.push(eff);
    }
}

// ---- fixtures -----------------------------------------------------------

fn module() -> RunsModule {
    RunsModule::new(
        "runs",
        "chat",
        "saga",
        "tagging",
        "dispatch",
        "agent",
        Some("tasks".into()),
        Some("jobs".into()),
    )
}

fn user(byte: u8) -> Origin {
    Origin::External(vec![byte; 32])
}

/// entity tags carry the ACTING module's id — the unified agent identity.
fn agent_tag(agent_id: &str) -> EntityRef {
    EntityRef {
        module: "runs".into(),
        entity: agent_id.into(),
    }
}

fn record(agent_id: &str, actions: &[&str]) -> AgentRecord {
    AgentRecord {
        agent_id: agent_id.into(),
        owner: SagaOrigin::External(vec![9; 32]),
        display_name: agent_id.to_uppercase(),
        capability: "model-1".into(),
        allowed_actions: actions.iter().map(|s| s.to_string()).collect(),
        status: AgentStatus::Active,
        created_at: 0,
        updated_at: 0,
        recipe_hash: Vec::new(),
        caps: agent::ResourceCaps::default(),
        skills: Vec::new(),
    }
}

fn registry(agents: &[(&str, &[&str])]) -> Registry {
    agents
        .iter()
        .map(|(id, actions)| ((*id).to_string(), record(id, actions)))
        .collect()
}

fn pause(registry: &mut Registry, agent_id: &str) {
    registry.get_mut(agent_id).expect("registered").status = AgentStatus::Paused;
}

fn message_in(
    channel: &str,
    seq: u64,
    author: AuthorRef,
    text: &str,
    thread: Option<u64>,
) -> MessageView {
    MessageView {
        channel_id: channel.into(),
        seq,
        head: MessageHead {
            message_id: format!("{channel}-m{seq}"),
            author,
            blocks: vec![Block::paragraph(text)],
            created_at: 0,
            rev: 0,
            edited_at: None,
            base_rev: None,
            deleted: false,
            thread,
            reply_count: 0,
            last_reply_seq: None,
        },
        reactions: Vec::new(),
        channel_head_seq: seq,
    }
}

fn message(seq: u64, text: &str) -> MessageView {
    message_in("general", seq, AuthorRef::User(vec![1; 32]), text, None)
}

fn transcript(n: u64) -> Vec<MessageView> {
    (1..=n).map(|i| message(i, &format!("msg {i}"))).collect()
}

fn admin(m: &RunsMsg) -> Msg {
    Msg {
        target: "runs".into(),
        payload: encode_msg(m),
    }
}

/// the tagging plane's routed report of a user post — the engagement
/// intake's payload. the plane's loop rule means these are always
/// user-authored in practice.
fn engagement(channel: &str, seq: u64, tags: Vec<EntityRef>) -> Msg {
    Msg {
        target: "runs".into(),
        payload: tagging_encode_event(&EngagementEvent {
            source: "chat".into(),
            container: channel.into(),
            content_seq: seq,
            author: Author::User(vec![1; 32]),
            tags,
        }),
    }
}

/// the dispatch plane's next-block delivery for a run.
fn result_event(run_id: &str, outcome: Result<Vec<u8>, String>) -> Msg {
    Msg {
        target: "runs".into(),
        payload: encode_result_event(&ResultEvent {
            dispatch_id: dispatch_id_for(run_id),
            recipe_id: recipe_id_for("bot"),
            outcome,
        }),
    }
}

/// a jobs-board submit notification, spec + matching hash included.
fn jobs_event(job_id: &str, kind: &str, spec: &str) -> Msg {
    Msg {
        target: "runs".into(),
        payload: jobs_encode_event(&JobsEvent::Submitted {
            job_id: job_id.into(),
            kind: kind.into(),
            submitter: "ext:01".into(),
            spec: spec.into(),
            spec_hash: job_spec_hash(spec.as_bytes()),
        }),
    }
}

/// the registry hook's payload (origin == agent).
fn agent_event(event: &AgentEvent) -> Msg {
    Msg {
        target: "runs".into(),
        payload: agent_encode_event(event),
    }
}

fn exec(m: &mut RunsModule, ctx: &mut CaptureCtx, op: &Msg) -> Result<(), Error> {
    block_on(m.execute(ctx, op))
}

fn commit(m: &mut RunsModule) {
    block_on(m.commit_block()).unwrap();
}

fn abort(m: &mut RunsModule) {
    block_on(m.abort_block()).unwrap();
}

fn pending_runs(m: &RunsModule) -> Vec<PendingRun> {
    let reply = block_on(m.query(&encode_query(&RunsQuery::PendingRuns))).unwrap();
    match runs_decode_reply(&reply).unwrap() {
        RunsReply::PendingRuns(runs) => runs,
        other => panic!("unexpected reply: {other:?}"),
    }
}

fn get_pending(m: &RunsModule, run_id: &str) -> Option<PendingRun> {
    pending_runs(m).into_iter().find(|p| p.run_id == run_id)
}

fn recent_runs(m: &RunsModule) -> Vec<RunRecord> {
    let reply = block_on(m.query(&encode_query(&RunsQuery::RecentRuns))).unwrap();
    match runs_decode_reply(&reply).unwrap() {
        RunsReply::RecentRuns(runs) => runs,
        other => panic!("unexpected reply: {other:?}"),
    }
}

// ---- shared forge fixtures (the compose lane + the PR sink) -------------------

/// a committed tracker item as the "forge" GetItem arm serves it.
fn forge_item_detail(
    number: u64,
    kind: forge::ItemKind,
    title: &str,
    body: &str,
    branches: Option<(&str, &str)>,
) -> forge::ItemDetail {
    forge::ItemDetail {
        summary: forge::ItemSummary {
            number,
            kind,
            title: title.into(),
            state: forge::ItemState::Open,
            author: AuthorRef::User(vec![1; 32]),
            created_at: 0,
            updated_at: 0,
        },
        body: body.into(),
        channel_id: format!("forge:app:{number}"),
        source_branch: branches.map(|(s, _)| s.to_string()),
        target_branch: branches.map(|(_, t)| t.to_string()),
        merge_oid: None,
        reviews: Vec::new(),
    }
}

fn forge_issue(number: u64, title: &str, body: &str) -> forge::ItemDetail {
    forge_item_detail(number, forge::ItemKind::Issue, title, body, None)
}

fn forge_pr(number: u64, title: &str, body: &str, src: &str, tgt: &str) -> forge::ItemDetail {
    forge_item_detail(number, forge::ItemKind::Pr, title, body, Some((src, tgt)))
}

/// a registry whose one agent "bot" holds the forge_read cap on "app".
fn forge_read_registry() -> Registry {
    let mut r = registry(&[("bot", &[ACTION_CHAT_POST])]);
    r.get_mut("bot").unwrap().caps.forge_read = vec!["app".into()];
    r
}

/// the forge-lane module: forge + files + pages wired (the production wiring).
fn forge_module() -> RunsModule {
    module()
        .with_sink_forge("forge")
        .with_files_module("files")
        .with_pages_module("pages")
}

/// a committed page in preorder as the "pages" GetPage arm serves it: the
/// root (title), a paragraph, and one unchecked todo — enough surface for
/// the injection assertions. the root names itself as `page`.
fn page_blocks(page_id: &str, title: &str) -> Vec<pages::Block> {
    let block = |id: &str, parent: Option<&str>, kind, text: &str| pages::Block {
        id: id.into(),
        parent: parent.map(str::to_string),
        page: page_id.into(),
        kind,
        text: text.into(),
        checked: false,
        children: Vec::new(),
    };
    vec![
        block(page_id, None, pages::BlockKind::Page, title),
        block(
            "b-p",
            Some(page_id),
            pages::BlockKind::Paragraph,
            "spec paragraph",
        ),
        block("b-t", Some(page_id), pages::BlockKind::Todo, "do the thing"),
    ]
}

/// a committed module with one watch on "general" under `policy`. the
/// registry itself lives in each ctx (`with_registry`), never here.
fn watched(policy: TurnPolicy, registry: &Registry) -> RunsModule {
    let mut m = module();
    let mut ctx = CaptureCtx::new()
        .with_origin(user(9))
        .with_registry(registry);
    exec(
        &mut m,
        &mut ctx,
        &admin(&RunsMsg::WatchChannel {
            channel_id: "general".into(),
            policy,
        }),
    )
    .unwrap();
    commit(&mut m);
    m
}

/// drive an engagement at `seq` (author user(1)) tagging `mentioned`.
fn engage_post(
    m: &mut RunsModule,
    registry: &Registry,
    seq: u64,
    mentioned: &[&str],
) -> CaptureCtx {
    let mut ctx = CaptureCtx::new()
        .at(seq)
        .with_tagging_origin()
        .with_registry(registry)
        .with_transcript("general", transcript(seq));
    let tags = mentioned.iter().map(|a| agent_tag(a)).collect();
    exec(m, &mut ctx, &engagement("general", seq, tags)).unwrap();
    ctx
}

/// build a faceted RunnerResult wrapper: the three core fields plus whatever
/// facet keys `facets` carries (data / effects / sink / status, and a
/// `workspace_receipt` override when present).
fn runner_wrapper(response_text: &str, facets: serde_json::Value) -> Vec<u8> {
    let mut obj = serde_json::json!({
        "ducktape_runner_result": 1,
        "response_text": response_text,
        "workspace_receipt": {
            "source_prefix": "/shared/agent-workspaces/bot",
            "source_snapshot": null,
            "output_snapshot": null,
            "commit_height": null,
            "rebased": false,
            "no_changes": true
        }
    });
    if let serde_json::Value::Object(extra) = facets {
        let base = obj.as_object_mut().expect("object");
        for (k, v) in extra {
            base.insert(k, v);
        }
    }
    serde_json::to_vec(&obj).expect("wrapper serializes")
}

/// the model's strict-output prose (a bare AgentResponse JSON), wrapped in
/// the host-assembled runner result the oracle now ALWAYS delivers (the
/// marker-less flat tolerance is gone — flag day).
fn response(reply: &[&str], actions: Vec<AgentAction>) -> Vec<u8> {
    let prose = String::from_utf8(response_json(reply, actions)).expect("utf-8");
    runner_wrapper(&prose, serde_json::json!({}))
}

/// the bare AgentResponse wire JSON — the PROSE inside [`response`], and the
/// expected-value shape assertions compare against.
fn response_json(reply: &[&str], actions: Vec<AgentAction>) -> Vec<u8> {
    agent::encode_response(&AgentResponse {
        reply_blocks: reply
            .iter()
            .map(|t| ReplyBlock {
                kind: "paragraph".into(),
                text: (*t).into(),
                lang: None,
            })
            .collect(),
        actions,
    })
}

/// a committed module holding one pending run for "bot" (granted
/// `actions`) at general/2, plus the registry and the run id.
fn awaiting_run(actions: &[&str]) -> (RunsModule, Registry, String) {
    let registry = registry(&[("bot", actions)]);
    let mut m = watched(TurnPolicy::All, &registry);
    engage_post(&mut m, &registry, 2, &[]);
    commit(&mut m);
    (m, registry, run_id_for("general", 2, "bot"))
}
/// the canned registry for the jobs lane: "duck" with task grants.
fn job_registry() -> Registry {
    registry(&[("duck", &[ACTION_TASKS_CREATE])])
}
mod admin;
mod composition;
mod delivery;
mod engagement;
mod facets;
mod job_runs;
mod pages_actions;
mod registry;
mod sessions;
mod state;
mod validation;
