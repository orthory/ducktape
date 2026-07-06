//! docs-harness — the reference Quack package's harness module (design D9).
//!
//! the ADR's worked example, end to end: this module owns the docs package's
//! lifecycle, engagement, and action semantics. `runs` stays the execution
//! adapter and the LLM only ever requests actions as data — THIS module is
//! the safety boundary that turns them into real `PageMsg` writes.
//!
//! - **`HarnessMsg` arms**, accepted ONLY from the package module's origin:
//!   install registers a hook on the pages module, registers each agent seed
//!   from module origin (owner = this harness) with a `PromptRef` pinned to
//!   the seeded memory path, and acks with `PackageMsg::MarkActive` LAST;
//!   suspend pauses the agents (job minting stops); resume reverses; unplug
//!   tombstones the agents and unregisters the hook, preserving user data.
//! - **`PageEvent` intake** (NO-FAIL), accepted ONLY from the wired pages
//!   module's origin: the `mention_or_assigned` policy — a comment whose text
//!   mentions `@<agent_id>` — mints exactly ONE idempotent
//!   `JobsMsg::Submit { kind: "agent/<agent_id>" }` per (comment, agent),
//!   probe-before-emit against the jobs board, idempotency keys committed.
//!   **loop prevention is ours**: pages notifies hooks of ALL writes,
//!   including the ones our own `Apply` follow-ups cause — an event authored
//!   by this module (or one of its agents) never mints.
//! - **the action-owner contract** for `pages.comment.add`,
//!   `pages.block.update_text`, `pages.thread.resolve`: `Probe` validates
//!   schema/target-existence/caps/`expected_hash` against pages via
//!   `Ctx::query`; `Apply` — gated to the RUNS module's origin, tighter than
//!   the origin-class gate — is NO-FAIL: decode-or-drop, re-validate, then
//!   translate to `PageMsg` follow-ups; a late conflict lands a committed
//!   error row plus a breadcrumb, never a block abort.
//!
//! ## the sibling-action caveat
//!
//! probe/apply validation sees staged-or-committed pages state, NEVER the
//! sibling actions of the same response: their follow-ups have not run yet.
//! two same-response actions may therefore both validate and still collide at
//! pages. minted comment/thread ids embed `(run_id, action_id)`, so the one
//! in-response collision that would poison the delivery block — a duplicated
//! `action_id` minting the same comment id twice — is deduped per block here.
//! a squatted MINTED comment id (pages has no comment-by-id read to probe)
//! remains the documented residual, the same class as the dummy package's
//! predictable job ids.
//!
//! ## prompt generation pin
//!
//! the install block publishes each prompt seed into memory BEFORE this
//! module's install arm runs, but queries observe committed state only — so
//! the harness pins generation 1, the committed generation a FRESH package
//! path lands on (the dummy-harness convention).

use std::collections::{BTreeMap, BTreeSet};

use agent::{AgentMsg, PromptRef, RENDERER_MEMORY_GENERATION, encode_msg as agent_encode_msg};
use jobs::{JobsMsg, JobsQuery, JobsReply, encode_msg as jobs_encode_msg};
use package::{
    HarnessMsg, InstallSpec, PackageActionMsg, PackageActionQuery, PackageActionReply, PackageMsg,
    decode_action_msg, decode_action_query, decode_harness_msg, encode_action_reply,
    encode_msg as package_encode_msg,
};
use pages::{
    AuthorRef, PageEvent, PageMsg, PageQuery, PageReply, ThreadView, decode_page_event,
    encode_msg as pages_encode_msg, encode_query as pages_encode_query,
};
use sdk::{Ctx, Error, Event, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle};
use sha2::{Digest, Sha256};

mod interface;
pub use interface::*;

/// the memory generation a fresh package prompt path commits at (see the
/// module docs on the pin assumption).
const FIRST_GENERATION: u64 = 1;

// ---- state -----------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Active,
    Suspended,
    Unplugged,
}

impl Phase {
    fn as_str(self) -> &'static str {
        match self {
            Phase::Active => "active",
            Phase::Suspended => "suspended",
            Phase::Unplugged => "unplugged",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Installed {
    package: String,
    phase: Phase,
    /// registered agent ids, in seed order.
    agents: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Store {
    installed: Option<Installed>,
    /// idempotency keys of already-minted jobs (`<comment>\x1f<agent>`).
    minted: BTreeSet<String>,
    /// bounded error-row log (oldest evicted past [`MAX_FAILURE_ROWS`]).
    failures: Vec<FailureRow>,
}

/// what one probe/apply validated — shared so the probe verdict and the
/// apply-time re-check cannot drift (the tasks-module idiom).
enum ValidatedAction {
    CommentAdd {
        thread_id: String,
        comment_id: String,
        target: String,
        text: String,
    },
    UpdateText {
        block_id: String,
        text: String,
    },
    ResolveThread {
        thread_id: String,
        resolved: bool,
    },
}

pub struct DocsHarness {
    id: ModuleId,
    /// the package registry (the only `HarnessMsg` origin; the ack target).
    package: ModuleId,
    /// the agent registry (agent seeds register here, harness-owned).
    agent: ModuleId,
    /// the jobs board (engagements mint here).
    jobs: ModuleId,
    /// the memory workspace (prompt seeds live here; `PromptRef.module`).
    memory: ModuleId,
    /// the pages module: the ONLY accepted event source, the query target of
    /// every probe, and the target of every `Apply` follow-up.
    pages: ModuleId,
    /// the runs module: the ONLY origin an action `Apply` is accepted from —
    /// deliberately tighter than the dummy's any-module gate.
    runs: ModuleId,
    committed: Store,
    pending: Option<Store>,
    /// `(run_id, action_id)` keys applied in the CURRENT block — the
    /// duplicated-action_id dedupe (see the sibling-action caveat above).
    /// transient: cleared at every block boundary, never in the root.
    applied_this_block: BTreeSet<String>,
}

impl DocsHarness {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<ModuleId>,
        package: impl Into<ModuleId>,
        agent: impl Into<ModuleId>,
        jobs: impl Into<ModuleId>,
        memory: impl Into<ModuleId>,
        pages: impl Into<ModuleId>,
        runs: impl Into<ModuleId>,
    ) -> Self {
        Self {
            id: id.into(),
            package: package.into(),
            agent: agent.into(),
            jobs: jobs.into(),
            memory: memory.into(),
            pages: pages.into(),
            runs: runs.into(),
            committed: Store::default(),
            pending: None,
            applied_this_block: BTreeSet::new(),
        }
    }

    /// the staged view — pending if this block already wrote, else committed.
    fn store(&self) -> &Store {
        self.pending.as_ref().unwrap_or(&self.committed)
    }

    fn store_mut(&mut self) -> &mut Store {
        if self.pending.is_none() {
            self.pending = Some(self.committed.clone());
        }
        self.pending.as_mut().expect("just populated")
    }

    fn breadcrumb(&self, ctx: &mut dyn Ctx, what: String) {
        ctx.emit_event(Event {
            source: self.id.clone(),
            payload: what.into_bytes(),
        });
    }

    // ---- the harness contract (origin == package module) ----------------------

    fn install(
        &mut self,
        ctx: &mut dyn Ctx,
        package: String,
        spec: InstallSpec,
    ) -> Result<(), Error> {
        if self.store().installed.is_some() {
            return Err(Error::Module("docs-harness already hosts a package".into()));
        }
        let bindings: BTreeMap<&str, &str> = spec
            .modules
            .iter()
            .map(|b| (b.logical.as_str(), b.module_id.as_str()))
            .collect();

        // this harness consumes exactly ONE event source — its wired pages
        // module. an engagement bound anywhere else is a manifest bug.
        for rule in &spec.engagements {
            let source = bindings.get(rule.source.as_str()).ok_or_else(|| {
                Error::Module(format!("engagement source is not bound: {}", rule.source))
            })?;
            if *source != self.pages {
                return Err(Error::Module(format!(
                    "engagement source {source:?} is not the wired pages module {:?}",
                    self.pages
                )));
            }
        }
        if !spec.engagements.is_empty() {
            ctx.emit_msg(Msg {
                target: self.pages.clone(),
                payload: pages_encode_msg(&PageMsg::RegisterHook {}),
            });
        }

        // register each agent seed FROM THIS MODULE'S ORIGIN (harness-owned),
        // its prompt pinned to the seed path at the fresh generation.
        for seed in &spec.agents {
            let prompt = spec
                .prompts
                .iter()
                .find(|p| p.logical == seed.prompt)
                .ok_or_else(|| {
                    Error::Module(format!("agent prompt is not seeded: {}", seed.agent_id))
                })?;
            ctx.emit_msg(Msg {
                target: self.agent.clone(),
                payload: agent_encode_msg(&AgentMsg::RegisterAgent {
                    agent_id: seed.agent_id.clone(),
                    display_name: seed.display_name.clone(),
                    capability: seed.capability.clone(),
                    prompt: Some(PromptRef {
                        module: self.memory.clone(),
                        target: format!("{}@{FIRST_GENERATION}", prompt.path),
                        renderer: RENDERER_MEMORY_GENERATION.into(),
                        sha256: prompt.sha256.clone(),
                    }),
                    allowed_actions: seed.actions.clone(),
                }),
            });
            if !seed.active {
                ctx.emit_msg(Msg {
                    target: self.agent.clone(),
                    payload: agent_encode_msg(&AgentMsg::PauseAgent {
                        agent_id: seed.agent_id.clone(),
                    }),
                });
            }
        }

        self.store_mut().installed = Some(Installed {
            package: package.clone(),
            phase: Phase::Active,
            agents: spec.agents.iter().map(|a| a.agent_id.clone()).collect(),
        });

        // the ack that flips Installing -> Active — LAST, so it rides behind
        // every registration this arm staged.
        ctx.emit_msg(Msg {
            target: self.package.clone(),
            payload: package_encode_msg(&PackageMsg::MarkActive { package }),
        });
        Ok(())
    }

    /// the shared lifecycle transition: check the recorded package + expected
    /// phase, flip, and hand back the record for the caller's follow-ups.
    fn transition(&mut self, package: &str, from: &[Phase], to: Phase) -> Result<Installed, Error> {
        let installed = self
            .store()
            .installed
            .clone()
            .ok_or_else(|| Error::Module("docs-harness hosts no package".into()))?;
        if installed.package != package {
            return Err(Error::Module(format!(
                "docs-harness hosts {:?}, not {package:?}",
                installed.package
            )));
        }
        if !from.contains(&installed.phase) {
            return Err(Error::Module(format!(
                "package {package} is {:?}, not {from:?}",
                installed.phase.as_str()
            )));
        }
        let store = self.store_mut();
        let record = store.installed.as_mut().expect("checked above");
        record.phase = to;
        Ok(record.clone())
    }

    fn suspend(&mut self, ctx: &mut dyn Ctx, package: String) -> Result<(), Error> {
        let record = self.transition(&package, &[Phase::Active], Phase::Suspended)?;
        for agent_id in &record.agents {
            ctx.emit_msg(Msg {
                target: self.agent.clone(),
                payload: agent_encode_msg(&AgentMsg::PauseAgent {
                    agent_id: agent_id.clone(),
                }),
            });
        }
        Ok(())
    }

    fn resume(&mut self, ctx: &mut dyn Ctx, package: String) -> Result<(), Error> {
        let record = self.transition(&package, &[Phase::Suspended], Phase::Active)?;
        for agent_id in &record.agents {
            ctx.emit_msg(Msg {
                target: self.agent.clone(),
                payload: agent_encode_msg(&AgentMsg::ResumeAgent {
                    agent_id: agent_id.clone(),
                }),
            });
        }
        Ok(())
    }

    fn unplug(&mut self, ctx: &mut dyn Ctx, package: String) -> Result<(), Error> {
        let record = self.transition(
            &package,
            &[Phase::Active, Phase::Suspended],
            Phase::Unplugged,
        )?;
        // tombstone the agents (terminal, audit-preserving) and drop the
        // hook; pages content and comments — user data — stay untouched
        // (preserve-by-default), as do our own audit rows.
        for agent_id in &record.agents {
            ctx.emit_msg(Msg {
                target: self.agent.clone(),
                payload: agent_encode_msg(&AgentMsg::TombstoneAgent {
                    agent_id: agent_id.clone(),
                }),
            });
        }
        ctx.emit_msg(Msg {
            target: self.pages.clone(),
            payload: pages_encode_msg(&PageMsg::UnregisterHook {}),
        });
        Ok(())
    }

    // ---- engagement intake (origin == the pages module; NO-FAIL) ---------------

    async fn on_page_event(&mut self, ctx: &mut dyn Ctx, event: PageEvent) {
        let Some(installed) = self.store().installed.clone() else {
            return; // unreachable: the hook exists only while installed.
        };
        if installed.phase != Phase::Active {
            return; // suspended/unplugged packages mint nothing.
        }
        let PageEvent::CommentAdded {
            page_id,
            target,
            thread_id,
            comment_id,
            author,
            text,
        } = event
        else {
            return; // only comments engage the editor.
        };
        // LOOP PREVENTION: pages fans out ALL writes, including the ones our
        // own Apply follow-ups cause — an event we (or one of our agents)
        // authored must never re-engage, whatever its text says.
        if self.is_own_author(&author, &installed) {
            return;
        }
        for agent_id in &installed.agents {
            if !text.contains(&format!("@{agent_id}")) {
                continue;
            }
            // idempotency: one job per (comment, agent), across redeliveries.
            let key = format!("{comment_id}\u{1f}{agent_id}");
            if self.store().minted.contains(&key) {
                self.breadcrumb(
                    ctx,
                    format!("comment {comment_id} already minted a job for {agent_id}"),
                );
                continue;
            }
            // probe-before-emit: a squatted job id would make the Submit
            // follow-up abort the COMMENTER's block (this arm is no-fail).
            let job_id = engagement_job_id(agent_id, &comment_id);
            match self.job_exists(ctx, &job_id).await {
                Ok(false) => {
                    self.store_mut().minted.insert(key);
                    ctx.emit_msg(Msg {
                        target: self.jobs.clone(),
                        payload: jobs_encode_msg(&JobsMsg::Submit {
                            job_id,
                            kind: format!("agent/{agent_id}"),
                            spec: encode_engagement_spec(&EngagementSpec {
                                page_id: page_id.clone(),
                                target: target.clone(),
                                thread_id: thread_id.clone(),
                                comment_id: comment_id.clone(),
                                // a bounded excerpt, NEVER the full comment: a
                                // near-cap comment would push the spec past
                                // the jobs cap and abort the commenter's block.
                                text: engagement_excerpt(&text),
                            }),
                        }),
                    });
                }
                Ok(true) => {
                    self.breadcrumb(ctx, format!("job id already taken: {job_id}"));
                }
                Err(e) => {
                    self.breadcrumb(ctx, format!("jobs probe failed for {job_id}: {e}"));
                }
            }
        }
    }

    /// whether a pages write was authored by this module or one of its
    /// registered agents — the loop-prevention predicate.
    fn is_own_author(&self, author: &AuthorRef, installed: &Installed) -> bool {
        match author {
            AuthorRef::Module(module) => *module == self.id,
            AuthorRef::Agent { module, agent_id } => {
                *module == self.id || installed.agents.contains(agent_id)
            }
            _ => false,
        }
    }

    async fn job_exists(&self, ctx: &dyn Ctx, job_id: &str) -> Result<bool, String> {
        let reply = ctx
            .query(
                &self.jobs,
                &jobs::encode_query(&JobsQuery::Get {
                    job_id: job_id.into(),
                }),
            )
            .await
            .map_err(|e| e.to_string())?;
        match jobs::decode_reply(&reply) {
            Ok(JobsReply::Job(job)) => Ok(job.is_some()),
            Ok(other) => Err(format!("unexpected jobs reply: {other:?}")),
            Err(e) => Err(e),
        }
    }

    // ---- the action-owner contract ---------------------------------------------

    /// read one block from the wired pages module (staged-over-committed).
    async fn block_of(
        &self,
        ctx: &dyn Ctx,
        block_id: &str,
    ) -> Result<Option<pages::Block>, String> {
        let reply = ctx
            .query(
                &self.pages,
                &pages_encode_query(&PageQuery::GetBlock {
                    block_id: block_id.into(),
                }),
            )
            .await
            .map_err(|e| format!("pages query failed: {e}"))?;
        match pages::decode_reply(&reply) {
            Ok(PageReply::Block(block)) => Ok(block),
            Ok(other) => Err(format!("unexpected pages reply: {other:?}")),
            Err(e) => Err(e),
        }
    }

    /// read one comment thread from the wired pages module.
    async fn thread_of(
        &self,
        ctx: &dyn Ctx,
        thread_id: &str,
    ) -> Result<Option<ThreadView>, String> {
        let reply = ctx
            .query(
                &self.pages,
                &pages_encode_query(&PageQuery::CommentThread {
                    thread_id: thread_id.into(),
                }),
            )
            .await
            .map_err(|e| format!("pages query failed: {e}"))?;
        match pages::decode_reply(&reply) {
            Ok(PageReply::CommentThread(view)) => Ok(view),
            Ok(other) => Err(format!("unexpected pages reply: {other:?}")),
            Err(e) => Err(e),
        }
    }

    /// validate one owned action against STAGED-OR-COMMITTED pages state —
    /// the read-only half of `Probe` and the re-check `Apply` runs.
    async fn validate_action(
        &self,
        ctx: &dyn Ctx,
        action_id: &str,
        tag: &str,
        payload: &[u8],
        run_context: &[u8],
    ) -> Result<ValidatedAction, String> {
        let active = self
            .store()
            .installed
            .as_ref()
            .is_some_and(|i| i.phase == Phase::Active);
        if !active {
            return Err("the docs package is not active".into());
        }
        match tag {
            ACTION_COMMENT_ADD => {
                let p: CommentAddPayload = serde_json::from_slice(payload)
                    .map_err(|e| format!("malformed {tag} payload: {e}"))?;
                if p.text.is_empty() || p.text.len() > MAX_COMMENT_TEXT_BYTES {
                    return Err(format!("text must be 1..={MAX_COMMENT_TEXT_BYTES} bytes"));
                }
                if self.block_of(ctx, &p.target).await?.is_none() {
                    return Err(format!("unknown comment target: {}", p.target));
                }
                // the minted ids embed (run_id, action_id) — bound the parts.
                if action_id.is_empty() || action_id.len() > MAX_ACTION_ID_BYTES {
                    return Err(format!("action_id must be 1..={MAX_ACTION_ID_BYTES} bytes"));
                }
                let rc: RunContext = serde_json::from_slice(run_context)
                    .map_err(|e| format!("malformed run context: {e}"))?;
                let thread_id = match &p.thread_id {
                    Some(thread_id) => {
                        let view = self
                            .thread_of(ctx, thread_id)
                            .await?
                            .ok_or_else(|| format!("unknown thread: {thread_id}"))?;
                        if view.thread.target != p.target {
                            return Err(format!(
                                "thread {thread_id} targets {:?}, not {:?}",
                                view.thread.target, p.target
                            ));
                        }
                        if view.thread.comment_ids.len() >= pages::MAX_COMMENTS_PER_THREAD {
                            return Err(format!("thread is full: {thread_id}"));
                        }
                        thread_id.clone()
                    }
                    None => {
                        // opening a new thread under a minted id: a squatted
                        // id would abort the delivery block (a comment write
                        // is not idempotent), so probe it away here.
                        let minted = minted_thread_id(&rc.run_id, action_id);
                        if self.thread_of(ctx, &minted).await?.is_some() {
                            return Err(format!("minted thread id already taken: {minted}"));
                        }
                        minted
                    }
                };
                Ok(ValidatedAction::CommentAdd {
                    thread_id,
                    comment_id: minted_comment_id(&rc.run_id, action_id),
                    target: p.target,
                    text: p.text,
                })
            }
            ACTION_BLOCK_UPDATE_TEXT => {
                let p: BlockUpdateTextPayload = serde_json::from_slice(payload)
                    .map_err(|e| format!("malformed {tag} payload: {e}"))?;
                if p.text.len() > MAX_BLOCK_TEXT_BYTES {
                    return Err(format!("text must be at most {MAX_BLOCK_TEXT_BYTES} bytes"));
                }
                let block = self
                    .block_of(ctx, &p.block_id)
                    .await?
                    .ok_or_else(|| format!("unknown block: {}", p.block_id))?;
                if let Some(expected) = &p.expected_hash {
                    let pin = parse_sha256_field(expected)
                        .ok_or_else(|| format!("malformed expected_hash: {expected}"))?;
                    let current: Vec<u8> = Sha256::digest(block.text.as_bytes()).to_vec();
                    if current != pin {
                        return Err(format!(
                            "expected_hash mismatch: block {} changed since the agent read it",
                            p.block_id
                        ));
                    }
                }
                Ok(ValidatedAction::UpdateText {
                    block_id: p.block_id,
                    text: p.text,
                })
            }
            ACTION_THREAD_RESOLVE => {
                let p: ThreadResolvePayload = serde_json::from_slice(payload)
                    .map_err(|e| format!("malformed {tag} payload: {e}"))?;
                if self.thread_of(ctx, &p.thread_id).await?.is_none() {
                    return Err(format!("unknown thread: {}", p.thread_id));
                }
                Ok(ValidatedAction::ResolveThread {
                    thread_id: p.thread_id,
                    resolved: p.resolved,
                })
            }
            other => Err(format!("docs-harness does not own action tag: {other}")),
        }
    }

    /// NO-FAIL: an accepted action's `Apply` rides the runs module's delivery
    /// block — decode-or-drop, re-validate against now-staged pages state,
    /// then translate to the `PageMsg` follow-up; a late conflict lands a
    /// committed error row + breadcrumb instead of a block abort.
    async fn apply_action(&mut self, ctx: &mut dyn Ctx, apply: &PackageActionMsg) {
        let PackageActionMsg::Apply {
            action_id,
            tag,
            payload,
            run_context,
        } = apply;
        let validated = match self
            .validate_action(&*ctx, action_id, tag, payload, run_context)
            .await
        {
            Ok(validated) => validated,
            Err(reason) => {
                self.record_failure(ctx, action_id, tag, reason);
                return;
            }
        };
        // the duplicated-action_id dedupe (see the sibling-action caveat in
        // the module docs): a same-block re-apply of one (run, action) key
        // would mint identical page ids and poison the delivery block.
        let dedupe = match serde_json::from_slice::<RunContext>(run_context) {
            Ok(rc) => format!("{}\u{1f}{action_id}", rc.run_id),
            Err(_) => format!("\u{1f}{action_id}"), // only reachable for tags that skip rc
        };
        if !self.applied_this_block.insert(dedupe) {
            self.record_failure(
                ctx,
                action_id,
                tag,
                "duplicate action_id in one delivery".into(),
            );
            return;
        }
        let follow_up = match validated {
            ValidatedAction::CommentAdd {
                thread_id,
                comment_id,
                target,
                text,
            } => PageMsg::AddComment {
                thread_id,
                comment_id,
                target,
                text,
            },
            ValidatedAction::UpdateText { block_id, text } => {
                PageMsg::UpdateText { block_id, text }
            }
            ValidatedAction::ResolveThread {
                thread_id,
                resolved,
            } => PageMsg::ResolveThread {
                thread_id,
                resolved,
            },
        };
        ctx.emit_msg(Msg {
            target: self.pages.clone(),
            payload: pages_encode_msg(&follow_up),
        });
    }

    /// land one error row (bounded, oldest evicted) + its breadcrumb — the
    /// committed half of "mutate nothing, record failure".
    fn record_failure(&mut self, ctx: &mut dyn Ctx, action_id: &str, tag: &str, reason: String) {
        self.breadcrumb(ctx, format!("action {action_id} ({tag}) dropped: {reason}"));
        let failures = &mut self.store_mut().failures;
        if failures.len() >= MAX_FAILURE_ROWS {
            failures.remove(0);
        }
        failures.push(FailureRow {
            action_id: action_id.into(),
            tag: tag.into(),
            reason,
        });
    }

    // ---- root / snapshot ---------------------------------------------------------

    fn root_of(store: &Store) -> StateRoot {
        StateRoot(Sha256::digest(store.encode()).into())
    }

    /// the exact `root()` preimage (the platform snapshot-bytes convention).
    pub fn snapshot(&self) -> Vec<u8> {
        self.committed.encode()
    }

    /// verify-then-adopt a peer image (the memory/tasks pattern).
    pub fn install_snapshot(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        let store = Store::decode(bytes)?;
        if Self::root_of(&store) != expected {
            return Err(Error::Module("snapshot root mismatch".into()));
        }
        self.committed = store;
        self.pending = None;
        self.applied_this_block.clear();
        Ok(())
    }
}

// ---- canonical encode / decode ------------------------------------------------------

impl Store {
    // u64-le counts, length-prefixed strings, single tag/phase bytes, sorted
    // keys — the state-based module encoding discipline.
    fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        match &self.installed {
            None => out.push(0),
            Some(installed) => {
                out.push(1);
                push_str(&mut out, &installed.package);
                out.push(match installed.phase {
                    Phase::Active => 0,
                    Phase::Suspended => 1,
                    Phase::Unplugged => 2,
                });
                out.extend_from_slice(&(installed.agents.len() as u64).to_le_bytes());
                for agent in &installed.agents {
                    push_str(&mut out, agent);
                }
            }
        }
        out.extend_from_slice(&(self.minted.len() as u64).to_le_bytes());
        for key in &self.minted {
            push_str(&mut out, key);
        }
        out.extend_from_slice(&(self.failures.len() as u64).to_le_bytes());
        for row in &self.failures {
            push_str(&mut out, &row.action_id);
            push_str(&mut out, &row.tag);
            push_str(&mut out, &row.reason);
        }
        out
    }

    /// strict decode: only canonical encodings of execute-reachable states
    /// are accepted (sorted unique keys, valid phase, bounded failure log,
    /// no trailing bytes).
    fn decode(bytes: &[u8]) -> Result<Store, Error> {
        let mut off = 0usize;
        let installed = match read_byte(bytes, &mut off)? {
            0 => None,
            1 => {
                let package = read_string(bytes, &mut off)?;
                if package.is_empty() {
                    return Err(Error::Module("snapshot package id is empty".into()));
                }
                let phase = match read_byte(bytes, &mut off)? {
                    0 => Phase::Active,
                    1 => Phase::Suspended,
                    2 => Phase::Unplugged,
                    _ => return Err(Error::Module("snapshot phase is invalid".into())),
                };
                let agent_count = read_count(bytes, &mut off)?;
                let mut agents: Vec<String> = Vec::new();
                for _ in 0..agent_count {
                    let agent = read_string(bytes, &mut off)?;
                    if agent.is_empty() || agents.contains(&agent) {
                        return Err(Error::Module("snapshot agents are invalid".into()));
                    }
                    agents.push(agent);
                }
                Some(Installed {
                    package,
                    phase,
                    agents,
                })
            }
            _ => return Err(Error::Module("snapshot installed tag is invalid".into())),
        };

        let minted_count = read_count(bytes, &mut off)?;
        let mut minted: BTreeSet<String> = BTreeSet::new();
        for _ in 0..minted_count {
            let key = read_string(bytes, &mut off)?;
            if key.is_empty() || minted.last().is_some_and(|last| *last >= key) {
                return Err(Error::Module(
                    "snapshot minted keys not strictly ascending".into(),
                ));
            }
            minted.insert(key);
        }

        let failure_count = read_count(bytes, &mut off)?;
        if failure_count > MAX_FAILURE_ROWS as u64 {
            return Err(Error::Module("snapshot failure log exceeds its cap".into()));
        }
        let mut failures: Vec<FailureRow> = Vec::new();
        for _ in 0..failure_count {
            let action_id = read_string(bytes, &mut off)?;
            let tag = read_string(bytes, &mut off)?;
            let reason = read_string(bytes, &mut off)?;
            if tag.is_empty() || reason.is_empty() {
                return Err(Error::Module("snapshot failure row is invalid".into()));
            }
            failures.push(FailureRow {
                action_id,
                tag,
                reason,
            });
        }

        if off != bytes.len() {
            return Err(Error::Module("snapshot has trailing bytes".into()));
        }
        Ok(Store {
            installed,
            minted,
            failures,
        })
    }
}

/// parse a `"sha256:<64 lowercase hex>"` field into raw digest bytes; `None`
/// on any other shape (a malformed guard is a clean rejection).
fn parse_sha256_field(field: &str) -> Option<Vec<u8>> {
    let hex = field.strip_prefix("sha256:")?;
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
        .collect()
}

fn push_str(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(&(value.len() as u64).to_le_bytes());
    out.extend_from_slice(value.as_bytes());
}

fn read_byte(bytes: &[u8], off: &mut usize) -> Result<u8, Error> {
    let b = *bytes
        .get(*off)
        .ok_or_else(|| Error::Module("snapshot truncated".into()))?;
    *off += 1;
    Ok(b)
}

fn read_count(bytes: &[u8], off: &mut usize) -> Result<u64, Error> {
    let end = off
        .checked_add(8)
        .filter(|&end| end <= bytes.len())
        .ok_or_else(|| Error::Module("snapshot truncated".into()))?;
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&bytes[*off..end]);
    *off = end;
    let n = u64::from_le_bytes(buf);
    if n > (bytes.len() - *off) as u64 {
        return Err(Error::Module("snapshot truncated".into()));
    }
    Ok(n)
}

fn read_string(bytes: &[u8], off: &mut usize) -> Result<String, Error> {
    let len = read_count(bytes, off)? as usize;
    let value = std::str::from_utf8(&bytes[*off..*off + len])
        .map_err(|_| Error::Module("snapshot string is not utf-8".into()))?
        .to_owned();
    *off += len;
    Ok(value)
}

// ---- the module seam ------------------------------------------------------------

#[async_trait::async_trait(?Send)]
impl Module for DocsHarness {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn root(&self) -> StateRoot {
        Self::root_of(&self.committed)
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::SnapshotBytes(self.snapshot()))
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        let origin = ctx.env().origin.clone();

        // the harness contract — ONLY the package module's origin. a
        // HarnessMsg-shaped payload from anyone else falls through to the
        // rejection below: it is never acted on.
        if origin == Origin::Module(self.package.clone()) {
            return match decode_harness_msg(&msg.payload).map_err(Error::Module)? {
                HarnessMsg::InstallPackage { package, spec } => self.install(ctx, package, spec),
                HarnessMsg::SuspendPackage { package } => self.suspend(ctx, package),
                HarnessMsg::ResumePackage { package } => self.resume(ctx, package),
                HarnessMsg::UnplugPackage { package } => self.unplug(ctx, package),
            };
        }

        // engagement intake from the wired pages module — NO-FAIL: this rides
        // the WRITER's block (a commenter must never be aborted by us).
        if origin == Origin::Module(self.pages.clone()) {
            match decode_page_event(&msg.payload) {
                Ok(event) => self.on_page_event(ctx, event).await,
                Err(e) => self.breadcrumb(ctx, format!("dropped undecodable page event: {e}")),
            }
            return Ok(());
        }

        // an accepted action's Apply, riding the delivery block — NO-FAIL,
        // and gated to the RUNS module's origin specifically.
        if origin == Origin::Module(self.runs.clone()) {
            match decode_action_msg(&msg.payload) {
                Ok(apply) => self.apply_action(ctx, &apply).await,
                Err(e) => self.breadcrumb(ctx, format!("dropped undecodable apply: {e}")),
            }
            return Ok(());
        }

        Err(Error::Module(
            "docs-harness accepts HarnessMsg from the package module, PageEvents from the \
             pages module, and package-action Applies from the runs module"
                .into(),
        ))
    }

    /// external reads serve COMMITTED state only.
    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        let reply = match interface::decode_query(req).map_err(Error::Module)? {
            DocsQuery::Status => {
                DocsReply::Status(self.committed.installed.as_ref().map(|i| DocsStatus {
                    package: i.package.clone(),
                    phase: i.phase.as_str().into(),
                    agents: i.agents.clone(),
                    minted: self.committed.minted.len() as u64,
                    failures: self.committed.failures.len() as u64,
                }))
            }
            DocsQuery::Failures => DocsReply::Failures(self.committed.failures.clone()),
        };
        Ok(interface::encode_reply(&reply))
    }

    /// the action owner's `Probe` rides the same query lane (the wire shapes
    /// are disjoint, so decode picks); the verdict reads STAGED-or-committed
    /// state — which is exactly why a probe can never see sibling actions
    /// from its own response: their applies have not staged yet.
    async fn query_with(&self, ctx: &dyn Ctx, req: &[u8]) -> Result<Vec<u8>, Error> {
        if let Ok(PackageActionQuery::Probe {
            action_id,
            tag,
            payload,
            run_context,
        }) = decode_action_query(req)
        {
            let verdict = match self
                .validate_action(ctx, &action_id, &tag, &payload, &run_context)
                .await
            {
                Ok(_) => PackageActionReply::Accepted,
                Err(reason) => PackageActionReply::Rejected { reason },
            };
            return Ok(encode_action_reply(&verdict));
        }
        self.query(req).await
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        if let Some(pending) = self.pending.take() {
            self.committed = pending;
        }
        self.applied_this_block.clear();
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.pending = None;
        self.applied_this_block.clear();
        Ok(())
    }
}

// ---- module-level tests --------------------------------------------------------
// the seams the end-to-end package_loop suite cannot reach through real
// modules: literal redelivery of one event (pages never redelivers), events
// authored by ourselves (loop prevention against a canned author), malformed
// events/applies, probe verdicts against canned pages state, and origin gates.

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;
    use package::{
        ActionRoute, AgentSeed, EngagementRule, MANIFEST_HASH_LEN, ModuleBinding, PromptSeed,
        UninstallPolicy, encode_action_msg, encode_action_query, encode_harness_msg,
    };
    use pages::{Block, BlockKind, Comment, Thread};

    const HARNESS: &str = "docs-harness";
    const PKG: &str = "org.ducktape.docs";
    const AGENT: &str = "docs.editor";
    const PROMPT_PATH: &str = "/packages/org.ducktape.docs/prompts/docs-editor.md";
    const RUN_CONTEXT: &[u8] = br#"{"run_id":"r1","agent_id":"docs.editor"}"#;

    /// canned pages state the ctx serves: block b1 ("old text") on page p1,
    /// thread t1 anchored to b1.
    struct TestCtx {
        env: sdk::Env,
        emitted: Vec<Msg>,
        events: Vec<String>,
        job_taken: bool,
    }

    impl TestCtx {
        fn at(origin: Origin) -> Self {
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
            }
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
                        other => return Err(Error::Module(format!("unexpected query: {other:?}"))),
                    };
                    Ok(pages::encode_reply(&reply))
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

    fn spec() -> InstallSpec {
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

    fn module() -> DocsHarness {
        DocsHarness::new(
            HARNESS, "package", "agent", "jobs", "memory", "pages", "runs",
        )
    }

    fn package_origin() -> Origin {
        Origin::Module("package".into())
    }

    fn exec(m: &mut DocsHarness, ctx: &mut TestCtx, payload: Vec<u8>) -> Result<(), Error> {
        block_on(m.execute(
            ctx,
            &Msg {
                target: HARNESS.into(),
                payload,
            },
        ))
    }

    fn commit(m: &mut DocsHarness) {
        block_on(m.commit_block()).unwrap();
    }

    fn installed(m: &mut DocsHarness) {
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

    fn comment_event(comment_id: &str, text: &str) -> Vec<u8> {
        comment_event_by(comment_id, text, AuthorRef::User(vec![7; 32]))
    }

    fn comment_event_by(comment_id: &str, text: &str, author: AuthorRef) -> Vec<u8> {
        pages::encode_page_event(&PageEvent::CommentAdded {
            page_id: "p1".into(),
            target: "b1".into(),
            thread_id: "t1".into(),
            comment_id: comment_id.into(),
            author,
            text: text.into(),
        })
    }

    fn apply(action_id: &str, tag: &str, payload: serde_json::Value) -> Vec<u8> {
        encode_action_msg(&PackageActionMsg::Apply {
            action_id: action_id.into(),
            tag: tag.into(),
            payload: serde_json::to_vec(&payload).unwrap(),
            run_context: RUN_CONTEXT.to_vec(),
        })
    }

    fn probe(m: &DocsHarness, tag: &str, payload: serde_json::Value) -> PackageActionReply {
        let ctx = TestCtx::at(Origin::Module("runs".into()));
        let req = encode_action_query(&PackageActionQuery::Probe {
            action_id: "a1".into(),
            tag: tag.into(),
            payload: serde_json::to_vec(&payload).unwrap(),
            run_context: RUN_CONTEXT.to_vec(),
        });
        package::decode_action_reply(&block_on(m.query_with(&ctx, &req)).unwrap()).unwrap()
    }

    fn rejects(reply: &PackageActionReply, needle: &str) -> bool {
        matches!(reply, PackageActionReply::Rejected { reason } if reason.contains(needle))
    }

    // ---- install / lifecycle ------------------------------------------------------

    #[test]
    fn install_registers_hook_and_agent_then_acks_last() {
        let mut m = module();
        let mut ctx = TestCtx::at(package_origin());
        exec(
            &mut m,
            &mut ctx,
            encode_harness_msg(&HarnessMsg::InstallPackage {
                package: PKG.into(),
                spec: spec(),
            }),
        )
        .unwrap();

        assert_eq!(ctx.emitted.len(), 3);
        assert_eq!(ctx.emitted[0].target, "pages");
        assert_eq!(
            pages::decode_msg(&ctx.emitted[0].payload).unwrap(),
            PageMsg::RegisterHook {}
        );
        assert_eq!(ctx.emitted[1].target, "agent");
        match agent::decode_msg(&ctx.emitted[1].payload).unwrap() {
            AgentMsg::RegisterAgent {
                agent_id,
                prompt,
                allowed_actions,
                ..
            } => {
                assert_eq!(agent_id, AGENT);
                let prompt = prompt.expect("prompt pinned");
                assert_eq!(prompt.module, "memory");
                assert_eq!(prompt.target, format!("{PROMPT_PATH}@1"));
                assert_eq!(prompt.renderer, RENDERER_MEMORY_GENERATION);
                assert_eq!(prompt.sha256, Sha256::digest(b"# Docs Editor\n").to_vec());
                assert_eq!(
                    allowed_actions,
                    vec![
                        ACTION_COMMENT_ADD.to_string(),
                        ACTION_BLOCK_UPDATE_TEXT.to_string(),
                        ACTION_THREAD_RESOLVE.to_string(),
                    ]
                );
            }
            other => panic!("expected RegisterAgent, got {other:?}"),
        }
        assert_eq!(ctx.emitted[2].target, "package");
        assert_eq!(
            package::decode_msg(&ctx.emitted[2].payload).unwrap(),
            PackageMsg::MarkActive {
                package: PKG.into()
            }
        );

        // a second install rejects.
        commit(&mut m);
        let mut again = TestCtx::at(package_origin());
        assert!(
            exec(
                &mut m,
                &mut again,
                encode_harness_msg(&HarnessMsg::InstallPackage {
                    package: "org.example.other".into(),
                    spec: spec(),
                }),
            )
            .is_err()
        );
    }

    #[test]
    fn install_rejects_an_engagement_source_that_is_not_the_wired_pages_module() {
        let mut m = module();
        let mut bad = spec();
        bad.modules.push(ModuleBinding {
            logical: "chat".into(),
            module_id: "chat".into(),
        });
        bad.engagements[0].source = "chat".into();
        let mut ctx = TestCtx::at(package_origin());
        let err = exec(
            &mut m,
            &mut ctx,
            encode_harness_msg(&HarnessMsg::InstallPackage {
                package: PKG.into(),
                spec: bad,
            }),
        )
        .expect_err("a non-pages engagement source must reject");
        assert!(err.to_string().contains("pages"), "{err}");
    }

    #[test]
    fn harness_msgs_from_non_package_origins_are_never_acted_on() {
        let mut m = module();
        let payload = encode_harness_msg(&HarnessMsg::InstallPackage {
            package: PKG.into(),
            spec: spec(),
        });
        for origin in [
            Origin::External(b"mallory".to_vec()),
            Origin::Module("pages".into()),
            Origin::Module("runs".into()),
            Origin::System,
        ] {
            let mut ctx = TestCtx::at(origin.clone());
            let result = exec(&mut m, &mut ctx, payload.clone());
            // pages/runs origins land in the NO-FAIL lanes (dropped with a
            // breadcrumb); everything else is rejected outright. either way
            // nothing registers and no state lands.
            match &origin {
                Origin::Module(id) if id == "pages" || id == "runs" => {
                    result.expect("the no-fail lanes never abort");
                }
                _ => {
                    result.expect_err("other origins must be rejected");
                }
            }
            assert!(ctx.emitted.is_empty(), "{origin:?} must emit nothing");
        }
        commit(&mut m);
        assert_eq!(m.committed.installed, None, "no state landed");
    }

    #[test]
    fn suspend_pauses_resume_reverses_unplug_tombstones_and_unregisters() {
        let mut m = module();
        installed(&mut m);

        // suspend: agents pause, minting stops.
        let mut ctx = TestCtx::at(package_origin());
        exec(
            &mut m,
            &mut ctx,
            encode_harness_msg(&HarnessMsg::SuspendPackage {
                package: PKG.into(),
            }),
        )
        .unwrap();
        assert_eq!(ctx.emitted.len(), 1);
        assert!(matches!(
            agent::decode_msg(&ctx.emitted[0].payload).unwrap(),
            AgentMsg::PauseAgent { .. }
        ));
        commit(&mut m);
        let mut quiet = TestCtx::at(Origin::Module("pages".into()));
        exec(
            &mut m,
            &mut quiet,
            comment_event("c9", "@docs.editor anyone?"),
        )
        .unwrap();
        assert!(
            quiet.emitted.is_empty(),
            "a suspended package mints nothing"
        );

        // resume: agents resume, minting restarts.
        let mut ctx = TestCtx::at(package_origin());
        exec(
            &mut m,
            &mut ctx,
            encode_harness_msg(&HarnessMsg::ResumePackage {
                package: PKG.into(),
            }),
        )
        .unwrap();
        assert_eq!(ctx.emitted.len(), 1);
        assert!(matches!(
            agent::decode_msg(&ctx.emitted[0].payload).unwrap(),
            AgentMsg::ResumeAgent { .. }
        ));
        commit(&mut m);
        let mut minting = TestCtx::at(Origin::Module("pages".into()));
        exec(
            &mut m,
            &mut minting,
            comment_event("c10", "@docs.editor welcome back"),
        )
        .unwrap();
        assert_eq!(minting.emitted.len(), 1, "resume restores minting");
        commit(&mut m);

        // unplug (from active): tombstones + unregisters the hook.
        let mut ctx = TestCtx::at(package_origin());
        exec(
            &mut m,
            &mut ctx,
            encode_harness_msg(&HarnessMsg::UnplugPackage {
                package: PKG.into(),
            }),
        )
        .unwrap();
        assert_eq!(ctx.emitted.len(), 2);
        assert!(matches!(
            agent::decode_msg(&ctx.emitted[0].payload).unwrap(),
            AgentMsg::TombstoneAgent { .. }
        ));
        assert_eq!(ctx.emitted[1].target, "pages");
        assert_eq!(
            pages::decode_msg(&ctx.emitted[1].payload).unwrap(),
            PageMsg::UnregisterHook {}
        );
        commit(&mut m);
        assert!(
            !m.committed.minted.is_empty(),
            "audit state (idempotency keys) is preserved"
        );

        // and no further lifecycle op is accepted.
        let mut again = TestCtx::at(package_origin());
        assert!(
            exec(
                &mut m,
                &mut again,
                encode_harness_msg(&HarnessMsg::ResumePackage {
                    package: PKG.into(),
                }),
            )
            .is_err()
        );
    }

    // ---- engagement intake --------------------------------------------------------

    #[test]
    fn a_mention_comment_mints_one_job_idempotently() {
        let mut m = module();
        installed(&mut m);

        let event = comment_event("c1", "hey @docs.editor tighten this");
        let mut ctx = TestCtx::at(Origin::Module("pages".into()));
        exec(&mut m, &mut ctx, event.clone()).unwrap();
        assert_eq!(ctx.emitted.len(), 1);
        assert_eq!(ctx.emitted[0].target, "jobs");
        match jobs::decode_msg(&ctx.emitted[0].payload).unwrap() {
            JobsMsg::Submit { job_id, kind, spec } => {
                assert_eq!(job_id, engagement_job_id(AGENT, "c1"));
                assert_eq!(kind, format!("agent/{AGENT}"));
                let spec = decode_engagement_spec(&spec).expect("spec is the engagement shape");
                assert_eq!(spec.comment_id, "c1");
                assert_eq!(spec.target, "b1");
                assert_eq!(spec.page_id, "p1");
            }
            other => panic!("expected Submit, got {other:?}"),
        }
        commit(&mut m);

        // literal redelivery of the same event: nothing mints again.
        let mut again = TestCtx::at(Origin::Module("pages".into()));
        exec(&mut m, &mut again, event).unwrap();
        assert!(again.emitted.is_empty(), "redelivery must not re-mint");
        assert!(again.events.iter().any(|e| e.contains("already minted")));

        // a non-mention comment engages nothing.
        let mut quiet = TestCtx::at(Origin::Module("pages".into()));
        exec(&mut m, &mut quiet, comment_event("c2", "no robots here")).unwrap();
        assert!(quiet.emitted.is_empty());

        // a squatted job id: breadcrumb, no emit, no key burned.
        let mut squat = TestCtx::at(Origin::Module("pages".into()));
        squat.job_taken = true;
        exec(&mut m, &mut squat, comment_event("c3", "@docs.editor go")).unwrap();
        assert!(squat.emitted.is_empty());
        assert!(squat.events.iter().any(|e| e.contains("already taken")));
    }

    #[test]
    fn a_near_cap_comment_mints_one_bounded_job_and_never_aborts() {
        let mut m = module();
        installed(&mut m);

        // a comment near pages' 64 KiB cap, escape-heavy on purpose: embedded
        // verbatim, its JSON escaping alone would push the encoded spec past
        // the jobs board's 64 KiB spec cap and make Submit abort the
        // COMMENTER's block — the intake arm must bound the excerpt instead.
        let mut text = String::from("@docs.editor tighten this ");
        text.push_str(&"\"".repeat(pages::MAX_COMMENT_TEXT_BYTES - text.len()));
        assert_eq!(text.len(), pages::MAX_COMMENT_TEXT_BYTES);

        let mut ctx = TestCtx::at(Origin::Module("pages".into()));
        exec(&mut m, &mut ctx, comment_event("c-big", &text))
            .expect("the no-fail intake must not abort on a near-cap comment");
        assert_eq!(ctx.emitted.len(), 1, "exactly one job minted");
        match jobs::decode_msg(&ctx.emitted[0].payload).unwrap() {
            JobsMsg::Submit { job_id, spec, .. } => {
                assert_eq!(job_id, engagement_job_id(AGENT, "c-big"));
                assert!(
                    spec.len() <= jobs::MAX_SPEC,
                    "the encoded spec ({} bytes) must fit the jobs cap",
                    spec.len()
                );
                let spec = decode_engagement_spec(&spec).expect("spec decodes");
                assert!(spec.text.len() <= MAX_COMMENT_TEXT_BYTES);
                assert!(text.starts_with(&spec.text), "the excerpt is a prefix");
            }
            other => panic!("expected Submit, got {other:?}"),
        }
        commit(&mut m);
        assert_eq!(m.committed.minted.len(), 1, "the block committed the key");
    }

    #[test]
    fn own_authored_events_never_mint_the_loop_prevention_gate() {
        let mut m = module();
        installed(&mut m);

        // the three shapes our own writes (or our agents') can wear.
        for author in [
            AuthorRef::Module(HARNESS.into()),
            AuthorRef::Agent {
                module: HARNESS.into(),
                agent_id: "anything".into(),
            },
            AuthorRef::Agent {
                module: "agent".into(),
                agent_id: AGENT.into(),
            },
        ] {
            let mut ctx = TestCtx::at(Origin::Module("pages".into()));
            exec(
                &mut m,
                &mut ctx,
                comment_event_by("c-self", "@docs.editor see my edit", author.clone()),
            )
            .unwrap();
            assert!(
                ctx.emitted.is_empty(),
                "an own-authored event must never mint ({author:?})"
            );
        }
        commit(&mut m);
        assert!(m.committed.minted.is_empty());
    }

    #[test]
    fn a_malformed_page_event_from_pages_is_a_no_op_observation() {
        let mut m = module();
        installed(&mut m);
        let root = m.root();
        let mut ctx = TestCtx::at(Origin::Module("pages".into()));
        exec(&mut m, &mut ctx, b"not a page event".to_vec())
            .expect("a malformed event must NOT abort the writer's block");
        assert!(ctx.emitted.is_empty());
        assert!(ctx.events.iter().any(|e| e.contains("undecodable")));
        commit(&mut m);
        assert_eq!(m.root(), root, "nothing staged");
    }

    // ---- the action-owner contract ---------------------------------------------

    #[test]
    fn probe_validates_against_pages_state() {
        let mut m = module();
        installed(&mut m);

        // comment.add: an existing target accepts; open-a-new-thread mints.
        assert_eq!(
            probe(
                &m,
                ACTION_COMMENT_ADD,
                serde_json::json!({"target": "b1", "text": "done, see the edit"}),
            ),
            PackageActionReply::Accepted
        );
        // appending to an existing, matching thread accepts.
        assert_eq!(
            probe(
                &m,
                ACTION_COMMENT_ADD,
                serde_json::json!({"target": "b1", "thread_id": "t1", "text": "replying"}),
            ),
            PackageActionReply::Accepted
        );
        // an unknown target rejects.
        assert!(rejects(
            &probe(
                &m,
                ACTION_COMMENT_ADD,
                serde_json::json!({"target": "ghost", "text": "hi"}),
            ),
            "ghost"
        ));
        // an unknown thread rejects.
        assert!(rejects(
            &probe(
                &m,
                ACTION_COMMENT_ADD,
                serde_json::json!({"target": "b1", "thread_id": "t9", "text": "hi"}),
            ),
            "t9"
        ));
        // a thread anchored elsewhere rejects (t1 targets b1, not p1).
        assert!(rejects(
            &probe(
                &m,
                ACTION_COMMENT_ADD,
                serde_json::json!({"target": "p1", "thread_id": "t1", "text": "hi"}),
            ),
            "target"
        ));
        // empty text rejects.
        assert!(matches!(
            probe(
                &m,
                ACTION_COMMENT_ADD,
                serde_json::json!({"target": "b1", "text": ""}),
            ),
            PackageActionReply::Rejected { .. }
        ));

        // update_text: existing block accepts; the expected_hash guard bites.
        assert_eq!(
            probe(
                &m,
                ACTION_BLOCK_UPDATE_TEXT,
                serde_json::json!({"block_id": "b1", "text": "new text"}),
            ),
            PackageActionReply::Accepted
        );
        let good_hash = format!("sha256:{}", hex(&Sha256::digest(b"old text")));
        assert_eq!(
            probe(
                &m,
                ACTION_BLOCK_UPDATE_TEXT,
                serde_json::json!({"block_id": "b1", "text": "new text",
                                   "expected_hash": good_hash}),
            ),
            PackageActionReply::Accepted
        );
        let stale_hash = format!("sha256:{}", hex(&Sha256::digest(b"someone else's text")));
        assert!(rejects(
            &probe(
                &m,
                ACTION_BLOCK_UPDATE_TEXT,
                serde_json::json!({"block_id": "b1", "text": "new text",
                                   "expected_hash": stale_hash}),
            ),
            "expected_hash"
        ));
        assert!(rejects(
            &probe(
                &m,
                ACTION_BLOCK_UPDATE_TEXT,
                serde_json::json!({"block_id": "b1", "text": "x", "expected_hash": "md5:nope"}),
            ),
            "malformed"
        ));
        assert!(rejects(
            &probe(
                &m,
                ACTION_BLOCK_UPDATE_TEXT,
                serde_json::json!({"block_id": "ghost", "text": "x"}),
            ),
            "ghost"
        ));

        // thread.resolve: existing thread accepts, unknown rejects.
        assert_eq!(
            probe(
                &m,
                ACTION_THREAD_RESOLVE,
                serde_json::json!({"thread_id": "t1", "resolved": true}),
            ),
            PackageActionReply::Accepted
        );
        assert!(rejects(
            &probe(
                &m,
                ACTION_THREAD_RESOLVE,
                serde_json::json!({"thread_id": "t9", "resolved": true}),
            ),
            "t9"
        ));

        // schema strictness: unknown fields reject; unknown tags reject.
        assert!(rejects(
            &probe(
                &m,
                ACTION_BLOCK_UPDATE_TEXT,
                serde_json::json!({"block_id": "b1", "text": "x", "page_id": "p1"}),
            ),
            "malformed"
        ));
        assert!(rejects(
            &probe(&m, "pages.block.delete", serde_json::json!({})),
            "does not own"
        ));

        // nothing above staged anything: probes are read-only.
        commit(&mut m);
        assert!(m.committed.failures.is_empty());
    }

    #[test]
    fn probe_rejects_while_suspended() {
        let mut m = module();
        installed(&mut m);
        let mut ctx = TestCtx::at(package_origin());
        exec(
            &mut m,
            &mut ctx,
            encode_harness_msg(&HarnessMsg::SuspendPackage {
                package: PKG.into(),
            }),
        )
        .unwrap();
        commit(&mut m);
        assert!(matches!(
            probe(
                &m,
                ACTION_COMMENT_ADD,
                serde_json::json!({"target": "b1", "text": "hi"}),
            ),
            PackageActionReply::Rejected { .. }
        ));
    }

    #[test]
    fn apply_translates_to_page_msgs_and_is_no_fail() {
        let mut m = module();
        installed(&mut m);

        // comment.add without a thread: minted thread + comment ids.
        let mut ctx = TestCtx::at(Origin::Module("runs".into()));
        exec(
            &mut m,
            &mut ctx,
            apply(
                "a1",
                ACTION_COMMENT_ADD,
                serde_json::json!({"target": "b1", "text": "done"}),
            ),
        )
        .unwrap();
        assert_eq!(ctx.emitted.len(), 1);
        assert_eq!(ctx.emitted[0].target, "pages");
        assert_eq!(
            pages::decode_msg(&ctx.emitted[0].payload).unwrap(),
            PageMsg::AddComment {
                thread_id: minted_thread_id("r1", "a1"),
                comment_id: minted_comment_id("r1", "a1"),
                target: "b1".into(),
                text: "done".into(),
            }
        );
        commit(&mut m);

        // update_text and thread.resolve translate too.
        let mut ctx = TestCtx::at(Origin::Module("runs".into()));
        exec(
            &mut m,
            &mut ctx,
            apply(
                "a2",
                ACTION_BLOCK_UPDATE_TEXT,
                serde_json::json!({"block_id": "b1", "text": "new text"}),
            ),
        )
        .unwrap();
        assert_eq!(
            pages::decode_msg(&ctx.emitted[0].payload).unwrap(),
            PageMsg::UpdateText {
                block_id: "b1".into(),
                text: "new text".into(),
            }
        );
        commit(&mut m);
        let mut ctx = TestCtx::at(Origin::Module("runs".into()));
        exec(
            &mut m,
            &mut ctx,
            apply(
                "a3",
                ACTION_THREAD_RESOLVE,
                serde_json::json!({"thread_id": "t1", "resolved": true}),
            ),
        )
        .unwrap();
        assert_eq!(
            pages::decode_msg(&ctx.emitted[0].payload).unwrap(),
            PageMsg::ResolveThread {
                thread_id: "t1".into(),
                resolved: true,
            }
        );
        commit(&mut m);

        // a late conflict (stale expected_hash): error row + breadcrumb, Ok.
        let stale = format!("sha256:{}", hex(&Sha256::digest(b"stale")));
        let mut late = TestCtx::at(Origin::Module("runs".into()));
        exec(
            &mut m,
            &mut late,
            apply(
                "a4",
                ACTION_BLOCK_UPDATE_TEXT,
                serde_json::json!({"block_id": "b1", "text": "x", "expected_hash": stale}),
            ),
        )
        .expect("a late conflict must not abort the delivery block");
        assert!(late.emitted.is_empty(), "nothing mutated");
        assert!(late.events.iter().any(|e| e.contains("expected_hash")));
        commit(&mut m);
        assert_eq!(m.committed.failures.len(), 1);
        assert_eq!(m.committed.failures[0].action_id, "a4");

        // a malformed payload: error row + breadcrumb, Ok.
        let mut bad = TestCtx::at(Origin::Module("runs".into()));
        exec(
            &mut m,
            &mut bad,
            apply("a5", ACTION_COMMENT_ADD, serde_json::json!({"bogus": true})),
        )
        .expect("a malformed apply must not abort the delivery block");
        assert!(bad.emitted.is_empty());
        assert!(bad.events.iter().any(|e| e.contains("dropped")));
        commit(&mut m);
        assert_eq!(m.committed.failures.len(), 2);
    }

    #[test]
    fn a_duplicated_action_id_applies_once_per_block() {
        let mut m = module();
        installed(&mut m);

        // two applies with the SAME action_id in one block: identical minted
        // comment ids would abort the delivery block at pages — the second
        // must drop with an error row instead.
        let payload = apply(
            "a1",
            ACTION_COMMENT_ADD,
            serde_json::json!({"target": "b1", "text": "done"}),
        );
        let mut first = TestCtx::at(Origin::Module("runs".into()));
        exec(&mut m, &mut first, payload.clone()).unwrap();
        assert_eq!(first.emitted.len(), 1);
        let mut second = TestCtx::at(Origin::Module("runs".into()));
        exec(&mut m, &mut second, payload.clone()).unwrap();
        assert!(second.emitted.is_empty(), "the duplicate must not emit");
        assert!(second.events.iter().any(|e| e.contains("duplicate")));
        commit(&mut m);

        // the dedupe window is the block: a NEW block accepts the key again
        // (a real re-run never reuses (run_id, action_id) — see module docs).
        let mut next_block = TestCtx::at(Origin::Module("runs".into()));
        exec(&mut m, &mut next_block, payload).unwrap();
        assert_eq!(next_block.emitted.len(), 1);
    }

    #[test]
    fn applies_from_non_runs_origins_are_never_acted_on() {
        let mut m = module();
        installed(&mut m);
        let payload = apply(
            "a1",
            ACTION_COMMENT_ADD,
            serde_json::json!({"target": "b1", "text": "hi"}),
        );
        // module origins other than runs are rejected outright (tighter than
        // the dummy's any-module gate) — except pages, whose lane treats the
        // bytes as an undecodable event (no-fail, nothing acted on).
        for origin in [
            Origin::Module("mallory-module".into()),
            Origin::Module("agent".into()),
            Origin::External(b"mallory".to_vec()),
            Origin::System,
        ] {
            let mut ctx = TestCtx::at(origin.clone());
            assert!(
                exec(&mut m, &mut ctx, payload.clone()).is_err(),
                "{origin:?} must not drive an apply"
            );
            assert!(ctx.emitted.is_empty(), "{origin:?} must emit nothing");
        }
        let mut via_pages = TestCtx::at(Origin::Module("pages".into()));
        exec(&mut m, &mut via_pages, payload).unwrap();
        assert!(via_pages.emitted.is_empty());
        assert!(via_pages.events.iter().any(|e| e.contains("undecodable")));
        commit(&mut m);
        assert!(m.committed.failures.is_empty());
    }

    #[test]
    fn the_failure_log_is_bounded() {
        let mut m = module();
        installed(&mut m);
        for i in 0..(MAX_FAILURE_ROWS + 3) {
            let mut ctx = TestCtx::at(Origin::Module("runs".into()));
            exec(
                &mut m,
                &mut ctx,
                apply(
                    &format!("a{i}"),
                    ACTION_COMMENT_ADD,
                    serde_json::json!({"target": "ghost", "text": "hi"}),
                ),
            )
            .unwrap();
        }
        commit(&mut m);
        assert_eq!(m.committed.failures.len(), MAX_FAILURE_ROWS);
        // the oldest rows were evicted: a0..a2 are gone, a3 leads.
        assert_eq!(m.committed.failures[0].action_id, "a3");
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }
}
