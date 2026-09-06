use std::collections::BTreeMap;

use super::{
    AgentQuery, AgentRecord, AgentReply, AgentStatus, CONTEXT_WINDOW, ChatQuery, ChatReply, Ctx,
    DispatchMsg, DispatchQuery, DispatchReply, FilesQuery, FilesReply, MAX_PAYLOAD_BYTES,
    MessageView, ModuleId, Msg, PendingState, PreparedDispatch, RunsModule, SagaOrigin,
    SiblingReadBudget, SkillRef, agent_decode_reply, agent_encode_query, chat_decode_reply,
    chat_encode_query, dispatch_decode_reply, dispatch_encode_msg, dispatch_encode_query,
    dispatch_id_for, envelope, files_decode_reply, files_encode_query, inject, recipe_id_for,
};
use crate::facets::WireSink;

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

    /// The live registry record narrowed by this run's admission ceiling.
    /// Ordinary runs have no ceiling. Delegated runs re-intersect on every
    /// read so a later owner revocation narrows authority immediately while a
    /// later widening cannot escape what the caller originally granted.
    pub(super) async fn agent_for_run(
        &self,
        ctx: &dyn Ctx,
        entry: &PendingState,
    ) -> Result<Option<AgentRecord>, String> {
        Ok(self
            .agent_record(ctx, &entry.agent_id)
            .await?
            .map(|record| {
                entry
                    .authority
                    .as_ref()
                    .map_or(record.clone(), |authority| authority.apply(&record))
            }))
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

    // ---- explicit-request admission ---------------------------------------

    /// chat's answer to "may `user` post to `channel_id`" — the standing an
    /// explicit `RequestRun` submitter must hold before this module pins that
    /// channel's transcript and posts a reply into it under module authority.
    /// post standing covers read (chat's [`ChatReply::may_post`] implies
    /// `may_read`), so this one query is the whole gate. an unexpected or
    /// failed reply answers `false` — the submission fails closed rather than
    /// leaking a channel it could not confirm.
    pub(super) async fn may_post(
        &self,
        ctx: &dyn Ctx,
        user: &[u8],
        channel_id: &str,
    ) -> Result<bool, String> {
        let reply = ctx
            .query(
                &self.chat,
                &chat_encode_query(&ChatQuery::Access {
                    channel_id: channel_id.to_string(),
                    user: user.to_vec(),
                }),
            )
            .await
            .map_err(|e| format!("chat access query failed: {e}"))?;
        match chat_decode_reply(&reply) {
            Ok(ChatReply::Access(access)) => Ok(access.may_post),
            _ => Err("unexpected chat reply for an access query".into()),
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

    /// resolve the portable inputs every envelope composes with.
    ///
    /// the files module is genesis-uniform infrastructure: every production
    /// composer wires it, and every validator then resolves the SAME committed
    /// head (W2), so the composed bytes are consensus-uniform; a head query
    /// that FAILS becomes the run's error (a loud skip at the callsite), never
    /// a silent unpinned source. unwired (dev tools/tests) the head is an
    /// explicit null pin — the envelope still composes v1.
    pub(super) async fn portable_inputs(
        &self,
        ctx: &dyn Ctx,
        agent: &AgentRecord,
        extra: &[SkillRef],
    ) -> Result<envelope::PortableInputs, String> {
        self.portable_inputs_with_workspace(ctx, agent, agent, extra)
            .await
    }

    /// `extra` is the requester's per-run skill curation (`RequestRun.skills` or
    /// a delegation's), appended to the composed agent's own. every intake
    /// without a requester to choose them passes none.
    async fn portable_inputs_with_workspace(
        &self,
        ctx: &dyn Ctx,
        agent: &AgentRecord,
        workspace_agent: &AgentRecord,
        extra: &[SkillRef],
    ) -> Result<envelope::PortableInputs, String> {
        let source_snapshot = match self.files.clone() {
            Some(files) => self.duckfs_head(ctx, &files).await?,
            None => None,
        };
        // a pinned skill passes its snapshot through; a tracking skill (no
        // pin) resolves to the SAME committed head this run pins its workspace
        // to (W2) — deterministic across validators.
        let skills = envelope::resolve_skills(agent, extra, &source_snapshot);
        Ok(envelope::PortableInputs {
            workspace: envelope::duckfs_workspace(workspace_agent, source_snapshot),
            skills,
            sink: WireSink::Chain,
            context: None,
        })
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
    /// `run_id` is the caller's — the SAME id it keyed the dispatch by. it
    /// rides into the envelope so the executing host can name the run back to
    /// this module (the session bind, and every mid-run action).
    ///
    /// a `forge:<repo>:<n>` trigger channel selects the forge compose lane
    /// (M1): the item-session workspace source, the requested PR sink, and
    /// the injected item context — see [`super::forge_source`]. any other
    /// channel composes the duckfs lane exactly as before.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn prepare_dispatch(
        &self,
        ctx: &dyn Ctx,
        agent: &AgentRecord,
        run_id: &str,
        channel_id: &str,
        anchor_seq: u64,
        extra: &[SkillRef],
        budget: &SiblingReadBudget,
    ) -> Result<PreparedDispatch, String> {
        self.prepare_dispatch_with_context(
            ctx, agent, run_id, channel_id, anchor_seq, None, extra, budget,
        )
        .await
    }

    /// The ordinary chat/Forge composition path plus delegation's parent
    /// DuckFS workspace and bounded context. Forge already names the shared
    /// item branch, so the workspace override applies only to generic Chat.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn prepare_dispatch_with_context(
        &self,
        ctx: &dyn Ctx,
        agent: &AgentRecord,
        run_id: &str,
        channel_id: &str,
        anchor_seq: u64,
        delegation: Option<(&AgentRecord, &str)>,
        extra: &[SkillRef],
        budget: &SiblingReadBudget,
    ) -> Result<PreparedDispatch, String> {
        let (thread_root, transcript) = self.pin_context(ctx, channel_id, anchor_seq).await?;
        let mut portable = match super::forge_source::parse_forge_channel(channel_id) {
            Some(item_ref) => {
                self.forge_portable_inputs(ctx, agent, &item_ref, extra)
                    .await?
            }
            None => {
                self.portable_inputs_with_workspace(
                    ctx,
                    agent,
                    delegation.map(|(parent, _)| parent).unwrap_or(agent),
                    extra,
                )
                .await?
            }
        };
        // M2: `[[page:<id>]]` refs in the trigger message text or the injected
        // item body render referenced page subtrees into the same context
        // section — resolved from COMMITTED pages state at compose height,
        // appended after the M1 item context (which stays untouched).
        let anchor_text = transcript
            .last()
            .map(inject::message_text)
            .unwrap_or_default();
        let mut sources = vec![anchor_text.as_str()];
        if let Some(item) = portable.context.as_deref() {
            sources.push(item);
        }
        // resolve BOTH duck:// sections from the original sources (anchor text +
        // item body) before touching `portable.context` — `sources` borrows it,
        // so both queries run first, then the sections append in order (page,
        // then attachment). attachments render the referenced file's committed
        // TEXT; images/binaries are named, not inlined (the agent plane is
        // text-only).
        let mut remaining_reference_queries = super::MAX_SIBLING_QUERY_READS;
        let page_section = self
            .page_context(ctx, &sources, &mut remaining_reference_queries, budget)
            .await;
        let attachment_section = self
            .attachment_context(ctx, &sources, &mut remaining_reference_queries, budget)
            .await;
        for section in [page_section, attachment_section].into_iter().flatten() {
            portable.context = Some(match portable.context.take() {
                Some(item) => format!("{item}\n\n{section}"),
                None => section,
            });
        }
        if let Some((_, extra)) = delegation {
            portable.context = Some(match portable.context.take() {
                Some(context) => format!("{context}\n\n{extra}"),
                None => extra.to_string(),
            });
        }
        let payload =
            envelope::render_payload(&self.id, agent, run_id, &transcript, portable).into_bytes();
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

    /// Prepare a run triggered by one Pages comment. `ordinal` is 1-based in
    /// the thread and comes from Pages' same-block tag event. `run_id` is the
    /// caller's, exactly as in [`Self::prepare_dispatch`].
    pub(super) async fn prepare_page_dispatch(
        &self,
        ctx: &dyn Ctx,
        agent: &AgentRecord,
        run_id: &str,
        thread_id: &str,
        ordinal: u64,
        budget: &SiblingReadBudget,
    ) -> Result<PreparedDispatch, String> {
        let pages = self
            .pages
            .as_deref()
            .ok_or_else(|| "pages module is not configured".to_string())?;
        let reply = ctx
            .query(
                pages,
                &pages::encode_query(&pages::PageQuery::CommentThread {
                    thread_id: thread_id.to_string(),
                }),
            )
            .await
            .map_err(|e| format!("pages thread lookup failed: {e}"))?;
        let view = match pages::decode_reply(&reply) {
            Ok(pages::PageReply::CommentThread(Some(view))) => view,
            Ok(_) => return Err(format!("pages thread is missing: {thread_id}")),
            Err(e) => return Err(format!("pages thread reply failed to decode: {e}")),
        };
        let index = usize::try_from(ordinal.saturating_sub(1))
            .map_err(|_| "pages comment ordinal exceeds host usize".to_string())?;
        let comment = view
            .comments
            .get(index)
            .filter(|comment| !comment.deleted)
            .cloned()
            .ok_or_else(|| format!("pages comment is missing: {thread_id}/{ordinal}"))?;
        let author = match &comment.author {
            pages::AuthorRef::User(key) => format!("user:{}", crate::hex(key)),
            pages::AuthorRef::Agent { module, agent_id } => format!("{module}/{agent_id}"),
            pages::AuthorRef::Module(module) => format!("module:{module}"),
            pages::AuthorRef::System => "system".into(),
        };
        let block_reply = ctx
            .query(
                pages,
                &pages::encode_query(&pages::PageQuery::GetBlock {
                    block_id: view.thread.target.clone(),
                }),
            )
            .await
            .map_err(|e| format!("pages target lookup failed: {e}"))?;
        let page_id = match pages::decode_reply(&block_reply) {
            Ok(pages::PageReply::Block(Some(block))) => block.page,
            _ => return Err(format!("pages target is missing: {}", view.thread.target)),
        };
        let mut portable = self.portable_inputs(ctx, agent, &[]).await?;
        let blocks = self
            .page_blocks_for_execute(ctx, pages, &page_id, budget)
            .await;
        portable.context = Some(inject::render_pages_section(
            &[(page_id, blocks)],
            &self.net_query(),
        ));
        let payload = envelope::render_page_comment_payload(
            agent,
            run_id,
            thread_id,
            ordinal,
            &author,
            &comment.text,
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
            thread_root: None,
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
        demands: BTreeMap<String, u64>,
    ) {
        let workspace_agent_id = agent_id.clone();
        self.stage_scoped_dispatch_run(
            ctx,
            run_id,
            agent_id,
            workspace_agent_id,
            channel_id,
            anchor_seq,
            requester,
            prepared,
            demands,
            None,
            None,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn stage_scoped_dispatch_run(
        &mut self,
        ctx: &mut dyn Ctx,
        run_id: &str,
        agent_id: String,
        workspace_agent_id: String,
        channel_id: String,
        anchor_seq: u64,
        requester: SagaOrigin,
        prepared: PreparedDispatch,
        demands: BTreeMap<String, u64>,
        authority: Option<super::RunAuthority>,
        delegation_id: Option<String>,
    ) {
        let now = ctx.env().consensus_time;
        let dispatch_id = dispatch_id_for(run_id);
        ctx.emit_msg(Msg {
            target: self.dispatch.clone(),
            payload: dispatch_encode_msg(&DispatchMsg::Dispatch {
                dispatch_id: dispatch_id.clone(),
                recipe_id: recipe_id_for(&agent_id),
                payload: prepared.payload,
                demands,
                admission: dispatch::AdmissionPolicy::Queue,
            }),
        });
        self.pending_overlay.insert(
            dispatch_id,
            Some(PendingState {
                run_id: run_id.to_string(),
                workspace_agent_id,
                agent_id,
                authority,
                delegation_id,
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
