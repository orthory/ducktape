use super::{
    Ctx, EngagementEvent, EntityRef, Error, RunsModule, TurnPolicy, canonical_origin,
    dispatch_id_for, page_channel_id, page_run_id_for, run_id_for, tagging_decode_event,
};

impl RunsModule {
    // ---- the engagement intake (origin == tagging) ----------------------------------

    /// which agents an engagement engages under `policy`. only ACTIVE
    /// registered agents ever engage; every branch reads agreed state only
    /// (registry queries included — they are consensus reads).
    async fn engaged_agents(
        &self,
        ctx: &dyn Ctx,
        policy: &TurnPolicy,
        tags: &[EntityRef],
        seq: u64,
    ) -> Result<Vec<String>, String> {
        match policy {
            // entity tags naming THIS module's agents, in content order (the
            // content module dedupes them).
            TurnPolicy::Mention => {
                let mut engaged = Vec::new();
                for tag in tags {
                    if tag.module != self.id {
                        continue;
                    }
                    if self.active_agent(ctx, &tag.entity).await?.is_some() {
                        engaged.push(tag.entity.clone());
                    }
                }
                Ok(engaged)
            }
            TurnPolicy::All => self.active_agent_ids(ctx).await,
            TurnPolicy::Assigned(agent_id) => {
                Ok(if self.active_agent(ctx, agent_id).await?.is_some() {
                    vec![agent_id.clone()]
                } else {
                    Vec::new()
                })
            }
            TurnPolicy::RoundRobin => {
                let active = self.active_agent_ids(ctx).await?;
                Ok(if active.is_empty() {
                    Vec::new()
                } else {
                    vec![active[(seq % active.len() as u64) as usize].clone()]
                })
            }
        }
    }

    /// NO-FAIL ARM. the tagging plane routes a user post here in the same
    /// block as the post itself — an `Err` would abort the post (and every
    /// other subscriber's delivery), so malformed events, unwatched
    /// channels, failed context pins, and oversized payloads are all staged
    /// no-ops. the plane's loop rule guarantees the
    /// event is user-authored.
    pub(super) async fn on_engagement(
        &mut self,
        ctx: &mut dyn Ctx,
        payload: &[u8],
    ) -> Result<(), Error> {
        let Ok(event) = tagging_decode_event(payload) else {
            self.note(ctx, "dropped undecodable engagement event".into());
            return Ok(());
        };
        let EngagementEvent {
            source,
            container,
            content_seq: seq,
            author: _,
            tags,
        } = event;
        let is_page = self.pages.as_deref() == Some(source.as_str());
        if source != self.chat && !is_page {
            self.note(ctx, format!("dropped engagement from source {source}"));
            return Ok(());
        }
        let policy = if is_page {
            // A structured page mention is itself the opt-in. Tagging routes
            // it directly to this entity-owning module, so a fresh comment
            // thread needs no pre-existing channel watch.
            TurnPolicy::Mention
        } else {
            let Some(policy) = self.watch(&container).cloned() else {
                return Ok(());
            };
            policy
        };

        let engaged = match self.engaged_agents(&*ctx, &policy, &tags, seq).await {
            Ok(engaged) => engaged,
            Err(reason) => {
                self.note(ctx, format!("engagement skipped for {container}: {reason}"));
                return Ok(());
            }
        };
        let requester = canonical_origin(&ctx.env().origin);
        for agent_id in engaged {
            let run_id = if is_page {
                page_run_id_for(&container, seq, &agent_id)
            } else {
                run_id_for(&container, seq, &agent_id)
            };
            let dispatch_id = dispatch_id_for(&run_id);
            match self.turn_taken(&*ctx, &dispatch_id).await {
                // the turn claim: the first creation in consensus order won.
                Ok(true) => continue,
                Ok(false) => {}
                Err(reason) => {
                    self.note(ctx, format!("run skipped for {run_id}: {reason}"));
                    continue;
                }
            }
            let agent = match self.active_agent(&*ctx, &agent_id).await {
                Ok(Some(agent)) => agent,
                Ok(None) => continue,
                Err(reason) => {
                    self.note(ctx, format!("run skipped for {run_id}: {reason}"));
                    continue;
                }
            };
            let prepared = if is_page {
                self.prepare_page_dispatch(&*ctx, &agent, &container, seq)
                    .await
            } else {
                self.prepare_dispatch(&*ctx, &agent, &container, seq).await
            };
            match prepared {
                Ok(prepared) => self.stage_dispatch_run(
                    ctx,
                    &run_id,
                    agent_id,
                    if is_page {
                        page_channel_id(&container)
                    } else {
                        container.clone()
                    },
                    seq,
                    requester.clone(),
                    prepared,
                ),
                // a failed preparation must not poison the posting block —
                // same no-fail reasoning as the result intake.
                Err(reason) => self.note(ctx, format!("run skipped for {run_id}: {reason}")),
            }
        }
        Ok(())
    }
}
