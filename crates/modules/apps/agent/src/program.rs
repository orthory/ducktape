//! the program interpreter: validation of a submitted program, and the pure
//! evaluation of one invocation over its frame.
//!
//! evaluation DECIDES and never writes: it reads siblings through [`Reads`],
//! binds names in the frame it was handed, collects the reports the program
//! emits, and ends in one [`End`] — a request to wait on, a finish, or a
//! failure. the module stages and emits what the decision says, afterwards.
//!
//! control moves forward only. [`validate_program`] refuses a branch or a
//! failure continuation that names the current or an earlier step, so a run
//! from step `s` visits each step after `s` at most once and is bounded by
//! the program's length. the store holds validated programs only.
//!
//! every value that crosses to a sibling is rendered into JSON and encoded
//! canonically (object keys sorted), so the bytes a call carries are the same
//! on every build whatever map the JSON library was compiled with.

use std::collections::{BTreeMap, BTreeSet};

use attribution::{Actor, AttributionMsg, Change, ObjectRef, Relation};
use borsh::{BorshDeserialize, BorshSerialize};
use sdk::{AccountNumber, Cause, Error, ModuleId};
use serde::Serialize;

use crate::{
    CallOutcome, CallResult, Continuation, Decode, DispatchResult, Failure, JsonValue, Predicate,
    Program, ProgramFault, REF_ACCOUNT, REF_CAUSE, REF_CHANGE, REPORT_KIND, REPORT_REVISION,
    RESERVED_ROOTS, Reason, Step, Value,
};

/// the field separator inside composite keys (the shared [`sdk::KEY_SEP`]),
/// refused inside every caller-chosen identifier a key is built from.
const SEP: char = sdk::KEY_SEP;

// ---- the frame ------------------------------------------------------------------

/// one bound name's fact, held verbatim as it arrived: a sibling's reply
/// bytes, a call's outcome, a dispatch's result. its JSON is derived on
/// reference, so the store never holds a re-encoding.
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq, Eq)]
pub(crate) enum Fact {
    Reply {
        bytes: Vec<u8>,
    },
    Call {
        outcome: CallOutcome,
        decode: Decode,
    },
    Dispatch {
        outcome: Result<Vec<u8>, String>,
        decode: Decode,
    },
}

/// everything a step may reference: the account, the change that invoked it,
/// the context the change was delivered under, and every bound fact.
pub(crate) struct Frame {
    pub account: AccountNumber,
    pub seq: u64,
    pub cause: Cause,
    pub change: Change,
    pub facts: BTreeMap<String, Fact>,
}

// ---- how an invocation's evaluation ends ------------------------------------------

/// the sibling request a run stopped at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Request {
    Call { target: ModuleId, payload: Vec<u8> },
    Dispatch { recipe_id: String, payload: Vec<u8> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum End {
    /// the run stopped at `step` to wait on `request`.
    Await {
        step: u64,
        request: Request,
    },
    Finished {
        at_step: u64,
    },
    Failed {
        step: u64,
        failure: Failure,
    },
}

/// one evaluation's whole decision: the reports it emitted, in step order,
/// and how it ended. the frame it ran over holds its bindings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Run {
    pub reports: Vec<AttributionMsg>,
    pub end: End,
}

/// the sibling reads a decision may make — a query step's only effect.
#[async_trait::async_trait(?Send)]
pub(crate) trait Reads {
    async fn read(&self, module: &str, req: &[u8]) -> Result<Vec<u8>, Error>;
}

// ---- validation (pure) ------------------------------------------------------------

fn module_error(text: impl Into<String>) -> Error {
    Error::Module(text.into())
}

fn validate_ident(field: &str, value: &str) -> Result<(), Error> {
    if value.is_empty() {
        return Err(module_error(format!("{field} must be non-empty")));
    }
    if value.contains(SEP) {
        return Err(module_error(format!(
            "{field} must not contain the reserved separator"
        )));
    }
    Ok(())
}

/// a number renders only within what JSON wire numbers hold.
fn number_renders(number: i128) -> bool {
    let fits_unsigned = (0..=i128::from(u64::MAX)).contains(&number);
    let fits_signed = (i128::from(i64::MIN)..0).contains(&number);
    fits_unsigned || fits_signed
}

/// a value's static shape: numbers in range, references rooted at a frame
/// root or a name bound by an earlier step.
fn validate_value(step: u64, value: &Value, bound: &BTreeSet<&str>) -> Result<(), Error> {
    match value {
        Value::Null | Value::Bool(_) | Value::Text(_) | Value::Bytes(_) => Ok(()),
        Value::Number(number) => match number_renders(*number) {
            true => Ok(()),
            false => Err(module_error(format!(
                "step {step}: number {number} is outside the JSON integer range"
            ))),
        },
        Value::List(items) => items
            .iter()
            .try_for_each(|item| validate_value(step, item, bound)),
        Value::Map(entries) => entries
            .values()
            .try_for_each(|entry| validate_value(step, entry, bound)),
        Value::Ref(path) => {
            let Some(root) = path.first() else {
                return Err(module_error(format!(
                    "step {step}: a reference has an empty path"
                )));
            };
            let is_frame_root = RESERVED_ROOTS.contains(&root.as_str());
            let is_bound_earlier = bound.contains(root.as_str());
            let resolvable = is_frame_root || is_bound_earlier;
            if !resolvable {
                return Err(module_error(format!(
                    "step {step}: reference {root:?} names neither a frame root nor a name bound by an earlier step"
                )));
            }
            Ok(())
        }
    }
}

fn validate_predicate(
    step: u64,
    predicate: &Predicate,
    bound: &BTreeSet<&str>,
) -> Result<(), Error> {
    match predicate {
        Predicate::Equals { left, right } => {
            validate_value(step, left, bound)?;
            validate_value(step, right, bound)
        }
        Predicate::Defined(value) => validate_value(step, value, bound),
        Predicate::Not(inner) => validate_predicate(step, inner, bound),
        Predicate::All(items) | Predicate::Any(items) => items
            .iter()
            .try_for_each(|item| validate_predicate(step, item, bound)),
    }
}

/// a target is a later step, or the end of the program.
fn validate_target(step: u64, target: u64, len: u64) -> Result<(), Error> {
    let moves_forward = target > step;
    let within_program = target <= len;
    let valid = moves_forward && within_program;
    if !valid {
        return Err(module_error(format!(
            "step {step} targets step {target}; a target is a later step, or {len} for the end"
        )));
    }
    Ok(())
}

fn validate_continuation(step: u64, continuation: &Continuation, len: u64) -> Result<(), Error> {
    match continuation {
        Continuation::Step(target) => validate_target(step, *target, len),
        Continuation::Unhandled => Ok(()),
    }
}

fn validate_bind(step: u64, bind: &str) -> Result<(), Error> {
    validate_ident(&format!("step {step}: bind"), bind)?;
    let shadows_a_root = RESERVED_ROOTS.contains(&bind);
    if shadows_a_root {
        return Err(module_error(format!(
            "step {step}: bind {bind:?} is a frame root"
        )));
    }
    Ok(())
}

fn validate_reason(step: u64, reason: &Reason) -> Result<(), Error> {
    match reason {
        Reason::Defined(name) => validate_ident(&format!("step {step}: defined reason"), name),
        Reason::Mention
        | Reason::Authorship
        | Reason::Ownership
        | Reason::Assignment
        | Reason::Credit
        | Reason::Result
        | Reason::Report => Ok(()),
    }
}

/// a program as the module accepts it off the wire: every target a later
/// step or the end, every reference resolvable by construction, every module
/// a sibling (a program cannot query its own executor: that read is refused
/// by the host), every literal renderable.
pub(crate) fn validate_program(program: &Program, executor: &str) -> Result<(), Error> {
    let len = program.steps.len() as u64;
    let mut bound: BTreeSet<&str> = BTreeSet::new();
    for (index, step) in program.steps.iter().enumerate() {
        let at = index as u64;
        match step {
            Step::Query {
                module,
                query,
                bind,
            } => {
                validate_ident(&format!("step {at}: module"), module)?;
                let queries_the_executor = module == executor;
                if queries_the_executor {
                    return Err(module_error(format!(
                        "step {at}: a program cannot query {executor}, its own executor"
                    )));
                }
                validate_value(at, query, &bound)?;
                validate_bind(at, bind)?;
                bound.insert(bind);
            }
            Step::Call {
                module,
                msg,
                bind,
                decode: _,
                on_failure,
            } => {
                validate_ident(&format!("step {at}: module"), module)?;
                validate_value(at, msg, &bound)?;
                validate_continuation(at, on_failure, len)?;
                validate_bind(at, bind)?;
                bound.insert(bind);
            }
            Step::Dispatch {
                recipe_id,
                payload,
                bind,
                decode: _,
                on_failure,
            } => {
                validate_ident(&format!("step {at}: recipe_id"), recipe_id)?;
                validate_value(at, payload, &bound)?;
                validate_continuation(at, on_failure, len)?;
                validate_bind(at, bind)?;
                bound.insert(bind);
            }
            Step::Branch { test, then, or } => {
                validate_predicate(at, test, &bound)?;
                validate_target(at, *then, len)?;
                validate_target(at, *or, len)?;
            }
            Step::Report {
                recipient,
                reason,
                detail,
            } => {
                validate_value(at, recipient, &bound)?;
                validate_reason(at, reason)?;
                validate_value(at, detail, &bound)?;
            }
            Step::Finish => {}
        }
    }
    Ok(())
}

// ---- decoding (pure) ---------------------------------------------------------------

/// JSON holds integers exactly and floats approximately; only the former
/// may enter a deterministic evaluation.
fn integer_only(value: &JsonValue) -> bool {
    match value {
        JsonValue::Number(number) => !number.is_f64(),
        JsonValue::Array(items) => items.iter().all(integer_only),
        JsonValue::Object(entries) => entries.values().all(integer_only),
        JsonValue::Null | JsonValue::Bool(_) | JsonValue::String(_) => true,
    }
}

/// wire bytes as frame JSON: empty is `null`, anything else must parse as
/// integer-only JSON.
fn decode_json(what: &str, bytes: &[u8]) -> Result<JsonValue, ProgramFault> {
    if bytes.is_empty() {
        return Ok(JsonValue::Null);
    }
    let value: JsonValue =
        serde_json::from_slice(bytes).map_err(|e| ProgramFault::Undecodable {
            what: what.into(),
            detail: e.to_string(),
        })?;
    if !integer_only(&value) {
        return Err(ProgramFault::Undecodable {
            what: what.into(),
            detail: "a number is not an integer".into(),
        });
    }
    Ok(value)
}

/// opaque bytes in JSON's byte form: an array of numbers. the one form a
/// [`Value::Bytes`] literal and a [`Decode::Bytes`] output share, so a bound
/// output re-renders into a later call as the bytes it was.
fn bytes_json(bytes: &[u8]) -> JsonValue {
    JsonValue::Array(bytes.iter().map(|byte| JsonValue::from(*byte)).collect())
}

fn decode_bytes(what: &str, decode: Decode, bytes: &[u8]) -> Result<JsonValue, ProgramFault> {
    match decode {
        Decode::Json => decode_json(what, bytes),
        Decode::Bytes => Ok(bytes_json(bytes)),
        Decode::Text => String::from_utf8(bytes.to_vec())
            .map(JsonValue::String)
            .map_err(|e| ProgramFault::Undecodable {
                what: what.into(),
                detail: e.to_string(),
            }),
    }
}

fn to_json<T: Serialize>(value: &T) -> Result<JsonValue, ProgramFault> {
    serde_json::to_value(value).map_err(|e| ProgramFault::Unrenderable {
        detail: e.to_string(),
    })
}

/// the JSON a fact reads as: a reply verbatim, an outcome as its
/// [`CallResult`] or [`DispatchResult`] with the declared decoding applied.
pub(crate) fn fact_json(fact: &Fact) -> Result<JsonValue, ProgramFault> {
    match fact {
        Fact::Reply { bytes } => decode_json("reply", bytes),
        Fact::Call { outcome, decode } => {
            let result = match outcome {
                CallOutcome::Applied { output, assigned } => CallResult::Applied {
                    output: decode_bytes("output", *decode, output)?,
                    assigned: decode_json("assigned stamp", assigned)?,
                },
                CallOutcome::Rejected { reason } => CallResult::Rejected {
                    reason: reason.clone(),
                },
                CallOutcome::Refused(refusal) => CallResult::Refused(*refusal),
                CallOutcome::Unrepresentable { attempted } => CallResult::Unrepresentable {
                    attempted: *attempted,
                },
            };
            to_json(&result)
        }
        Fact::Dispatch { outcome, decode } => {
            let result = match outcome {
                Ok(output) => DispatchResult::Completed {
                    output: decode_bytes("result", *decode, output)?,
                },
                Err(reason) => DispatchResult::Failed {
                    reason: reason.clone(),
                },
            };
            to_json(&result)
        }
    }
}

// ---- rendering and resolution (pure) --------------------------------------------------

/// the same JSON with every object's keys in sorted order, whatever map the
/// library keeps them in — the one encoding every build agrees on.
fn canonical(value: JsonValue) -> JsonValue {
    match value {
        JsonValue::Object(entries) => {
            let mut sorted: Vec<(String, JsonValue)> = entries.into_iter().collect();
            sorted.sort_by(|a, b| a.0.cmp(&b.0));
            JsonValue::Object(
                sorted
                    .into_iter()
                    .map(|(key, entry)| (key, canonical(entry)))
                    .collect(),
            )
        }
        JsonValue::Array(items) => JsonValue::Array(items.into_iter().map(canonical).collect()),
        scalar => scalar,
    }
}

/// the wire bytes of a rendered value.
pub(crate) fn encode_canonical(value: JsonValue) -> Vec<u8> {
    sdk::wire::encode(&canonical(value))
}

fn unresolved(path: &[String]) -> ProgramFault {
    ProgramFault::Unresolved {
        path: path.to_vec(),
    }
}

/// walk `path` into the frame: the root, then object keys and array indexes.
pub(crate) fn resolve(frame: &Frame, path: &[String]) -> Result<JsonValue, ProgramFault> {
    let Some((root, rest)) = path.split_first() else {
        return Err(unresolved(path));
    };
    let mut value = match root.as_str() {
        REF_CHANGE => to_json(&frame.change)?,
        REF_CAUSE => to_json(&frame.cause)?,
        REF_ACCOUNT => JsonValue::from(frame.account),
        name => {
            let fact = frame.facts.get(name).ok_or_else(|| unresolved(path))?;
            fact_json(fact)?
        }
    };
    for segment in rest {
        value = match value {
            JsonValue::Object(mut entries) => {
                entries.remove(segment).ok_or_else(|| unresolved(path))?
            }
            JsonValue::Array(mut items) => {
                let index: usize = segment.parse().map_err(|_| unresolved(path))?;
                let in_range = index < items.len();
                if !in_range {
                    return Err(unresolved(path));
                }
                items.swap_remove(index)
            }
            _ => return Err(unresolved(path)),
        };
    }
    Ok(value)
}

fn render_number(number: i128) -> Result<JsonValue, ProgramFault> {
    if let Ok(unsigned) = u64::try_from(number) {
        return Ok(JsonValue::from(unsigned));
    }
    if let Ok(signed) = i64::try_from(number) {
        return Ok(JsonValue::from(signed));
    }
    Err(ProgramFault::Unrenderable {
        detail: format!("number {number} is outside the JSON integer range"),
    })
}

/// a template as the JSON it stands for, references spliced in.
pub(crate) fn render(frame: &Frame, value: &Value) -> Result<JsonValue, ProgramFault> {
    match value {
        Value::Null => Ok(JsonValue::Null),
        Value::Bool(flag) => Ok(JsonValue::Bool(*flag)),
        Value::Number(number) => render_number(*number),
        Value::Text(text) => Ok(JsonValue::String(text.clone())),
        Value::Bytes(bytes) => Ok(bytes_json(bytes)),
        Value::List(items) => Ok(JsonValue::Array(
            items
                .iter()
                .map(|item| render(frame, item))
                .collect::<Result<_, _>>()?,
        )),
        Value::Map(entries) => {
            let mut object = serde_json::Map::new();
            for (key, entry) in entries {
                object.insert(key.clone(), render(frame, entry)?);
            }
            Ok(JsonValue::Object(object))
        }
        Value::Ref(path) => resolve(frame, path),
    }
}

/// whether a predicate holds over the frame.
pub(crate) fn holds(frame: &Frame, predicate: &Predicate) -> Result<bool, ProgramFault> {
    match predicate {
        Predicate::Equals { left, right } => Ok(render(frame, left)? == render(frame, right)?),
        Predicate::Defined(value) => match render(frame, value) {
            Ok(rendered) => Ok(!rendered.is_null()),
            Err(ProgramFault::Unresolved { .. }) => Ok(false),
            Err(fault) => Err(fault),
        },
        Predicate::Not(inner) => Ok(!holds(frame, inner)?),
        Predicate::All(items) => {
            for item in items {
                if !holds(frame, item)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        Predicate::Any(items) => {
            for item in items {
                if holds(frame, item)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
    }
}

// ---- evaluation ------------------------------------------------------------------------

/// the attribution object one report step of one invocation writes.
pub(crate) fn report_object(account: AccountNumber, seq: u64, step: u64) -> String {
    format!("{account}/{seq}/{step}")
}

fn report(
    frame: &Frame,
    step: u64,
    recipient: &Value,
    reason: &Reason,
    detail: &Value,
) -> Result<AttributionMsg, ProgramFault> {
    let rendered = render(frame, recipient)?;
    let names_an_account = rendered.as_u64().filter(|number| *number != 0);
    let recipient = names_an_account.ok_or_else(|| ProgramFault::Recipient {
        rendered: rendered.to_string(),
    })?;
    let detail = encode_canonical(render(frame, detail)?);
    Ok(AttributionMsg::Attribute {
        object: ObjectRef {
            kind: REPORT_KIND.into(),
            object: report_object(frame.account, frame.seq, step),
        },
        revision: REPORT_REVISION,
        actor: Actor::Account(frame.account),
        relations: vec![Relation {
            recipient,
            reason: reason.clone(),
            detail,
        }],
        transfers: Vec::new(),
    })
}

async fn query(
    reads: &dyn Reads,
    frame: &mut Frame,
    module: &str,
    query: &Value,
    bind: &str,
) -> Result<(), ProgramFault> {
    let req = encode_canonical(render(frame, query)?);
    let bytes = reads
        .read(module, &req)
        .await
        .map_err(|error| ProgramFault::Query {
            module: module.into(),
            error: error.to_string(),
        })?;
    // decoded now, so no later reference can fault on a reply the program
    // already bound.
    decode_json(&format!("reply from {module}"), &bytes)?;
    frame.facts.insert(bind.into(), Fact::Reply { bytes });
    Ok(())
}

/// what one step decided: the next step, or where the run stops.
enum Flow {
    Next(u64),
    Stop(End),
}

async fn advance(
    reads: &dyn Reads,
    program: &Program,
    frame: &mut Frame,
    step: u64,
    reports: &mut Vec<AttributionMsg>,
) -> Result<Flow, ProgramFault> {
    let Some(current) = program.steps.get(step as usize) else {
        return Ok(Flow::Stop(End::Finished { at_step: step }));
    };
    match current {
        Step::Query {
            module,
            query: template,
            bind,
        } => {
            query(reads, frame, module, template, bind).await?;
            Ok(Flow::Next(step + 1))
        }
        Step::Call { module, msg, .. } => {
            let payload = encode_canonical(render(frame, msg)?);
            Ok(Flow::Stop(End::Await {
                step,
                request: Request::Call {
                    target: module.clone(),
                    payload,
                },
            }))
        }
        Step::Dispatch {
            recipe_id, payload, ..
        } => {
            let payload = encode_canonical(render(frame, payload)?);
            Ok(Flow::Stop(End::Await {
                step,
                request: Request::Dispatch {
                    recipe_id: recipe_id.clone(),
                    payload,
                },
            }))
        }
        Step::Branch { test, then, or } => match holds(frame, test)? {
            true => Ok(Flow::Next(*then)),
            false => Ok(Flow::Next(*or)),
        },
        Step::Report {
            recipient,
            reason,
            detail,
        } => {
            reports.push(report(frame, step, recipient, reason, detail)?);
            Ok(Flow::Next(step + 1))
        }
        Step::Finish => Ok(Flow::Stop(End::Finished { at_step: step })),
    }
}

/// evaluate `program` over `frame` from step `from` until it waits, finishes
/// or fails. reports emitted before a fault are kept: they happened.
pub(crate) async fn run(reads: &dyn Reads, program: &Program, frame: &mut Frame, from: u64) -> Run {
    let mut reports = Vec::new();
    let mut step = from;
    loop {
        match advance(reads, program, frame, step, &mut reports).await {
            Ok(Flow::Next(next)) => step = next,
            Ok(Flow::Stop(end)) => return Run { reports, end },
            Err(fault) => {
                return Run {
                    reports,
                    end: End::Failed {
                        step,
                        failure: Failure::Program(fault),
                    },
                };
            }
        }
    }
}

/// the step the invocation waited at, as the resumption needs it.
struct Waiting<'a> {
    bind: &'a str,
    decode: Decode,
    on_failure: &'a Continuation,
}

fn waiting_call(program: &Program, step: u64) -> Result<Waiting<'_>, Error> {
    match program.steps.get(step as usize) {
        Some(Step::Call {
            bind,
            decode,
            on_failure,
            ..
        }) => Ok(Waiting {
            bind,
            decode: *decode,
            on_failure,
        }),
        _ => Err(module_error(format!(
            "invocation waits at step {step}, which is not a call of its program"
        ))),
    }
}

fn waiting_dispatch(program: &Program, step: u64) -> Result<Waiting<'_>, Error> {
    match program.steps.get(step as usize) {
        Some(Step::Dispatch {
            bind,
            decode,
            on_failure,
            ..
        }) => Ok(Waiting {
            bind,
            decode: *decode,
            on_failure,
        }),
        _ => Err(module_error(format!(
            "invocation waits at step {step}, which is not a dispatch of its program"
        ))),
    }
}

/// the answer to the one request an invocation waits on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Answer {
    Call(CallOutcome),
    Dispatch(Result<Vec<u8>, String>),
}

impl Answer {
    /// the step that must be waiting for this answer, as the resumption
    /// needs it.
    fn waiting<'a>(&self, program: &'a Program, step: u64) -> Result<Waiting<'a>, Error> {
        match self {
            Answer::Call(_) => waiting_call(program, step),
            Answer::Dispatch(_) => waiting_dispatch(program, step),
        }
    }

    /// whether the request applied — the success path.
    fn applied(&self) -> bool {
        match self {
            Answer::Call(outcome) => matches!(outcome, CallOutcome::Applied { .. }),
            Answer::Dispatch(outcome) => outcome.is_ok(),
        }
    }

    /// the failure an invocation ends with when its program has no handler.
    fn unhandled(&self) -> Failure {
        match self {
            Answer::Call(outcome) => Failure::UnhandledCall(outcome.clone()),
            Answer::Dispatch(outcome) => Failure::UnhandledDispatch {
                reason: outcome.clone().err().unwrap_or_default(),
            },
        }
    }

    /// the fact the frame binds, under the step's declared decoding.
    fn fact(self, decode: Decode) -> Fact {
        match self {
            Answer::Call(outcome) => Fact::Call { outcome, decode },
            Answer::Dispatch(outcome) => Fact::Dispatch { outcome, decode },
        }
    }
}

/// resume an invocation waiting at `step` with the answer to its request:
/// bind the fact and continue at the next step when it applied, else at the
/// step's failure continuation. a fact whose declared decoding fails is a
/// fault at this step and is not bound. `Err` is a program that does not
/// hold the matching request at that step — a corrupt record, never a
/// program fault.
pub(crate) async fn resume(
    reads: &dyn Reads,
    program: &Program,
    frame: &mut Frame,
    step: u64,
    answer: Answer,
) -> Result<Run, Error> {
    let waiting = answer.waiting(program, step)?;
    let applied = answer.applied();
    let unhandled = answer.unhandled();
    let fact = answer.fact(waiting.decode);
    if let Err(fault) = fact_json(&fact) {
        return Ok(Run {
            reports: Vec::new(),
            end: End::Failed {
                step,
                failure: Failure::Program(fault),
            },
        });
    }
    frame.facts.insert(waiting.bind.into(), fact);
    let next = match (applied, waiting.on_failure) {
        (true, _) => step + 1,
        (false, Continuation::Step(target)) => *target,
        (false, Continuation::Unhandled) => {
            return Ok(Run {
                reports: Vec::new(),
                end: End::Failed {
                    step,
                    failure: unhandled,
                },
            });
        }
    };
    Ok(run(reads, program, frame, next).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use attribution::{ChangeKind, Source};
    use dispatch::Refusal;
    use futures::executor::block_on;
    use sdk::{Hop, ItemRef, Root};
    use std::cell::RefCell;

    const ALICE: AccountNumber = 1;
    const BOT: AccountNumber = 2;

    fn change(seq: u64, actor: Actor, reason: Reason) -> Change {
        Change {
            seq,
            source: Source {
                module: "chat".into(),
                kind: "message".into(),
                object: "m1".into(),
            },
            revision: 3,
            recipient: BOT,
            reason,
            kind: ChangeKind::Added,
            detail: b"{\"at\":4}".to_vec(),
            actor,
            cause: Cause::Direct,
            height: 10,
        }
    }

    fn delivered() -> Cause {
        let item = ItemRef {
            source: "attribution".into(),
            item: 5,
        };
        Cause::Chain {
            root: Root::Item(item.clone()),
            hop: Hop::Delivery(item),
        }
    }

    fn frame() -> Frame {
        Frame {
            account: BOT,
            seq: 7,
            cause: delivered(),
            change: change(7, Actor::Account(ALICE), Reason::Mention),
            facts: BTreeMap::new(),
        }
    }

    fn path(segments: &[&str]) -> Vec<String> {
        segments.iter().map(|s| s.to_string()).collect()
    }

    fn reference(segments: &[&str]) -> Value {
        Value::Ref(path(segments))
    }

    /// a scripted sibling: module → the reply it gives, recording requests.
    struct Siblings {
        replies: BTreeMap<String, Result<Vec<u8>, Error>>,
        requests: RefCell<Vec<(String, Vec<u8>)>>,
    }

    impl Siblings {
        fn answering(module: &str, reply: Result<Vec<u8>, Error>) -> Self {
            Self {
                replies: BTreeMap::from([(module.to_string(), reply)]),
                requests: RefCell::new(Vec::new()),
            }
        }
    }

    #[async_trait::async_trait(?Send)]
    impl Reads for Siblings {
        async fn read(&self, module: &str, req: &[u8]) -> Result<Vec<u8>, Error> {
            self.requests
                .borrow_mut()
                .push((module.to_string(), req.to_vec()));
            match self.replies.get(module) {
                Some(reply) => reply.clone(),
                None => Err(Error::UnknownModule(module.into())),
            }
        }
    }

    fn call_step(module: &str, msg: Value, bind: &str, on_failure: Continuation) -> Step {
        Step::Call {
            module: module.into(),
            msg,
            bind: bind.into(),
            decode: Decode::Json,
            on_failure,
        }
    }

    // ---- validation --------------------------------------------------------------------

    #[test]
    fn validation_refuses_every_malformed_shape() {
        let ok = Program {
            steps: vec![
                Step::Query {
                    module: "chat".into(),
                    query: Value::Null,
                    bind: "chan".into(),
                },
                call_step(
                    "chat",
                    reference(&["chan"]),
                    "posted",
                    Continuation::Step(3),
                ),
                Step::Branch {
                    test: Predicate::Defined(reference(&["posted", "applied"])),
                    then: 3,
                    or: 4,
                },
                Step::Report {
                    recipient: reference(&[REF_CHANGE, "actor", "account"]),
                    reason: Reason::Defined("noted".into()),
                    detail: reference(&[REF_ACCOUNT]),
                },
                Step::Finish,
            ],
        };
        assert_eq!(validate_program(&ok, "agent"), Ok(()));
        assert_eq!(
            validate_program(&Program { steps: vec![] }, "agent"),
            Ok(())
        );

        let refused: Vec<(&str, Program)> = vec![
            (
                "a backward branch",
                Program {
                    steps: vec![
                        Step::Finish,
                        Step::Branch {
                            test: Predicate::Defined(Value::Null),
                            then: 0,
                            or: 2,
                        },
                    ],
                },
            ),
            (
                "a branch to itself",
                Program {
                    steps: vec![Step::Branch {
                        test: Predicate::Defined(Value::Null),
                        then: 0,
                        or: 1,
                    }],
                },
            ),
            (
                "a branch past the end",
                Program {
                    steps: vec![Step::Branch {
                        test: Predicate::Defined(Value::Null),
                        then: 1,
                        or: 2,
                    }],
                },
            ),
            (
                "a backward failure continuation",
                Program {
                    steps: vec![
                        Step::Finish,
                        call_step("chat", Value::Null, "a", Continuation::Step(1)),
                    ],
                },
            ),
            (
                "a reference to a name bound later",
                Program {
                    steps: vec![
                        call_step("chat", reference(&["b"]), "a", Continuation::Unhandled),
                        call_step("chat", Value::Null, "b", Continuation::Unhandled),
                    ],
                },
            ),
            (
                "a reference to nothing",
                Program {
                    steps: vec![call_step(
                        "chat",
                        reference(&["nope", "x"]),
                        "a",
                        Continuation::Unhandled,
                    )],
                },
            ),
            (
                "an empty reference",
                Program {
                    steps: vec![call_step(
                        "chat",
                        reference(&[]),
                        "a",
                        Continuation::Unhandled,
                    )],
                },
            ),
            (
                "a bind over a frame root",
                Program {
                    steps: vec![call_step(
                        "chat",
                        Value::Null,
                        REF_CHANGE,
                        Continuation::Unhandled,
                    )],
                },
            ),
            (
                "an empty bind",
                Program {
                    steps: vec![call_step("chat", Value::Null, "", Continuation::Unhandled)],
                },
            ),
            (
                "a bind with the separator",
                Program {
                    steps: vec![call_step(
                        "chat",
                        Value::Null,
                        "a\x1fb",
                        Continuation::Unhandled,
                    )],
                },
            ),
            (
                "an empty module",
                Program {
                    steps: vec![call_step("", Value::Null, "a", Continuation::Unhandled)],
                },
            ),
            (
                "a query of the executor itself",
                Program {
                    steps: vec![Step::Query {
                        module: "agent".into(),
                        query: Value::Null,
                        bind: "me".into(),
                    }],
                },
            ),
            (
                "an empty recipe id",
                Program {
                    steps: vec![Step::Dispatch {
                        recipe_id: "".into(),
                        payload: Value::Null,
                        bind: "d".into(),
                        decode: Decode::Text,
                        on_failure: Continuation::Unhandled,
                    }],
                },
            ),
            (
                "a number below the JSON range",
                Program {
                    steps: vec![call_step(
                        "chat",
                        Value::Number(i128::from(i64::MIN) - 1),
                        "a",
                        Continuation::Unhandled,
                    )],
                },
            ),
            (
                "a number above the JSON range",
                Program {
                    steps: vec![call_step(
                        "chat",
                        Value::List(vec![Value::Number(i128::from(u64::MAX) + 1)]),
                        "a",
                        Continuation::Unhandled,
                    )],
                },
            ),
            (
                "a nested reference inside a predicate",
                Program {
                    steps: vec![Step::Branch {
                        test: Predicate::Not(Box::new(Predicate::All(vec![Predicate::Equals {
                            left: reference(&["later"]),
                            right: Value::Null,
                        }]))),
                        then: 1,
                        or: 1,
                    }],
                },
            ),
            (
                "an empty defined reason",
                Program {
                    steps: vec![Step::Report {
                        recipient: reference(&[REF_ACCOUNT]),
                        reason: Reason::Defined("".into()),
                        detail: Value::Null,
                    }],
                },
            ),
        ];
        for (name, program) in refused {
            assert!(
                validate_program(&program, "agent").is_err(),
                "{name} must be refused"
            );
        }
    }

    // ---- rendering and resolution -------------------------------------------------------

    #[test]
    fn rendering_splices_frame_values_as_json() {
        let mut frame = frame();
        frame.facts.insert(
            "chan".into(),
            Fact::Reply {
                bytes: br#"{"channel":{"id":"c1","members":[1,2]}}"#.to_vec(),
            },
        );
        let template = Value::Map(BTreeMap::from([
            ("zeta".into(), reference(&["chan", "channel", "id"])),
            (
                "alpha".into(),
                reference(&["chan", "channel", "members", "1"]),
            ),
            ("actor".into(), reference(&[REF_CHANGE, "actor"])),
            ("reason".into(), reference(&[REF_CHANGE, "reason"])),
            ("who".into(), reference(&[REF_ACCOUNT])),
            (
                "hop".into(),
                reference(&[REF_CAUSE, "Chain", "hop", "Delivery", "item"]),
            ),
            ("bytes".into(), Value::Bytes(vec![7, 8])),
            ("neg".into(), Value::Number(-2)),
            ("max".into(), Value::Number(i128::from(u64::MAX))),
            (
                "list".into(),
                Value::List(vec![Value::Bool(false), Value::Null]),
            ),
        ]));
        let rendered = render(&frame, &template).unwrap();
        assert_eq!(
            rendered,
            serde_json::json!({
                "zeta": "c1",
                "alpha": 2,
                "actor": {"account": 1},
                "reason": "mention",
                "who": 2,
                "hop": 5,
                "bytes": [7, 8],
                "neg": -2,
                "max": u64::MAX,
                "list": [false, null],
            })
        );
        // the canonical encoding sorts keys at every depth.
        assert_eq!(
            String::from_utf8(encode_canonical(
                serde_json::json!({"b": {"y": 1, "x": 2}, "a": [ {"q": 1, "p": 2} ]})
            ))
            .unwrap(),
            r#"{"a":[{"p":2,"q":1}],"b":{"x":2,"y":1}}"#
        );
    }

    #[test]
    fn resolution_faults_are_named() {
        let mut frame = frame();
        frame.facts.insert(
            "chan".into(),
            Fact::Reply {
                bytes: br#"{"members":[1]}"#.to_vec(),
            },
        );
        for missing in [
            path(&["nope"]),
            path(&["chan", "absent"]),
            path(&["chan", "members", "1"]),
            path(&["chan", "members", "x"]),
            path(&["chan", "members", "0", "deeper"]),
            path(&[REF_CHANGE, "nope"]),
            path(&[]),
        ] {
            assert_eq!(
                resolve(&frame, &missing),
                Err(ProgramFault::Unresolved {
                    path: missing.clone()
                }),
                "{missing:?}"
            );
        }
    }

    #[test]
    fn facts_decode_explicitly_and_refuse_floats() {
        let applied = Fact::Call {
            outcome: CallOutcome::Applied {
                output: br#"{"id":"m2"}"#.to_vec(),
                assigned: Vec::new(),
            },
            decode: Decode::Json,
        };
        assert_eq!(
            fact_json(&applied).unwrap(),
            serde_json::json!({"applied": {"output": {"id": "m2"}, "assigned": null}})
        );
        let text = Fact::Call {
            outcome: CallOutcome::Applied {
                output: b"plain".to_vec(),
                assigned: br#"{"seq":3}"#.to_vec(),
            },
            decode: Decode::Text,
        };
        assert_eq!(
            fact_json(&text).unwrap(),
            serde_json::json!({"applied": {"output": "plain", "assigned": {"seq": 3}}})
        );
        let not_json = Fact::Call {
            outcome: CallOutcome::Applied {
                output: b"plain".to_vec(),
                assigned: Vec::new(),
            },
            decode: Decode::Json,
        };
        assert!(matches!(
            fact_json(&not_json),
            Err(ProgramFault::Undecodable { what, .. }) if what == "output"
        ));
        let float = Fact::Reply {
            bytes: br#"{"ratio":0.5}"#.to_vec(),
        };
        assert!(matches!(
            fact_json(&float),
            Err(ProgramFault::Undecodable { what, .. }) if what == "reply"
        ));
        let bad_utf8 = Fact::Dispatch {
            outcome: Ok(vec![0xff]),
            decode: Decode::Text,
        };
        assert!(matches!(
            fact_json(&bad_utf8),
            Err(ProgramFault::Undecodable { what, .. }) if what == "result"
        ));
        let refused = Fact::Call {
            outcome: CallOutcome::Refused(Refusal::Revoked),
            decode: Decode::Json,
        };
        assert_eq!(
            fact_json(&refused).unwrap(),
            serde_json::json!({"refused": "revoked"})
        );
        let failed = Fact::Dispatch {
            outcome: Err("timeout".into()),
            decode: Decode::Json,
        };
        assert_eq!(
            fact_json(&failed).unwrap(),
            serde_json::json!({"failed": {"reason": "timeout"}})
        );
    }

    #[test]
    fn predicates_inspect_the_frame() {
        let mut frame = frame();
        frame.facts.insert(
            "posted".into(),
            Fact::Call {
                outcome: CallOutcome::Rejected {
                    reason: "closed".into(),
                },
                decode: Decode::Json,
            },
        );
        let is_mention = Predicate::Equals {
            left: reference(&[REF_CHANGE, "reason"]),
            right: Value::Text("mention".into()),
        };
        let self_authored = Predicate::Equals {
            left: reference(&[REF_CHANGE, "actor"]),
            right: Value::Map(BTreeMap::from([(
                "account".into(),
                reference(&[REF_ACCOUNT]),
            )])),
        };
        let rejected = Predicate::Defined(reference(&["posted", "rejected"]));
        let applied = Predicate::Defined(reference(&["posted", "applied"]));
        let in_a_chain = Predicate::Defined(reference(&[REF_CAUSE, "Chain"]));
        assert_eq!(holds(&frame, &is_mention), Ok(true));
        assert_eq!(holds(&frame, &self_authored), Ok(false));
        assert_eq!(holds(&frame, &rejected), Ok(true));
        assert_eq!(holds(&frame, &applied), Ok(false));
        assert_eq!(holds(&frame, &in_a_chain), Ok(true));
        assert_eq!(
            holds(
                &frame,
                &Predicate::All(vec![
                    is_mention.clone(),
                    Predicate::Not(Box::new(self_authored.clone()))
                ])
            ),
            Ok(true)
        );
        assert_eq!(
            holds(&frame, &Predicate::Any(vec![self_authored, applied])),
            Ok(false)
        );
        // an unresolved reference inside Equals is a fault, not `false`.
        assert!(matches!(
            holds(
                &frame,
                &Predicate::Equals {
                    left: reference(&["posted", "applied", "output"]),
                    right: Value::Null,
                }
            ),
            Err(ProgramFault::Unresolved { .. })
        ));
        frame.change = change(8, Actor::Account(BOT), Reason::Authorship);
        assert_eq!(holds(&frame, &is_mention), Ok(false));
        assert_eq!(
            holds(
                &frame,
                &Predicate::Equals {
                    left: reference(&[REF_CHANGE, "actor"]),
                    right: Value::Map(BTreeMap::from([(
                        "account".into(),
                        reference(&[REF_ACCOUNT])
                    )])),
                }
            ),
            Ok(true)
        );
    }

    // ---- evaluation --------------------------------------------------------------------

    #[test]
    fn a_run_queries_then_stops_at_the_first_call() {
        let program = Program {
            steps: vec![
                Step::Query {
                    module: "chat".into(),
                    query: Value::Map(BTreeMap::from([(
                        "channel".into(),
                        Value::Text("c1".into()),
                    )])),
                    bind: "chan".into(),
                },
                Step::Report {
                    recipient: reference(&[REF_CHANGE, "actor", "account"]),
                    reason: Reason::Report,
                    detail: reference(&["chan", "name"]),
                },
                call_step(
                    "chat",
                    Value::Map(BTreeMap::from([
                        ("channel".into(), reference(&["chan", "id"])),
                        ("text".into(), Value::Text("hi".into())),
                    ])),
                    "posted",
                    Continuation::Unhandled,
                ),
                Step::Finish,
            ],
        };
        let siblings = Siblings::answering("chat", Ok(br#"{"id":"c1","name":"general"}"#.to_vec()));
        let mut frame = frame();
        let run = block_on(run(&siblings, &program, &mut frame, 0));
        assert_eq!(
            siblings.requests.borrow().as_slice(),
            &[("chat".to_string(), br#"{"channel":"c1"}"#.to_vec())]
        );
        assert_eq!(
            run.end,
            End::Await {
                step: 2,
                request: Request::Call {
                    target: "chat".into(),
                    payload: br#"{"channel":"c1","text":"hi"}"#.to_vec(),
                },
            }
        );
        let [
            AttributionMsg::Attribute {
                object,
                revision,
                actor,
                relations,
                transfers,
            },
        ] = run.reports.as_slice()
        else {
            panic!("one report");
        };
        assert_eq!(object.kind, REPORT_KIND);
        assert_eq!(object.object, "2/7/1");
        assert_eq!(*revision, REPORT_REVISION);
        assert_eq!(*actor, Actor::Account(BOT));
        assert_eq!(relations.len(), 1);
        assert_eq!(relations[0].recipient, ALICE);
        assert_eq!(relations[0].reason, Reason::Report);
        assert_eq!(relations[0].detail, br#""general""#);
        assert!(transfers.is_empty());
        assert!(frame.facts.contains_key("chan"));
    }

    /// an opaque output is bound in JSON's byte form, never as text, and a
    /// later call carries those exact bytes: the reference renders to the
    /// same array a `Value::Bytes` literal renders to.
    #[test]
    fn a_binary_output_chains_into_a_later_call() {
        let opaque = vec![0xff, 0x00, 0x9f];
        let program = Program {
            steps: vec![
                Step::Call {
                    module: "blobs".into(),
                    msg: Value::Null,
                    bind: "blob".into(),
                    decode: Decode::Bytes,
                    on_failure: Continuation::Unhandled,
                },
                call_step(
                    "sink",
                    Value::Map(BTreeMap::from([
                        ("data".into(), reference(&["blob", "applied", "output"])),
                        ("literal".into(), Value::Bytes(opaque.clone())),
                    ])),
                    "sunk",
                    Continuation::Unhandled,
                ),
                Step::Finish,
            ],
        };
        validate_program(&program, "agent").unwrap();
        let siblings = Siblings::answering("none", Ok(Vec::new()));
        let mut frame = frame();
        let run = block_on(resume(
            &siblings,
            &program,
            &mut frame,
            0,
            Answer::Call(CallOutcome::Applied {
                output: opaque.clone(),
                assigned: Vec::new(),
            }),
        ))
        .unwrap();
        assert_eq!(
            fact_json(&frame.facts["blob"]).unwrap(),
            serde_json::json!({"applied": {"output": [255, 0, 159], "assigned": null}})
        );
        assert_eq!(
            run.end,
            End::Await {
                step: 1,
                request: Request::Call {
                    target: "sink".into(),
                    payload: br#"{"data":[255,0,159],"literal":[255,0,159]}"#.to_vec(),
                },
            }
        );
        // the same bytes under a text decoding are refused, never a string.
        let as_text = Fact::Call {
            outcome: CallOutcome::Applied {
                output: opaque,
                assigned: Vec::new(),
            },
            decode: Decode::Text,
        };
        assert!(matches!(
            fact_json(&as_text),
            Err(ProgramFault::Undecodable { what, .. }) if what == "output"
        ));
    }

    #[test]
    fn a_resumed_call_binds_its_outcome_and_continues_or_fails() {
        let program = Program {
            steps: vec![
                call_step("chat", Value::Null, "posted", Continuation::Step(2)),
                Step::Finish,
                Step::Report {
                    recipient: reference(&[REF_CHANGE, "actor", "account"]),
                    reason: Reason::Report,
                    detail: reference(&["posted", "rejected", "reason"]),
                },
                call_step(
                    "tasks",
                    reference(&["posted"]),
                    "task",
                    Continuation::Unhandled,
                ),
            ],
        };
        let siblings = Siblings::answering("none", Ok(Vec::new()));

        // applied: next step, which finishes.
        let mut frame = frame();
        let run = block_on(resume(
            &siblings,
            &program,
            &mut frame,
            0,
            Answer::Call(CallOutcome::Applied {
                output: br#""m2""#.to_vec(),
                assigned: Vec::new(),
            }),
        ))
        .unwrap();
        assert_eq!(run.end, End::Finished { at_step: 1 });
        assert!(run.reports.is_empty());

        // rejected: the failure continuation reports and requests recovery.
        let mut frame = self::frame();
        let run = block_on(resume(
            &siblings,
            &program,
            &mut frame,
            0,
            Answer::Call(CallOutcome::Rejected {
                reason: "closed".into(),
            }),
        ))
        .unwrap();
        assert_eq!(run.reports.len(), 1);
        assert_eq!(
            run.end,
            End::Await {
                step: 3,
                request: Request::Call {
                    target: "tasks".into(),
                    payload: br#"{"rejected":{"reason":"closed"}}"#.to_vec(),
                },
            }
        );

        // the recovery call itself rejects without a handler: unhandled, the
        // earlier binding kept.
        let run = block_on(resume(
            &siblings,
            &program,
            &mut frame,
            3,
            Answer::Call(CallOutcome::Refused(Refusal::StaleGeneration)),
        ))
        .unwrap();
        assert_eq!(
            run.end,
            End::Failed {
                step: 3,
                failure: Failure::UnhandledCall(CallOutcome::Refused(Refusal::StaleGeneration)),
            }
        );
        assert!(frame.facts.contains_key("posted"));
        assert!(frame.facts.contains_key("task"));

        // a completion for a step that is not a call is a corrupt record.
        assert!(
            block_on(resume(
                &siblings,
                &program,
                &mut frame,
                1,
                Answer::Call(CallOutcome::Applied {
                    output: Vec::new(),
                    assigned: Vec::new()
                }),
            ))
            .is_err()
        );
    }

    #[test]
    fn faults_end_the_run_at_their_step_keeping_earlier_reports() {
        let program = Program {
            steps: vec![
                Step::Report {
                    recipient: reference(&[REF_CHANGE, "actor", "account"]),
                    reason: Reason::Report,
                    detail: Value::Null,
                },
                call_step(
                    "chat",
                    reference(&[REF_CHANGE, "nope"]),
                    "a",
                    Continuation::Unhandled,
                ),
            ],
        };
        let siblings = Siblings::answering("none", Ok(Vec::new()));
        let mut frame = frame();
        let run = block_on(run(&siblings, &program, &mut frame, 0));
        assert_eq!(run.reports.len(), 1);
        assert_eq!(
            run.end,
            End::Failed {
                step: 1,
                failure: Failure::Program(ProgramFault::Unresolved {
                    path: path(&[REF_CHANGE, "nope"]),
                }),
            }
        );

        // a sibling that errors, a reply that is not JSON, a recipient that
        // is not an account, and a text output where JSON was declared.
        let erroring = Program {
            steps: vec![Step::Query {
                module: "chat".into(),
                query: Value::Null,
                bind: "c".into(),
            }],
        };
        let refusing = Siblings::answering("chat", Err(Error::Module("closed".into())));
        let run = block_on(super::run(&refusing, &erroring, &mut self::frame(), 0));
        assert!(matches!(
            run.end,
            End::Failed { step: 0, failure: Failure::Program(ProgramFault::Query { ref module, .. }) } if module == "chat"
        ));
        let garbling = Siblings::answering("chat", Ok(b"not json".to_vec()));
        let run = block_on(super::run(&garbling, &erroring, &mut self::frame(), 0));
        assert!(matches!(
            run.end,
            End::Failed {
                step: 0,
                failure: Failure::Program(ProgramFault::Undecodable { .. })
            }
        ));
        let bad_recipient = Program {
            steps: vec![Step::Report {
                recipient: Value::Text("alice".into()),
                reason: Reason::Report,
                detail: Value::Null,
            }],
        };
        let run = block_on(super::run(&siblings, &bad_recipient, &mut self::frame(), 0));
        assert_eq!(
            run.end,
            End::Failed {
                step: 0,
                failure: Failure::Program(ProgramFault::Recipient {
                    rendered: "\"alice\"".into()
                }),
            }
        );
        let zero_recipient = Program {
            steps: vec![Step::Report {
                recipient: Value::Number(0),
                reason: Reason::Report,
                detail: Value::Null,
            }],
        };
        let run = block_on(super::run(
            &siblings,
            &zero_recipient,
            &mut self::frame(),
            0,
        ));
        assert!(matches!(
            run.end,
            End::Failed {
                step: 0,
                failure: Failure::Program(ProgramFault::Recipient { .. })
            }
        ));
        let mut frame = self::frame();
        let run = block_on(resume(
            &siblings,
            &Program {
                steps: vec![call_step("chat", Value::Null, "a", Continuation::Unhandled)],
            },
            &mut frame,
            0,
            Answer::Call(CallOutcome::Applied {
                output: b"plain text".to_vec(),
                assigned: Vec::new(),
            }),
        ))
        .unwrap();
        assert!(matches!(
            run.end,
            End::Failed {
                step: 0,
                failure: Failure::Program(ProgramFault::Undecodable { .. })
            }
        ));
        assert!(frame.facts.is_empty(), "an undecodable fact is not bound");
    }

    #[test]
    fn branches_and_dispatches_move_forward_only() {
        let program = Program {
            steps: vec![
                Step::Branch {
                    test: Predicate::Equals {
                        left: reference(&[REF_CHANGE, "reason"]),
                        right: Value::Text("mention".into()),
                    },
                    then: 2,
                    or: 1,
                },
                Step::Finish,
                Step::Dispatch {
                    recipe_id: "summarize".into(),
                    payload: reference(&[REF_CHANGE, "detail"]),
                    bind: "summary".into(),
                    decode: Decode::Text,
                    on_failure: Continuation::Step(4),
                },
                Step::Finish,
                Step::Report {
                    recipient: reference(&[REF_CHANGE, "actor", "account"]),
                    reason: Reason::Report,
                    detail: reference(&["summary", "failed", "reason"]),
                },
            ],
        };
        let siblings = Siblings::answering("none", Ok(Vec::new()));
        let mut frame = frame();
        let run = block_on(run(&siblings, &program, &mut frame, 0));
        assert_eq!(
            run.end,
            End::Await {
                step: 2,
                request: Request::Dispatch {
                    recipe_id: "summarize".into(),
                    payload: b"[123,34,97,116,34,58,52,125]".to_vec(),
                },
            }
        );
        let run = block_on(resume(
            &siblings,
            &program,
            &mut frame,
            2,
            Answer::Dispatch(Ok(b"a summary".to_vec())),
        ))
        .unwrap();
        assert_eq!(run.end, End::Finished { at_step: 3 });
        assert_eq!(
            fact_json(&frame.facts["summary"]).unwrap(),
            serde_json::json!({"completed": {"output": "a summary"}})
        );
        let run = block_on(resume(
            &siblings,
            &program,
            &mut self::frame(),
            2,
            Answer::Dispatch(Err("timeout".into())),
        ))
        .unwrap();
        assert_eq!(run.reports.len(), 1);
        assert_eq!(run.end, End::Finished { at_step: 5 });

        // a self-authored change takes the `or` branch and finishes at once.
        let mut own = self::frame();
        own.change = change(8, Actor::Account(BOT), Reason::Authorship);
        let run = block_on(super::run(&siblings, &program, &mut own, 0));
        assert_eq!(run.end, End::Finished { at_step: 1 });

        // an empty program finishes at step 0; running off the end finishes
        // at the length.
        let run = block_on(super::run(
            &siblings,
            &Program { steps: vec![] },
            &mut self::frame(),
            0,
        ));
        assert_eq!(run.end, End::Finished { at_step: 0 });
        let tail = Program {
            steps: vec![Step::Report {
                recipient: reference(&[REF_ACCOUNT]),
                reason: Reason::Report,
                detail: Value::Null,
            }],
        };
        let run = block_on(super::run(&siblings, &tail, &mut self::frame(), 0));
        assert_eq!(run.end, End::Finished { at_step: 1 });
    }
}
