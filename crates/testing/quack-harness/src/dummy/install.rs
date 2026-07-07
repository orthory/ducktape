//! the `HarnessMsg` arms — install/suspend/resume/unplug, accepted ONLY from
//! the package module's origin — including the prompt pin and the agent +
//! hook registrations (see the module docs).

use std::collections::{BTreeMap, BTreeSet};

use agent::{AgentMsg, PromptRef, RENDERER_MEMORY_GENERATION, encode_msg as agent_encode_msg};
use package::{InstallSpec, PackageMsg, encode_msg as package_encode_msg};
use pages::{PageMsg, encode_msg as pages_encode_msg};
use sdk::{Ctx, Error, ModuleId, Msg};

use super::DummyHarness;
use super::state::{Installed, Phase};

/// the memory generation a fresh package prompt path commits at (see the
/// module docs on the pin assumption).
const FIRST_GENERATION: u64 = 1;

impl DummyHarness {
    // ---- the harness contract (origin == package module) ----------------------

    pub(crate) fn install(
        &mut self,
        ctx: &mut dyn Ctx,
        package: String,
        spec: InstallSpec,
    ) -> Result<(), Error> {
        if self.store().installed.is_some() {
            return Err(Error::Module(
                "dummy-harness already hosts a package".into(),
            ));
        }
        let bindings: BTreeMap<&str, &str> = spec
            .modules
            .iter()
            .map(|b| (b.logical.as_str(), b.module_id.as_str()))
            .collect();

        // hook every engagement source (the registry validated the bindings).
        let mut sources: BTreeSet<ModuleId> = BTreeSet::new();
        for rule in &spec.engagements {
            let source = bindings.get(rule.source.as_str()).ok_or_else(|| {
                Error::Module(format!("engagement source is not bound: {}", rule.source))
            })?;
            sources.insert((*source).to_string());
        }
        for source in &sources {
            ctx.emit_msg(Msg {
                target: source.clone(),
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
            sources: sources.into_iter().collect(),
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
    /// phase, flip, and hand back the agents for the caller's follow-ups.
    fn transition(&mut self, package: &str, from: &[Phase], to: Phase) -> Result<Installed, Error> {
        let installed = self
            .store()
            .installed
            .clone()
            .ok_or_else(|| Error::Module("dummy-harness hosts no package".into()))?;
        if installed.package != package {
            return Err(Error::Module(format!(
                "dummy-harness hosts {:?}, not {package:?}",
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

    pub(crate) fn suspend(&mut self, ctx: &mut dyn Ctx, package: String) -> Result<(), Error> {
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

    pub(crate) fn resume(&mut self, ctx: &mut dyn Ctx, package: String) -> Result<(), Error> {
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

    pub(crate) fn unplug(&mut self, ctx: &mut dyn Ctx, package: String) -> Result<(), Error> {
        let record = self.transition(
            &package,
            &[Phase::Active, Phase::Suspended],
            Phase::Unplugged,
        )?;
        // tombstone the agents (terminal, audit-preserving) and drop the
        // hooks; the notes — user data — stay untouched (preserve-by-default).
        for agent_id in &record.agents {
            ctx.emit_msg(Msg {
                target: self.agent.clone(),
                payload: agent_encode_msg(&AgentMsg::TombstoneAgent {
                    agent_id: agent_id.clone(),
                }),
            });
        }
        for source in &record.sources {
            ctx.emit_msg(Msg {
                target: source.clone(),
                payload: pages_encode_msg(&PageMsg::UnregisterHook {}),
            });
        }
        Ok(())
    }
}

// ---- install / lifecycle tests --------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use package::{HarnessMsg, encode_harness_msg};
    use sdk::Origin;

    use crate::dummy::ACTION_NOTE_ADD;
    use crate::dummy::testutil::*;

    #[test]
    fn install_registers_hook_and_agents_then_acks_last() {
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
                agent_id, prompt, ..
            } => {
                assert_eq!(agent_id, "dummy.note-taker");
                let prompt = prompt.expect("prompt pinned");
                assert_eq!(prompt.module, "memory");
                assert_eq!(
                    prompt.target,
                    "/packages/org.example.dummy/prompts/dummy.md@1"
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
    fn suspend_pauses_and_stops_minting_and_unplug_tombstones_preserving_notes() {
        let mut m = module();
        installed(&mut m);

        // a note lands (user data).
        let mut ctx = TestCtx::at(Origin::Module("runs".into()));
        exec(
            &mut m,
            &mut ctx,
            apply(
                ACTION_NOTE_ADD,
                serde_json::json!({"note_id": "n1", "text": "kept"}),
            ),
        )
        .unwrap();
        commit(&mut m);

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
            comment_event("c9", "@dummy.note-taker anyone?"),
        )
        .unwrap();
        assert!(
            quiet.emitted.is_empty(),
            "a suspended package mints nothing"
        );

        // unplug (from suspended): tombstones + unregisters, notes preserved.
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
        assert_eq!(
            pages::decode_msg(&ctx.emitted[1].payload).unwrap(),
            PageMsg::UnregisterHook {}
        );
        commit(&mut m);
        assert_eq!(
            m.committed.notes.get("n1").map(String::as_str),
            Some("kept"),
            "user data preserved"
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
}
