//! `[session]` — thread-continuity plumbing for one executor capability.
//!
//! agentic CLIs keep their own conversation state keyed by a session id.
//! this module lets a spec say how to CAPTURE that id from a run's stdout
//! and how to build the argv that RESUMES it, and gives [`crate::CliProvider`]
//! a tiny host-local store `<sessions_root>/<agent_id>/<sha256(thread_key)>`
//! mapping a consensus thread key to the executor's session id.
//!
//! this is host-local plumbing, NOT the removed consensus model routing:
//! nothing here touches consensus, and the `{session_id}` slot is the one
//! documented substitution in the whole spec format — filled host-side with
//! an id the executor itself minted, never with job content. sessions are
//! also ASSIGNEE-LOCAL by design: another node executing the same thread's
//! next run finds no session file and simply starts cold, which the prompt
//! envelope (full transcript every run) already makes correct.
//!
//! failure posture: capture is tolerant (no id in the output = no store,
//! never an error), and a resume that fails degrades to ONE cold retry after
//! deleting the stale session file — an expired executor session must never
//! break an agent.

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest as _, Sha256};

/// the documented single substitution slot in resume argv.
const SESSION_ID_SLOT: &str = "{session_id}";

/// a captured session id is written to disk and substituted into argv —
/// bound and shape-check it so a weird stdout can only ever produce "no
/// capture", not a corrupt store or a mangled argv.
const MAX_SESSION_ID_BYTES: usize = 128;

/// a validated `[session]` section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSpec {
    /// how the session id is read out of a successful run's stdout.
    pub capture: SessionCapture,
    /// how a stored session id turns into the resuming invocation's argv.
    pub resume: ResumeArgv,
}

/// the named capture modes. a CLOSED set like `[output].format`: each name
/// is a tested parser for a real CLI's output contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionCapture {
    /// a JSONL event stream: the first event carrying a session identity
    /// wins (`thread.started`'s `thread_id`, a top-level `session_id`, or
    /// the older `msg.session_configured` envelope's `session_id`).
    JsonlEvents,
    /// the single `{"type":"result",...}` object's named string field.
    JsonResultField(String),
}

/// how the resume argv is built from the spec and the stored session id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResumeArgv {
    /// a FULL replacement argv (for CLIs where resuming is a different
    /// subcommand), with `{session_id}` substituted.
    Replace(Vec<String>),
    /// the spec's own args plus these (for CLIs where resuming is a flag),
    /// with `{session_id}` substituted — inherits each variant's model and
    /// effort flags for free because the base argv is the variant's own.
    Append(Vec<String>),
}

/// the on-disk `[session]` shape — a dumb serde mirror; unknown fields fail
/// loud like everywhere else in the spec format.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawSession {
    capture: String,
    #[serde(default)]
    resume_args: Option<Vec<String>>,
    #[serde(default)]
    resume_args_append: Option<Vec<String>>,
}

/// validate one `[session]` section: a known capture mode plus EXACTLY one
/// resume style carrying the `{session_id}` slot.
pub(crate) fn parse_session(raw: &RawSession, origin: &str) -> Result<SessionSpec, String> {
    let capture = match raw.capture.as_str() {
        "jsonl-events" => SessionCapture::JsonlEvents,
        other => match other.strip_prefix("json-result-field:") {
            Some(field) if !field.is_empty() => SessionCapture::JsonResultField(field.into()),
            _ => {
                return Err(format!(
                    "{origin}: session.capture {other:?} is not a known mode \
                     (want jsonl-events | json-result-field:<field>)"
                ));
            }
        },
    };
    let resume = match (&raw.resume_args, &raw.resume_args_append) {
        (Some(args), None) => ResumeArgv::Replace(args.clone()),
        (None, Some(args)) => ResumeArgv::Append(args.clone()),
        // both or neither is operator confusion, not a precedence question.
        _ => {
            return Err(format!(
                "{origin}: [session] needs exactly one of resume_args (full \
                 replacement) or resume_args_append"
            ));
        }
    };
    validate_slot(resume.args(), origin)?;
    Ok(SessionSpec { capture, resume })
}

impl ResumeArgv {
    fn args(&self) -> &[String] {
        match self {
            ResumeArgv::Replace(args) | ResumeArgv::Append(args) => args,
        }
    }
}

/// a resume argv that never uses the session id cannot resume anything —
/// reject it at parse time, where the operator can see it.
pub(crate) fn validate_slot(args: &[String], origin: &str) -> Result<(), String> {
    if args.iter().any(|a| a.contains(SESSION_ID_SLOT)) {
        return Ok(());
    }
    Err(format!(
        "{origin}: resume argv must carry the {SESSION_ID_SLOT} slot"
    ))
}

/// compose the resuming invocation's argv: substitute the slot, appending to
/// `base_args` for the flag style.
pub(crate) fn resume_argv(
    base_args: &[String],
    resume: &ResumeArgv,
    session_id: &str,
) -> Vec<String> {
    let fill = |args: &[String]| -> Vec<String> {
        args.iter()
            .map(|a| a.replace(SESSION_ID_SLOT, session_id))
            .collect()
    };
    match resume {
        ResumeArgv::Replace(args) => fill(args),
        ResumeArgv::Append(extra) => {
            let mut argv = base_args.to_vec();
            argv.extend(fill(extra));
            argv
        }
    }
}

/// pull a session id out of one successful run's stdout, per the capture
/// mode. tolerant by contract: any shape surprise is `None`, never an error
/// — a run that answered fine but exposed no id just stays cold.
pub(crate) fn capture_session_id(capture: &SessionCapture, stdout: &str) -> Option<String> {
    let found = match capture {
        SessionCapture::JsonlEvents => capture_jsonl(stdout),
        SessionCapture::JsonResultField(field) => capture_result_field(stdout, field),
    };
    found.filter(|id| plausible_session_id(id))
}

/// the JSONL scan, tolerant of the shapes seen in the wild (mirrors the
/// output parser's posture): `{"type":"thread.started","thread_id":…}` (the
/// current codex stream — verified live against codex-cli 0.142), any event
/// with a top-level `session_id`, and the older `msg` envelope's
/// `session_configured`.
fn capture_jsonl(stdout: &str) -> Option<String> {
    for line in stdout.lines() {
        let line = line.trim();
        if !line.starts_with('{') {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if v.get("type").and_then(Value::as_str) == Some("thread.started")
            && let Some(id) = v.get("thread_id").and_then(Value::as_str)
        {
            return Some(id.to_string());
        }
        if let Some(id) = v.get("session_id").and_then(Value::as_str) {
            return Some(id.to_string());
        }
        if let Some(msg) = v.get("msg")
            && msg.get("type").and_then(Value::as_str) == Some("session_configured")
            && let Some(id) = msg.get("session_id").and_then(Value::as_str)
        {
            return Some(id.to_string());
        }
    }
    None
}

/// the named string field of the single result object — the same candidate
/// scan `parse_json_result` uses (whole output first, then per-line against
/// banner noise), minus the error posture.
fn capture_result_field(stdout: &str, field: &str) -> Option<String> {
    let candidates = std::iter::once(stdout.trim()).chain(stdout.lines().rev().map(str::trim));
    for candidate in candidates {
        let Ok(v) = serde_json::from_str::<Value>(candidate) else {
            continue;
        };
        if v.get("type").and_then(Value::as_str) != Some("result") {
            continue;
        }
        return v.get(field).and_then(Value::as_str).map(str::to_string);
    }
    None
}

/// a captured id lands in argv and on disk — accept only short, printable,
/// space-free tokens (uuids and thread names both pass).
fn plausible_session_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_SESSION_ID_BYTES
        && id.bytes().all(|b| b.is_ascii_graphic())
}

/// one (agent, thread) slot in the host-local session store.
///
/// the file is `<root>/<agent_id>/<sha256(thread_key) hex>`: the agent id is
/// caller-validated as a path component ([`crate::workspace::safe_path_component`]),
/// and hashing the thread key makes ANY key content filesystem-safe.
pub(crate) struct SessionStore<'a> {
    root: &'a Path,
    agent_id: &'a str,
    thread_key: &'a str,
}

impl<'a> SessionStore<'a> {
    pub(crate) fn new(root: &'a Path, agent_id: &'a str, thread_key: &'a str) -> Self {
        Self {
            root,
            agent_id,
            thread_key,
        }
    }

    fn path(&self) -> PathBuf {
        let mut h = Sha256::new();
        h.update(self.thread_key.as_bytes());
        let digest = h.finalize();
        let mut name = String::with_capacity(64);
        for b in digest {
            name.push_str(&format!("{b:02x}"));
        }
        self.root.join(self.agent_id).join(name)
    }

    /// the stored session id, if a valid one is on disk. an unreadable or
    /// implausible file reads as "no session" — the cold path is always safe.
    pub(crate) fn load(&self) -> Option<String> {
        let id = std::fs::read_to_string(self.path()).ok()?;
        let id = id.trim();
        plausible_session_id(id).then(|| id.to_string())
    }

    /// capture from `stdout` and persist. BEST-EFFORT on purpose: the run
    /// already succeeded, and a session is an optimization — a store failure
    /// warns and costs continuity, never the result. runs on every success
    /// (resumed too) so a CLI that rotates ids stays resumable.
    pub(crate) fn store_captured(&self, capture: &SessionCapture, stdout: &str) {
        let Some(id) = capture_session_id(capture, stdout) else {
            return; // no id in the output = no store, by contract.
        };
        let path = self.path();
        let write = || -> std::io::Result<()> {
            std::fs::create_dir_all(path.parent().expect("session file has a parent"))?;
            std::fs::write(&path, format!("{id}\n"))
        };
        if let Err(e) = write() {
            eprintln!(
                "[capability-host] session store {} failed ({e}); the next \
                 run of this thread starts cold",
                path.display()
            );
        }
    }

    /// drop a stale session so the retry — and every later run — goes cold
    /// instead of re-hitting a dead id.
    pub(crate) fn forget(&self) {
        let _ = std::fs::remove_file(self.path());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(capture: &str, resume: Option<&[&str]>, append: Option<&[&str]>) -> RawSession {
        let own = |a: Option<&[&str]>| a.map(|a| a.iter().map(|s| s.to_string()).collect());
        RawSession {
            capture: capture.into(),
            resume_args: own(resume),
            resume_args_append: own(append),
        }
    }

    #[test]
    fn session_sections_parse_both_capture_modes_and_one_resume_style() {
        let s = parse_session(
            &raw("jsonl-events", Some(&["resume", "{session_id}"]), None),
            "t",
        )
        .unwrap();
        assert_eq!(s.capture, SessionCapture::JsonlEvents);
        assert!(matches!(s.resume, ResumeArgv::Replace(_)));

        let s = parse_session(
            &raw(
                "json-result-field:session_id",
                None,
                Some(&["--resume", "{session_id}"]),
            ),
            "t",
        )
        .unwrap();
        assert_eq!(
            s.capture,
            SessionCapture::JsonResultField("session_id".into())
        );
        assert!(matches!(s.resume, ResumeArgv::Append(_)));
    }

    #[test]
    fn malformed_session_sections_fail_by_name() {
        let err = parse_session(&raw("csv", Some(&["{session_id}"]), None), "t").unwrap_err();
        assert!(err.contains("not a known mode"), "got {err:?}");

        let err =
            parse_session(&raw("json-result-field:", Some(&["{session_id}"]), None), "t")
                .unwrap_err();
        assert!(err.contains("not a known mode"), "got {err:?}");

        // neither resume style, and both, are refused.
        let err = parse_session(&raw("jsonl-events", None, None), "t").unwrap_err();
        assert!(err.contains("exactly one"), "got {err:?}");
        let err = parse_session(
            &raw("jsonl-events", Some(&["{session_id}"]), Some(&["x"])),
            "t",
        )
        .unwrap_err();
        assert!(err.contains("exactly one"), "got {err:?}");

        // a resume argv without the slot can never resume — parse-time error.
        let err = parse_session(&raw("jsonl-events", Some(&["resume"]), None), "t").unwrap_err();
        assert!(err.contains("{session_id}"), "got {err:?}");
    }

    #[test]
    fn resume_argv_substitutes_the_slot_in_both_styles() {
        let base = vec!["run".to_string(), "--json".to_string()];
        let argv = resume_argv(
            &base,
            &ResumeArgv::Replace(vec!["resume".into(), "{session_id}".into(), "-".into()]),
            "sid-1",
        );
        assert_eq!(argv, ["resume", "sid-1", "-"]);

        let argv = resume_argv(
            &base,
            &ResumeArgv::Append(vec!["--resume".into(), "{session_id}".into()]),
            "sid-1",
        );
        assert_eq!(argv, ["run", "--json", "--resume", "sid-1"]);
    }

    #[test]
    fn jsonl_capture_reads_the_shapes_seen_in_the_wild_and_tolerates_noise() {
        // the current codex stream (verified live).
        let id = capture_session_id(
            &SessionCapture::JsonlEvents,
            "banner noise\n{\"type\":\"thread.started\",\"thread_id\":\"019f-abc\"}\n{\"type\":\"turn.started\"}\n",
        );
        assert_eq!(id.as_deref(), Some("019f-abc"));

        // the older msg envelope.
        let id = capture_session_id(
            &SessionCapture::JsonlEvents,
            "{\"msg\":{\"type\":\"session_configured\",\"session_id\":\"old-1\"}}\n",
        );
        assert_eq!(id.as_deref(), Some("old-1"));

        // a top-level session_id on any event.
        let id = capture_session_id(
            &SessionCapture::JsonlEvents,
            "{\"type\":\"session.created\",\"session_id\":\"top-1\"}\n",
        );
        assert_eq!(id.as_deref(), Some("top-1"));

        // no id anywhere = no capture, never an error.
        assert_eq!(
            capture_session_id(&SessionCapture::JsonlEvents, "{\"type\":\"turn.completed\"}\n"),
            None
        );
        assert_eq!(capture_session_id(&SessionCapture::JsonlEvents, "not json"), None);
    }

    #[test]
    fn result_field_capture_reads_the_result_object_only() {
        let capture = SessionCapture::JsonResultField("session_id".into());
        let id = capture_session_id(
            &capture,
            "{\"type\":\"result\",\"result\":\"hi\",\"session_id\":\"f9cd-1\"}",
        );
        assert_eq!(id.as_deref(), Some("f9cd-1"));

        // a result object without the field, and a non-result object with
        // it, both mean no capture.
        assert_eq!(
            capture_session_id(&capture, "{\"type\":\"result\",\"result\":\"hi\"}"),
            None
        );
        assert_eq!(
            capture_session_id(&capture, "{\"type\":\"event\",\"session_id\":\"x\"}"),
            None
        );
    }

    #[test]
    fn implausible_ids_are_never_captured() {
        for bad in [
            "id with spaces",
            "",
            &"x".repeat(MAX_SESSION_ID_BYTES + 1),
            "line\nbreak",
        ] {
            let stdout = serde_json::json!({"type":"thread.started","thread_id":bad}).to_string();
            assert_eq!(
                capture_session_id(&SessionCapture::JsonlEvents, &stdout),
                None,
                "{bad:?} must not be captured"
            );
        }
    }

    #[test]
    fn the_store_round_trips_and_forget_forces_cold() {
        let root = std::env::temp_dir().join(format!(
            "capability-host-session-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let store = SessionStore::new(&root, "bot", "general#7");
        assert_eq!(store.load(), None, "empty store reads cold");

        store.store_captured(
            &SessionCapture::JsonlEvents,
            "{\"type\":\"thread.started\",\"thread_id\":\"sid-9\"}\n",
        );
        assert_eq!(store.load().as_deref(), Some("sid-9"));

        // same agent, different thread: a different slot.
        assert_eq!(SessionStore::new(&root, "bot", "general#8").load(), None);

        store.forget();
        assert_eq!(store.load(), None, "forgotten sessions read cold");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_capture_less_run_stores_nothing() {
        let root = std::env::temp_dir().join(format!(
            "capability-host-session-nostore-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let store = SessionStore::new(&root, "bot", "k");
        store.store_captured(&SessionCapture::JsonlEvents, "{\"type\":\"turn.completed\"}\n");
        assert_eq!(store.load(), None);
        assert!(!root.exists(), "no capture writes nothing at all");
    }
}
