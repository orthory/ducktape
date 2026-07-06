//! golden fixtures: the scripted step list a capsule ships at
//! `harness/golden.json`, executed by [`run_golden`] — deterministic by
//! construction (heights auto-increment with each submitted block, the
//! oracle is canned data, no wall clock, no randomness). the same script a
//! package author's crate test drives is what the CLI's `package test`
//! replays against the binary's native module catalog.

use std::collections::BTreeMap;
use std::fmt;

use host::SubmitError;
use jobs::JobStatus;
use quack::Capsule;
use sdk::Origin;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::install::{InstallReport, install_spec_from_capsule_defaulted};
use crate::testbed::PackageTestBed;

/// where the fixture lives inside a capsule.
pub const GOLDEN_PATH: &str = "harness/golden.json";
/// the fixture schema this build understands.
pub const GOLDEN_SCHEMA_V1: u32 = 1;

/// the capsule's scripted harness proof: which package it drives, which
/// logical module is the harness, the logical -> concrete id bindings the
/// install resolves through (empty = every module's manifest `default_id`),
/// and the ordered steps.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoldenFixture {
    pub schema: u32,
    pub package: String,
    /// the harness module's LOGICAL id. optional: when omitted, the install
    /// step resolves it from the manifest's top-level `harness` key; when
    /// present it overrides that key.
    #[serde(default)]
    pub harness: Option<String>,
    #[serde(default)]
    pub bindings: BTreeMap<String, String>,
    pub steps: Vec<GoldenStep>,
}

impl GoldenFixture {
    /// parse fixture bytes STRICTLY: unknown step kinds, unknown fields, and
    /// unsupported schemas all reject.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let fixture: GoldenFixture =
            serde_json::from_slice(bytes).map_err(|e| format!("golden fixture: {e}"))?;
        if fixture.schema != GOLDEN_SCHEMA_V1 {
            return Err(format!(
                "unsupported golden schema {} (this build understands {GOLDEN_SCHEMA_V1})",
                fixture.schema
            ));
        }
        Ok(fixture)
    }

    /// read the fixture from a capsule's `harness/golden.json`.
    pub fn from_capsule(capsule: &Capsule) -> Result<Self, String> {
        let bytes = capsule
            .files
            .get(GOLDEN_PATH)
            .ok_or_else(|| format!("capsule has no {GOLDEN_PATH}"))?;
        Self::parse(bytes)
    }
}

/// whether a `submit` step expects its block to commit or reject.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubmitExpect {
    #[default]
    Ok,
    Rejected,
}

/// one scripted step. origins are written `"external:<lowercase hex>"` or
/// `"module:<id>"` (see [`parse_origin`]); payloads/queries/expectations are
/// the modules' canonical serde_json wire values, verbatim.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum GoldenStep {
    /// install the capsule under the fixture's harness + bindings.
    Install { origin: String },
    /// submit one op as its own block.
    Submit {
        origin: String,
        target: String,
        payload: Value,
        #[serde(default)]
        expect: SubmitExpect,
    },
    /// answer the oldest pending dispatch with canned provider output:
    /// exactly one of `response` (a strict `AgentResponse` JSON value,
    /// submitted as the raw model text) or `error` (a provider failure).
    Oracle {
        #[serde(default)]
        response: Option<Value>,
        #[serde(default)]
        error: Option<String>,
    },
    /// advance one benign block — what injects committed deliveries.
    Deliver {},
    /// assert the jobs board: jobs matching the selectors number `count`.
    ExpectJob {
        #[serde(default)]
        job_id: Option<String>,
        #[serde(default)]
        kind_prefix: Option<String>,
        #[serde(default)]
        status: Option<String>,
        count: u64,
    },
    /// query a module and compare the reply as canonical JSON (structural
    /// equality; mismatches report the first divergent path).
    ExpectQuery {
        module: String,
        query: Value,
        expect: Value,
    },
    /// assert a pending (not-yet-delivered) run matching the selectors
    /// exists (or does not). at least one selector is required.
    ExpectRun {
        #[serde(default)]
        run_id: Option<String>,
        #[serde(default)]
        agent: Option<String>,
        exists: bool,
    },
    /// assert the MOST RECENTLY COMMITTED block recorded a failure
    /// breadcrumb: an event from `source` whose text contains `contains`.
    ExpectFailureRow { source: String, contains: String },
    /// run the full snapshot round-trip sweep at the current boundary.
    SnapshotRoundtrip {},
}

impl GoldenStep {
    fn label(&self) -> &'static str {
        match self {
            GoldenStep::Install { .. } => "install",
            GoldenStep::Submit { .. } => "submit",
            GoldenStep::Oracle { .. } => "oracle",
            GoldenStep::Deliver {} => "deliver",
            GoldenStep::ExpectJob { .. } => "expect_job",
            GoldenStep::ExpectQuery { .. } => "expect_query",
            GoldenStep::ExpectRun { .. } => "expect_run",
            GoldenStep::ExpectFailureRow { .. } => "expect_failure_row",
            GoldenStep::SnapshotRoundtrip {} => "snapshot_roundtrip",
        }
    }
}

/// a failed golden run: which step (1-based), what kind, and why.
#[derive(Clone, Debug)]
pub struct GoldenError {
    pub step: usize,
    pub label: String,
    pub message: String,
}

impl fmt::Display for GoldenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "step {} ({}): {}", self.step, self.label, self.message)
    }
}

impl std::error::Error for GoldenError {}

/// a completed golden run: the executed step labels (the CLI's pass table)
/// and the install step's report, when the script installed.
#[derive(Debug)]
pub struct GoldenRun {
    pub steps: Vec<String>,
    pub install: Option<InstallReport>,
}

/// execute `fixture` against `bed`, step by step, failing fast with the
/// 1-based step index and a readable message.
pub async fn run_golden(
    bed: &mut PackageTestBed,
    capsule: &Capsule,
    fixture: &GoldenFixture,
) -> Result<GoldenRun, GoldenError> {
    let mut run = GoldenRun {
        steps: Vec::new(),
        install: None,
    };
    for (index, step) in fixture.steps.iter().enumerate() {
        let fail = |message: String| GoldenError {
            step: index + 1,
            label: step.label().to_string(),
            message,
        };
        match step {
            GoldenStep::Install { origin } => {
                let origin = parse_origin(origin).map_err(&fail)?;
                // the fixture must drive the capsule it ships in: a package
                // id mismatch is a fixture bug, caught before any block.
                let spec = install_spec_from_capsule_defaulted(
                    capsule,
                    fixture.harness.as_deref(),
                    &fixture.bindings,
                )
                .map_err(&fail)?;
                if spec.package != fixture.package {
                    return Err(fail(format!(
                        "fixture drives package {:?} but the capsule manifest declares {:?}",
                        fixture.package, spec.package
                    )));
                }
                let report = bed
                    .install_capsule(capsule, &spec.harness, &fixture.bindings, origin)
                    .await
                    .map_err(&fail)?;
                run.install = Some(report);
            }
            GoldenStep::Submit {
                origin,
                target,
                payload,
                expect,
            } => {
                let origin = parse_origin(origin).map_err(&fail)?;
                let outcome = bed.submit_json(origin, target, payload).await;
                match (outcome, expect) {
                    (Ok(_), SubmitExpect::Ok) => {}
                    (Ok(_), SubmitExpect::Rejected) => {
                        return Err(fail(
                            "the block committed but the fixture expected rejection".into(),
                        ));
                    }
                    (Err(SubmitError::Rejected(_)), SubmitExpect::Rejected) => {}
                    (Err(e), _) => return Err(fail(format!("block rejected: {e:?}"))),
                }
            }
            GoldenStep::Oracle { response, error } => {
                let outcome = match (response, error) {
                    (Some(value), None) => {
                        Ok(serde_json::to_vec(value).expect("a json value serializes"))
                    }
                    (None, Some(reason)) => Err(reason.clone()),
                    _ => {
                        return Err(fail(
                            "oracle step needs exactly one of `response` or `error`".into(),
                        ));
                    }
                };
                bed.oracle(outcome).await.map_err(&fail)?;
            }
            GoldenStep::Deliver {} => {
                bed.deliver().await.map_err(&fail)?;
            }
            GoldenStep::ExpectJob {
                job_id,
                kind_prefix,
                status,
                count,
            } => {
                let want_status = match status.as_deref() {
                    None => None,
                    Some("pending") => Some(JobStatus::Pending),
                    Some("processing") => Some(JobStatus::Processing),
                    Some("done") => Some(JobStatus::Done),
                    Some("failed") => Some(JobStatus::Failed),
                    Some("cancelled") => Some(JobStatus::Cancelled),
                    Some(other) => {
                        return Err(fail(format!("unknown job status selector {other:?}")));
                    }
                };
                let jobs = bed
                    .jobs_matching(kind_prefix.as_deref().unwrap_or(""))
                    .await;
                let matching: Vec<_> = jobs
                    .iter()
                    .filter(|j| job_id.as_deref().is_none_or(|id| j.job_id == id))
                    .filter(|j| want_status.as_ref().is_none_or(|s| j.status == *s))
                    .collect();
                if matching.len() as u64 != *count {
                    return Err(fail(format!(
                        "expected {count} jobs matching job_id={job_id:?} \
                         kind_prefix={kind_prefix:?} status={status:?}, found {}: {:?}",
                        matching.len(),
                        matching
                            .iter()
                            .map(|j| j.job_id.as_str())
                            .collect::<Vec<_>>()
                    )));
                }
            }
            GoldenStep::ExpectQuery {
                module,
                query,
                expect,
            } => {
                let actual = bed.query_json(module, query).await.map_err(&fail)?;
                if let Some(diff) = diff_json(expect, &actual) {
                    return Err(fail(format!("{module} reply mismatch: {diff}")));
                }
            }
            GoldenStep::ExpectRun {
                run_id,
                agent,
                exists,
            } => {
                if run_id.is_none() && agent.is_none() {
                    return Err(fail(
                        "expect_run needs at least one selector (`run_id` or `agent`)".into(),
                    ));
                }
                let runs = bed.pending_runs().await;
                let found = runs.iter().any(|r| {
                    run_id.as_deref().is_none_or(|id| r.run_id == id)
                        && agent.as_deref().is_none_or(|a| r.agent_id == a)
                });
                if found != *exists {
                    return Err(fail(format!(
                        "pending run matching run_id={run_id:?} agent={agent:?}: expected \
                         exists={exists}, pending runs: {:?}",
                        runs.iter().map(|r| r.run_id.as_str()).collect::<Vec<_>>()
                    )));
                }
            }
            GoldenStep::ExpectFailureRow { source, contains } => {
                if !bed.has_failure_breadcrumb(source, contains) {
                    return Err(fail(format!(
                        "no event from {source:?} containing {contains:?} in the last block; \
                         events: {:?}",
                        bed.last_events()
                    )));
                }
            }
            GoldenStep::SnapshotRoundtrip {} => {
                bed.snapshot_roundtrip_all().await.map_err(&fail)?;
            }
        }
        run.steps.push(step.label().to_string());
    }
    Ok(run)
}

/// parse a fixture origin string STRICTLY: `"external:<lowercase hex>"` (a
/// non-empty, even-length key) or `"module:<id>"` (a non-empty id within the
/// platform's module-id byte bound). every other form rejects.
pub fn parse_origin(s: &str) -> Result<Origin, String> {
    if let Some(hex) = s.strip_prefix("external:") {
        if hex.is_empty()
            || !hex.len().is_multiple_of(2)
            || !hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(format!(
                "malformed external origin {s:?} (want non-empty, even-length lowercase hex)"
            ));
        }
        let key = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("validated hex"))
            .collect();
        return Ok(Origin::External(key));
    }
    if let Some(id) = s.strip_prefix("module:") {
        if id.is_empty() || id.len() > package::MAX_MODULE_ID_BYTES {
            return Err(format!("malformed module origin {s:?}"));
        }
        return Ok(Origin::Module(id.to_string()));
    }
    Err(format!(
        "unknown origin form {s:?} (want \"external:<hex>\" or \"module:<id>\")"
    ))
}

/// structural JSON comparison with a readable verdict: `None` when equal,
/// else the FIRST divergent path (json-path style, `$` = root) with both
/// sides rendered.
pub fn diff_json(expected: &Value, actual: &Value) -> Option<String> {
    fn walk(path: &str, expected: &Value, actual: &Value) -> Option<String> {
        match (expected, actual) {
            (Value::Object(e), Value::Object(a)) => {
                let mut keys: Vec<&String> = e.keys().chain(a.keys()).collect();
                keys.sort();
                keys.dedup();
                for key in keys {
                    let child = format!("{path}.{key}");
                    match (e.get(key), a.get(key)) {
                        (Some(ev), Some(av)) => {
                            if let Some(diff) = walk(&child, ev, av) {
                                return Some(diff);
                            }
                        }
                        (Some(ev), None) => {
                            return Some(format!(
                                "at {child}: expected {}, got nothing",
                                short(ev)
                            ));
                        }
                        (None, Some(av)) => {
                            return Some(format!(
                                "at {child}: expected nothing, got {}",
                                short(av)
                            ));
                        }
                        (None, None) => unreachable!("key came from one of the maps"),
                    }
                }
                None
            }
            (Value::Array(e), Value::Array(a)) => {
                for (i, (ev, av)) in e.iter().zip(a.iter()).enumerate() {
                    if let Some(diff) = walk(&format!("{path}[{i}]"), ev, av) {
                        return Some(diff);
                    }
                }
                if e.len() != a.len() {
                    return Some(format!(
                        "at {path}: expected an array of {}, got {}",
                        e.len(),
                        a.len()
                    ));
                }
                None
            }
            _ => (expected != actual).then(|| {
                format!(
                    "at {path}: expected {}, got {}",
                    short(expected),
                    short(actual)
                )
            }),
        }
    }

    /// render a value compactly, truncated so a mismatch stays one readable line.
    fn short(value: &Value) -> String {
        let mut s = value.to_string();
        if s.len() > 120 {
            s.truncate(117);
            s.push_str("...");
        }
        s
    }

    walk("$", expected, actual)
}
