//! runs' read model: the run journal — every lifecycle fact the module
//! committed about a run, in commit order, and the run's current state as
//! the fold of them — folded from the applied-op feed into runs' per-module
//! index database.
//!
//! canonical runs state keeps only what consensus needs: the in-flight
//! correlation entries, the live sessions, and a short delivered-runs ring.
//! the lifecycle itself — who dispatched a run, which node picked it up,
//! what it did, how it ended — is what an operator watches, and it lives
//! here, where the engine iterates natively and nothing is pruned.
//!
//! the feed carries the facts as the module's own assigned stamp
//! ([`crate::RunEvent`], [`crate::decode_assigned`]): an op that moved a run
//! stamps the exact transitions it committed, so the fold never re-derives a
//! transition from a payload. an op with an empty stamp moved no run.
//!
//! key spaces (inside runs' per-module index database):
//! - `run/{dispatch_id}` — one [`RunView`] per run, its current state.
//! - `journal/{dispatch_id}/{height:016x}/{seq:08x}/{n:04x}` — one
//!   [`JournalRow`] per fact, in commit order: the op's position in the
//!   chain, then the fact's position within the op.
//! - `recent/{!height:016x}/{!seq:08x}/{!n:04x}` — the dispatch id of every
//!   run, newest dispatch first (the counters are bitwise inverted so an
//!   ascending scan reads newest-first).
//! - `agent/{agent_id}/{!height:016x}/{!seq:08x}/{!n:04x}` — the same, per
//!   agent.
//!
//! the dispatch id (hex sha256 of the run id, [`crate::dispatch_id_for`]) is
//! the key segment because a run id carries the run separator and
//! caller-chosen text; the digest is fixed-width and separator-free.
//!
//! this file is the DECISION core — pure functions over [`StateRead`],
//! compiled natively and unit-tested against a plain map. the wasm shell
//! (`src/index_guest.rs`, feature `index-guest`) wires it into the engine.

use std::collections::BTreeMap;

use index_guest::{Fail, MAX_SCAN_LIMIT, OpRow, StateRead, Writes};
use serde::{Deserialize, Serialize};

use crate::{RunEvent, RunFact, RunOutcome, decode_assigned, dispatch_id_for};
use sdk::Origin as RunOrigin;

/// [`Fail`] code: an applied op's assigned stamp did not decode — interface
/// drift, which only a refold can honestly repair.
const FAIL_ASSIGNED_DECODE: i32 = 2;
/// [`Fail`] code: a stored row did not decode — a damaged read model.
const FAIL_ROW_DECODE: i32 = 3;
/// [`Fail`] code: a view request this mapper does not speak.
const FAIL_BAD_REQUEST: i32 = 4;

/// where in the chain a fact was committed.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Stamp {
    pub height: u64,
    /// the block's agreed timestamp (consensus time).
    pub time: u64,
}

/// a run's lifecycle position: the fold of its journal.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum RunState {
    /// staged on the dispatch plane; no node has bound a session yet.
    Dispatched,
    /// the lease holder bound its session key.
    Running { attempt: u32, holder: String },
    /// delivered. `outcome` mirrors the module's ring: a later result-action
    /// refusal turns an accepted result into [`RunOutcome::ActionRejected`],
    /// never the other way.
    Settled {
        outcome: RunOutcome,
        reason: Option<String>,
        degraded: bool,
        executing_node: String,
        output_ref: Option<String>,
        at: Stamp,
    },
}

/// one run as the list and detail views return it.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RunView {
    pub run_id: String,
    pub dispatch_id: String,
    pub agent_id: String,
    /// empty for job-backed runs.
    pub channel_id: String,
    /// 0 for job-backed runs.
    pub anchor_seq: u64,
    pub job_id: Option<String>,
    pub delegation_id: Option<String>,
    pub requester: RunOrigin,
    pub dispatched: Stamp,
    pub state: RunState,
    /// actions the run staged, on either lane.
    pub actions: u64,
    /// the forge PR the run opened or updated, once authenticated.
    pub pr_number: Option<u64>,
}

/// one journal entry as the detail view returns it.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct JournalRow {
    pub height: u64,
    pub time: u64,
    pub fact: RunFact,
}

/// one run with its whole journal.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RunDetail {
    pub run: RunView,
    pub journal: Vec<JournalRow>,
}

/// runs' view requests, externally tagged:
/// `{"recent": {"agent_id": "bot", "limit": 50}}`, `{"run": {"run_id": "…"}}`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunsViewQuery {
    /// runs newest-dispatch first, every agent's or one agent's. `limit`
    /// defaults to, and is clamped at, one scan page ([`MAX_SCAN_LIMIT`]).
    Recent {
        #[serde(default)]
        agent_id: Option<String>,
        #[serde(default)]
        limit: Option<usize>,
    },
    /// one run and its journal.
    Run { run_id: String },
}

/// runs' view replies.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunsViewReply {
    Runs(Vec<RunView>),
    /// boxed only to keep the reply enum small — serde is transparent over
    /// `Box`, so the wire shape is the bare detail or `null`.
    Run(Option<Box<RunDetail>>),
}

fn run_key(dispatch_id: &str) -> String {
    format!("run/{dispatch_id}")
}

fn journal_prefix(dispatch_id: &str) -> String {
    format!("journal/{dispatch_id}/")
}

fn journal_key(dispatch_id: &str, op: &OpRow, n: usize) -> String {
    format!(
        "{}{:016x}/{:08x}/{:04x}",
        journal_prefix(dispatch_id),
        op.height,
        op.seq,
        n
    )
}

/// the newest-first ordinal of a fact: every counter bitwise inverted.
fn newest_first(op: &OpRow, n: usize) -> String {
    format!("{:016x}/{:08x}/{:04x}", !op.height, !op.seq, !(n as u32))
}

const RECENT_PREFIX: &str = "recent/";

fn recent_key(op: &OpRow, n: usize) -> String {
    format!("{RECENT_PREFIX}{}", newest_first(op, n))
}

fn agent_prefix(agent_id: &str) -> String {
    format!("agent/{agent_id}/")
}

fn agent_key(agent_id: &str, op: &OpRow, n: usize) -> String {
    format!("{}{}", agent_prefix(agent_id), newest_first(op, n))
}

fn decode_row<T: for<'de> Deserialize<'de>>(value: &[u8]) -> Result<T, Fail> {
    serde_json::from_slice(value).map_err(|e| Fail::new(FAIL_ROW_DECODE, e.to_string()))
}

fn encode_row<T: Serialize>(row: &T) -> Result<Vec<u8>, Fail> {
    serde_json::to_vec(row).map_err(|e| Fail::new(FAIL_ROW_DECODE, e.to_string()))
}

/// the run rows this op has already rewritten, read ahead of the store so a
/// second fact about one run in the same op sees the first.
type Touched = BTreeMap<String, RunView>;

fn load_run(
    read: &impl StateRead,
    touched: &Touched,
    dispatch_id: &str,
) -> Result<Option<RunView>, Fail> {
    if let Some(run) = touched.get(dispatch_id) {
        return Ok(Some(run.clone()));
    }
    read.get(run_key(dispatch_id).as_bytes())
        .map(|value| decode_row(&value))
        .transpose()
}

fn stamp(op: &OpRow) -> Stamp {
    Stamp {
        height: op.height,
        time: op.time,
    }
}

/// fold one fact into the run row it is about. a fact about a run this
/// index never saw dispatched (the mapper was installed after the dispatch:
/// no backfill) changes no run row; its journal row is still written.
fn fold_fact(
    read: &impl StateRead,
    touched: &mut Touched,
    out: &mut Writes,
    op: &OpRow,
    n: usize,
    event: &RunEvent,
) -> Result<(), Fail> {
    let dispatch_id = dispatch_id_for(&event.run_id);
    index_guest::put(
        out,
        journal_key(&dispatch_id, op, n),
        encode_row(&JournalRow {
            height: op.height,
            time: op.time,
            fact: event.fact.clone(),
        })?,
    );
    let run = match &event.fact {
        RunFact::Dispatched {
            agent_id,
            channel_id,
            anchor_seq,
            job_id,
            delegation_id,
            requester,
        } => {
            index_guest::put(out, recent_key(op, n), dispatch_id.clone());
            index_guest::put(out, agent_key(agent_id, op, n), dispatch_id.clone());
            RunView {
                run_id: event.run_id.clone(),
                dispatch_id: dispatch_id.clone(),
                agent_id: agent_id.clone(),
                channel_id: channel_id.clone(),
                anchor_seq: *anchor_seq,
                job_id: job_id.clone(),
                delegation_id: delegation_id.clone(),
                requester: requester.clone(),
                dispatched: stamp(op),
                state: RunState::Dispatched,
                actions: 0,
                pr_number: None,
            }
        }
        RunFact::SessionOpened { attempt, holder } => {
            let Some(mut run) = load_run(read, touched, &dispatch_id)? else {
                return Ok(());
            };
            run.state = RunState::Running {
                attempt: *attempt,
                holder: holder.clone(),
            };
            run
        }
        RunFact::Acted { .. } => {
            let Some(mut run) = load_run(read, touched, &dispatch_id)? else {
                return Ok(());
            };
            run.actions += 1;
            run
        }
        RunFact::Settled {
            outcome,
            reason,
            degraded,
            executing_node,
            output_ref,
            pr_number,
        } => {
            let Some(mut run) = load_run(read, touched, &dispatch_id)? else {
                return Ok(());
            };
            run.state = RunState::Settled {
                outcome: *outcome,
                reason: reason.clone(),
                degraded: *degraded,
                executing_node: executing_node.clone(),
                output_ref: output_ref.clone(),
                at: stamp(op),
            };
            run.pr_number = pr_number.or(run.pr_number);
            run
        }
        RunFact::ResultActionRefused { .. } => {
            let Some(mut run) = load_run(read, touched, &dispatch_id)? else {
                return Ok(());
            };
            // the module's own rule: a refusal cannot erase a failure, and a
            // later success cannot erase a refusal.
            if let RunState::Settled {
                outcome: outcome @ RunOutcome::ResultAccepted,
                ..
            } = &mut run.state
            {
                *outcome = RunOutcome::ActionRejected;
            }
            run
        }
        RunFact::PrLinked { number } => {
            let Some(mut run) = load_run(read, touched, &dispatch_id)? else {
                return Ok(());
            };
            run.pr_number = Some(*number);
            run
        }
    };
    index_guest::put(out, run_key(&dispatch_id), encode_row(&run)?);
    touched.insert(dispatch_id, run);
    Ok(())
}

/// fold one applied op into derived writes: every fact its stamp carries,
/// in order. an applied op passed the module's own validation (a failed op
/// aborts its unit and never reaches the feed), so the fold mirrors the
/// transitions without re-judging them.
pub fn fold_op(op: &OpRow, read: &impl StateRead) -> Result<Writes, Fail> {
    let mut out = Writes::new();
    if op.assigned.is_empty() {
        return Ok(out);
    }
    let journal = decode_assigned(&op.assigned).map_err(|e| Fail::new(FAIL_ASSIGNED_DECODE, e))?;
    let mut touched = Touched::new();
    for (n, event) in journal.iter().enumerate() {
        fold_fact(read, &mut touched, &mut out, op, n, event)?;
    }
    Ok(out)
}

/// the runs an ordinal index lists, newest first, resolved to their rows.
fn runs_under(read: &impl StateRead, prefix: &str, limit: usize) -> Result<Vec<RunView>, Fail> {
    let page = read.scan_page(prefix.as_bytes(), None, limit.clamp(1, MAX_SCAN_LIMIT));
    let mut runs = Vec::with_capacity(page.entries.len());
    for (_key, dispatch_id) in &page.entries {
        let dispatch_id = String::from_utf8_lossy(dispatch_id);
        if let Some(run) = load_run(read, &Touched::new(), &dispatch_id)? {
            runs.push(run);
        }
    }
    Ok(runs)
}

/// a run's whole journal in commit order, page after page.
fn journal_of(read: &impl StateRead, dispatch_id: &str) -> Result<Vec<JournalRow>, Fail> {
    let prefix = journal_prefix(dispatch_id);
    let mut rows = Vec::new();
    let mut after: Option<Vec<u8>> = None;
    loop {
        let page = read.scan_page(prefix.as_bytes(), after.as_deref(), MAX_SCAN_LIMIT);
        for (_key, value) in &page.entries {
            rows.push(decode_row(value)?);
        }
        if !page.has_more {
            return Ok(rows);
        }
        after = page.next_after.map(String::into_bytes);
    }
}

/// serve one materialized-view request.
pub fn serve_view(read: &impl StateRead, req: &[u8]) -> Result<Vec<u8>, Fail> {
    let query: RunsViewQuery =
        serde_json::from_slice(req).map_err(|e| Fail::new(FAIL_BAD_REQUEST, e.to_string()))?;
    let reply = match query {
        RunsViewQuery::Recent { agent_id, limit } => {
            let prefix = match agent_id {
                Some(agent_id) => agent_prefix(&agent_id),
                None => RECENT_PREFIX.to_string(),
            };
            RunsViewReply::Runs(runs_under(read, &prefix, limit.unwrap_or(MAX_SCAN_LIMIT))?)
        }
        RunsViewQuery::Run { run_id } => {
            let dispatch_id = dispatch_id_for(&run_id);
            let detail = match load_run(read, &Touched::new(), &dispatch_id)? {
                Some(run) => Some(Box::new(RunDetail {
                    journal: journal_of(read, &dispatch_id)?,
                    run,
                })),
                None => None,
            };
            RunsViewReply::Run(detail)
        }
    };
    serde_json::to_vec(&reply).map_err(|e| Fail::new(FAIL_BAD_REQUEST, e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encode_assigned;
    use index_guest::{OriginTag, apply_to_map};

    type Map = BTreeMap<Vec<u8>, Vec<u8>>;

    const RUN: &str = "chat\x1fgeneral\x1f2\x1fbot";
    const OTHER: &str = "chat\x1fgeneral\x1f3\x1fduck";

    fn op(height: u64, seq: u32, journal: &[RunEvent]) -> OpRow {
        OpRow {
            height,
            seq,
            time: height * 10,
            origin: OriginTag::external("node"),
            payload: Vec::new(),
            assigned: match journal.is_empty() {
                true => Vec::new(),
                false => encode_assigned(journal),
            },
        }
    }

    fn event(run_id: &str, fact: RunFact) -> RunEvent {
        RunEvent {
            run_id: run_id.into(),
            fact,
        }
    }

    fn dispatched(agent_id: &str, anchor_seq: u64) -> RunFact {
        RunFact::Dispatched {
            agent_id: agent_id.into(),
            channel_id: "general".into(),
            anchor_seq,
            job_id: None,
            delegation_id: None,
            requester: RunOrigin::External(vec![7; 32]),
        }
    }

    fn settled(outcome: RunOutcome) -> RunFact {
        RunFact::Settled {
            outcome,
            reason: None,
            degraded: false,
            executing_node: "ab".into(),
            output_ref: None,
            pr_number: None,
        }
    }

    fn fold(map: &mut Map, op: &OpRow) {
        let writes = fold_op(op, map).expect("fold");
        apply_to_map(map, writes);
    }

    fn recent(map: &Map, agent_id: Option<&str>) -> Vec<RunView> {
        let req = serde_json::json!({"recent": {"agent_id": agent_id}});
        let reply: RunsViewReply =
            serde_json::from_slice(&serve_view(map, &serde_json::to_vec(&req).unwrap()).unwrap())
                .unwrap();
        match reply {
            RunsViewReply::Runs(runs) => runs,
            other => panic!("unexpected reply: {other:?}"),
        }
    }

    fn detail(map: &Map, run_id: &str) -> Option<RunDetail> {
        let req = serde_json::json!({"run": {"run_id": run_id}});
        let reply: RunsViewReply =
            serde_json::from_slice(&serve_view(map, &serde_json::to_vec(&req).unwrap()).unwrap())
                .unwrap();
        match reply {
            RunsViewReply::Run(detail) => detail.map(|detail| *detail),
            other => panic!("unexpected reply: {other:?}"),
        }
    }

    #[test]
    fn a_run_folds_from_dispatch_through_settlement_and_keeps_its_journal() {
        let mut map = Map::new();
        fold(&mut map, &op(1, 0, &[event(RUN, dispatched("bot", 2))]));
        let run = &recent(&map, None)[0];
        assert_eq!(run.state, RunState::Dispatched);
        assert_eq!(
            run.dispatched,
            Stamp {
                height: 1,
                time: 10
            }
        );
        assert_eq!(run.dispatch_id, dispatch_id_for(RUN));

        fold(
            &mut map,
            &op(
                2,
                3,
                &[event(
                    RUN,
                    RunFact::SessionOpened {
                        attempt: 0,
                        holder: "ab".into(),
                    },
                )],
            ),
        );
        for (seq, request) in ["r0", "r1"].into_iter().enumerate() {
            fold(
                &mut map,
                &op(
                    3,
                    seq as u32,
                    &[event(
                        RUN,
                        RunFact::Acted {
                            request_id: request.into(),
                            lane: crate::LaneKind::Live,
                            operation: "chat.react".into(),
                            result: serde_json::Value::Null,
                        },
                    )],
                ),
            );
        }
        let run = &recent(&map, None)[0];
        assert_eq!(
            run.state,
            RunState::Running {
                attempt: 0,
                holder: "ab".into()
            }
        );
        assert_eq!(run.actions, 2);

        fold(
            &mut map,
            &op(4, 0, &[event(RUN, settled(RunOutcome::ResultAccepted))]),
        );
        let known = detail(&map, RUN).expect("the run is known");
        let RunState::Settled { outcome, at, .. } = &known.run.state else {
            panic!("settled: {:?}", known.run.state);
        };
        assert_eq!(*outcome, RunOutcome::ResultAccepted);
        assert_eq!(
            *at,
            Stamp {
                height: 4,
                time: 40
            }
        );
        let facts: Vec<(u64, &str)> = known
            .journal
            .iter()
            .map(|row| {
                let name = match &row.fact {
                    RunFact::Dispatched { .. } => "dispatched",
                    RunFact::SessionOpened { .. } => "session_opened",
                    RunFact::Acted { .. } => "acted",
                    RunFact::Settled { .. } => "settled",
                    RunFact::ResultActionRefused { .. } => "result_action_refused",
                    RunFact::PrLinked { .. } => "pr_linked",
                };
                (row.height, name)
            })
            .collect();
        assert_eq!(
            facts,
            [
                (1, "dispatched"),
                (2, "session_opened"),
                (3, "acted"),
                (3, "acted"),
                (4, "settled")
            ]
        );
        assert_eq!(detail(&map, "chat\x1fnowhere\x1f9\x1fbot"), None);
    }

    #[test]
    fn recent_lists_newest_dispatch_first_and_narrows_to_an_agent() {
        let mut map = Map::new();
        fold(&mut map, &op(5, 0, &[event(RUN, dispatched("bot", 2))]));
        // two runs staged by one op keep their in-op order, newest first
        fold(
            &mut map,
            &op(
                7,
                1,
                &[
                    event(OTHER, dispatched("duck", 3)),
                    event("chat\x1fgeneral\x1f3\x1fbot", dispatched("bot", 3)),
                ],
            ),
        );
        let ids: Vec<String> = recent(&map, None)
            .into_iter()
            .map(|run| run.run_id)
            .collect();
        assert_eq!(ids, ["chat\x1fgeneral\x1f3\x1fbot", OTHER, RUN]);
        let bots: Vec<String> = recent(&map, Some("bot"))
            .into_iter()
            .map(|run| run.run_id)
            .collect();
        assert_eq!(bots, ["chat\x1fgeneral\x1f3\x1fbot", RUN]);
        assert!(recent(&map, Some("nobody")).is_empty());
    }

    #[test]
    fn a_settled_run_takes_its_pr_link_and_a_result_action_refusal() {
        let mut map = Map::new();
        fold(&mut map, &op(1, 0, &[event(RUN, dispatched("bot", 2))]));
        fold(
            &mut map,
            &op(2, 0, &[event(RUN, settled(RunOutcome::ResultAccepted))]),
        );
        fold(
            &mut map,
            &op(3, 0, &[event(RUN, RunFact::PrLinked { number: 12 })]),
        );
        let run = &recent(&map, None)[0];
        assert_eq!(run.pr_number, Some(12));
        fold(
            &mut map,
            &op(
                4,
                0,
                &[event(
                    RUN,
                    RunFact::ResultActionRefused {
                        request_id: "result/x/0".into(),
                    },
                )],
            ),
        );
        let run = &recent(&map, None)[0];
        assert!(
            matches!(
                &run.state,
                RunState::Settled {
                    outcome: RunOutcome::ActionRejected,
                    ..
                }
            ),
            "{:?}",
            run.state
        );
        // a failure is never upgraded by a refusal
        fold(&mut map, &op(5, 0, &[event(OTHER, dispatched("duck", 3))]));
        fold(
            &mut map,
            &op(6, 0, &[event(OTHER, settled(RunOutcome::Failed))]),
        );
        fold(
            &mut map,
            &op(
                7,
                0,
                &[event(
                    OTHER,
                    RunFact::ResultActionRefused {
                        request_id: "result/y/0".into(),
                    },
                )],
            ),
        );
        let other = detail(&map, OTHER).unwrap().run;
        assert!(matches!(
            other.state,
            RunState::Settled {
                outcome: RunOutcome::Failed,
                ..
            }
        ));
    }

    #[test]
    fn an_op_that_moved_no_run_and_a_fact_about_an_unseen_run_write_only_what_they_can() {
        let mut map = Map::new();
        assert!(fold_op(&op(1, 0, &[]), &map).unwrap().is_empty());
        // installed after the dispatch: the journal row lands, no run row
        fold(
            &mut map,
            &op(2, 0, &[event(RUN, settled(RunOutcome::Failed))]),
        );
        assert!(recent(&map, None).is_empty());
        assert_eq!(detail(&map, RUN), None);
        assert_eq!(journal_of(&map, &dispatch_id_for(RUN)).unwrap().len(), 1);
    }
}
