use super::{
    AgentQuery, AgentRecord, AgentReply, AgentStatus, CONTEXT_WINDOW, ChatQuery, ChatReply, Ctx,
    DispatchMsg, DispatchQuery, DispatchReply, FilesQuery, FilesReply, MAX_PAYLOAD_BYTES,
    MessageView, ModuleId, Msg, PendingState, PreparedDispatch, RunsModule, SagaOrigin, WireSink,
    agent_decode_reply, agent_encode_query, chat_decode_reply, chat_encode_query,
    dispatch_decode_reply, dispatch_encode_msg, dispatch_encode_query, dispatch_id_for, envelope,
    files_decode_reply, files_encode_query, inject, recipe_id_for,
};

impl RunsModule {
    // ---- registry reads --------------------------------------------------------
    // the agent registry is another module's state; these queries see staged
    // same-block registrations through the host's live query routing, so a
    // register-watch-engage cascade works within one block — deterministically,
    // on every validator.

    /// one registry record, or `None` when the agent isn't registered.
    pub(super) async fn agent_record(
        &self,
        ctx: &dyn Ctx,
        agent_id: &str,
    ) -> Result<Option<AgentRecord>, String> {
        let reply = ctx
            .query(
                &self.agent,
                &agent_encode_query(&AgentQuery::Agent {
                    agent_id: agent_id.to_string(),
                }),
            )
            .await
            .map_err(|e| format!("agent registry query failed: {e}"))?;
        match agent_decode_reply(&reply) {
            Ok(AgentReply::Agent(record)) => Ok(record),
            _ => Err("unexpected agent reply for an agent lookup".into()),
        }
    }

    /// the record, but only while the agent may engage new runs.
    pub(super) async fn active_agent(
        &self,
        ctx: &dyn Ctx,
        agent_id: &str,
    ) -> Result<Option<AgentRecord>, String> {
        Ok(self
            .agent_record(ctx, agent_id)
            .await?
            .filter(|a| a.status == AgentStatus::Active))
    }

    /// every ACTIVE registered agent, sorted — the deterministic engagement
    /// domain for `All` and `RoundRobin`.
    pub(super) async fn active_agent_ids(&self, ctx: &dyn Ctx) -> Result<Vec<String>, String> {
        let reply = ctx
            .query(&self.agent, &agent_encode_query(&AgentQuery::Agents))
            .await
            .map_err(|e| format!("agent registry query failed: {e}"))?;
        match agent_decode_reply(&reply) {
            Ok(AgentReply::Agents(records)) => Ok(records
                .into_iter()
                .filter(|a| a.status == AgentStatus::Active)
                .map(|a| a.agent_id)
                .collect()),
            _ => Err("unexpected agent reply for an agents listing".into()),
        }
    }

    // ---- the turn claim --------------------------------------------------------

    /// whether this dispatch id's turn is already taken: a staged/committed
    /// pending entry settles same-block races; the dispatch module's
    /// PERMANENT dispatch record (committed-only query) settles everything
    /// after — including turns whose pending entry already pruned. without
    /// the second layer a repeat request would re-stage an entry no
    /// `ResultEvent` will ever prune (the dispatch module no-ops duplicate
    /// dispatch ids without telling the dispatcher).
    pub(super) async fn turn_taken(
        &self,
        ctx: &dyn Ctx,
        dispatch_id: &str,
    ) -> Result<bool, String> {
        if self.pending_entry(dispatch_id).is_some() {
            return Ok(true);
        }
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
            Ok(DispatchReply::Dispatch(view)) => Ok(view.is_some()),
            _ => Err("unexpected dispatch reply for a dispatch lookup".into()),
        }
    }

    // ---- context pinning (P4) --------------------------------------------------

    /// pin the transcript window ending at `anchor_seq`: query chat for the
    /// (at most [`CONTEXT_WINDOW`]) newest messages up to and including the
    /// anchor. staged same-block writes are visible through the host's live
    /// query routing, so an engagement fired by a post sees the post itself —
    /// deterministically, on every validator. also returns the anchor's
    /// thread root so the reply can join the same thread.
    pub(super) async fn pin_context(
        &self,
        ctx: &dyn Ctx,
        channel_id: &str,
        anchor_seq: u64,
    ) -> Result<(Option<u64>, Vec<MessageView>), String> {
        if anchor_seq == 0 {
            return Err("anchor_seq must be >= 1".into());
        }
        let from = anchor_seq.saturating_sub(CONTEXT_WINDOW - 1).max(1);
        let reply = ctx
            .query(
                &self.chat,
                &chat_encode_query(&ChatQuery::MessagesRange {
                    channel_id: channel_id.to_string(),
                    from_seq: from,
                    limit: anchor_seq - from + 1,
                }),
            )
            .await
            .map_err(|e| format!("chat query failed: {e}"))?;
        let window = match chat_decode_reply(&reply) {
            Ok(ChatReply::Messages(window)) => window,
            _ => return Err("unexpected chat reply for a transcript query".into()),
        };
        // the sequence space is gap-free (P3), so the anchor exists exactly
        // when it is the window's last message.
        let Some(anchor) = window.last().filter(|m| m.seq == anchor_seq) else {
            return Err(format!("anchor does not exist: {channel_id}/{anchor_seq}"));
        };
        let thread_root = anchor.head.thread;
        Ok((thread_root, window))
    }

    // ---- payload preparation (the dispatch plane's composition rule) -----

    /// resolve the portable (v3) inputs, or `None` when the files module is
    /// unwired.
    ///
    /// the files module's PRESENCE is the composer's v2-vs-v3 selector — it is
    /// genesis-uniform infrastructure, not a version tag. unwired, no files query
    /// is issued and the composer takes its byte-identical v2 path. wired, every
    /// validator resolves the SAME committed head, so the composed v3 bytes are
    /// consensus-uniform; a head query that FAILS becomes the run's error (a
    /// loud skip at the callsite), never a silent unpinned source.
    pub(super) async fn portable_inputs(
        &self,
        ctx: &dyn Ctx,
        agent: &AgentRecord,
    ) -> Result<Option<envelope::PortableInputs>, String> {
        let Some(files) = self.files.clone() else {
            return Ok(None);
        };
        let source_snapshot = self.duckfs_head(ctx, &files).await?;
        // a pinned skill passes its snapshot through; a tracking skill (no
        // pin) resolves to the SAME committed head this run pins its workspace
        // to (W2) — deterministic across validators.
        let skills = envelope::resolve_skills(agent, &source_snapshot);
        Ok(Some(envelope::PortableInputs {
            workspace: envelope::duckfs_workspace(agent, source_snapshot),
            skills,
            sink: WireSink::Chain,
            context: None,
        }))
    }

    /// the committed duckfs head — a dispatch-start committed read, so the
    /// pinned id is consensus-uniform across validators (W2). errors become the
    /// run's (a clean skip/error at the callsite), never a silent unpinned
    /// compose. `RefsInfo.head` is `None` on a fresh network (a legitimate null
    /// pin), which is distinct from the files module being unwired.
    pub(super) async fn duckfs_head(
        &self,
        ctx: &dyn Ctx,
        files: &ModuleId,
    ) -> Result<Option<String>, String> {
        let reply = ctx
            .query(files, &files_encode_query(&FilesQuery::Refs {}))
            .await
            .map_err(|e| format!("files refs query failed: {e}"))?;
        match files_decode_reply(&reply) {
            Ok(FilesReply::Refs(info)) => Ok(info.head),
            Ok(_) => Err("unexpected files reply for a refs query".into()),
            Err(e) => Err(format!("files refs reply failed to decode: {e}")),
        }
    }

    /// everything a chat run's dispatch needs, prepared read-only: the pinned
    /// context (P4) and the fully composed payload. any failure here is a
    /// clean skip for the no-fail engagement intake and a clean error for an
    /// explicit `RequestRun`.
    ///
    /// a `forge:<repo>:<n>` trigger channel selects the forge compose lane
    /// (M1): the item-session workspace source, the requested PR sink, and
    /// the injected item context — see [`super::forge_source`]. any other
    /// channel composes the duckfs lane exactly as before.
    pub(super) async fn prepare_dispatch(
        &self,
        ctx: &dyn Ctx,
        agent: &AgentRecord,
        channel_id: &str,
        anchor_seq: u64,
    ) -> Result<PreparedDispatch, String> {
        let (thread_root, transcript) = self.pin_context(ctx, channel_id, anchor_seq).await?;
        let mut portable = match super::forge_source::parse_forge_channel(channel_id) {
            Some(item_ref) => Some(self.forge_portable_inputs(ctx, agent, &item_ref).await?),
            None => self.portable_inputs(ctx, agent).await?,
        };
        // M2: `[[page:<id>]]` refs in the trigger message text or the injected
        // item body render referenced page subtrees into the same context
        // section — resolved from COMMITTED pages state at compose height,
        // appended after the M1 item context (which stays untouched).
        if let Some(inputs) = portable.as_mut() {
            let anchor_text = transcript
                .last()
                .map(inject::message_text)
                .unwrap_or_default();
            let mut sources = vec![anchor_text.as_str()];
            if let Some(item) = inputs.context.as_deref() {
                sources.push(item);
            }
            if let Some(section) = self.page_context(ctx, &sources).await {
                inputs.context = Some(match inputs.context.take() {
                    Some(item) => format!("{item}\n\n{section}"),
                    None => section,
                });
            }
        }
        let payload = envelope::render_payload(
            &self.id,
            agent,
            channel_id,
            anchor_seq,
            thread_root,
            &transcript,
            portable,
        )
        .into_bytes();
        if payload.len() > MAX_PAYLOAD_BYTES {
            return Err(format!(
                "composed payload is {} bytes; the dispatch cap is {MAX_PAYLOAD_BYTES}",
                payload.len()
            ));
        }
        Ok(PreparedDispatch {
            thread_root,
            payload,
        })
    }

    /// stage a chat run's dispatch — one atomic unit with whatever op caused
    /// it (P2). the recipe is the agent's own (`agent/{agent_id}`, registered
    /// by the registry hook); the result lands as a next-block `ResultEvent`
    /// keyed by the dispatch id, which prunes the entry staged here.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn stage_dispatch_run(
        &mut self,
        ctx: &mut dyn Ctx,
        run_id: &str,
        agent_id: String,
        channel_id: String,
        anchor_seq: u64,
        requester: SagaOrigin,
        prepared: PreparedDispatch,
    ) {
        let now = ctx.env().consensus_time;
        let dispatch_id = dispatch_id_for(run_id);
        ctx.emit_msg(Msg {
            target: self.dispatch.clone(),
            payload: dispatch_encode_msg(&DispatchMsg::Dispatch {
                dispatch_id: dispatch_id.clone(),
                recipe_id: recipe_id_for(&agent_id),
                payload: prepared.payload,
            }),
        });
        self.pending_overlay.insert(
            dispatch_id,
            Some(PendingState {
                agent_id,
                channel_id,
                anchor_seq,
                thread_root: prepared.thread_root,
                job_id: None,
                job_claim_height: 0,
                requester,
                created_at: now,
            }),
        );
    }
}
