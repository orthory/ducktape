use super::{
    Ctx, EngagementEvent, EntityRef, Error, RunsModule, TurnPolicy, canonical_origin,
    dispatch_id_for, run_id_for, tagging_decode_event,
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
            container: channel_id,
            content_seq: seq,
            author: _,
            tags,
        } = event;
        if source != self.chat {
            // this module only understands chat containers; a subscription
            // to another source would be a config bug, not a block abort.
            self.note(ctx, format!("dropped engagement from source {source}"));
            return Ok(());
        }
        let Some(policy) = self.watch(&channel_id).cloned() else {
            // an engagement for a channel we no longer watch (subscription
            // and watch drift within a block): a no-op, never an error.
            return Ok(());
        };

        let engaged = match self.engaged_agents(&*ctx, &policy, &tags, seq).await {
            Ok(engaged) => engaged,
            Err(reason) => {
                self.note(
                    ctx,
                    format!("engagement skipped for {channel_id}: {reason}"),
                );
                return Ok(());
            }
        };
        let requester = canonical_origin(&ctx.env().origin);
        for agent_id in engaged {
            let run_id = run_id_for(&channel_id, seq, &agent_id);
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
            match self.prepare_dispatch(&*ctx, &agent, &channel_id, seq).await {
                Ok(prepared) => self.stage_dispatch_run(
                    ctx,
                    &run_id,
                    agent_id,
                    channel_id.clone(),
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
