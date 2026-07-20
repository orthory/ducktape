//! saga's materialized view: the usage ledger.
//!
//! a node-local derived index over the saga op stream — NO consensus change.
//! it answers "whose subscription carried how much": every finalized attempt
//! (one [`SagaMsg::OracleResult`], Ok or Err — retries fan out, failures also
//! bill) is attributed to its EXECUTOR, the External node key that submitted
//! the result op (`OpMeta.origin.id`, lowercase hex). node→account resolution
//! is app-side via identity's `OfNode` — a mapper can only read its own
//! module's index.
//! token counters are executor-reported observability, not provider-attested
//! proof; quotas, rewards, and billing must never trust them.
//!
//! key spaces (inside saga's per-module index database):
//! - `saga/<saga_id>` — the trigger row: `{capability, createdHeight}`. read
//!   back when a later `OracleResult` folds, to attach the capability tag and
//!   compute the attempt's duration.
//! - `attempt/<executor_hex>/<saga_id>/<attempt:08x>` — one finalized attempt
//!   ([`AttemptRow`]). an exact duplicate result (module-level no-op) rewrites
//!   the same key, so it never double-bills.
//!
//! durations are BLOCK/HEIGHT deltas, not seconds: on the real validator
//! network `consensus_time == height`. render them as blocks/ticks.
//!
//! UNLIKE chat's mapper, this one NEVER returns `Err` from `index_op` for a
//! payload it cannot make sense of: index-store poison is store-wide (one
//! flag freezes chat/tasks/pages search too), and billing display is not
//! worth that. undecodable payload, unexpected variant, unattributable
//! origin → skip and continue.
//!
//! no from-state rebuild (`supports_rebuild` = false): canonical saga state
//! prunes terminal sagas, so history is not re-derivable. the ledger accrues
//! from the deploy boundary forward; a `Trigger` folded before the boundary
//! surfaces its attempts with capability `"unknown"` and duration 0.

use crate::{SagaMsg, decode_msg};
use indexer::{
    ApplyCtx, Derived, Error, MAX_SCAN_LIMIT, ModuleIndexer, OpMeta, OriginKind, Result, ViewReader,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

fn trigger_key(saga_id: &str) -> String {
    format!("saga/{saga_id}")
}

fn attempt_key(executor: &str, saga_id: &str, attempt: u32) -> String {
    format!("attempt/{executor}/{saga_id}/{attempt:08x}")
}

const ATTEMPT_PREFIX: &[u8] = b"attempt/";

/// capability rendered when the trigger carried none (valset assignment).
const UNTAGGED: &str = "untagged";
/// capability rendered when no trigger row exists (pre-boundary saga).
const UNKNOWN: &str = "unknown";

/// the stored trigger row: what a later `OracleResult` needs from its saga.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TriggerRow {
    capability: Option<String>,
    created_height: u64,
}

/// one finalized attempt, as folded. `height` is the block the result landed
/// in — the `sinceHeight` filter cuts on it.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptRow {
    pub executor_hex: String,
    pub capability: String,
    pub outcome_ok: bool,
    pub duration_blocks: u64,
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub cached_input_tokens: u64,
    #[serde(default)]
    pub cache_write_input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub reasoning_output_tokens: u64,
    pub height: u64,
}

/// the view request: `{"usage": {"sinceHeight": 100}}`. `sinceHeight` keeps
/// only attempts whose result landed at or after it; absent = all-time.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UsageViewQuery {
    #[serde(rename_all = "camelCase")]
    Usage {
        #[serde(default)]
        since_height: Option<u64>,
    },
}

/// the view reply: `{"usage": [<UsageRow>…]}` in (executor, capability,
/// outcome) order.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum UsageViewReply {
    Usage(Vec<UsageRow>),
}

/// one aggregated ledger line: runs and total duration for an (executor,
/// capability, outcome) bucket.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UsageRow {
    pub executor_hex: String,
    pub capability: String,
    pub outcome_ok: bool,
    pub runs: u64,
    pub total_duration_blocks: u64,
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub cache_write_input_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_output_tokens: u64,
}

#[derive(Default)]
struct UsageTotals {
    runs: u64,
    duration_blocks: u64,
    input_tokens: u64,
    cached_input_tokens: u64,
    cache_write_input_tokens: u64,
    output_tokens: u64,
    reasoning_output_tokens: u64,
}

/// the saga usage mapper. register with the module's genesis id ("saga").
pub struct UsageIndex {
    module: String,
}

impl UsageIndex {
    pub fn new(module: impl Into<String>) -> Self {
        Self {
            module: module.into(),
        }
    }
}

#[async_trait::async_trait(?Send)]
impl ModuleIndexer for UsageIndex {
    fn module(&self) -> &str {
        &self.module
    }

    fn index_op(
        &self,
        ctx: &ApplyCtx,
        meta: &OpMeta,
        payload: &[u8],
        out: &mut Derived,
    ) -> Result<()> {
        // billing is non-critical: anything this fold cannot make sense of is
        // SKIPPED, never an Err — an index_op Err poisons the whole store.
        let Ok(msg) = decode_msg(payload) else {
            return Ok(());
        };
        match msg {
            SagaMsg::Trigger {
                saga_id,
                capability,
                ..
            } => {
                let key = trigger_key(&saga_id);
                // a duplicate trigger is a module-level no-op; keep the
                // original clock and tag.
                if ctx.get(key.as_bytes())?.is_some() {
                    return Ok(());
                }
                let row = TriggerRow {
                    capability,
                    created_height: meta.height,
                };
                if let Ok(bytes) = serde_json::to_vec(&row) {
                    out.put(key, bytes);
                }
            }
            SagaMsg::OracleResult {
                saga_id,
                attempt,
                outcome,
                usage,
            } => {
                // the executor is the External submitter of the result op;
                // anything else cannot be attributed — skip, don't poison.
                if meta.origin.kind != OriginKind::External {
                    return Ok(());
                }
                let Some(executor) = meta.origin.id.clone() else {
                    return Ok(());
                };
                let (capability, duration_blocks) =
                    match ctx.get(trigger_key(&saga_id).as_bytes())? {
                        Some(bytes) => match serde_json::from_slice::<TriggerRow>(&bytes) {
                            Ok(trigger) => (
                                trigger.capability.unwrap_or_else(|| UNTAGGED.into()),
                                meta.height.saturating_sub(trigger.created_height),
                            ),
                            // a damaged row reads as a missing one — still bill.
                            Err(_) => (UNKNOWN.into(), 0),
                        },
                        // the trigger predates this mapper (deploy boundary).
                        None => (UNKNOWN.into(), 0),
                    };
                let usage = usage.unwrap_or_default();
                let row = AttemptRow {
                    executor_hex: executor.clone(),
                    capability,
                    outcome_ok: outcome.is_ok(),
                    duration_blocks,
                    input_tokens: usage.input_tokens,
                    cached_input_tokens: usage.cached_input_tokens,
                    cache_write_input_tokens: usage.cache_write_input_tokens,
                    output_tokens: usage.output_tokens,
                    reasoning_output_tokens: usage.reasoning_output_tokens,
                    height: meta.height,
                };
                if let Ok(bytes) = serde_json::to_vec(&row) {
                    out.put(attempt_key(&executor, &saga_id, attempt), bytes);
                }
            }
            // accepts, cranks, cancels, and prunes change no billing.
            SagaMsg::Accept { .. }
            | SagaMsg::RenewLease { .. }
            | SagaMsg::Reassign { .. }
            | SagaMsg::Crank {}
            | SagaMsg::Cancel { .. }
            | SagaMsg::Prune { .. } => {}
        }
        Ok(())
    }

    fn serve_view(&self, reader: &ViewReader, req: &[u8]) -> Result<Vec<u8>> {
        let query: UsageViewQuery =
            serde_json::from_slice(req).map_err(|e| Error::View(e.to_string()))?;
        match query {
            UsageViewQuery::Usage { since_height } => {
                let since = since_height.unwrap_or(0);
                // ponytail: full attempt/ scan per request — page a cursor or
                // maintain rollup rows if the ledger outgrows it.
                let mut agg: BTreeMap<(String, String, bool), UsageTotals> = BTreeMap::new();
                let mut after: Option<Vec<u8>> = None;
                loop {
                    let page = reader.scan(ATTEMPT_PREFIX, after.as_deref(), MAX_SCAN_LIMIT)?;
                    for (_key, value) in &page.entries {
                        let Ok(row) = serde_json::from_slice::<AttemptRow>(value) else {
                            continue;
                        };
                        if row.height < since {
                            continue;
                        }
                        let bucket = agg
                            .entry((row.executor_hex, row.capability, row.outcome_ok))
                            .or_default();
                        bucket.runs = bucket.runs.saturating_add(1);
                        bucket.duration_blocks =
                            bucket.duration_blocks.saturating_add(row.duration_blocks);
                        bucket.input_tokens = bucket.input_tokens.saturating_add(row.input_tokens);
                        bucket.cached_input_tokens = bucket
                            .cached_input_tokens
                            .saturating_add(row.cached_input_tokens);
                        bucket.cache_write_input_tokens = bucket
                            .cache_write_input_tokens
                            .saturating_add(row.cache_write_input_tokens);
                        bucket.output_tokens =
                            bucket.output_tokens.saturating_add(row.output_tokens);
                        bucket.reasoning_output_tokens = bucket
                            .reasoning_output_tokens
                            .saturating_add(row.reasoning_output_tokens);
                    }
                    match page.next_after {
                        Some(cursor) => after = Some(cursor.into_bytes()),
                        None => break,
                    }
                }
                let rows: Vec<UsageRow> = agg
                    .into_iter()
                    .map(
                        |((executor_hex, capability, outcome_ok), totals)| UsageRow {
                            executor_hex,
                            capability,
                            outcome_ok,
                            runs: totals.runs,
                            total_duration_blocks: totals.duration_blocks,
                            input_tokens: totals.input_tokens,
                            cached_input_tokens: totals.cached_input_tokens,
                            cache_write_input_tokens: totals.cache_write_input_tokens,
                            output_tokens: totals.output_tokens,
                            reasoning_output_tokens: totals.reasoning_output_tokens,
                        },
                    )
                    .collect();
                serde_json::to_vec(&UsageViewReply::Usage(rows))
                    .map_err(|e| Error::View(e.to_string()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode_msg;
    use indexer::{AppliedOp, BlockOps, IndexStore, OriginTag};

    fn store(dir: &std::path::Path) -> IndexStore {
        IndexStore::open(dir, &["saga"])
            .expect("open store")
            .with_indexer(Box::new(UsageIndex::new("saga")))
    }

    fn op(origin: OriginTag, msg: &SagaMsg) -> AppliedOp {
        AppliedOp {
            module: "saga".into(),
            origin,
            payload: encode_msg(msg),
        }
    }

    fn trigger(saga_id: &str, capability: Option<&str>) -> AppliedOp {
        op(
            OriginTag::module("dispatch"),
            &SagaMsg::Trigger {
                saga_id: saga_id.into(),
                spec: b"work".to_vec(),
                reply_to: None,
                reply_payload: Vec::new(),
                deadline: None,
                max_attempts: 3,
                lease_views: None,
                capability: capability.map(Into::into),
                demands: Default::default(),
                pinned_assignee: None,
            },
        )
    }

    fn result(executor: &str, saga_id: &str, attempt: u32, ok: bool) -> AppliedOp {
        result_with_usage(executor, saga_id, attempt, ok, None)
    }

    fn result_with_usage(
        executor: &str,
        saga_id: &str,
        attempt: u32,
        ok: bool,
        usage: Option<crate::TokenUsage>,
    ) -> AppliedOp {
        op(
            OriginTag::external(executor),
            &SagaMsg::OracleResult {
                saga_id: saga_id.into(),
                attempt,
                outcome: if ok {
                    Ok(b"done".to_vec())
                } else {
                    Err("boom".into())
                },
                usage,
            },
        )
    }

    fn apply(store: &IndexStore, height: u64, ops: Vec<AppliedOp>) {
        store
            .apply_block(&BlockOps {
                height,
                time: height,
                ops,
                record: None,
            })
            .expect("apply");
    }

    fn usage(store: &IndexStore, req: serde_json::Value) -> Vec<UsageRow> {
        let bytes = store
            .view("saga", &serde_json::to_vec(&req).unwrap())
            .expect("view");
        match serde_json::from_slice(&bytes).expect("reply decodes") {
            UsageViewReply::Usage(rows) => rows,
        }
    }

    #[test]
    fn attempts_bill_per_result_with_capability_and_block_duration() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path());
        apply(&store, 5, vec![trigger("runs\u{1f}d1", Some("model-1"))]);
        // attempt 1 fails on aa…, attempt 2 (the retry) lands on bb… — both
        // bill, to different executors.
        apply(&store, 9, vec![result("aa11", "runs\u{1f}d1", 1, false)]);
        apply(
            &store,
            12,
            vec![result_with_usage(
                "bb22",
                "runs\u{1f}d1",
                2,
                true,
                Some(crate::TokenUsage {
                    input_tokens: 100,
                    cached_input_tokens: 60,
                    cache_write_input_tokens: 5,
                    output_tokens: 20,
                    reasoning_output_tokens: 7,
                }),
            )],
        );
        // an untagged saga on the same executor.
        apply(&store, 13, vec![trigger("runs\u{1f}d2", None)]);
        apply(&store, 15, vec![result("bb22", "runs\u{1f}d2", 1, true)]);

        let rows = usage(&store, serde_json::json!({"usage": {}}));
        assert_eq!(
            rows,
            vec![
                UsageRow {
                    executor_hex: "aa11".into(),
                    capability: "model-1".into(),
                    outcome_ok: false,
                    runs: 1,
                    total_duration_blocks: 4,
                    input_tokens: 0,
                    cached_input_tokens: 0,
                    cache_write_input_tokens: 0,
                    output_tokens: 0,
                    reasoning_output_tokens: 0,
                },
                UsageRow {
                    executor_hex: "bb22".into(),
                    capability: "model-1".into(),
                    outcome_ok: true,
                    runs: 1,
                    total_duration_blocks: 7,
                    input_tokens: 100,
                    cached_input_tokens: 60,
                    cache_write_input_tokens: 5,
                    output_tokens: 20,
                    reasoning_output_tokens: 7,
                },
                UsageRow {
                    executor_hex: "bb22".into(),
                    capability: "untagged".into(),
                    outcome_ok: true,
                    runs: 1,
                    total_duration_blocks: 2,
                    input_tokens: 0,
                    cached_input_tokens: 0,
                    cache_write_input_tokens: 0,
                    output_tokens: 0,
                    reasoning_output_tokens: 0,
                },
            ]
        );
    }

    #[test]
    fn since_height_cuts_on_the_result_block() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path());
        apply(&store, 5, vec![trigger("s1", Some("model-1"))]);
        apply(&store, 9, vec![result("aa11", "s1", 1, false)]);
        apply(&store, 5, vec![]); // no-op guard against test drift
        apply(&store, 12, vec![result("aa11", "s1", 2, true)]);

        let rows = usage(&store, serde_json::json!({"usage": {"sinceHeight": 12}}));
        assert_eq!(rows.len(), 1);
        assert!(rows[0].outcome_ok);
        assert_eq!(rows[0].runs, 1);
    }

    #[test]
    fn result_without_trigger_bills_as_unknown_not_a_skip() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path());
        // the trigger predates the mapper's deploy boundary.
        apply(&store, 20, vec![result("aa11", "ghost", 1, true)]);

        let rows = usage(&store, serde_json::json!({"usage": {}}));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].capability, "unknown");
        assert_eq!(rows[0].total_duration_blocks, 0);
    }

    #[test]
    fn garbage_and_unattributable_ops_skip_without_poisoning() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path());
        // undecodable payload — apply_block must succeed (no store poison).
        apply(
            &store,
            1,
            vec![AppliedOp {
                module: "saga".into(),
                origin: OriginTag::external("aa11"),
                payload: b"not json".to_vec(),
            }],
        );
        // a result with a non-External origin cannot be attributed.
        apply(
            &store,
            2,
            vec![op(
                OriginTag::module("saga"),
                &SagaMsg::OracleResult {
                    saga_id: "s1".into(),
                    attempt: 1,
                    outcome: Ok(Vec::new()),
                    usage: None,
                },
            )],
        );
        // non-billing variants are no-ops for the ledger.
        apply(&store, 3, vec![op(OriginTag::external("aa11"), &SagaMsg::Crank {})]);
        assert!(usage(&store, serde_json::json!({"usage": {}})).is_empty());

        // the store still folds: a later real attempt lands.
        apply(&store, 4, vec![trigger("s2", Some("model-1"))]);
        apply(&store, 6, vec![result("aa11", "s2", 1, true)]);
        let rows = usage(&store, serde_json::json!({"usage": {}}));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].total_duration_blocks, 2);
    }

    #[test]
    fn duplicate_result_rewrites_the_same_row_no_double_bill() {
        let dir = tempfile::tempdir().unwrap();
        let store = store(dir.path());
        apply(&store, 5, vec![trigger("s1", Some("model-1"))]);
        apply(&store, 9, vec![result("aa11", "s1", 1, true)]);
        // the module treats a duplicate (saga_id, attempt) as a no-op; the
        // fold rewrites the same key, so runs stays 1.
        apply(&store, 10, vec![result("aa11", "s1", 1, true)]);

        let rows = usage(&store, serde_json::json!({"usage": {}}));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].runs, 1);
    }
}
