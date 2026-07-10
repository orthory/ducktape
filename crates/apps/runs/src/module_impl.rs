use super::{
    Ctx, Error, Module, ModuleId, Msg, Origin, RunsModule, RunsQuery, RunsReply, StateRoot,
    StateSyncHandle, WatchView, committed_root, decode_query, encode_reply,
};

#[async_trait::async_trait(?Send)]
impl Module for RunsModule {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    /// state-based commitment: sha256 over the canonical committed encoding —
    /// a length-prefixed fold of every watch and pending-entry field in
    /// sorted-key order. sensitive to every field, so any transition moves
    /// the root. the preimage IS the snapshot encoding.
    fn root(&self) -> StateRoot {
        committed_root(&self.watches, &self.pending)
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::SnapshotBytes(self.snapshot()))
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        // payload namespaces routed by the HOST-ASSIGNED origin — spoof-proof
        // by construction: only the tagging plane's follow-ups reach the
        // engagement intake, only dispatch's reach the result intake, only
        // the registry's reach the recipe hook, and everything else (external
        // submitters included) must decode as a RunsMsg. saga and chat
        // origins are dead-lettered: neither may ever abort the block that
        // carried them.
        let origin = ctx.env().origin.clone();
        match origin {
            Origin::Module(module) if module == self.tagging => {
                self.on_engagement(ctx, &msg.payload).await
            }
            Origin::Module(module) if module == self.dispatch => {
                self.on_result_event(ctx, &msg.payload).await
            }
            Origin::Module(module) if self.jobs.as_ref() == Some(&module) => {
                self.on_jobs_event(ctx, &msg.payload).await
            }
            Origin::Module(module) if module == self.agent => {
                self.on_agent_event(ctx, &msg.payload)
            }
            Origin::Module(module) if module == self.saga => {
                // dead letter: nothing here rides the saga directly, but any
                // trigger's reply_to can point a callback at this module —
                // it must never abort the saga's terminal block.
                self.note(ctx, "dropped a direct saga callback".into());
                Ok(())
            }
            Origin::Module(module) if module == self.chat => {
                // dead letter: chat never notifies this module directly. a
                // stray follow-up must never abort a posting block.
                self.note(ctx, "dropped a direct chat follow-up".into());
                Ok(())
            }
            _ => self.on_admin(ctx, msg).await,
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
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.pending_watches.clear();
        self.pending_overlay.clear();
        Ok(())
    }
}
