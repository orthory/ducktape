//! shared doubles for the module-level test mods: the empty-board `Ctx`,
//! the reference install spec, and the exec/commit drivers.
//!
//! the module-level tests cover the seams the end-to-end framework suite
//! cannot reach through real modules: literal redelivery of one event (pages
//! never redelivers), malformed events from a "source", and snapshot tamper
//! rejection.

use futures::executor::block_on;
use jobs::JobsReply;
use memory::{MemoryQuery, MemoryReply};
use package::{
    ActionRoute, AgentSeed, EngagementRule, HarnessMsg, InstallSpec, MANIFEST_HASH_LEN,
    ModuleBinding, PackageActionMsg, PromptSeed, UninstallPolicy, encode_action_msg,
    encode_harness_msg,
};
use pages::PageEvent;
use sdk::{Ctx, Error, Event, Module, Msg, Origin, StateRoot};
use sha2::{Digest, Sha256};

use super::{ACTION_NOTE_ADD, DummyHarness};

pub(crate) const HARNESS: &str = "dummy-harness";
pub(crate) const PKG: &str = "org.example.dummy";

pub(crate) struct TestCtx {
    env: sdk::Env,
    pub(crate) emitted: Vec<Msg>,
    pub(crate) events: Vec<String>,
    /// the latest generation the canned memory `Stat` reports for any prompt
    /// path (what the staged seed publish landed on) — default 1 (a fresh
    /// path, no squatter).
    pub(crate) prompt_generation: u64,
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
            prompt_generation: 1,
        }
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
            // the jobs probe-before-emit: an empty board.
            "jobs" => Ok(jobs::encode_reply(&JobsReply::Job(None))),
            // the install arm's prompt-generation resolution.
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
    let content = "be a dummy";
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
                logical: "dummy-harness".into(),
                module_id: HARNESS.into(),
            },
        ],
        harness: "dummy-harness".into(),
        prompts: vec![PromptSeed {
            logical: "p".into(),
            path: "/packages/org.example.dummy/prompts/dummy.md".into(),
            content: content.into(),
            sha256: Sha256::digest(content.as_bytes()).to_vec(),
        }],
        agents: vec![AgentSeed {
            agent_id: "dummy.note-taker".into(),
            display_name: "Dummy".into(),
            capability: "mock-llm-1".into(),
            prompt: "p".into(),
            actions: vec![ACTION_NOTE_ADD.into()],
            active: true,
        }],
        actions: vec![ActionRoute {
            tag: ACTION_NOTE_ADD.into(),
            owner: "dummy-harness".into(),
        }],
        engagements: vec![EngagementRule {
            source: "pages".into(),
            event: "comment_added".into(),
            agent: "dummy.note-taker".into(),
            policy: "mention".into(),
        }],
        uninstall: UninstallPolicy {
            pending_runs: "drain".into(),
            user_data: "preserve".into(),
        },
    }
}

pub(crate) fn module() -> DummyHarness {
    DummyHarness::new(HARNESS, "package", "agent", "jobs", "memory", "runs")
}

pub(crate) fn package_origin() -> Origin {
    Origin::Module("package".into())
}

pub(crate) fn exec(m: &mut DummyHarness, ctx: &mut TestCtx, payload: Vec<u8>) -> Result<(), Error> {
    block_on(m.execute(
        ctx,
        &Msg {
            target: HARNESS.into(),
            payload,
        },
    ))
}

pub(crate) fn commit(m: &mut DummyHarness) {
    block_on(m.commit_block()).unwrap();
}

pub(crate) fn installed(m: &mut DummyHarness) {
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
    pages::encode_page_event(&PageEvent::CommentAdded {
        page_id: "p1".into(),
        target: "p1".into(),
        thread_id: "t1".into(),
        comment_id: comment_id.into(),
        author: pages::AuthorRef::User(vec![7; 32]),
        text: text.into(),
    })
}

pub(crate) fn apply(tag: &str, payload: serde_json::Value) -> Vec<u8> {
    encode_action_msg(&PackageActionMsg::Apply {
        action_id: "a1".into(),
        tag: tag.into(),
        payload: serde_json::to_vec(&payload).unwrap(),
        run_context: b"{}".to_vec(),
    })
}
