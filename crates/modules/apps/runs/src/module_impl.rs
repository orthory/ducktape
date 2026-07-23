use super::{
    Ctx, Error, Event, Module, ModuleId, Msg, Origin, RunsModule, RunsQuery, RunsReply,
    SiblingReadBudget, StateRoot, StateSyncHandle, WatchView, committed_root, decode_query,
    encode_reply,
};

#[derive(Clone, Copy)]
enum ExecuteKind {
    Engagement,
    Result,
    Jobs,
    Agent,
    Saga,
    Chat,
    Admin,
}

struct BudgetCtx<'ctx, 'budget> {
    inner: &'ctx mut dyn Ctx,
    budget: &'budget SiblingReadBudget,
}

#[async_trait::async_trait(?Send)]
impl Ctx for BudgetCtx<'_, '_> {
    fn env(&self) -> &sdk::Env {
        self.inner.env()
    }

    fn module_root(&self, target: &str) -> Option<StateRoot> {
        self.budget
            .reserve_root(target)
            .then(|| self.inner.module_root(target))
            .flatten()
    }

    async fn query(&self, target: &str, req: &[u8]) -> Result<Vec<u8>, Error> {
        if !self.budget.reserve_query(target, req) {
            return Err(Error::Module(format!(
                "runs sibling-read budget exceeded ({})",
                super::MAX_SIBLING_QUERY_READS
            )));
        }
        self.inner.query(target, req).await
    }

    fn emit_msg(&mut self, msg: Msg) {
        self.inner.emit_msg(msg);
    }

    fn emit_event(&mut self, event: Event) {
        self.inner.emit_event(event);
    }

    fn relay(&self) -> Option<&sdk::Relay> {
        self.inner.relay()
    }

    fn set_output(&mut self, bytes: Vec<u8>) {
        self.inner.set_output(bytes);
    }
}

impl RunsModule {
    fn execute_kind(&self, origin: &Origin) -> ExecuteKind {
        let Origin::Module(module) = origin else {
            return ExecuteKind::Admin;
        };
        [
            (Some(self.tagging.as_str()), ExecuteKind::Engagement),
            (Some(self.dispatch.as_str()), ExecuteKind::Result),
            (self.jobs.as_deref(), ExecuteKind::Jobs),
            (Some(self.agent.as_str()), ExecuteKind::Agent),
            (Some(self.saga.as_str()), ExecuteKind::Saga),
            (Some(self.chat.as_str()), ExecuteKind::Chat),
        ]
        .into_iter()
        .find_map(|(id, kind)| (id == Some(module.as_str())).then_some(kind))
        .unwrap_or(ExecuteKind::Admin)
    }

    async fn execute_engagement(&mut self, ctx: &mut dyn Ctx, payload: &[u8]) -> Result<(), Error> {
        let budget = SiblingReadBudget::default();
        self.on_engagement(
            &mut BudgetCtx {
                inner: ctx,
                budget: &budget,
            },
            payload,
            &budget,
        )
        .await
    }

    async fn execute_result(&mut self, ctx: &mut dyn Ctx, payload: &[u8]) -> Result<(), Error> {
        let budget = SiblingReadBudget::default();
        self.on_result_event(
            &mut BudgetCtx {
                inner: ctx,
                budget: &budget,
            },
            payload,
        )
        .await
    }

    async fn execute_jobs(&mut self, ctx: &mut dyn Ctx, payload: &[u8]) -> Result<(), Error> {
        let budget = SiblingReadBudget::default();
        self.on_jobs_event(
            &mut BudgetCtx {
                inner: ctx,
                budget: &budget,
            },
            payload,
        )
        .await
    }

    fn execute_agent(&mut self, ctx: &mut dyn Ctx, payload: &[u8]) -> Result<(), Error> {
        let budget = SiblingReadBudget::default();
        self.on_agent_event(
            &mut BudgetCtx {
                inner: ctx,
                budget: &budget,
            },
            payload,
        )
    }

    fn drop_saga_callback(&mut self, ctx: &mut dyn Ctx) -> Result<(), Error> {
        self.note(ctx, "dropped a direct saga callback".into());
        Ok(())
    }

    fn drop_chat_follow_up(&mut self, ctx: &mut dyn Ctx) -> Result<(), Error> {
        self.note(ctx, "dropped a direct chat follow-up".into());
        Ok(())
    }

    async fn execute_admin(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        let budget = SiblingReadBudget::default();
        self.on_admin(
            &mut BudgetCtx {
                inner: ctx,
                budget: &budget,
            },
            msg,
            &budget,
        )
        .await
    }
}

#[async_trait::async_trait(?Send)]
impl Module for RunsModule {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    // The current canonical Runs state schema.
    fn state_schema_revision(&self) -> u32 {
        1
    }

    /// state-based commitment: sha256 over the canonical committed encoding —
    /// a length-prefixed fold of every watch, pending-entry, and agent-session
    /// field in sorted-key order. sensitive to every field, so any transition
    /// moves the root — opening a session, spending one of its actions, and
    /// pruning it each move the app-hash, because the session registry IS the
    /// mid-run ACL and every validator must hold the same one. the preimage IS
    /// the snapshot encoding.
    fn root(&self) -> StateRoot {
        committed_root(
            &self.watches,
            &self.pending,
            &self.sessions,
            &self.delegations,
        )
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::SnapshotBytes(self.snapshot()))
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        // The one visible origin dispatch. Each arm delegates once to a
        // budgeted handler whose stack-owned ledger spans that whole execute.
        match self.execute_kind(&ctx.env().origin) {
            ExecuteKind::Engagement => self.execute_engagement(ctx, &msg.payload).await,
            ExecuteKind::Result => self.execute_result(ctx, &msg.payload).await,
            ExecuteKind::Jobs => self.execute_jobs(ctx, &msg.payload).await,
            ExecuteKind::Agent => self.execute_agent(ctx, &msg.payload),
            ExecuteKind::Saga => self.drop_saga_callback(ctx),
            ExecuteKind::Chat => self.drop_chat_follow_up(ctx),
            ExecuteKind::Admin => self.execute_admin(ctx, msg).await,
        }
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            RunsQuery::PendingRuns => {
                let runs = Self::visible_ids(&self.pending, &self.pending_overlay)
                    .into_iter()
                    .filter_map(|dispatch_id| {
                        self.pending_entry(&dispatch_id)
                            .map(|p| Self::pending_view(&dispatch_id, p))
                    })
                    .collect();
                Ok(encode_reply(&RunsReply::PendingRuns(runs)))
            }
            RunsQuery::Watches => {
                let watches = Self::visible_ids(&self.watches, &self.pending_watches)
                    .into_iter()
                    .filter_map(|channel_id| {
                        self.watch(&channel_id).map(|policy| WatchView {
                            channel_id: channel_id.clone(),
                            policy: policy.clone(),
                        })
                    })
                    .collect();
                Ok(encode_reply(&RunsReply::Watches(watches)))
            }
            RunsQuery::RecentRuns => Ok(encode_reply(&RunsReply::RecentRuns(
                // newest first: the ring appends at the back.
                self.history.iter().rev().cloned().collect(),
            ))),
            // the audit surface: who holds a key right now, and how much of the
            // budget they have spent. ascending by run id.
            RunsQuery::AgentSessions => {
                let sessions = Self::visible_ids(&self.sessions, &self.pending_sessions)
                    .into_iter()
                    .filter_map(|run_id| self.session(&run_id).cloned())
                    .collect();
                Ok(encode_reply(&RunsReply::AgentSessions(sessions)))
            }
            RunsQuery::Delegations { caller_run_id } => {
                let delegations = self
                    .delegation_ids()
                    .into_iter()
                    .filter_map(|id| self.delegation(&id))
                    .filter(|state| state.view.caller_run_id == caller_run_id)
                    .map(|state| state.view.clone())
                    .collect();
                Ok(encode_reply(&RunsReply::Delegations(delegations)))
            }
        }
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        for (id, staged) in std::mem::take(&mut self.pending_watches) {
            match staged {
                Some(policy) => {
                    self.watches.insert(id, policy);
                }
                None => {
                    self.watches.remove(&id);
                }
            }
        }
        for (dispatch_id, staged) in std::mem::take(&mut self.pending_overlay) {
            match staged {
                Some(entry) => {
                    self.pending.insert(dispatch_id, entry);
                }
                None => {
                    self.pending.remove(&dispatch_id);
                }
            }
        }
        for (run_id, staged) in std::mem::take(&mut self.pending_sessions) {
            match staged {
                Some(session) => {
                    self.sessions.insert(run_id, session);
                }
                None => {
                    self.sessions.remove(&run_id);
                }
            }
        }
        for (id, staged) in std::mem::take(&mut self.pending_delegations) {
            match staged {
                Some(delegation) => {
                    self.delegations.insert(id, delegation);
                }
                None => {
                    self.delegations.remove(&id);
                }
            }
        }
        for record in std::mem::take(&mut self.pending_history) {
            self.history.push_back(record);
            if self.history.len() > super::RUN_HISTORY_CAP {
                self.history.pop_front();
            }
        }
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.pending_watches.clear();
        self.pending_overlay.clear();
        self.pending_sessions.clear();
        self.pending_delegations.clear();
        self.pending_history.clear();
        Ok(())
    }
}
