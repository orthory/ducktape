//! the `HarnessMsg` arms — install/suspend/resume/unplug, accepted ONLY from
//! the package module's origin — including the prompt generation pin and the
//! agent + hook registrations (see the crate docs).

use std::collections::BTreeMap;

use agent::{AgentMsg, PromptRef, RENDERER_MEMORY_GENERATION, encode_msg as agent_encode_msg};
use memory::{MemoryQuery, MemoryReply, encode_query as memory_encode_query};
use package::{InstallSpec, PackageMsg, encode_msg as package_encode_msg};
use pages::{PageMsg, encode_msg as pages_encode_msg};
use sdk::{Ctx, Error, Msg};

use crate::DocsHarness;
use crate::state::{Installed, Phase};

impl DocsHarness {
    // ---- the harness contract (origin == package module) ----------------------

    pub(crate) async fn install(
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
        // its prompt pinned to the generation the seed ACTUALLY landed on —
        // never an assumed generation 1, which a path squatter could displace
        // (see the module docs on the prompt generation pin).
        for seed in &spec.agents {
            let prompt = spec
                .prompts
                .iter()
                .find(|p| p.logical == seed.prompt)
                .ok_or_else(|| {
                    Error::Module(format!("agent prompt is not seeded: {}", seed.agent_id))
                })?;
            let generation = self.seeded_generation(&*ctx, &prompt.path).await?;
            ctx.emit_msg(Msg {
                target: self.agent.clone(),
                payload: agent_encode_msg(&AgentMsg::RegisterAgent {
                    agent_id: seed.agent_id.clone(),
                    display_name: seed.display_name.clone(),
                    capability: seed.capability.clone(),
                    prompt: Some(PromptRef {
                        module: self.memory.clone(),
                        target: format!("{}@{generation}", prompt.path),
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

    /// the generation the just-staged prompt seed landed on. the package
    /// module publishes every seed BEFORE this install arm runs in the same
    /// block, and memory serves same-block queries staged-over-committed, so
    /// the path's live latest IS the seed — whatever a squatter parked at
    /// older generations. a missing stat means the seed never preceded us:
    /// a wiring bug, and the install arm MAY fail (it rides the installer's
    /// own block).
    async fn seeded_generation(&self, ctx: &dyn Ctx, path: &str) -> Result<u64, Error> {
        let reply = ctx
            .query(
                &self.memory,
                &memory_encode_query(&MemoryQuery::Stat { path: path.into() }),
            )
            .await
            .map_err(|e| Error::Module(format!("memory stat of the prompt seed failed: {e}")))?;
        match memory::decode_reply(&reply) {
            Ok(MemoryReply::Stat(Some(stat))) => Ok(stat.latest_generation),
            Ok(MemoryReply::Stat(None)) => Err(Error::Module(format!(
                "prompt seed is not staged in memory: {path}"
            ))),
            Ok(other) => Err(Error::Module(format!("unexpected memory reply: {other:?}"))),
            Err(e) => Err(Error::Module(e)),
        }
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
}

// ---- install / lifecycle tests --------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use package::{HarnessMsg, ModuleBinding, encode_harness_msg};
    use sdk::Origin;
    use sha2::{Digest, Sha256};

    use crate::testutil::*;
    use crate::{ACTION_BLOCK_UPDATE_TEXT, ACTION_COMMENT_ADD, ACTION_THREAD_RESOLVE};

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
    fn install_pins_the_staged_seed_generation_not_an_assumed_first() {
        // a squatter pre-published junk at the predictable prompt path, so
        // the staged seed landed at generation 3 — the PromptRef must pin 3,
        // or every future run fails pin-mismatch forever.
        let mut m = module();
        let mut ctx = TestCtx::at(package_origin());
        ctx.prompt_generation = 3;
        exec(
            &mut m,
            &mut ctx,
            encode_harness_msg(&HarnessMsg::InstallPackage {
                package: PKG.into(),
                spec: spec(),
            }),
        )
        .unwrap();
        match agent::decode_msg(&ctx.emitted[1].payload).unwrap() {
            AgentMsg::RegisterAgent { prompt, .. } => {
                let prompt = prompt.expect("prompt pinned");
                assert_eq!(prompt.target, format!("{PROMPT_PATH}@3"));
                assert_eq!(prompt.sha256, Sha256::digest(b"# Docs Editor\n").to_vec());
            }
            other => panic!("expected RegisterAgent, got {other:?}"),
        }
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
}
