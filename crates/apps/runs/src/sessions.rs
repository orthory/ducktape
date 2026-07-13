//! the agent session lane: an agent's MID-RUN writes, made unforgeable.
//!
//! the settle path validates what a run RETURNS. this lane validates what an
//! agent does WHILE it runs — the same grant, the same caps, the same code
//! ([`RunsModule::validate_response`] and [`RunsModule::pages_action_msg`]),
//! reached through a different door. there is exactly ONE definition of what an
//! agent may do; a second one would be the hole this lane exists to close.
//!
//! ## why a session key at all
//!
//! the frameless `/v1/submit` lane lets a caller NAME an origin, and `bin/node`
//! discards it — it signs every op with its own node key. so an agent's write
//! was byte-indistinguishable from the human's at the same keyboard: no audit
//! trail, the wrong account cross-node, and an ACL that only ever ran in a host
//! binary, where consensus could not see it.
//!
//! a frame's origin, by contrast, IS its verified public key (`node::decode_frame`
//! binds `(origin, seq, target, payload)`, and every honest validator rejects a
//! forged frame identically). so the executing node mints an ephemeral keypair
//! per run, binds the PUBLIC half here, and hands only the private half to the
//! agent's tool server. an op signed by it provably came from that run — and
//! consensus refuses it the moment it exceeds the owner's committed grant.
//!
//! the owner's authority is never asked for: `AgentRecord { owner,
//! allowed_actions, caps }` IS the capability grant, already committed.
//! registering an agent with `pages.comment` is the act of authorizing it. what
//! this lane adds is not authority but PROOF of who is exercising it.
//!
//! ## the two authorizations (X2)
//!
//! - **open** — the origin must be the run's committed LEASE-HOLDER: the node
//!   the dispatch plane actually handed the work to (`DispatchView::assignee`,
//!   which the read facade derives from saga's committed lease). self-authorizing,
//!   so an automated issue-mention run works with nobody at a keyboard, and
//!   correct cross-node, because the lease names the node really executing.
//! - **act** — the origin must BE the bound session key. no other origin, not
//!   even the owner's or the assignee's, may act through a session.
//!
//! ## LOUD, not degraded (the deliberate divergence from the settle path)
//!
//! `emit_pages_effects` degrades a bad pages action to a breadcrumb: it runs
//! inside the no-fail delivery block, and a page annotation is never worth
//! failing a run over. an action HERE is the opposite: an explicit, synchronous
//! op the agent submitted and is waiting on. a refusal it never sees is a lie,
//! so every refusal is an `Err` the submitter reads.
//!
//! ## the no-fail rule does NOT bind this lane (and why that matters)
//!
//! the settle path runs inside the dispatch plane's DELIVERY injection: a
//! follow-up its target rejects aborts that whole block, the committed mailbox
//! re-injects the delivery next block, and it aborts again — forever. that is
//! the no-fail rule, and it is why the settle path must prove every follow-up
//! valid before emitting any of them.
//!
//! an `AgentAction` is a ROOT op — the agent's own frame, isolated by the host
//! like any submitter's op. a follow-up the target rejects rolls THIS op back
//! and returns the error to the agent that sent it. nothing re-injects it,
//! nothing else in the block is touched, and the next op is unaffected: a
//! rejection here is a rejection, not a wedge.
//!
//! the probes still run — an agent deserves the refusal SYNCHRONOUSLY, and the
//! shared validator is the one definition of what it may do — but the failure
//! POLICIES are what differ, and conflating them is a trap: the settle path
//! emits the run's reply AND every action of one response into a SINGLE block,
//! all probed up front against committed state, so its probes must also count
//! what that same response already staged (chat's thread cap, the duplicate
//! task ids) or a sibling silently moves the cap out from under them. here each
//! op stages exactly ONE follow-up and drains it before the next op executes,
//! and a module query reads its own pending overlay first — so a sibling's post
//! is already visible to the next probe. that is why this lane needs no
//! same-block counter and the settle path does.

use super::pages_effects::is_pages_action;
use super::{
    AgentAction, AgentResponse, AgentSession, Ctx, DispatchQuery, DispatchReply, Error, Lane,
    MAX_ACTIONS_PER_SESSION, Origin, RunsModule, SESSION_KEY_LEN, dispatch_decode_reply,
    dispatch_encode_query, dispatch_id_for,
};

impl RunsModule {
    /// bind an ephemeral session key to a live run — the EXECUTING node's op.
    pub(super) async fn open_agent_session(
        &mut self,
        ctx: &mut dyn Ctx,
        run_id: String,
        session_key: Vec<u8>,
    ) -> Result<(), Error> {
        if session_key.len() != SESSION_KEY_LEN {
            return Err(Error::Module(format!(
                "a session key must be {SESSION_KEY_LEN} bytes, not {}",
                session_key.len()
            )));
        }
        // the ONLY origin shape that can hold a lease: a node key. a module or
        // system origin executes nothing.
        let Origin::External(submitter) = &ctx.env().origin else {
            return Err(Error::Module(
                "only the node executing a run may open its agent session".into(),
            ));
        };
        let submitter = submitter.clone();
        // the run must still be IN FLIGHT. a settled run has no lease, no
        // agent working, and nothing a session could legitimately write; an
        // unknown one never had any.
        let dispatch_id = dispatch_id_for(&run_id);
        let Some(entry) = self.pending_entry(&dispatch_id).cloned() else {
            return Err(Error::Module(format!("run is not in flight: {run_id}")));
        };
        // one session per run, first binding wins. re-opening is REFUSED rather
        // than overwriting: the live session's key is the authority the agent is
        // currently acting under, and a silent replace would let a second (or
        // squatting) opener revoke it mid-run and take over its remaining
        // budget. a lost key ends with the run — sessions are cheap, runs are
        // bounded.
        if self.session(&run_id).is_some() {
            return Err(Error::Module(format!(
                "run already has an open agent session: {run_id}"
            )));
        }
        // THE AUTHORIZATION: the run's own committed lease.
        let holder = self
            .lease_holder(&*ctx, &dispatch_id)
            .await
            .map_err(Error::Module)?;
        if submitter != holder {
            return Err(Error::Module(format!(
                "only the node holding the run's execution lease may open its agent session: {run_id}"
            )));
        }
        // the agent id comes from the run's COMMITTED entry, never from the
        // payload — identity is never a submitter's to assert.
        self.pending_sessions.insert(
            run_id.clone(),
            Some(AgentSession {
                run_id,
                agent_id: entry.agent_id,
                session_key,
                opened_at: ctx.env().consensus_time,
                actions: 0,
            }),
        );
        Ok(())
    }

    /// apply ONE agent action, signed by the run's bound session key.
    pub(super) async fn agent_action(
        &mut self,
        ctx: &mut dyn Ctx,
        run_id: String,
        action: AgentAction,
    ) -> Result<(), Error> {
        let Origin::External(submitter) = &ctx.env().origin else {
            return Err(Error::Module(
                "an agent action must be signed by the run's session key".into(),
            ));
        };
        let Some(session) = self.session(&run_id).cloned() else {
            return Err(Error::Module(format!(
                "run has no open agent session: {run_id}"
            )));
        };
        // THE ACL. the origin is the frame's VERIFIED public key, so this
        // comparison is authorship consensus can trust — the whole point of the
        // lane. the owner's key does not pass it either: an owner acts as
        // themselves, never as their agent.
        if *submitter != session.session_key {
            return Err(Error::Module(format!(
                "only the bound session key may act for run {run_id}"
            )));
        }
        // the session is pruned with its run, so a live session implies a live
        // run — but the two are separate maps, and a check that costs nothing is
        // cheaper than an invariant that only holds by argument.
        let dispatch_id = dispatch_id_for(&run_id);
        let Some(entry) = self.pending_entry(&dispatch_id).cloned() else {
            return Err(Error::Module(format!("run is not in flight: {run_id}")));
        };
        if session.actions >= MAX_ACTIONS_PER_SESSION {
            return Err(Error::Module(format!(
                "session for run {run_id} has spent its budget of {MAX_ACTIONS_PER_SESSION} actions"
            )));
        }
        let lane = Lane::Session(session.actions);

        // THE SAME VALIDATOR the settle path runs, on a one-action response:
        // grant + caps + every probe that keeps an emitted follow-up from being
        // rejected by its target. a pages action passes through it untouched (the
        // pages gate is its own, below); anything else is fully checked here.
        let validated = self
            .validate_response(
                &*ctx,
                &run_id,
                &entry,
                lane,
                AgentResponse {
                    reply_blocks: Vec::new(),
                    actions: vec![action.clone()],
                    commit_message: None,
                },
            )
            .await
            .map_err(Error::Module)?;

        if is_pages_action(&action) {
            let pages = self
                .pages
                .clone()
                .ok_or_else(|| Error::Module("no pages module is configured".into()))?;
            let agent = self
                .agent_record(&*ctx, &entry.agent_id)
                .await
                .map_err(Error::Module)?
                .ok_or_else(|| {
                    Error::Module(format!("agent is not registered: {}", entry.agent_id))
                })?;
            // THE SAME pages gate the settle path applies — grant, cap, target
            // resolution, id safety, freshness probes — but its `Err` is
            // returned to the submitter instead of degrading to a breadcrumb.
            // `already_staged` is 0: this op emits exactly one follow-up, and a
            // sibling op's comment is already visible to these probes (each root
            // op's follow-ups drain before the next op executes).
            let msg = self
                .pages_action_msg(&*ctx, &pages, &agent, &run_id, &lane.slot(0), &action, 0)
                .await
                .map_err(Error::Module)?;
            ctx.emit_msg(msg);
        } else {
            // MODULE origin, exactly like the settle path's — which is what lets
            // chat refine `as_agent` into `AuthorRef::Agent { module, agent_id }`:
            // the attribution the frameless lane could not produce at all.
            self.emit_response(ctx, &run_id, &entry, lane, validated)
                .await;
        }

        // spend the budget. the counter is committed state: it is both the audit
        // record and the id salt the NEXT action mints from, so it must move on
        // every applied action and on no refused one (a refusal is an `Err`, and
        // the host rolls this op's staging back with it).
        self.pending_sessions.insert(
            run_id,
            Some(AgentSession {
                actions: session.actions + 1,
                ..session
            }),
        );
        Ok(())
    }

    /// the node key holding the run's execution lease — the dispatch plane's
    /// `assignee`, which its read facade resolves from saga's COMMITTED lease at
    /// query time. `None` (a delivered run runs nowhere) and a missing dispatch
    /// are both refusals: neither names a node that could be executing this run.
    async fn lease_holder(&self, ctx: &dyn Ctx, dispatch_id: &str) -> Result<Vec<u8>, String> {
        let reply = ctx
            .query(
                &self.dispatch,
                &dispatch_encode_query(&DispatchQuery::Dispatch {
                    receiver: self.id.clone(),
                    dispatch_id: dispatch_id.to_string(),
                }),
            )
            .await
            .map_err(|e| format!("dispatch lookup failed: {e}"))?;
        match dispatch_decode_reply(&reply) {
            Ok(DispatchReply::Dispatch(Some(view))) => view
                .assignee
                .ok_or_else(|| "the run holds no execution lease".to_string()),
            Ok(DispatchReply::Dispatch(None)) => Err("the run has no dispatch record".into()),
            _ => Err("unexpected dispatch reply for a dispatch lookup".into()),
        }
    }
}
