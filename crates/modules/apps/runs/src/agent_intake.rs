use super::{
    AgentEvent, Ctx, DispatchMsg, Error, Msg, OutputContract, RUN_DEADLINE_VIEWS, RUN_LEASE_VIEWS,
    RUN_MAX_ATTEMPTS, Routing, RunsModule, agent_decode_event, dispatch_encode_msg, recipe_id_for,
};

impl RunsModule {
    // ---- the registry hook (origin == agent) ------------------------------------

    /// keep an agent's dispatch-plane recipe in lockstep with its registry
    /// record. this intake rides the registry write's own block and — unlike
    /// every other module intake — MAY error: an `Err` aborts that write,
    /// which is exactly the atomicity the recipe seam needs (a squatted or
    /// oversized recipe id must fail the registration, not land a record
    /// without a recipe).
    pub(super) fn on_agent_event(
        &mut self,
        ctx: &mut dyn Ctx,
        payload: &[u8],
    ) -> Result<(), Error> {
        match agent_decode_event(payload).map_err(Error::Module)? {
            AgentEvent::Registered {
                agent_id,
                capability,
            } => {
                // the agent's recipe id must fit the dispatch plane's id cap,
                // or the recipe registration below could never land.
                if recipe_id_for(&agent_id).len() > dispatch::MAX_ID_BYTES {
                    return Err(Error::Module(format!(
                        "agent_id is too long for its dispatch recipe id (cap {})",
                        dispatch::MAX_ID_BYTES - recipe_id_for("").len()
                    )));
                }
                ctx.emit_msg(Msg {
                    target: self.dispatch.clone(),
                    payload: dispatch_encode_msg(&DispatchMsg::RegisterRecipe {
                        recipe_id: recipe_id_for(&agent_id),
                        description: format!("runs for agent {agent_id}"),
                        capability,
                        routing: Routing::Rendezvous,
                        // Text on purpose: the oracle returns the model's raw
                        // answer and THIS module normalizes it — a strict
                        // Json contract would fail every prose reply.
                        output_contract: OutputContract::Text,
                        max_attempts: RUN_MAX_ATTEMPTS,
                        deadline_views: Some(RUN_DEADLINE_VIEWS),
                        lease_views: Some(RUN_LEASE_VIEWS),
                    }),
                });
                Ok(())
            }
            AgentEvent::CapabilityChanged {
                agent_id,
                capability,
            } => {
                // keep the agent's dispatch recipe on the same tag,
                // atomically with the record change.
                ctx.emit_msg(Msg {
                    target: self.dispatch.clone(),
                    payload: dispatch_encode_msg(&DispatchMsg::UpdateRecipe {
                        recipe_id: recipe_id_for(&agent_id),
                        description: None,
                        capability: Some(capability),
                        routing: None,
                        output_contract: None,
                        max_attempts: None,
                    }),
                });
                Ok(())
            }
            AgentEvent::Deregistered { agent_id } => {
                // retire the recipe with the agent: dispatch's own
                // `RemoveRecipe` already lets an in-flight dispatch finish
                // against the manifest it captured, so no run-liveness check
                // belongs here — that seam is deterministic on its own.
                ctx.emit_msg(Msg {
                    target: self.dispatch.clone(),
                    payload: dispatch_encode_msg(&DispatchMsg::RemoveRecipe {
                        recipe_id: recipe_id_for(&agent_id),
                    }),
                });
                Ok(())
            }
        }
    }
}
