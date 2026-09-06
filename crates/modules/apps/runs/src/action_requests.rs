//! External compute proposes effects; the account's program executes them.
//! The session signer proves which run proposed work and is never an account key.
use super::*;
use sdk::{Ack, CallId, Cause, DeliveryOutcome, Hop, PendingItem};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Publication {
    Queued,
    Delivered(DeliveryOutcome),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum RequestScope {
    Session { holder: Vec<u8> },
    Result,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(super) struct ActionRequest {
    pub view: ActionRequestView,
    pub item: u64,
    pub cause: Cause,
    pub publication: Publication,
    pub scope: RequestScope,
    pub model_id: String,
    pub grant: RunAuthority,
}

/// Capture target intents while retaining the original deterministic query surface.
pub(super) struct EffectsCtx<'a> {
    pub inner: &'a mut dyn Ctx,
    pub messages: Vec<Msg>,
}

#[async_trait::async_trait(?Send)]
impl Ctx for EffectsCtx<'_> {
    fn env(&self) -> &sdk::Env {
        self.inner.env()
    }
    fn module_root(&self, target: &str) -> Option<StateRoot> {
        self.inner.module_root(target)
    }
    async fn query(&self, target: &str, req: &[u8]) -> Result<Vec<u8>, Error> {
        self.inner.query(target, req).await
    }
    fn emit_msg(&mut self, msg: Msg) {
        self.messages.push(msg);
    }
    fn emit_event(&mut self, event: Event) {
        self.inner.emit_event(event);
    }
    fn set_output(&mut self, bytes: Vec<u8>) {
        self.inner.set_output(bytes);
    }
    fn set_assigned(&mut self, bytes: Vec<u8>) {
        self.inner.set_assigned(bytes);
    }
}

impl RunsModule {
    pub(super) fn action_request(&self, id: &str) -> Option<&ActionRequest> {
        self.pending_action_requests
            .get(id)
            .or_else(|| self.action_requests.get(id))
    }

    pub(super) fn stage_action_request(
        &mut self,
        entry: &PendingState,
        id: String,
        scope: RequestScope,
        msg: Msg,
    ) -> Result<(), Error> {
        let payload: serde_json::Value = sdk::wire::decode(&msg.payload).map_err(Error::Module)?;
        let request = ActionRequestView {
            request_id: id.clone(),
            account: entry.account,
            generation: entry.generation,
            run_id: entry.run_id.clone(),
            target: msg.target,
            payload,
            status: ActionStatus::AwaitingProgram,
        };
        if let Some(existing) = self.action_request(&id) {
            let exact = existing.view.account == request.account
                && existing.view.generation == request.generation
                && existing.view.run_id == request.run_id
                && existing.view.target == request.target
                && existing.view.payload == request.payload;
            if !exact {
                return Err(Error::Module(
                    "action request id is already bound to different work".into(),
                ));
            }
            return Ok(());
        }
        let item = self
            .staged_next_action_item
            .unwrap_or(self.next_action_item);
        let next = item
            .checked_add(1)
            .ok_or_else(|| Error::Module("action delivery counter exhausted".into()))?;
        let record = ActionRequest {
            view: request,
            item,
            cause: entry.cause.clone(),
            publication: Publication::Queued,
            scope,
            model_id: entry.agent_id.clone(),
            grant: RunAuthority::from_record(self.model(&entry.agent_id).ok_or_else(|| Error::Module("run model no longer exists".into()))?),
        };
        let representable = sdk::wire::encode(&record).len() <= sdk::MAX_STORE_VALUE_BYTES;
        if !representable {
            return Err(Error::Module(
                "action request exceeds the store value bound".into(),
            ));
        }
        self.pending_action_requests.insert(id, record);
        self.staged_next_action_item = Some(next);
        Ok(())
    }

    pub(super) fn action_deliveries(&self) -> Vec<PendingItem> {
        let mut queued: Vec<_> = self
            .action_requests
            .values()
            .filter(|request| matches!(request.publication, Publication::Queued))
            .collect();
        queued.sort_by_key(|request| request.item);
        queued
            .into_iter()
            .take(sdk::MAX_DELIVERIES_PER_BLOCK)
            .map(|request| PendingItem {
                item: request.item,
                target: self.id.clone(),
                payload: encode_msg(&RunsMsg::PublishActionRequest {
                    request_id: request.view.request_id.clone(),
                }),
                cause: Cause::Chain {
                    root: request.cause.root_for_item(&sdk::ItemRef { source: self.id.clone(), item: request.item }),
                    hop: Hop::Delivery(sdk::ItemRef { source: self.id.clone(), item: request.item }),
                },
            })
            .collect()
    }

    pub(super) fn publish_action_request(
        &mut self,
        ctx: &mut dyn Ctx,
        id: String,
    ) -> Result<(), Error> {
        let Some(request) = self.action_request(&id) else {
            return Err(Error::Module("unknown action request".into()));
        };
        let Cause::Chain {
            hop: Hop::Delivery(item),
            ..
        } = &ctx.env().cause
        else {
            return Err(Error::Module(
                "action publication requires its source delivery".into(),
            ));
        };
        let authenticated = ctx.env().origin == Origin::Module(self.id.clone())
            && item.source == self.id
            && item.item == request.item;
        if !authenticated {
            return Err(Error::Module(
                "action publication is not its source delivery".into(),
            ));
        }
        ctx.emit_msg(Msg {
            target: self.attribution.clone(),
            payload: attribution::encode_msg(&AttributionMsg::Attribute {
                object: ObjectRef {
                    kind: "action_request".into(),
                    object: id,
                },
                revision: 1,
                actor: Actor::Module(self.id.clone()),
                relations: vec![Relation {
                    recipient: request.view.account,
                    reason: Reason::Defined("action_request".into()),
                    detail: Vec::new(),
                }],
                transfers: Vec::new(),
            }),
        });
        Ok(())
    }

    pub(super) fn acknowledge_action(&mut self, ctx: &dyn Ctx, ack: &Ack) -> Result<(), Error> {
        let authentic = ctx.env().origin == Origin::System && ack.target == self.id;
        if !authentic {
            return Err(Error::Module(
                "action acknowledgment requires the host finalizer".into(),
            ));
        }
        let Some(request) = self
            .action_requests
            .values()
            .chain(self.pending_action_requests.values())
            .find(|request| request.item == ack.item)
            .cloned()
        else {
            return Err(Error::Module("unknown action delivery".into()));
        };
        let request = self
            .action_request(&request.view.request_id)
            .expect("found request")
            .clone();
        if let Publication::Delivered(previous) = &request.publication {
            if previous == &ack.outcome {
                return Ok(());
            }
            return Err(Error::Module(
                "conflicting action delivery acknowledgment".into(),
            ));
        }
        let mut request = request;
        request.publication = Publication::Delivered(ack.outcome.clone());
        match &ack.outcome {
            DeliveryOutcome::Applied => {}
            DeliveryOutcome::Failed { reason } => {
                request.view.status = ActionStatus::Rejected {
                    reason: reason.clone(),
                }
            }
            DeliveryOutcome::Unrepresentable => {
                request.view.status = ActionStatus::Rejected {
                    reason: "action publication was not representable".into(),
                }
            }
        }
        self.pending_action_requests
            .insert(request.view.request_id.clone(), request);
        Ok(())
    }

    async fn request_authority(&self, ctx: &dyn Ctx, request: &ActionRequest) -> Result<(), Error> {
        let generation = self.active_generation(ctx, request.view.account).await?;
        if generation != request.view.generation {
            return Err(Error::Module(
                "program authority changed since this run began".into(),
            ));
        }
        let Some(model) = self.model(&request.model_id) else {
            return Err(Error::Module("run model was removed".into()));
        };
        let unchanged_grant = model.account == request.view.account
            && model.status == ModelStatus::Active
            && RunAuthority::from_record(model) == request.grant;
        if !unchanged_grant {
            return Err(Error::Module("run model authority changed after this action was proposed".into()));
        }
        match &request.scope {
            RequestScope::Result => Ok(()),
            RequestScope::Session { holder } => {
                let Some(session) = self.session(&request.view.run_id) else {
                    return Err(Error::Module("run session closed".into()));
                };
                if &session.holder != holder {
                    return Err(Error::Module("run execution lease changed".into()));
                }
                self.session_holds_lease(ctx, &request.view.run_id, session)
                    .await
            }
        }
    }

    fn request_call(&self, ctx: &dyn Ctx, request: &ActionRequest) -> Result<CallId, Error> {
        let Origin::Program(account) = ctx.env().origin else {
            return Err(Error::Module(
                "only the requesting program may decide a tool action".into(),
            ));
        };
        let Cause::Chain {
            hop: Hop::Call(call),
            ..
        } = &ctx.env().cause
        else {
            return Err(Error::Module(
                "tool decision requires a program call".into(),
            ));
        };
        let owns_request = account == request.view.account && call.requester == self.agent;
        if !owns_request {
            return Err(Error::Module(
                "tool decision belongs to another program".into(),
            ));
        }
        Ok(call.clone())
    }

    pub(super) async fn claim_action_request(
        &mut self,
        ctx: &mut dyn Ctx,
        id: String,
        target_step: u64,
    ) -> Result<(), Error> {
        let Some(mut request) = self.action_request(&id).cloned() else {
            return Err(Error::Module("unknown action request".into()));
        };
        let call = self.request_call(ctx, &request)?;
        self.request_authority(ctx, &request).await?;
        let bytes = ctx.query(&self.attribution, &attribution::encode_query(&attribution::AttributionQuery::ChangesOf {
            source: attribution::Source { module: self.id.clone(), kind: "action_request".into(), object: id.clone() }, after: 0, limit: 1,
        })).await?;
        let attribution::AttributionReply::Changes(changes) = attribution::decode_reply(&bytes).map_err(Error::Module)? else { return Err(Error::Module("unexpected action attribution reply".into())); };
        let Some(change) = changes.first() else { return Err(Error::Module("action was not attributed".into())); };
        let bytes = ctx.query(&self.agent, &agent::encode_query(&agent::AgentQuery::Invocation { account: request.view.account, seq: change.change.seq })).await?;
        let agent::AgentReply::Invocation(Some(invocation)) = agent::decode_reply(&bytes).map_err(Error::Module)? else { return Err(Error::Module("action has no program invocation".into())); };
        let expected_invocation = matches!(invocation.status, agent::Status::Running { awaiting: agent::Outstanding::Call(ref awaiting), .. } if awaiting == &call);
        if !expected_invocation { return Err(Error::Module("claim is outside this action's attributed invocation".into())); }
        match &request.view.status {
            ActionStatus::AwaitingProgram => {}
            ActionStatus::Claimed { call: expected }
                if expected.invocation == call.invocation && expected.step == target_step => {}
            ActionStatus::Claimed { .. }
            | ActionStatus::Completed { .. }
            | ActionStatus::Rejected { .. } => {
                return Err(Error::Module("action request was already decided".into()));
            }
        }
        if target_step <= call.step {
            return Err(Error::Module(
                "action target must be a later program step".into(),
            ));
        }
        request.view.status = ActionStatus::Claimed {
            call: CallId {
                step: target_step,
                ..call.clone()
            },
        };
        ctx.set_output(sdk::wire::encode(&serde_json::json!({
            "target": request.view.target, "payload": request.view.payload,
            "requester": call.requester, "invocation": call.invocation,
        })));
        self.pending_action_requests.insert(id, request);
        Ok(())
    }

    pub(super) async fn complete_action_request(
        &mut self,
        ctx: &mut dyn Ctx,
        id: String,
        call: CallId,
    ) -> Result<(), Error> {
        let Some(mut request) = self.action_request(&id).cloned() else {
            return Err(Error::Module("unknown action request".into()));
        };
        let completing = self.request_call(ctx, &request)?;
        if let ActionStatus::Completed { call: previous, .. } = &request.view.status {
            let exact_retry = previous == &call && completing.invocation == call.invocation;
            if exact_retry {
                ctx.set_output(sdk::wire::encode(&request.view.status));
                return Ok(());
            }
        }
        let ActionStatus::Claimed { call: expected } = &request.view.status else {
            return Err(Error::Module("action request has not been claimed".into()));
        };
        let same_invocation = &call == expected
            && completing.invocation == expected.invocation
            && call.requester == self.agent;
        if !same_invocation {
            return Err(Error::Module(
                "completion is outside the claiming invocation".into(),
            ));
        }
        let bytes = ctx
            .query(
                &self.dispatch,
                &dispatch_encode_query(&DispatchQuery::Call { id: call.clone() }),
            )
            .await?;
        let DispatchReply::Call(Some(view)) =
            dispatch_decode_reply(&bytes).map_err(Error::Module)?
        else {
            return Err(Error::Module("action call does not exist".into()));
        };
        let digest: [u8; 32] = Sha256::digest(sdk::wire::encode(&request.view.payload)).into();
        let matches_request = view.account == request.view.account
            && view.generation == request.view.generation
            && view.target == request.view.target
            && view.payload_digest == digest;
        if !matches_request {
            return Err(Error::Module(
                "call does not execute this action request".into(),
            ));
        }
        let outcome = match view.status {
            dispatch::CallStatus::Queued => {
                return Err(Error::Module("action call has not completed".into()));
            }
            dispatch::CallStatus::Completed { outcome }
            | dispatch::CallStatus::Delivered { outcome, .. } => outcome,
        };
        request.view.status = ActionStatus::Completed { call, outcome };
        ctx.set_output(sdk::wire::encode(&request.view.status));
        self.pending_action_requests.insert(id, request);
        Ok(())
    }

    pub(super) async fn reject_action_request(
        &mut self,
        ctx: &mut dyn Ctx,
        id: String,
        reason: String,
    ) -> Result<(), Error> {
        let Some(mut request) = self.action_request(&id).cloned() else {
            return Err(Error::Module("unknown action request".into()));
        };
        self.request_call(ctx, &request)?;
        self.request_authority(ctx, &request).await?;
        let decided = !matches!(request.view.status, ActionStatus::AwaitingProgram);
        if decided {
            return Err(Error::Module("action request was already decided".into()));
        }
        request.view.status = ActionStatus::Rejected { reason };
        self.pending_action_requests.insert(id, request);
        Ok(())
    }

    pub(super) async fn action_view(
        &self,
        ctx: &dyn Ctx,
        id: &str,
    ) -> Result<Option<ActionRequestView>, Error> {
        let Some(request) = self.action_request(id) else {
            return Ok(None);
        };
        let mut view = request.view.clone();
        let finished = matches!(
            view.status,
            ActionStatus::Completed { .. } | ActionStatus::Rejected { .. }
        );
        if finished {
            return Ok(Some(view));
        }
        if let ActionStatus::Claimed { call } = &view.status {
            let bytes = ctx
                .query(
                    &self.dispatch,
                    &dispatch_encode_query(&DispatchQuery::Call { id: call.clone() }),
                )
                .await?;
            let DispatchReply::Call(queued) =
                dispatch_decode_reply(&bytes).map_err(Error::Module)?
            else {
                return Err(Error::Module("unexpected action call reply".into()));
            };
            if let Some(queued) = queued {
                let digest: [u8; 32] =
                    Sha256::digest(sdk::wire::encode(&request.view.payload)).into();
                let exact = queued.account == view.account
                    && queued.generation == view.generation
                    && queued.target == view.target
                    && queued.payload_digest == digest;
                if !exact {
                    view.status = ActionStatus::Rejected {
                        reason: "program called a different action than it claimed".into(),
                    };
                    return Ok(Some(view));
                }
                match queued.status {
                    dispatch::CallStatus::Queued => {}
                    dispatch::CallStatus::Completed { outcome }
                    | dispatch::CallStatus::Delivered { outcome, .. } => {
                        view.status = ActionStatus::Completed {
                            call: call.clone(),
                            outcome,
                        };
                        return Ok(Some(view));
                    }
                }
            }
        }
        // A grant or lease change is not a terminal receipt: the program may
        // already have authorized a target call. A generation change fences
        // every such call permanently, so it can terminate a waiting request.
        let control = self.account_control(ctx, view.account).await?;
        let same_generation = matches!(control, identity::Control::Program { generation, executor, standing: identity::ProgramStanding::Active, .. } if generation == view.generation && executor == self.agent);
        if !same_generation {
            view.status = ActionStatus::Rejected { reason: "program authority changed since this action was proposed".into() };
            return Ok(Some(view));
        }
        let query = attribution::AttributionQuery::ChangesOf {
            source: attribution::Source {
                module: self.id.clone(),
                kind: "action_request".into(),
                object: id.into(),
            },
            after: 0,
            limit: 1,
        };
        let bytes = ctx
            .query(&self.attribution, &attribution::encode_query(&query))
            .await?;
        let attribution::AttributionReply::Changes(changes) =
            attribution::decode_reply(&bytes).map_err(Error::Module)?
        else {
            return Err(Error::Module("unexpected action attribution reply".into()));
        };
        let Some(change) = changes.first() else {
            return Ok(Some(view));
        };
        let bytes = ctx
            .query(
                &self.agent,
                &agent::encode_query(&agent::AgentQuery::Invocation {
                    account: view.account,
                    seq: change.change.seq,
                }),
            )
            .await?;
        let agent::AgentReply::Invocation(invocation) =
            agent::decode_reply(&bytes).map_err(Error::Module)?
        else {
            return Err(Error::Module("unexpected action invocation reply".into()));
        };
        if invocation.is_none() {
            let bytes = ctx.query(&self.attribution, &attribution::encode_query(&attribution::AttributionQuery::DeliveryOf { subscriber: self.agent.clone(), seq: change.change.seq })).await?;
            let attribution::AttributionReply::Delivery(delivery) = attribution::decode_reply(&bytes).map_err(Error::Module)? else { return Err(Error::Module("unexpected program delivery reply".into())); };
            let no_invocation = delivery.as_ref().is_none_or(|delivery| matches!(delivery.state, attribution::DeliveryState::Retired(_)));
            if no_invocation {
                view.status = ActionStatus::Rejected { reason: "action attribution was not handled by a program".into() };
            }
            return Ok(Some(view));
        }
        let program_ended = invocation
            .as_ref()
            .is_some_and(|invocation| !matches!(invocation.status, agent::Status::Running { .. }));
        if program_ended {
            view.status = ActionStatus::Rejected {
                reason: "program finished without completing this action".into(),
            };
        }
        Ok(Some(view))
    }
}
