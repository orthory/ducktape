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
//!   the dispatch plane actually handed the work to (the `assignee` on the saga
//!   the dispatch names, read directly). self-authorizing,
//!   so an automated issue-mention run works with nobody at a keyboard, and
//!   correct cross-node, because the lease names the node really executing.
//! - **act** — the origin must BE the bound session key, AND the lease must
//!   still sit where it did when the session opened. no other origin, not even
//!   the owner's or the assignee's, may act through a session; and a lease that
//!   moves (reassignment, expiry) strands the old session on the spot instead
//!   of handing an evicted node the agent's grant for the rest of the run.
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
use super::response::is_duckfs_action;
use super::{
    AgentAction, AgentResponse, AgentSession, AgentStatus, BTreeMap, Ctx, DELEGATED_CHILD_CORES,
    DELEGATED_CHILD_MEM_GB, DelegationRequest, DelegationState, DelegationStatus, DelegationView,
    DispatchQuery, DispatchReply, Error, Lane, MAX_ACTIONS_PER_SESSION,
    MAX_DELEGATION_INSTRUCTION_BYTES, MAX_DELEGATION_REQUEST_ID_BYTES, MAX_DELEGATIONS_BYTES,
    MAX_DELEGATIONS_PER_RUN, Origin, RunAuthority, RunsModule, SESSION_KEY_LEN,
    SiblingReadBudget, delegated_run_id_for, delegation_id_for, dispatch_decode_reply,
    dispatch_encode_query, dispatch_id_for, page_thread_id,
};
use dispatch::DispatchStatus;
use saga::{
    SagaQuery, SagaReply, decode_reply as saga_decode_reply, encode_query as saga_encode_query,
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
        // one session per LEASE, first binding wins. re-opening under the same
        // lease is REFUSED rather than overwriting: the live session's key is
        // the authority the agent is currently acting under, and a silent
        // replace would let a squatting opener revoke it mid-run and take over
        // its remaining budget. but a lease that MOVED leaves a session whose
        // holder no longer executes anything (its acting ops refuse from that
        // moment on, see `session_holds_lease`) — and the node genuinely
        // running the work now must be able to open its own, or the run has no
        // write lane at all for the rest of its life.
        let bound_to_this_lease = self
            .session(&run_id)
            .is_some_and(|open| open.holder == holder);
        if bound_to_this_lease {
            return Err(Error::Module(format!(
                "run already has an open agent session: {run_id}"
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
                holder,
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
        self.session_holds_lease(&*ctx, &run_id, &session).await?;
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
                .agent_for_run(&*ctx, &entry)
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
        } else if is_duckfs_action(&action) {
            let agent = self
                .agent_for_run(&*ctx, &entry)
                .await
                .map_err(Error::Module)?
                .ok_or_else(|| {
                    Error::Module(format!("agent is not registered: {}", entry.agent_id))
                })?;
            // THE SAME duckfs gate the settle path applies — grant,
            // shape/cap/permission, the per-path base probe — but its `Err` is
            // returned to the submitter instead of degrading to a breadcrumb.
            let msg = self
                .duckfs_write_msg(&*ctx, &agent, &action)
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

    /// Start one caller/callee edge while the caller is live. This deliberately
    /// does not mutate either AgentRecord: hierarchy is unnecessary when the
    /// actual relation lasts only for these two runs.
    pub(super) async fn delegate_run(
        &mut self,
        ctx: &mut dyn Ctx,
        run_id: String,
        request_id: String,
        request: DelegationRequest,
        budget: &SiblingReadBudget,
    ) -> Result<(), Error> {
        let Origin::External(submitter) = &ctx.env().origin else {
            return Err(Error::Module(
                "an agent call must be signed by the caller's session key".into(),
            ));
        };
        let session = self
            .session(&run_id)
            .cloned()
            .ok_or_else(|| Error::Module(format!("run has no open agent session: {run_id}")))?;
        if *submitter != session.session_key {
            return Err(Error::Module(format!(
                "only the bound session key may delegate for run {run_id}"
            )));
        }
        self.session_holds_lease(&*ctx, &run_id, &session).await?;
        let entry = self
            .pending_entry(&dispatch_id_for(&run_id))
            .cloned()
            .ok_or_else(|| Error::Module(format!("run is not in flight: {run_id}")))?;
        if entry.job_id.is_some() || page_thread_id(&entry.channel_id).is_some() {
            return Err(Error::Module(
                "agent calls currently require a chat or Forge run".into(),
            ));
        }
        if request_id.is_empty()
            || request_id.len() > MAX_DELEGATION_REQUEST_ID_BYTES
            || super::contains_run_separator(&request_id)
        {
            return Err(Error::Module(format!(
                "request_id must be 1..={MAX_DELEGATION_REQUEST_ID_BYTES} bytes and contain no reserved separator"
            )));
        }
        let delegation_id = delegation_id_for(&run_id, &request_id);
        if let Some(existing) = self.delegation(&delegation_id) {
            return if existing.view.caller_run_id == run_id && existing.request == request {
                Ok(())
            } else {
                Err(Error::Module(
                    "request_id was already used for a different agent call".into(),
                ))
            };
        }
        if session.actions >= MAX_ACTIONS_PER_SESSION {
            return Err(Error::Module(format!(
                "session for run {run_id} has spent its budget of {MAX_ACTIONS_PER_SESSION} actions"
            )));
        }

        if request.agent_id == entry.agent_id {
            return Err(Error::Module("an agent cannot call itself".into()));
        }
        super::reject_run_separator("callee agent_id", &request.agent_id)?;
        let instruction = request.instruction.trim();
        if instruction.is_empty() || request.instruction.len() > MAX_DELEGATION_INSTRUCTION_BYTES {
            return Err(Error::Module(format!(
                "instruction must be non-empty and at most {MAX_DELEGATION_INSTRUCTION_BYTES} bytes"
            )));
        }
        if serde_json::to_vec(&request)
            .expect("delegation requests serialize")
            .len()
            > MAX_DELEGATIONS_BYTES
        {
            return Err(Error::Module(format!(
                "agent call exceeds the {MAX_DELEGATIONS_BYTES}-byte request cap"
            )));
        }

        let caller = self
            .agent_for_run(&*ctx, &entry)
            .await
            .map_err(Error::Module)?
            .ok_or_else(|| {
                Error::Module(format!(
                    "caller agent is not registered: {}",
                    entry.agent_id
                ))
            })?;
        if caller.status != AgentStatus::Active {
            return Err(Error::Module(format!(
                "caller agent is paused: {}",
                caller.agent_id
            )));
        }
        if caller.caps.subagent_budget == 0 {
            return Err(Error::Module(format!(
                "caller agent {} has no subagent budget",
                caller.agent_id
            )));
        }
        let root_run_id = match entry.delegation_id.as_deref() {
            Some(id) => self
                .delegation(id)
                .map(|state| state.view.root_run_id.clone())
                .ok_or_else(|| Error::Module("caller run has no delegation edge".into()))?,
            None => run_id.clone(),
        };
        let root_entry = self
            .pending_entry(&dispatch_id_for(&root_run_id))
            .cloned()
            .ok_or_else(|| Error::Module("delegation root is no longer in flight".into()))?;
        let root = self
            .agent_for_run(&*ctx, &root_entry)
            .await
            .map_err(Error::Module)?
            .ok_or_else(|| Error::Module("delegation root agent is not registered".into()))?;
        let spent = self
            .delegation_ids()
            .into_iter()
            .filter_map(|id| self.delegation(&id))
            .filter(|state| {
                state.view.root_run_id == root_run_id
                    && state.view.status == DelegationStatus::Pending
            })
            .count();
        let limit = usize::try_from(root.caps.subagent_budget)
            .unwrap_or(usize::MAX)
            .min(MAX_DELEGATIONS_PER_RUN);
        if spent >= limit {
            return Err(Error::Module(format!(
                "delegation tree has reached its concurrency limit of {limit} calls"
            )));
        }

        let callee = self
            .active_agent(&*ctx, &request.agent_id)
            .await
            .map_err(Error::Module)?
            .ok_or_else(|| {
                Error::Module(format!("callee agent is unavailable: {}", request.agent_id))
            })?;
        let scoped_callee = caller.scoped_for_call(&callee);
        let extra = crate::envelope::library_skills(&request.skills).map_err(Error::Module)?;
        if let Some(skill) = extra
            .iter()
            .find(|skill| !caller.permits(&agent::CapRequest::DuckfsRead(&skill.source_prefix)))
        {
            return Err(Error::Module(format!(
                "the call authority cannot read delegated skill {}",
                skill.name
            )));
        }
        let workspace_agent = self
            .agent_record(&*ctx, &entry.workspace_agent_id)
            .await
            .map_err(Error::Module)?
            .ok_or_else(|| Error::Module("call workspace agent is not registered".into()))?;
        let callee_run_id = delegated_run_id_for(&delegation_id, &callee.agent_id);
        if self
            .turn_taken(&*ctx, &dispatch_id_for(&callee_run_id))
            .await
            .map_err(Error::Module)?
        {
            return Err(Error::Module(format!(
                "delegated run is already taken: {callee_run_id}"
            )));
        }
        let context = format!(
            "## Agent call\nCaller run: {run_id}\nCaller agent: {}\nRoot run: {root_run_id}\n\nInstruction:\n{instruction}",
            caller.agent_id
        );
        let prepared = self
            .prepare_dispatch_with_context(
                &*ctx,
                &scoped_callee,
                &callee_run_id,
                &entry.channel_id,
                entry.anchor_seq,
                Some((&workspace_agent, &context)),
                &extra,
                budget,
            )
            .await
            .map_err(Error::Module)?;
        let now = ctx.env().consensus_time;
        self.pending_delegations.insert(
            delegation_id.clone(),
            Some(DelegationState {
                view: DelegationView {
                    delegation_id: delegation_id.clone(),
                    request_id,
                    caller_run_id: run_id.clone(),
                    root_run_id,
                    callee_run_id: callee_run_id.clone(),
                    callee_agent_id: callee.agent_id.clone(),
                    status: DelegationStatus::Pending,
                    result: None,
                    created_at: now,
                    completed_at: None,
                },
                request,
            }),
        );
        self.stage_scoped_dispatch_run(
            ctx,
            &callee_run_id,
            callee.agent_id,
            entry.workspace_agent_id,
            entry.channel_id,
            entry.anchor_seq,
            entry.requester,
            prepared,
            BTreeMap::from([
                ("cores".into(), DELEGATED_CHILD_CORES),
                ("mem_gb".into(), DELEGATED_CHILD_MEM_GB),
            ]),
            Some(RunAuthority::from_record(&scoped_callee)),
            Some(delegation_id),
        );
        self.pending_sessions.insert(
            run_id,
            Some(AgentSession {
                actions: session.actions + 1,
                ..session
            }),
        );
        Ok(())
    }

    /// the lease the session was opened under must still BE the run's lease.
    /// `open_agent_session` reads it once, at bind time; a lease that moves —
    /// an explicit `ReassignRun`, or an expiry saga re-leasing on its own —
    /// otherwise leaves the ex-holder's key bound, spending the agent's whole
    /// grant for the rest of the run on a node that stopped executing it. the
    /// session's authority IS the lease, so every acting op re-reads it.
    async fn session_holds_lease(
        &self,
        ctx: &dyn Ctx,
        run_id: &str,
        session: &AgentSession,
    ) -> Result<(), Error> {
        let holder = self
            .lease_holder(ctx, &dispatch_id_for(run_id))
            .await
            .map_err(Error::Module)?;
        if holder != session.holder {
            return Err(Error::Module(format!(
                "the run's execution lease has moved; its agent session is no longer authoritative: {run_id}"
            )));
        }
        Ok(())
    }

    /// the node key holding the run's execution lease. dispatch names the saga
    /// carrying the work (only while still `AwaitingResult` — a delivered run
    /// runs nowhere), and saga owns the live lease: its `assignee` IS the holder.
    /// a missing dispatch, a terminal dispatch, and a saga with no committed
    /// lease are all refusals — none names a node that could be executing this
    /// run right now.
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
        let view = match dispatch_decode_reply(&reply) {
            Ok(DispatchReply::Dispatch(Some(view))) => view,
            Ok(DispatchReply::Dispatch(None)) => {
                return Err("the run has no dispatch record".into());
            }
            _ => return Err("unexpected dispatch reply for a dispatch lookup".into()),
        };
        // a lease exists only while the dispatch still awaits its saga; the saga
        // id it names is the one whose committed lease we read.
        let DispatchStatus::AwaitingResult { saga_id } = view.status else {
            return Err("the run holds no execution lease".into());
        };
        let reply = ctx
            .query(&self.saga, &saga_encode_query(&SagaQuery::Get { saga_id }))
            .await
            .map_err(|e| format!("saga lookup failed: {e}"))?;
        match saga_decode_reply(&reply) {
            Ok(SagaReply::Saga(Some(saga))) => saga
                .assignee
                .ok_or_else(|| "the run holds no execution lease".to_string()),
            Ok(SagaReply::Saga(None)) => Err("the run holds no execution lease".into()),
            _ => Err("unexpected saga reply for a saga lookup".into()),
        }
    }
}
