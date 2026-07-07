//! shared doubles for the module-level test mods: the canned-pages `Ctx`,
//! the reference install spec, and the exec/commit drivers.
//!
//! the module-level tests cover the seams the end-to-end package_loop suite
//! cannot reach through real modules: literal redelivery of one event (pages
//! never redelivers), events authored by ourselves (loop prevention against
//! a canned author), malformed events/applies, probe verdicts against canned
//! pages state, and origin gates.

use futures::executor::block_on;
use jobs::JobsReply;
use memory::{MemoryQuery, MemoryReply};
use package::{
    ActionRoute, AgentSeed, EngagementRule, HarnessMsg, InstallSpec, MANIFEST_HASH_LEN,
    ModuleBinding, PackageActionMsg, PromptSeed, UninstallPolicy, encode_action_msg,
    encode_harness_msg,
};
use pages::{
    AuthorRef, Block, BlockKind, Comment, PageEvent, PageQuery, PageReply, Thread, ThreadView,
};
use sdk::{Ctx, Error, Event, Module, Msg, Origin, StateRoot};
use sha2::{Digest, Sha256};

use crate::{ACTION_BLOCK_UPDATE_TEXT, ACTION_COMMENT_ADD, ACTION_THREAD_RESOLVE, DocsHarness};

pub(crate) const HARNESS: &str = "docs-harness";
pub(crate) const PKG: &str = "org.ducktape.docs";
pub(crate) const AGENT: &str = "docs.editor";
pub(crate) const PROMPT_PATH: &str = "/packages/org.ducktape.docs/prompts/docs-editor.md";
pub(crate) const RUN_CONTEXT: &[u8] = br#"{"run_id":"r1","agent_id":"docs.editor"}"#;

/// canned pages state the ctx serves: block b1 ("old text") on page p1,
/// thread t1 anchored to b1.
pub(crate) struct TestCtx {
    env: sdk::Env,
    pub(crate) emitted: Vec<Msg>,
    pub(crate) events: Vec<String>,
    pub(crate) job_taken: bool,
    /// the latest generation the canned memory Stat reports for any
    /// prompt path (what the staged seed publish landed on).
    pub(crate) prompt_generation: u64,
    /// a comment id squatted in pages (GetComment answers Some for it).
    pub(crate) squatted_comment: Option<String>,
}

impl TestCtx {
    pub(crate) fn at(origin: Origin) -> Self {
        Self {
            env: sdk::Env {
                protocol_version: 0,
                height: 1,
                consensus_time: 1,
                origin,
                me: HARNESS.into(),
            },
            emitted: Vec::new(),
            events: Vec::new(),
            job_taken: false,
            prompt_generation: 1,
            squatted_comment: None,
        }
    }

    /// the canned comment store: the thread t1 opener `c0` plus any
    /// squatted id — GetComment existence is what the probe checks.
    fn canned_comment(&self, comment_id: &str) -> Option<Comment> {
        (comment_id == "c0" || self.squatted_comment.as_deref() == Some(comment_id)).then(|| {
            Comment {
                id: comment_id.into(),
                thread_id: "t1".into(),
                author: AuthorRef::User(vec![7; 32]),
                text: "taken".into(),
                created_at: 1,
                edited_at: None,
                deleted: false,
            }
        })
    }

    fn canned_block(block_id: &str) -> Option<Block> {
        match block_id {
            "p1" => Some(Block {
                id: "p1".into(),
                parent: None,
                page: "p1".into(),
                kind: BlockKind::Page,
                text: "Docs".into(),
                checked: false,
                children: vec!["b1".into()],
            }),
            "b1" => Some(Block {
                id: "b1".into(),
                parent: Some("p1".into()),
                page: "p1".into(),
                kind: BlockKind::Paragraph,
                text: "old text".into(),
                checked: false,
                children: Vec::new(),
            }),
            _ => None,
        }
    }

    fn canned_thread(thread_id: &str) -> Option<ThreadView> {
        (thread_id == "t1").then(|| ThreadView {
            thread: Thread {
                id: "t1".into(),
                target: "b1".into(),
                opener: AuthorRef::User(vec![7; 32]),
                created_at: 1,
                resolved: false,
                resolved_by: None,
                comment_ids: vec!["c0".into()],
            },
            comments: vec![Comment {
                id: "c0".into(),
                thread_id: "t1".into(),
                author: AuthorRef::User(vec![7; 32]),
                text: "opener".into(),
                created_at: 1,
                edited_at: None,
                deleted: false,
            }],
        })
    }
}

#[async_trait::async_trait(?Send)]
impl Ctx for TestCtx {
    fn env(&self) -> &sdk::Env {
        &self.env
    }
    fn module_root(&self, _target: &str) -> Option<StateRoot> {
        Some(StateRoot::ZERO)
    }
    async fn query(&self, target: &str, req: &[u8]) -> Result<Vec<u8>, Error> {
        match target {
            "jobs" => Ok(jobs::encode_reply(&JobsReply::Job(self.job_taken.then(
                || jobs::Job {
                    job_id: "taken".into(),
                    kind: "agent/docs.editor".into(),
                    spec: String::new(),
                    submitter: "ext:07".into(),
                    status: jobs::JobStatus::Pending,
                    attempt: 0,
                    claim: None,
                    result: None,
                    created_at_height: 1,
                    updated_at_height: 1,
                },
            )))),
            "pages" => {
                let reply = match pages::decode_query(req).map_err(Error::Module)? {
                    PageQuery::GetBlock { block_id } => {
                        PageReply::Block(Self::canned_block(&block_id))
                    }
                    PageQuery::CommentThread { thread_id } => {
                        PageReply::CommentThread(Self::canned_thread(&thread_id))
                    }
                    PageQuery::GetComment { comment_id } => {
                        PageReply::Comment(self.canned_comment(&comment_id))
                    }
                    other => return Err(Error::Module(format!("unexpected query: {other:?}"))),
                };
                Ok(pages::encode_reply(&reply))
            }
            "memory" => {
                let reply = match memory::decode_query(req).map_err(Error::Module)? {
                    MemoryQuery::Stat { path } => MemoryReply::Stat(Some(memory::FileStat {
                        path,
                        latest_generation: self.prompt_generation,
                        generations: 1,
                        latest_meta: memory::Meta::new(),
                        latest_author: "package".into(),
                        latest_published_at_height: 1,
                        body_len: 0,
                    })),
                    other => return Err(Error::Module(format!("unexpected query: {other:?}"))),
                };
                Ok(memory::encode_reply(&reply))
            }
            other => Err(Error::UnknownModule(other.into())),
        }
    }
    fn emit_msg(&mut self, msg: Msg) {
        self.emitted.push(msg);
    }
    fn emit_event(&mut self, ev: Event) {
        self.events
            .push(String::from_utf8_lossy(&ev.payload).into_owned());
    }
    fn request_effect(&mut self, _eff: sdk::Effect) {}
}

pub(crate) fn spec() -> InstallSpec {
    let content = "# Docs Editor\n";
    InstallSpec {
        package: PKG.into(),
        version: "0.1.0".into(),
        manifest_hash: vec![7u8; MANIFEST_HASH_LEN],
        modules: vec![
            ModuleBinding {
                logical: "pages".into(),
                module_id: "pages".into(),
            },
            ModuleBinding {
                logical: HARNESS.into(),
                module_id: HARNESS.into(),
            },
        ],
        harness: HARNESS.into(),
        prompts: vec![PromptSeed {
            logical: "docs_editor_prompt".into(),
            path: PROMPT_PATH.into(),
            content: content.into(),
            sha256: Sha256::digest(content.as_bytes()).to_vec(),
        }],
        agents: vec![AgentSeed {
            agent_id: AGENT.into(),
            display_name: "Docs Editor".into(),
            capability: "codex".into(),
            prompt: "docs_editor_prompt".into(),
            actions: vec![
                ACTION_COMMENT_ADD.into(),
                ACTION_BLOCK_UPDATE_TEXT.into(),
                ACTION_THREAD_RESOLVE.into(),
            ],
            active: true,
        }],
        actions: [
            ACTION_COMMENT_ADD,
            ACTION_BLOCK_UPDATE_TEXT,
            ACTION_THREAD_RESOLVE,
        ]
        .iter()
        .map(|tag| ActionRoute {
            tag: (*tag).into(),
            owner: HARNESS.into(),
        })
        .collect(),
        engagements: vec![EngagementRule {
            source: "pages".into(),
            event: "comment_added".into(),
            agent: AGENT.into(),
            policy: "mention_or_assigned".into(),
        }],
        uninstall: UninstallPolicy {
            pending_runs: "drain".into(),
            user_data: "preserve".into(),
        },
    }
}

pub(crate) fn module() -> DocsHarness {
    DocsHarness::new(
        HARNESS, "package", "agent", "jobs", "memory", "pages", "runs",
    )
}

pub(crate) fn package_origin() -> Origin {
    Origin::Module("package".into())
}

pub(crate) fn exec(m: &mut DocsHarness, ctx: &mut TestCtx, payload: Vec<u8>) -> Result<(), Error> {
    block_on(m.execute(
        ctx,
        &Msg {
            target: HARNESS.into(),
            payload,
        },
    ))
}

pub(crate) fn commit(m: &mut DocsHarness) {
    block_on(m.commit_block()).unwrap();
}

pub(crate) fn installed(m: &mut DocsHarness) {
    let mut ctx = TestCtx::at(package_origin());
    exec(
        m,
        &mut ctx,
        encode_harness_msg(&HarnessMsg::InstallPackage {
            package: PKG.into(),
            spec: spec(),
        }),
    )
    .unwrap();
    commit(m);
}

pub(crate) fn comment_event(comment_id: &str, text: &str) -> Vec<u8> {
    comment_event_by(comment_id, text, AuthorRef::User(vec![7; 32]))
}

pub(crate) fn comment_event_by(comment_id: &str, text: &str, author: AuthorRef) -> Vec<u8> {
    pages::encode_page_event(&PageEvent::CommentAdded {
        page_id: "p1".into(),
        target: "b1".into(),
        thread_id: "t1".into(),
        comment_id: comment_id.into(),
        author,
        text: text.into(),
    })
}

pub(crate) fn apply(action_id: &str, tag: &str, payload: serde_json::Value) -> Vec<u8> {
    encode_action_msg(&PackageActionMsg::Apply {
        action_id: action_id.into(),
        tag: tag.into(),
        payload: serde_json::to_vec(&payload).unwrap(),
        run_context: RUN_CONTEXT.to_vec(),
    })
}
