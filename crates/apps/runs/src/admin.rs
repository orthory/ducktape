use super::{
    AgentStatus, Ctx, DispatchMsg, Error, JobsMsg, LIBRARIAN_REGENESIS_REQUIRED, Msg, Origin,
    RunsModule, RunsMsg, TaggingMsg, TurnPolicy, canonical_origin, decode_msg,
    dispatch_encode_msg, dispatch_id_for, jobs_encode_msg, reject_run_separator, run_id_for,
    tagging_encode_msg,
};

impl RunsModule {
    // ---- admin ops + explicit runs (any other origin) --------------------------------

    async fn controlled_dispatch_id(
        &self,
        ctx: &dyn Ctx,
        run_id: &str,
        action: &str,
    ) -> Result<Option<String>, Error> {
        let submitter = canonical_origin(&ctx.env().origin);
        let dispatch_id = dispatch_id_for(run_id);
        let Some(entry) = self.pending_entry(&dispatch_id).cloned() else {
            return match self.turn_taken(ctx, &dispatch_id).await {
                Ok(true) => Ok(None),
                Ok(false) => Err(Error::Module(format!("unknown run: {run_id}"))),
                Err(reason) => Err(Error::Module(reason)),
            };
        };
        let owner = self
            .agent_record(ctx, &entry.agent_id)
            .await
            .map_err(Error::Module)?
            .map(|a| a.owner);
        if submitter != entry.requester && Some(&submitter) != owner.as_ref() {
            return Err(Error::Module(format!(
                "only the run creator or the agent owner may {action} a run"
            )));
        }
        Ok(Some(dispatch_id))
    }

    pub(super) async fn on_admin(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            RunsMsg::WatchChannel { channel_id, policy } => {
                Self::admin_origin(&ctx.env().origin)?;
                Self::validate_non_empty("channel_id", &channel_id)?;
                reject_run_separator("channel_id", &channel_id)?;
                if let TurnPolicy::Assigned(assignee) = &policy
                    && self
                        .agent_record(&*ctx, assignee)
                        .await
                        .map_err(Error::Module)?
                        .is_none()
                {
                    return Err(Error::Module(format!(
                        "assigned agent is not registered: {assignee}"
                    )));
                }
                // the watch and the plane subscription are ONE atomic unit
                // (P2): if the tagging plane rejects the subscription (bad
                // container, subscriber cap), the whole block aborts and the
                // staged watch vanishes with it.
                self.pending_watches
                    .insert(channel_id.clone(), Some(policy));
                ctx.emit_msg(Msg {
                    target: self.tagging.clone(),
                    payload: tagging_encode_msg(&TaggingMsg::Subscribe {
                        source: self.chat.clone(),
                        container: channel_id,
                    }),
                });
                Ok(())
            }
            RunsMsg::UnwatchChannel { channel_id } => {
                Self::admin_origin(&ctx.env().origin)?;
                if self.watch(&channel_id).is_none() {
                    // idempotent: unwatching an unwatched channel stages (and
                    // emits) nothing.
                    return Ok(());
                }
                self.pending_watches.insert(channel_id.clone(), None);
                ctx.emit_msg(Msg {
                    target: self.tagging.clone(),
                    payload: tagging_encode_msg(&TaggingMsg::Unsubscribe {
                        source: self.chat.clone(),
                        container: channel_id,
                    }),
                });
                Ok(())
            }
            RunsMsg::EnableJobWorker { enabled } => {
                Self::admin_origin(&ctx.env().origin)?;
                let jobs = self
                    .jobs
                    .clone()
                    .ok_or_else(|| Error::Module("no jobs module is configured".into()))?;
                let payload = if enabled {
                    jobs_encode_msg(&JobsMsg::RegisterWorker {})
                } else {
                    jobs_encode_msg(&JobsMsg::UnregisterWorker {})
                };
                ctx.emit_msg(Msg {
                    target: jobs,
                    payload,
                });
                Ok(())
            }
            RunsMsg::RequestRun {
                agent_id,
                channel_id,
                anchor_seq,
                demands,
            } => {
                // an explicit turn claim: same run id, same dedup as the
                // engagement path — first in consensus order wins, the loser
                // no-ops.
                let requester = match &ctx.env().origin {
                    Origin::External(key) if key.is_empty() => {
                        return Err(Error::Module(
                            "run requests require a non-empty submitter id".into(),
                        ));
                    }
                    other => canonical_origin(other),
                };
                let Some(agent) = self
                    .agent_record(&*ctx, &agent_id)
                    .await
                    .map_err(Error::Module)?
                else {
                    return Err(Error::Module(format!("unknown agent: {agent_id}")));
                };
                reject_run_separator("channel_id", &channel_id)?;
                let run_id = run_id_for(&channel_id, anchor_seq, &agent_id);
                if self
                    .turn_taken(&*ctx, &dispatch_id_for(&run_id))
                    .await
                    .map_err(Error::Module)?
                {
                    return Ok(());
                }
                if agent.status != AgentStatus::Active {
                    return Err(Error::Module(format!("agent is paused: {agent_id}")));
                }
                // unlike the engagement intake, an explicit request REJECTS
                // on a failed preparation: this is the root op of its own
                // block, so an error poisons nothing but the request itself.
                let prepared = self
                    .prepare_dispatch(&*ctx, &agent, &run_id, &channel_id, anchor_seq)
                    .await
                    .map_err(Error::Module)?;
                self.stage_dispatch_run(
                    ctx, &run_id, agent_id, channel_id, anchor_seq, requester, prepared, demands,
                );
                Ok(())
            }
            RunsMsg::CancelRun { run_id } => {
                let Some(dispatch_id) = self
                    .controlled_dispatch_id(&*ctx, &run_id, "cancel")
                    .await?
                else {
                    return Ok(());
                };
                // cancel through the dispatch plane; the entry stays pending
                // and the plane's Err("cancelled") delivery prunes it (and
                // finalizes a job-backed run's job) through the ONE result
                // path — no second lifecycle machine here.
                ctx.emit_msg(Msg {
                    target: self.dispatch.clone(),
                    payload: dispatch_encode_msg(&DispatchMsg::CancelDispatch { dispatch_id }),
                });
                Ok(())
            }
            RunsMsg::ReassignRun { run_id, attempt } => {
                let Some(dispatch_id) = self
                    .controlled_dispatch_id(&*ctx, &run_id, "reassign")
                    .await?
                else {
                    return Ok(());
                };
                ctx.emit_msg(Msg {
                    target: self.dispatch.clone(),
                    payload: dispatch_encode_msg(&DispatchMsg::ReassignDispatch {
                        dispatch_id,
                        attempt,
                    }),
                });
                Ok(())
            }
            RunsMsg::BeginLibrarianCall { .. } | RunsMsg::CancelLibrarianCall { .. } => {
                // Global re-genesis fence: do not validate identity, consult the
                // registry/budget/dispatch planes, or touch module state.
                Err(Error::Module(LIBRARIAN_REGENESIS_REQUIRED.into()))
            }
            // the session lane (see [`super::sessions`]). deliberately NOT under
            // `admin_origin`: these two ops carry their own, STRICTER
            // authorization — the run's committed lease-holder opens, and only
            // the bound session key acts — and neither is a capability any
            // "non-empty submitter" has.
            RunsMsg::OpenAgentSession {
                run_id,
                session_key,
            } => self.open_agent_session(ctx, run_id, session_key).await,
            RunsMsg::AgentAction { run_id, action } => self.agent_action(ctx, run_id, action).await,
        }
    }
}
