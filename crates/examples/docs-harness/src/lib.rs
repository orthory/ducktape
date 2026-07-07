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
//! squatted MINTED ids are probed away: the minted thread id via
//! `PageQuery::CommentThread`, the minted comment id via
//! `PageQuery::GetComment` (comment ids are globally unique in pages, so a
//! squat in ANY thread collides) — both at probe time and again on the apply
//! re-check, where a late squat lands an error row instead of a block abort.
//!
//! ## prompt generation pin
//!
//! the install block publishes each prompt seed into memory BEFORE this
//! module's install arm runs, and memory serves same-block queries
//! staged-over-committed — so the install arm asks memory for each seed
//! path's ACTUAL latest generation and pins THAT. assuming generation 1
//! instead would hand anyone who pre-publishes junk at the predictable path
//! a permanent brick: the seed would land at generation 2 while the agent
//! pins 1, and every run would fail pin-mismatch with no repair path (the
//! agents are harness-owned and the package id stays claimed).

use std::collections::BTreeSet;

use package::{
    HarnessMsg, PackageActionQuery, PackageActionReply, decode_action_msg, decode_action_query,
    decode_harness_msg, encode_action_reply,
};
use pages::decode_page_event;
use sdk::{Ctx, Error, Event, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle};

mod actions;
mod events;
mod install;
mod interface;
mod state;
#[cfg(test)]
mod testutil;

pub use interface::*;

use state::Store;

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

    fn breadcrumb(&self, ctx: &mut dyn Ctx, what: String) {
        ctx.emit_event(Event {
            source: self.id.clone(),
            payload: what.into_bytes(),
        });
    }
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
                HarnessMsg::InstallPackage { package, spec } => {
                    self.install(ctx, package, spec).await
                }
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

// ---- origin-gate tests -----------------------------------------------------------
// the execute seam itself: payloads of every shape, arriving from origins the
// lanes above must never act on. the per-lane behavior lives with its lane
// (install.rs / events.rs / actions.rs).

#[cfg(test)]
mod tests {
    use super::*;
    use package::encode_harness_msg;

    use crate::testutil::*;

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
}
