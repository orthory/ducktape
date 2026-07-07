//! `PageEvent` intake (no-fail, rides the WRITER's block): the mention
//! policy and idempotent engagement-job minting (see the module docs).

use jobs::{JobsMsg, JobsQuery, JobsReply, encode_msg as jobs_encode_msg};
use pages::PageEvent;
use sdk::{Ctx, Msg};

use super::DummyHarness;
use super::state::Phase;

impl DummyHarness {
    // ---- engagement intake (origin == a recorded source; NO-FAIL) ---------------

    pub(crate) async fn on_page_event(&mut self, ctx: &mut dyn Ctx, event: PageEvent) {
        let Some(installed) = self.store().installed.clone() else {
            return; // unreachable: the hook exists only while installed.
        };
        if installed.phase != Phase::Active {
            return; // suspended/unplugged packages mint nothing.
        }
        let PageEvent::CommentAdded {
            comment_id, text, ..
        } = event
        else {
            return; // only comments engage the note taker.
        };
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
            let job_id = format!("dummy:{agent_id}:{comment_id}");
            match self.job_exists(ctx, &job_id).await {
                Ok(false) => {
                    self.store_mut().minted.insert(key);
                    ctx.emit_msg(Msg {
                        target: self.jobs.clone(),
                        payload: jobs_encode_msg(&JobsMsg::Submit {
                            job_id,
                            kind: format!("agent/{agent_id}"),
                            spec: text.clone(),
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

    use crate::dummy::testutil::*;

    #[test]
    fn a_mention_comment_mints_one_job_idempotently() {
        let mut m = module();
        installed(&mut m);

        let event = comment_event("c1", "hey @dummy.note-taker note this");
        let mut ctx = TestCtx::at(Origin::Module("pages".into()));
        exec(&mut m, &mut ctx, event.clone()).unwrap();
        assert_eq!(ctx.emitted.len(), 1);
        assert_eq!(ctx.emitted[0].target, "jobs");
        match jobs::decode_msg(&ctx.emitted[0].payload).unwrap() {
            JobsMsg::Submit { job_id, kind, .. } => {
                assert_eq!(job_id, "dummy:dummy.note-taker:c1");
                assert_eq!(kind, "agent/dummy.note-taker");
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
    }

    #[test]
    fn a_malformed_page_event_from_a_source_is_a_no_op_observation() {
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
