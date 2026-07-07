//! [`HarnessError`] — the one error the crate's public fallible surface
//! returns: install-spec mapping (parse/validate/digest-verify a capsule's
//! manifest, resolve the harness logical, map prompts/agents/actions), the
//! testbed's submit/oracle/deliver/snapshot-roundtrip operations, and golden
//! fixture parsing. before this, the public API mixed `host::SubmitError`,
//! `Result<_, String>`, and `GoldenError` — one typed enum, thiserror-style
//! (mirrors the workspace convention: `quack::ManifestError`/`CapsuleError`),
//! replaces the stringly-typed half of that mix.
//!
//! [`crate::GoldenError`] stays a separate, deliberately stringly-rendered
//! report type (`step`/`label`/`message: String`) — the golden runner's own
//! per-step diagnostics (fixture-script assertions like "expected N jobs,
//! found M") are not really API errors, and its callers (the CLI's `package
//! test` table) only ever print `message`, never match on it. every error
//! that actually escapes a public operation during a golden run (install,
//! submit, oracle, deliver, snapshot round-trip) IS a [`HarnessError`] at the
//! point it is produced; the golden runner renders it into that report via
//! `Display`.

use host::SubmitError;

use crate::golden::GOLDEN_SCHEMA_V1;
use quack::GOLDEN_PATH;

#[derive(Debug, thiserror::Error)]
pub enum HarnessError {
    // ---- manifest / capsule -------------------------------------------------
    #[error("no quack.toml in the capsule")]
    NoManifest,
    #[error("manifest: {0}")]
    Manifest(#[from] quack::ManifestError),
    #[error("content digests: {0}")]
    Digests(#[from] quack::CapsuleError),
    #[error("open {path}: {source}")]
    OpenDir {
        path: String,
        source: quack::CapsuleError,
    },

    // ---- install-spec mapping -----------------------------------------------
    #[error("no harness logical: pass one explicitly or set the manifest's `harness` key")]
    NoHarnessLogical,
    #[error("harness logical {logical:?} is not a declared [[modules]] entry")]
    UnboundHarness { logical: String },
    #[error("prompt {logical} is not utf-8: {path}")]
    PromptNotUtf8 { logical: String, path: String },
    #[error("prompt {logical} hash field is malformed: {hash}")]
    MalformedPromptHash { logical: String, hash: String },
    #[error("agent {agent_id} has unknown status {status:?} (want \"active\" or \"paused\")")]
    UnknownAgentStatus { agent_id: String, status: String },

    // ---- install driving -----------------------------------------------------
    #[error("required module {module:?} is not registered on the testbed")]
    MissingRequiredModule { module: String },
    #[error("{context}: {source}")]
    Submit {
        context: &'static str,
        source: SubmitError,
    },

    // ---- build_report (post-install verification) -----------------------------
    #[error("{module} query failed: {reason}")]
    Query {
        module: &'static str,
        reason: String,
    },
    #[error("unexpected {module} reply: {reply}")]
    UnexpectedReply { module: &'static str, reply: String },
    #[error("package {package} has no committed row")]
    NoCommittedRow { package: String },
    #[error("prompt {logical} was not seeded at {path}")]
    PromptNotSeeded { logical: String, path: String },
    #[error("prompt {logical} body is not inline")]
    PromptBodyNotInline { logical: String },
    #[error("prompt {logical} committed content does not hash to its pin")]
    PromptPinMismatch { logical: String },
    #[error("agent {agent_id} was not registered")]
    AgentNotRegistered { agent_id: String },
    #[error("action {tag} was not routed")]
    ActionNotRouted { tag: String },

    // ---- testbed plumbing -------------------------------------------------------
    #[error("query of {module} failed: {reason}")]
    JsonQuery { module: String, reason: String },
    #[error("{module} reply is not canonical JSON: {reason}")]
    NotCanonicalJson { module: String, reason: String },
    #[error("no pending oracle request (no WorkerRequest effect outstanding)")]
    NoPendingOracleRequest,
    #[error("no pending oracle request for capability {capability:?} (pending: {pending:?})")]
    NoPendingOracleForCapability {
        capability: String,
        pending: Vec<String>,
    },
    #[error("effect is not a WorkerRequest: {0}")]
    NotAWorkerRequest(String),
    #[error("WorkerRequest spec is not a dispatch WorkSpec: {0}")]
    NotAWorkSpec(String),

    // ---- snapshot round-trip sweep ----------------------------------------------
    #[error("capture failed: {0}")]
    CaptureFailed(String),
    #[error("module {module} failed to re-install its snapshot into a fresh instance: {reason}")]
    ReinstallFailed { module: String, reason: String },
    #[error(
        "module {module}: snapshot bytes do not hash to root(): the framework can only \
         preimage-verify caller-supplied snapshot-bytes modules (sha256(snapshot) == root, the \
         memory/tasks/package convention); a module with a different root derivation must prove \
         its round-trip in its own snapshot suite"
    )]
    NotPreimageVerified { module: String },
    #[error("module {module}: resolver target: {reason}")]
    ResolverTarget { module: String, reason: String },
    #[error("module {module}: served sync target root does not match the committed root")]
    ResolverRootMismatch { module: String },
    #[error(
        "module {module} declares no state-sync surface ({reason}) — the ADR requires every \
         module's snapshots/state sync to reproduce its root"
    )]
    NoStateSyncSurface { module: String, reason: String },

    // ---- golden fixtures -----------------------------------------------------------
    #[error("golden fixture: {0}")]
    GoldenFixtureJson(String),
    #[error("unsupported golden schema {0} (this build understands {GOLDEN_SCHEMA_V1})")]
    UnsupportedGoldenSchema(u32),
    #[error("capsule has no {GOLDEN_PATH}")]
    NoGoldenFixture,
    #[error("malformed external origin {0:?} (want non-empty, even-length lowercase hex)")]
    MalformedExternalOrigin(String),
    #[error("malformed module origin {0:?}")]
    MalformedModuleOrigin(String),
    #[error("unknown origin form {0:?} (want \"external:<hex>\" or \"module:<id>\")")]
    UnknownOriginForm(String),
}
