//! `PageEvent` intake (NO-FAIL, rides the WRITER's block): the
//! `mention_or_assigned` policy, idempotent engagement-job minting, and loop
//! prevention against our own writes (see the crate docs).

use jobs::{JobsMsg, JobsQuery, JobsReply, encode_msg as jobs_encode_msg};
use pages::{AuthorRef, PageEvent};
use sdk::{Ctx, Msg};

use crate::state::{Installed, Phase};
use crate::{
    DocsHarness, EngagementSpec, encode_engagement_spec, engagement_excerpt, engagement_job_id,
};

impl DocsHarness {
    // ---- engagement intake (origin == the pages module; NO-FAIL) ---------------

    pub(crate) async fn on_page_event(&mut self, ctx: &mut dyn Ctx, event: PageEvent) {
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
}

// ---- engagement intake tests -----------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use sdk::{Module, Origin};

    use crate::testutil::*;
    use crate::{MAX_COMMENT_TEXT_BYTES, decode_engagement_spec};

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
}
