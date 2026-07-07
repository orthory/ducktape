//! the canned-oracle seam: answer a pending `WorkerRequest` effect as an
//! ordinary op (the N2 laundering seam — a non-deterministic LLM becomes
//! consensus-safe once its output is submitted as data), plus `deliver`, the
//! benign block that triggers the host's committed delivery injection.

use chat::{ChatMsg, PostPolicy, encode_msg as chat_encode_msg};
use dispatch::{WorkSpec, decode_work_spec};
use host::BlockOutcome;
use saga::{SagaMsg, WorkerRequest, decode_worker_request, encode_msg as saga_encode_msg};
use sdk::{Effect, Origin};

use super::PackageTestBed;
use crate::error::HarnessError;

/// the oracle's external origin — mirrors `collaboration_loop.rs`.
const ORACLE_KEY: &[u8] = b"oracle";

impl PackageTestBed {
    /// decode one pending effect as a `WorkerRequest` whose spec is a dispatch
    /// `WorkSpec` — the kind gate a real worker applies (the spec must be
    /// what the recipe promised the capability provider).
    fn decode_pending(effect: &Effect) -> Result<(WorkerRequest, WorkSpec), HarnessError> {
        let request = decode_worker_request(&effect.0).map_err(HarnessError::NotAWorkerRequest)?;
        let spec = decode_work_spec(&request.spec).map_err(HarnessError::NotAWorkSpec)?;
        Ok((request, spec))
    }

    async fn submit_oracle_result(
        &mut self,
        request: WorkerRequest,
        outcome: Result<Vec<u8>, String>,
    ) -> Result<BlockOutcome, HarnessError> {
        self.submit_with_context(
            "oracle block rejected",
            Origin::External(ORACLE_KEY.to_vec()),
            "saga",
            saga_encode_msg(&SagaMsg::OracleResult {
                saga_id: request.saga_id,
                attempt: request.attempt,
                outcome,
            }),
        )
        .await
    }

    /// answer the OLDEST pending `WorkerRequest` effect with a canned oracle
    /// outcome, submitted as an ordinary op (the N2 laundering seam): raw
    /// model bytes on `Ok`, a provider failure on `Err`. fine for a
    /// single-capability package; a package with several agents (several
    /// capabilities) should target the right one via [`Self::oracle_for`]
    /// instead of relying on queue order.
    pub async fn oracle(
        &mut self,
        outcome: Result<Vec<u8>, String>,
    ) -> Result<BlockOutcome, HarnessError> {
        let effect = self
            .pending_effects
            .pop_front()
            .ok_or(HarnessError::NoPendingOracleRequest)?;
        let (request, _spec) = Self::decode_pending(&effect)?;
        self.submit_oracle_result(request, outcome).await
    }

    /// answer the OLDEST pending `WorkerRequest` effect whose dispatch
    /// `WorkSpec` carries `capability` — the multi-capability oracle gate: a
    /// package with several agents enqueues one `WorkerRequest` per engaged
    /// agent, each stamped with ITS agent's capability tag, and a scripted
    /// test must answer the ONE meant for the capability it stands in for,
    /// never whichever request happens to sit at the front of the queue.
    pub async fn oracle_for(
        &mut self,
        capability: &str,
        outcome: Result<Vec<u8>, String>,
    ) -> Result<BlockOutcome, HarnessError> {
        let mut pending_capabilities = Vec::new();
        let mut found = None;
        for (index, effect) in self.pending_effects.iter().enumerate() {
            let (request, spec) = Self::decode_pending(effect)?;
            if spec.capability == capability {
                found = Some((index, request));
                break;
            }
            pending_capabilities.push(spec.capability);
        }
        let (index, request) = found.ok_or_else(|| HarnessError::NoPendingOracleForCapability {
            capability: capability.to_string(),
            pending: pending_capabilities,
        })?;
        self.pending_effects.remove(index).expect("index in range");
        self.submit_oracle_result(request, outcome).await
    }

    /// script one oracle turn whose raw model text is the compact encoding of
    /// `response` (a strict `AgentResponse`-shaped JSON value).
    pub async fn oracle_response_json(
        &mut self,
        response: &serde_json::Value,
    ) -> Result<BlockOutcome, HarnessError> {
        let raw = serde_json::to_vec(response).expect("a json value serializes");
        self.oracle(Ok(raw)).await
    }

    /// [`Self::oracle_response_json`], targeted at `capability` (see
    /// [`Self::oracle_for`]).
    pub async fn oracle_response_json_for(
        &mut self,
        capability: &str,
        response: &serde_json::Value,
    ) -> Result<BlockOutcome, HarnessError> {
        let raw = serde_json::to_vec(response).expect("a json value serializes");
        self.oracle_for(capability, Ok(raw)).await
    }

    /// advance one block with a benign op — what triggers the host's
    /// committed delivery injection (the never-pop-stack rule's other half).
    /// the platform has no empty-block primitive, so this is a real chat
    /// `CreateChannel` from the framework driver, exactly like the
    /// `collaboration_loop.rs` noop blocks.
    pub async fn deliver(&mut self) -> Result<BlockOutcome, HarnessError> {
        self.noop_seq += 1;
        let channel = format!("quack-harness-noop-{}", self.noop_seq);
        self.submit_with_context(
            "delivery block rejected",
            self.driver(),
            "chat",
            chat_encode_msg(&ChatMsg::CreateChannel {
                channel_id: channel,
                name: "Quack Harness Noop".into(),
                post_policy: PostPolicy::Open,
            }),
        )
        .await
    }
}
