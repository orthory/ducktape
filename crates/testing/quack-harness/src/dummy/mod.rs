//! the framework's reference package: the smallest COMPLETE harness module.
//!
//! **this module is a TEST FIXTURE and a COPYABLE TEMPLATE — it is not a
//! production package.** `DummyHarness` implements the whole harness contract
//! (design D4) against a trivial domain — a keyed note pad — so the framework
//! can prove itself end-to-end without depending on the real `docs-harness`,
//! and so a package author has the smallest COMPLETE reference to copy the
//! contract from. it ships as unconditional public API (this crate's own
//! tests drive it from an external `tests/` integration test, so it cannot be
//! `#[cfg(test)]`-gated) — never wire it into a real module catalog.
//!
//! - `HarnessMsg` arms, accepted ONLY from the package module's origin:
//!   install registers a `PageMsg::RegisterHook` on every engagement source,
//!   registers each agent seed from module origin (owner = this harness) with
//!   a `PromptRef` pinned to the seeded memory path, and acks with
//!   `PackageMsg::MarkActive`; suspend/resume/unplug pause/resume/tombstone
//!   the agents (unplug also unregisters the hooks, preserving user data).
//! - `PageEvent` intake (no-fail) from the recorded sources: a comment
//!   mentioning `@<agent_id>` mints ONE idempotent `JobsMsg::Submit`
//!   (`kind = "agent/<agent_id>"`), probe-before-emit against the jobs board.
//! - the action-owner contract for `dummy.note.add` / `dummy.note.set_text`:
//!   `Probe` validates against staged-or-committed state; `Apply` — gated to
//!   the RUNS module's origin, tighter than an any-module gate — is no-fail
//!   (decode-or-drop, re-check, breadcrumb on late conflict).
//!
//! ## prompt generation pin
//!
//! the install block publishes each prompt seed into memory BEFORE this
//! module's install arm runs, and memory serves same-block queries
//! staged-over-committed — so the install arm asks memory for each seed
//! path's ACTUAL latest generation and pins THAT, exactly like the
//! `docs-harness` reference (see its module docs on this same pin): a
//! pre-published path would otherwise make the pin miss at compose time,
//! which fails the RUN deterministically (never the block) with no repair
//! path — the ADR's prompt-pin rule, not a harness concern, but the copyable
//! template must model it correctly.

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

pub struct DummyHarness {
    id: ModuleId,
    /// the package registry module (the only origin `HarnessMsg` is accepted
    /// from, and the `MarkActive` ack target).
    package: ModuleId,
    /// the agent registry (agent seeds register here, harness-owned).
    agent: ModuleId,
    /// the jobs board (engagements mint here).
    jobs: ModuleId,
    /// the memory workspace (prompt seeds live here; `PromptRef.module`).
    memory: ModuleId,
    /// the runs module: the ONLY origin an action `Apply` is accepted from
    /// (the docs-harness reference's tighter-than-any-module gate).
    runs: ModuleId,
    committed: Store,
    pending: Option<Store>,
}

impl DummyHarness {
    pub fn new(
        id: impl Into<ModuleId>,
        package: impl Into<ModuleId>,
        agent: impl Into<ModuleId>,
        jobs: impl Into<ModuleId>,
        memory: impl Into<ModuleId>,
        runs: impl Into<ModuleId>,
    ) -> Self {
        Self {
            id: id.into(),
            package: package.into(),
            agent: agent.into(),
            jobs: jobs.into(),
            memory: memory.into(),
            runs: runs.into(),
            committed: Store::default(),
            pending: None,
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
impl Module for DummyHarness {
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

        if let Origin::Module(emitter) = &origin {
            // engagement intake from a recorded source — NO-FAIL: this rides
            // the WRITER's block (a commenter must never be aborted by us).
            let recorded = self
                .store()
                .installed
                .as_ref()
                .is_some_and(|i| i.sources.contains(emitter));
            if recorded {
                match decode_page_event(&msg.payload) {
                    Ok(event) => self.on_page_event(ctx, event).await,
                    Err(e) => self.breadcrumb(ctx, format!("dropped undecodable page event: {e}")),
                }
                return Ok(());
            }
        }

        // an accepted action's Apply, riding the delivery block — NO-FAIL,
        // and gated to the RUNS module's origin specifically (tighter than an
        // any-module gate — the copyable template must model this, not the
        // any-module hole the dummy shipped with originally).
        if origin == Origin::Module(self.runs.clone()) {
            match decode_action_msg(&msg.payload) {
                Ok(apply) => self.apply_action(ctx, &apply),
                Err(e) => self.breadcrumb(ctx, format!("dropped undecodable apply: {e}")),
            }
            return Ok(());
        }

        Err(Error::Module(
            "dummy-harness accepts HarnessMsg from the package module, PageEvents from its \
             recorded sources, and package-action Applies from the runs module"
                .into(),
        ))
    }

    /// external reads serve COMMITTED state only.
    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        let reply = match decode_query(req).map_err(Error::Module)? {
            DummyQuery::Notes => DummyReply::Notes(
                self.committed
                    .notes
                    .iter()
                    .map(|(note_id, text)| Note {
                        note_id: note_id.clone(),
                        text: text.clone(),
                    })
                    .collect(),
            ),
            DummyQuery::Status => {
                DummyReply::Status(self.committed.installed.as_ref().map(|i| DummyStatus {
                    package: i.package.clone(),
                    phase: i.phase.as_str().into(),
                    agents: i.agents.clone(),
                    minted: self.committed.minted.len() as u64,
                }))
            }
        };
        Ok(encode_reply(&reply))
    }

    /// the action owner's `Probe` rides the same query lane (the wire shapes
    /// are disjoint, so decode picks); the verdict reads STAGED-or-committed
    /// state — which is exactly why a probe can never see sibling actions
    /// from its own response: their applies have not staged yet.
    async fn query_with(&self, _ctx: &dyn Ctx, req: &[u8]) -> Result<Vec<u8>, Error> {
        if let Ok(PackageActionQuery::Probe { tag, payload, .. }) = decode_action_query(req) {
            let verdict = match self.validate_action(&tag, &payload) {
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
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.pending = None;
        Ok(())
    }
}

// ---- origin-gate tests -----------------------------------------------------------
// the execute seam itself: payloads arriving from origins the lanes above
// must never act on. the per-lane behavior lives with its lane
// (install.rs / events.rs / actions.rs / state.rs).

#[cfg(test)]
mod tests {
    use super::*;
    use package::encode_harness_msg;

    use super::testutil::*;

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
            Origin::System,
        ] {
            let mut ctx = TestCtx::at(origin.clone());
            assert!(
                exec(&mut m, &mut ctx, payload.clone()).is_err(),
                "{origin:?} must not drive the harness"
            );
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
            ACTION_NOTE_ADD,
            serde_json::json!({"note_id": "n1", "text": "hi"}),
        );
        // module origins other than runs are rejected outright — tighter than
        // the old any-module gate — except a recorded source (pages), whose
        // lane treats the bytes as an undecodable event (no-fail, nothing
        // acted on either way).
        for origin in [
            Origin::External(b"mallory".to_vec()),
            Origin::Module("mallory-module".into()),
            Origin::Module("agent".into()),
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
        assert_eq!(m.committed.notes.len(), 0, "no note landed");
    }
}
