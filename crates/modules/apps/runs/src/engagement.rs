//! Source context is read only after the user's program explicitly requests model work.
use super::*;

impl RunsModule {
    pub(super) async fn request_attributed_run(
        &mut self,
        ctx: &mut dyn Ctx,
        agent_id: String,
        change_seq: u64,
        budget: &SiblingReadBudget,
    ) -> Result<(), Error> {
        let Origin::Program(account) = ctx.env().origin else {
            return Err(Error::Module(
                "attributed model work requires a program call".into(),
            ));
        };
        let Some(model) = self
            .active_agent(&*ctx, &agent_id)
            .await
            .map_err(Error::Module)?
        else {
            return Err(Error::Module("model is not active".into()));
        };
        if model.account != account {
            return Err(Error::Module("model belongs to another account".into()));
        }
        let after = change_seq
            .checked_sub(1)
            .ok_or_else(|| Error::Module("attribution changes start at one".into()))?;
        let bytes = ctx
            .query(
                &self.attribution,
                &attribution::encode_query(&attribution::AttributionQuery::Changes {
                    after,
                    limit: 1,
                }),
            )
            .await?;
        let attribution::AttributionReply::Changes(changes) =
            attribution::decode_reply(&bytes).map_err(Error::Module)?
        else {
            return Err(Error::Module("unexpected attribution reply".into()));
        };
        let Some(entry) = changes.first() else {
            return Err(Error::Module("attribution does not exist".into()));
        };
        let change = &entry.change;
        let addressed = change.seq == change_seq && change.recipient == account;
        if !addressed {
            return Err(Error::Module(
                "attribution belongs to another account".into(),
            ));
        }
        let run_request = change.source.module == self.id && change.source.kind == "run_request";
        if run_request {
            if let RunsMsg::RequestJobRun {
                agent_id: requested,
                job_id,
            } = decode_msg(&change.detail).map_err(Error::Module)?
            {
                if requested != agent_id {
                    return Err(Error::Module("job request names another model".into()));
                }
                return self.request_job_run(ctx, agent_id, job_id).await;
            }
        }
        let run_id = format!("attributed/{change_seq}/{agent_id}");
        if self
            .turn_taken(&*ctx, &dispatch_id_for(&run_id))
            .await
            .map_err(Error::Module)?
        {
            return Ok(());
        }
        let (channel, anchor, prepared, demands) =
            match (change.source.module.as_str(), change.source.kind.as_str()) {
                (source, "message") if source == self.chat => {
                    let bytes = ctx
                        .query(
                            &self.chat,
                            &chat_encode_query(&ChatQuery::Message {
                                message_id: change.source.object.clone(),
                            }),
                        )
                        .await?;
                    let ChatReply::Message(Some(message)) =
                        chat_decode_reply(&bytes).map_err(Error::Module)?
                    else {
                        return Err(Error::Module(
                            "attributed chat message is unavailable".into(),
                        ));
                    };
                    let channel = message.channel_id.clone();
                    let prepared = self
                        .prepare_dispatch(
                            &*ctx,
                            &model,
                            &run_id,
                            &channel,
                            message.seq,
                            &[],
                            budget,
                        )
                        .await
                        .map_err(Error::Module)?;
                    (channel, message.seq, prepared, BTreeMap::new())
                }
                (source, "comment") if Some(source) == self.pages.as_deref() => {
                    let bytes = ctx
                        .query(
                            source,
                            &pages::encode_query(&pages::PageQuery::GetComment {
                                comment_id: change.source.object.clone(),
                            }),
                        )
                        .await?;
                    let pages::PageReply::Comment(Some(comment)) =
                        pages::decode_reply(&bytes).map_err(Error::Module)?
                    else {
                        return Err(Error::Module(
                            "attributed page comment is unavailable".into(),
                        ));
                    };
                    let bytes = ctx
                        .query(
                            source,
                            &pages::encode_query(&pages::PageQuery::CommentThread {
                                thread_id: comment.thread_id.clone(),
                            }),
                        )
                        .await?;
                    let pages::PageReply::CommentThread(Some(thread)) =
                        pages::decode_reply(&bytes).map_err(Error::Module)?
                    else {
                        return Err(Error::Module(
                            "attributed comment thread is unavailable".into(),
                        ));
                    };
                    let Some(index) = thread
                        .comments
                        .iter()
                        .position(|item| item.id == comment.id)
                    else {
                        return Err(Error::Module("comment is not in its thread".into()));
                    };
                    let ordinal = index as u64 + 1;
                    let prepared = self
                        .prepare_page_dispatch(
                            &*ctx,
                            &model,
                            &run_id,
                            &comment.thread_id,
                            ordinal,
                            budget,
                        )
                        .await
                        .map_err(Error::Module)?;
                    (
                        page_channel_id(&comment.thread_id),
                        ordinal,
                        prepared,
                        BTreeMap::new(),
                    )
                }
                (source, "run_request") if source == self.id => {
                    let RunsMsg::RequestRun {
                        agent_id: requested,
                        channel_id,
                        anchor_seq,
                        demands,
                        skills,
                    } = decode_msg(&change.detail).map_err(Error::Module)?
                    else {
                        return Err(Error::Module("unexpected run request detail".into()));
                    };
                    if requested != agent_id {
                        return Err(Error::Module("run request names another model".into()));
                    }
                    let skills = envelope::library_skills(&skills).map_err(Error::Module)?;
                    let prepared = self
                        .prepare_dispatch(
                            &*ctx,
                            &model,
                            &run_id,
                            &channel_id,
                            anchor_seq,
                            &skills,
                            budget,
                        )
                        .await
                        .map_err(Error::Module)?;
                    (channel_id, anchor_seq, prepared, demands)
                }
                _ => {
                    return Err(Error::Module(
                        "this model workflow has no composer for the attribution source".into(),
                    ));
                }
            };
        self.stage_dispatch_run(
            ctx,
            &run_id,
            agent_id,
            channel,
            anchor,
            RunOrigin::Program(account),
            prepared,
            demands,
        );
        ctx.set_output(sdk::wire::encode(&serde_json::json!({"run_id": run_id})));
        Ok(())
    }
}
