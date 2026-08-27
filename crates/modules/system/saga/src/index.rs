//! saga's materialized view: the usage ledger.
//!
//! a node-local derived index over the saga op stream — NO consensus change.
//! it answers "whose subscription carried how much": every finalized attempt
//! (one [`SagaMsg::OracleResult`], Ok or Err — retries fan out, failures also
//! bill) is attributed to its EXECUTOR, the External node key that submitted
//! the result op (the op row's `origin.id`, lowercase hex). node→account
//! resolution is app-side via identity's `OfKey` — a mapper can only read
//! its own module's index.
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
//! UNLIKE chat's mapper, this fold NEVER fails on a payload it cannot make
//! sense of: a fold failure holds the module's feed queue, and a
//! deterministic decode failure would wedge the ledger permanently — billing
//! display is not worth a stuck fold. undecodable payload, unexpected
//! variant, unattributable origin → skip and continue.
//!
//! the ledger accrues from the deploy boundary forward (canonical saga state
//! prunes terminal sagas, so pre-boundary history is not re-derivable); a
//! `Trigger` folded before the boundary surfaces its attempts with
//! capability `"unknown"` and duration 0.
//!
//! this file is the DECISION core — pure functions over [`StateRead`],
//! compiled natively and unit-tested against a plain map. the wasm shell
//! (`src/index_guest.rs`, feature `index-guest`) wires it into the engine.

use std::collections::BTreeMap;

use index_guest::{Fail, MAX_SCAN_LIMIT, OpRow, OriginKind, StateRead, Writes};
use serde::{Deserialize, Serialize};

use crate::{SagaMsg, decode_msg};

/// [`Fail`] code: a view request this mapper does not speak.
const FAIL_BAD_REQUEST: i32 = 4;

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
struct TriggerRow {
    capability: Option<String>,
    created_height: u64,
}

/// one finalized attempt, as folded. `height` is the block the result landed
/// in — the `since_height` filter cuts on it.
#[derive(Debug, Serialize, Deserialize)]
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

/// the view request: `{"usage": {"since_height": 100}}`. `since_height` keeps
/// only attempts whose result landed at or after it; absent = all-time.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageViewQuery {
    Usage {
        #[serde(default)]
        since_height: Option<u64>,
    },
}

/// the view reply: `{"usage": [<UsageRow>…]}` in (executor, capability,
/// outcome) order.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageViewReply {
    Usage(Vec<UsageRow>),
}

/// one aggregated ledger line: runs and total duration for an (executor,
/// capability, outcome) bucket.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
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

/// fold one applied op into derived writes. billing is non-critical: anything
/// this fold cannot make sense of is SKIPPED, never a [`Fail`] — a
/// deterministic failure would hold the feed queue forever.
pub fn fold_op(op: &OpRow, read: &impl StateRead) -> Writes {
    let mut out = Writes::new();
    let Ok(msg) = decode_msg(&op.payload) else {
        return out;
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
            if read.get(key.as_bytes()).is_some() {
                return out;
            }
            let row = TriggerRow {
                capability,
                created_height: op.height,
            };
            if let Ok(bytes) = serde_json::to_vec(&row) {
                index_guest::put(&mut out, key, bytes);
            }
        }
        SagaMsg::OracleResult {
            saga_id,
            attempt,
            outcome,
            usage,
        } => {
            // the executor is the External submitter of the result op;
            // anything else cannot be attributed — skip.
            if op.origin.kind != OriginKind::External {
                return out;
            }
            let Some(executor) = op.origin.id.clone() else {
                return out;
            };
            let (capability, duration_blocks) = match read.get(trigger_key(&saga_id).as_bytes()) {
                Some(bytes) => match serde_json::from_slice::<TriggerRow>(&bytes) {
                    Ok(trigger) => (
                        trigger.capability.unwrap_or_else(|| UNTAGGED.into()),
                        op.height.saturating_sub(trigger.created_height),
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
                height: op.height,
            };
            if let Ok(bytes) = serde_json::to_vec(&row) {
                index_guest::put(&mut out, attempt_key(&executor, &saga_id, attempt), bytes);
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
    out
}

/// serve one materialized-view request.
pub fn serve_view(read: &impl StateRead, req: &[u8]) -> Result<Vec<u8>, Fail> {
    let query: UsageViewQuery =
        serde_json::from_slice(req).map_err(|e| Fail::new(FAIL_BAD_REQUEST, e.to_string()))?;
    match query {
        UsageViewQuery::Usage { since_height } => {
            let since = since_height.unwrap_or(0);
            // ponytail: full attempt/ scan per request — page a cursor or
            // maintain rollup rows if the ledger outgrows it.
            let mut agg: BTreeMap<(String, String, bool), UsageTotals> = BTreeMap::new();
            let mut after: Option<Vec<u8>> = None;
            loop {
                let page = read.scan_page(ATTEMPT_PREFIX, after.as_deref(), MAX_SCAN_LIMIT);
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
                    bucket.output_tokens = bucket.output_tokens.saturating_add(row.output_tokens);
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
                .map_err(|e| Fail::new(FAIL_BAD_REQUEST, e.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode_msg;
    use index_guest::{OriginTag, apply_to_map};

    type Map = BTreeMap<Vec<u8>, Vec<u8>>;

    fn op(height: u64, origin: OriginTag, msg: &SagaMsg) -> OpRow {
        OpRow {
            height,
            seq: 0,
            time: height,
            origin,
            payload: encode_msg(msg),
            assigned: Vec::new(),
        }
    }

    fn trigger(height: u64, saga_id: &str, capability: Option<&str>) -> OpRow {
        op(
            height,
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

    fn result(height: u64, executor: &str, saga_id: &str, attempt: u32, ok: bool) -> OpRow {
        result_with_usage(height, executor, saga_id, attempt, ok, None)
    }

    fn result_with_usage(
        height: u64,
        executor: &str,
        saga_id: &str,
        attempt: u32,
        ok: bool,
        usage: Option<crate::TokenUsage>,
    ) -> OpRow {
        op(
            height,
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

    fn fold(map: &mut Map, op: OpRow) {
        let writes = fold_op(&op, map);
        apply_to_map(map, writes);
    }

    fn usage(map: &Map, req: serde_json::Value) -> Vec<UsageRow> {
        let bytes = serve_view(map, &serde_json::to_vec(&req).unwrap()).expect("view");
        match serde_json::from_slice(&bytes).expect("reply decodes") {
            UsageViewReply::Usage(rows) => rows,
        }
    }

    #[test]
    fn attempts_bill_per_result_with_capability_and_block_duration() {
        let mut map = Map::new();
        fold(&mut map, trigger(5, "runs\u{1f}d1", Some("model-1")));
        // attempt 1 fails on aa…, attempt 2 (the retry) lands on bb… — both
        // bill, to different executors.
        fold(&mut map, result(9, "aa11", "runs\u{1f}d1", 1, false));
        fold(
            &mut map,
            result_with_usage(
                12,
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
            ),
        );
        // an untagged saga on the same executor.
        fold(&mut map, trigger(13, "runs\u{1f}d2", None));
        fold(&mut map, result(15, "bb22", "runs\u{1f}d2", 1, true));

        let rows = usage(&map, serde_json::json!({"usage": {}}));
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
        let mut map = Map::new();
        fold(&mut map, trigger(5, "s1", Some("model-1")));
        fold(&mut map, result(9, "aa11", "s1", 1, false));
        fold(&mut map, result(12, "aa11", "s1", 2, true));

        let rows = usage(&map, serde_json::json!({"usage": {"since_height": 12}}));
        assert_eq!(rows.len(), 1);
        assert!(rows[0].outcome_ok);
        assert_eq!(rows[0].runs, 1);
    }

    #[test]
    fn result_without_trigger_bills_as_unknown_not_a_skip() {
        let mut map = Map::new();
        // the trigger predates the mapper's deploy boundary.
        fold(&mut map, result(20, "aa11", "ghost", 1, true));

        let rows = usage(&map, serde_json::json!({"usage": {}}));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].capability, "unknown");
        assert_eq!(rows[0].total_duration_blocks, 0);
    }

    #[test]
    fn garbage_and_unattributable_ops_skip_without_failing() {
        let mut map = Map::new();
        // undecodable payload — the fold must decide "nothing", never fail.
        fold(
            &mut map,
            OpRow {
                height: 1,
                seq: 0,
                time: 1,
                origin: OriginTag::external("aa11"),
                payload: b"not json".to_vec(),
                assigned: Vec::new(),
            },
        );
        // a result with a non-External origin cannot be attributed.
        fold(
            &mut map,
            op(
                2,
                OriginTag::module("saga"),
                &SagaMsg::OracleResult {
                    saga_id: "s1".into(),
                    attempt: 1,
                    outcome: Ok(Vec::new()),
                    usage: None,
                },
            ),
        );
        // non-billing variants are no-ops for the ledger.
        fold(
            &mut map,
            op(3, OriginTag::external("aa11"), &SagaMsg::Crank {}),
        );
        assert!(usage(&map, serde_json::json!({"usage": {}})).is_empty());
        assert!(map.is_empty(), "nothing derived");
    }

    #[test]
    fn duplicate_result_rewrites_the_same_key() {
        let mut map = Map::new();
        fold(&mut map, trigger(5, "s1", Some("model-1")));
        fold(&mut map, result(9, "aa11", "s1", 1, true));
        fold(&mut map, result(9, "aa11", "s1", 1, true));

        let rows = usage(&map, serde_json::json!({"usage": {}}));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].runs, 1, "an exact duplicate never double-bills");
    }
}
