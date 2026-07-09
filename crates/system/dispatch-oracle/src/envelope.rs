//! the run envelope — the structured payload the runs module composes and
//! this worker assembles into the final model input.
//!
//! consensus commits a JSON envelope (marker `ducktape_run`) carrying the
//! agent's prompt PIN (`prompt_hash`, a 32-byte content address), a
//! thread-continuity key, generic fallback instructions, the strict output
//! contract, and the rendered conversation. the host resolves the pin to the
//! real prompt bytes through an injected [`BlobResolver`] and assembles
//! `<prompt-or-instructions>\n\n<contract>\n\n<conversation>` — so agents
//! finally run on their REGISTERED prompt, while consensus stays
//! deterministic (it committed the hash, and the blob is content-addressed,
//! so the exact bytes stay verifiable).
//!
//! payloads WITHOUT the marker are legacy flat strings and pass through
//! byte-identical — mixed in-flight ops across an upgrade keep working. a
//! payload that CLAIMS the marker but cannot be honored (unknown version,
//! malformed fields, unresolvable prompt) fails the run loudly: feeding a
//! half-understood envelope — or silently swapping an agent's registered
//! prompt for the generic instructions — is exactly the quiet corruption
//! this format exists to kill.

use std::sync::Arc;

use capability_host::RunContext;
use futures::future::BoxFuture;
use serde::Deserialize;
use serde_json::Value;

use crate::provision::{BaseTool, PortablePlan, RoMount};

/// the newest envelope version this worker assembles. v2 remains accepted for
/// in-flight legacy runs; v3 is the portable duckfs-workspace runner contract.
pub const RUN_ENVELOPE_VERSION: u64 = 3;
const LEGACY_RUN_ENVELOPE_VERSION: u64 = 2;
/// the runner-result wrapper version — the SINGLE owner across this crate. the
/// provisioning wrapper's [`crate::provision::assemble_runner_result`] stamps
/// it, the v3 accept slice validates the envelope requests it, and `runs`
/// reads it back as `u32 == 1`. never redeclare a second const.
pub const RUNNER_RESULT_VERSION: u64 = 1;

/// resolve one 32-byte content address to its blob bytes, `None` when this
/// node does not hold it. injected by the embedding binary (the node-local
/// blob store the app's putBlob lane feeds); the pool itself stays
/// storage-agnostic like its spawn/deliver seams.
pub type BlobResolver = Arc<dyn Fn(&[u8; 32]) -> BoxFuture<'static, Option<Vec<u8>>> + Send + Sync>;

/// the wire shape shared by supported envelopes. field ORDER is the composer's
/// business (committed bytes); decoding here is by name. unknown fields are
/// tolerated on purpose — an ADDITIVE field under the same version must not
/// kill in-flight runs mid-upgrade; semantic changes bump the marker instead.
#[derive(Deserialize)]
struct WireEnvelope {
    #[allow(
        dead_code,
        reason = "the version routed decoding; carried for completeness"
    )]
    ducktape_run: u64,
    agent_id: String,
    /// lowercase 64-hex of the agent's prompt pin, or null when the record
    /// carries none (the generic `instructions` apply).
    prompt_hash: Option<String>,
    thread_key: Option<String>,
    instructions: String,
    contract: String,
    conversation: String,
    workspace: Option<WireWorkspace>,
    base_tools: Option<Vec<WireBaseTool>>,
    skills: Option<Vec<WireSkill>>,
    result_contract: Option<WireResultContract>,
}

/// the portable workspace block: the duckfs SOURCE coordinates only. every
/// field is decoded so a malformed v3 envelope fails to parse (acceptance IS
/// validation); `source_prefix` is validated non-empty, and the block is
/// surfaced as a [`PortablePlan`] the pool acts on (or not) — the pinned
/// `source_snapshot` is the source the provisioner checks out.
///
/// carries NO `mount_path` (D7): the phase-5 composer emits source coords only
/// and the phase-2 wrapper chooses the per-run writable cwd. an OLD-shape v3
/// that still carries a `mount_path` decodes fine (the extra field is ignored),
/// so a mixed build never rejects an in-flight envelope.
#[derive(Deserialize)]
struct WireWorkspace {
    source_prefix: String,
    source_snapshot: Option<String>,
}

#[derive(Deserialize)]
struct WireBaseTool {
    name: String,
    version: String,
    exposure: String,
}

/// a C4 skill ref: a read-only duckfs source subtree the wrapper mounts for the
/// run. validated (non-empty `name` + `source_prefix`) and surfaced into the
/// plan's [`crate::provision::RoMount`] set; `source_snapshot` is consumed by
/// the provisioning wrapper at the flip.
#[derive(Deserialize)]
struct WireSkill {
    name: String,
    source_prefix: String,
    source_snapshot: Option<String>,
}

#[derive(Deserialize)]
struct WireResultContract {
    ducktape_runner_result: u64,
}

/// the assembled provider input plus the per-run context, and — for a v3
/// (portable) envelope — the pinned workspace plan. the pool acts on
/// `workspace` iff a provisioner is wired; otherwise the run stays accept-only
/// (dormant), so surfacing the plan here never activates a mount.
#[derive(Debug)]
pub struct Prepared {
    pub input: String,
    pub ctx: RunContext,
    /// `Some` only for a v3 envelope; `None` for v2/legacy.
    pub workspace: Option<PortablePlan>,
}

/// turn one dispatch payload into the provider's input and per-run context.
///
/// - no `ducktape_run` marker → legacy passthrough: the input IS the payload,
///   byte-identical, with a default context and no workspace plan.
/// - marker present → full envelope handling; every failure is a loud `Err`
///   that becomes the saga result (NEVER a silent fallback to the generic
///   instructions — the agent's registered prompt is the whole point).
pub async fn prepare(input: &str, resolver: Option<&BlobResolver>) -> Result<Prepared, String> {
    // marker detection is deliberately strict about what counts as a claim:
    // the payload must be a whole JSON object carrying the key. a flat
    // prompt that merely STARTS with '{' (or embeds the marker in prose)
    // fails the parse and passes through untouched.
    let claimed = match serde_json::from_str::<Value>(input) {
        Ok(Value::Object(map)) if map.contains_key("ducktape_run") => Value::Object(map),
        _ => {
            return Ok(Prepared {
                input: input.to_string(),
                ctx: RunContext::default(),
                workspace: None,
            });
        }
    };

    let version = claimed
        .get("ducktape_run")
        .and_then(Value::as_u64)
        .ok_or_else(|| "run envelope's ducktape_run marker is not an integer".to_string())?;
    if !matches!(version, LEGACY_RUN_ENVELOPE_VERSION | RUN_ENVELOPE_VERSION) {
        return Err(format!(
            "run envelope version {version} is not supported by this worker \
             (understands {LEGACY_RUN_ENVELOPE_VERSION} and {RUN_ENVELOPE_VERSION}); \
             upgrade the executing node"
        ));
    }
    let envelope: WireEnvelope =
        serde_json::from_value(claimed).map_err(|e| format!("run envelope is malformed: {e}"))?;

    let prompt = match &envelope.prompt_hash {
        None => envelope.instructions.clone(),
        Some(hex) => {
            let hash = decode_hash(hex).ok_or_else(|| {
                format!(
                    "run envelope for agent {:?} carries an invalid prompt_hash \
                     {hex:?} (want 64 hex chars)",
                    envelope.agent_id
                )
            })?;
            let resolver = resolver.ok_or_else(|| {
                format!(
                    "agent {:?} has a registered prompt (blob {hex}) but this \
                     worker has no blob resolver wired; refusing to run on the \
                     generic instructions instead",
                    envelope.agent_id
                )
            })?;
            let bytes = resolver(&hash).await.ok_or_else(|| {
                format!(
                    "agent {:?}'s prompt blob {hex} is not in this node's blob \
                     store; refusing to run on the generic instructions instead \
                     — re-save the agent's prompt from the app to restore it",
                    envelope.agent_id
                )
            })?;
            String::from_utf8(bytes).map_err(|_| {
                format!(
                    "agent {:?}'s prompt blob {hex} is not utf-8 text",
                    envelope.agent_id
                )
            })?
        }
    };

    let mut ctx = RunContext {
        agent_id: Some(envelope.agent_id),
        thread_key: envelope.thread_key,
        ..RunContext::default()
    };
    let workspace = if version == RUN_ENVELOPE_VERSION {
        accept_portable_envelope(
            &mut ctx,
            envelope.workspace,
            envelope.base_tools,
            envelope.skills,
            envelope.result_contract,
        )?
    } else {
        None
    };
    Ok(Prepared {
        input: format!(
            "{prompt}\n\n{}\n\n{}",
            envelope.contract, envelope.conversation
        ),
        ctx,
        workspace,
    })
}

/// ACCEPT a v3 (portable) envelope and surface its pinned plan, without
/// ACTIVATING portable execution HERE.
///
/// this worker validates the portable shape — proving mixed-network nodes on
/// this binary understand v3 before any node composes it — and marks the run
/// portable so no host-local native session is resumed for it. it deliberately
/// does NOT set the child's working directory or inject workspace env: the
/// envelope carries SOURCE coordinates only (no `mount_path`, D7), and turning
/// the plan into a real mount is the pool's job via the injected provisioner
/// (a consensus-supplied host path like the constant `/workspace` is exactly
/// the unwritable cwd that turned live runs into `create_dir_all` failures).
/// per the ADR (`2026-07-09-deterministic-agent-runtime`, ROL/M2 + W1) portable
/// ACTIVATION — a per-run writable mount and its bindings — happens in the pool
/// only when a provisioner is wired AND a coordinated flip emits v3; until then
/// the returned plan is inert data and the host's own scratch/persistent
/// workspace policy owns the cwd (see `capability-host::workdir_for`).
fn accept_portable_envelope(
    ctx: &mut RunContext,
    workspace: Option<WireWorkspace>,
    base_tools: Option<Vec<WireBaseTool>>,
    skills: Option<Vec<WireSkill>>,
    result_contract: Option<WireResultContract>,
) -> Result<Option<PortablePlan>, String> {
    let workspace = workspace.ok_or_else(|| "v3 run envelope is missing workspace".to_string())?;
    if workspace.source_prefix.is_empty() {
        return Err("v3 run envelope workspace.source_prefix must not be empty".into());
    }
    let result_contract =
        result_contract.ok_or_else(|| "v3 run envelope is missing result_contract".to_string())?;
    if result_contract.ducktape_runner_result != RUNNER_RESULT_VERSION {
        return Err(format!(
            "v3 run envelope requests runner result version {}, but this worker understands {RUNNER_RESULT_VERSION}",
            result_contract.ducktape_runner_result
        ));
    }
    let base_tools =
        base_tools.ok_or_else(|| "v3 run envelope is missing base_tools".to_string())?;
    if base_tools.is_empty() {
        return Err("v3 run envelope base_tools must not be empty".into());
    }
    // skills are optional, but each present entry must name a source (the
    // wrapper checks it out as a ro mount).
    let skills = skills.unwrap_or_default();
    if skills
        .iter()
        .any(|s| s.name.is_empty() || s.source_prefix.is_empty())
    {
        return Err("v3 run envelope skill entries must carry a name and source_prefix".into());
    }

    // mark portable (no host-local session resume) but set NO
    // workdir_override/env here (W1/M2) — the plan is data the pool decides
    // whether to act on.
    ctx.portable = true;
    Ok(Some(PortablePlan {
        source_prefix: workspace.source_prefix,
        source_snapshot: workspace.source_snapshot,
        base_tools: base_tools
            .into_iter()
            .map(|t| BaseTool {
                name: t.name,
                version: t.version,
                exposure: t.exposure,
            })
            .collect(),
        // each skill becomes a ro mount: its name is the mount subpath the
        // wrapper materializes it under.
        skills: skills
            .into_iter()
            .map(|s| RoMount {
                source_prefix: s.source_prefix,
                source_snapshot: s.source_snapshot,
                mount_subpath: s.name,
            })
            .collect(),
    }))
}

/// 64 lowercase-or-uppercase hex chars → 32 bytes. strict charset first:
/// `from_str_radix` alone would admit `+`-prefixed chunks.
fn decode_hash(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolver_with(hash: [u8; 32], bytes: Vec<u8>) -> BlobResolver {
        Arc::new(move |digest: &[u8; 32]| {
            let hit = (*digest == hash).then(|| bytes.clone());
            Box::pin(async move { hit })
        })
    }

    fn envelope_json(prompt_hash: Option<&str>) -> String {
        serde_json::json!({
            "ducktape_run": 2,
            "agent_id": "bot",
            "prompt_hash": prompt_hash,
            "thread_key": "general#7",
            "instructions": "GENERIC",
            "contract": "CONTRACT",
            "conversation": "CONVERSATION",
        })
        .to_string()
    }

    fn v3_envelope_json(prompt_hash: Option<&str>) -> String {
        serde_json::json!({
            "ducktape_run": 3,
            "agent_id": "bot",
            "prompt_hash": prompt_hash,
            "thread_key": "general#7",
            "instructions": "GENERIC",
            "contract": "CONTRACT",
            "conversation": "CONVERSATION",
            "workspace": {
                "source_prefix": "/shared/agent-workspaces/bot",
                "source_snapshot": "aa".repeat(32)
            },
            "base_tools": [
                {"name":"ducktape-files","version":"1","exposure":"cli"},
                {"name":"ducktape-index","version":"1","exposure":"cli"},
                {"name":"ducktape-chain","version":"1","exposure":"cli"}
            ],
            "skills": [
                {"name":"release","source_prefix":"/shared/skills/release","source_snapshot": "bb".repeat(32)}
            ],
            "result_contract": {"ducktape_runner_result": 1}
        })
        .to_string()
    }

    #[tokio::test]
    async fn legacy_flat_payloads_pass_through_byte_identical() {
        for legacy in [
            "a plain rendered prompt",
            "",
            // starts with '{' but is not JSON: must not be mangled.
            "{not json at all",
            // valid JSON but not an object: not a claim.
            "[1,2,3]",
            "\"just a string\"",
            // a JSON object WITHOUT the marker: not a claim either.
            r#"{"run_id":"r","agent_id":"a"}"#,
            // the marker as PROSE inside a flat prompt, not a JSON key.
            "the ducktape_run marker is discussed here",
        ] {
            let Prepared {
                input,
                ctx,
                workspace,
            } = prepare(legacy, None).await.unwrap();
            assert_eq!(input.as_bytes(), legacy.as_bytes(), "verbatim: {legacy:?}");
            assert_eq!(ctx, RunContext::default());
            assert!(workspace.is_none(), "legacy payloads carry no plan");
        }
    }

    #[tokio::test]
    async fn a_null_hash_envelope_assembles_instructions_contract_conversation() {
        let Prepared { input, ctx, .. } = prepare(&envelope_json(None), None).await.unwrap();
        assert_eq!(input, "GENERIC\n\nCONTRACT\n\nCONVERSATION");
        assert_eq!(ctx.agent_id.as_deref(), Some("bot"));
        assert_eq!(ctx.thread_key.as_deref(), Some("general#7"));
    }

    #[tokio::test]
    async fn a_resolved_prompt_replaces_the_generic_instructions() {
        let hash = [7u8; 32];
        let hex = "07".repeat(32);
        let resolver = resolver_with(hash, b"You are Bot, the release captain.".to_vec());
        let Prepared { input, .. } = prepare(&envelope_json(Some(&hex)), Some(&resolver))
            .await
            .unwrap();
        assert_eq!(
            input,
            "You are Bot, the release captain.\n\nCONTRACT\n\nCONVERSATION"
        );
        assert!(
            !input.contains("GENERIC"),
            "the generic instructions must NOT appear once the real prompt resolved"
        );
    }

    #[tokio::test]
    async fn unresolvable_prompts_fail_loudly_never_fall_back() {
        let hex = "07".repeat(32);

        // no resolver wired at all.
        let err = prepare(&envelope_json(Some(&hex)), None).await.unwrap_err();
        assert!(err.contains("no blob resolver"), "got {err:?}");
        assert!(err.contains("bot"), "names the agent: {err:?}");

        // a resolver that misses.
        let resolver = resolver_with([9u8; 32], b"other".to_vec());
        let err = prepare(&envelope_json(Some(&hex)), Some(&resolver))
            .await
            .unwrap_err();
        assert!(err.contains("not in this node's blob store"), "got {err:?}");
        assert!(err.contains(&hex), "names the blob: {err:?}");
        assert!(
            err.contains("re-save the agent's prompt from the app to restore it"),
            "ends with the remedy: {err:?}"
        );

        // a blob that is not utf-8.
        let resolver = resolver_with([7u8; 32], vec![0xff, 0xfe]);
        let err = prepare(&envelope_json(Some(&hex)), Some(&resolver))
            .await
            .unwrap_err();
        assert!(err.contains("not utf-8"), "got {err:?}");
    }

    #[tokio::test]
    async fn claimed_but_broken_envelopes_are_loud_errors_not_passthrough() {
        // an unknown version is a mixed-network signal, never model input.
        let err = prepare(r#"{"ducktape_run":99}"#, None).await.unwrap_err();
        assert!(err.contains("version 99"), "got {err:?}");

        // a non-integer marker.
        let err = prepare(r#"{"ducktape_run":"2"}"#, None).await.unwrap_err();
        assert!(err.contains("not an integer"), "got {err:?}");

        // version 2 with required fields missing.
        let err = prepare(r#"{"ducktape_run":2,"agent_id":"bot"}"#, None)
            .await
            .unwrap_err();
        assert!(err.contains("malformed"), "got {err:?}");

        // a bad hex pin (right marker, wrong pin shape).
        let short = envelope_json(Some("abc123"));
        let err = prepare(&short, None).await.unwrap_err();
        assert!(err.contains("invalid prompt_hash"), "got {err:?}");
        let plus = envelope_json(Some(&"+7".repeat(32)));
        let err = prepare(&plus, None).await.unwrap_err();
        assert!(err.contains("invalid prompt_hash"), "got {err:?}");
    }

    #[tokio::test]
    async fn additive_fields_under_the_same_version_are_tolerated() {
        // a newer composer may add an OPTIONAL field without a flag day; the
        // worker must not kill in-flight runs over it.
        let mut v: serde_json::Value = serde_json::from_str(&envelope_json(None)).unwrap();
        v["a_future_field"] = serde_json::json!("x");
        let Prepared { input, .. } = prepare(&v.to_string(), None).await.unwrap();
        assert_eq!(input, "GENERIC\n\nCONTRACT\n\nCONVERSATION");
    }

    #[tokio::test]
    async fn job_envelopes_carry_no_thread_key() {
        let mut v: serde_json::Value = serde_json::from_str(&envelope_json(None)).unwrap();
        v["thread_key"] = serde_json::Value::Null;
        let Prepared { ctx, .. } = prepare(&v.to_string(), None).await.unwrap();
        assert_eq!(ctx.agent_id.as_deref(), Some("bot"));
        assert_eq!(ctx.thread_key, None);
    }

    #[tokio::test]
    async fn v3_envelopes_are_accepted_and_marked_portable_without_activating_a_mount() {
        // the worker ACCEPTS v3 (proving readiness for a future coordinated
        // flip) and marks the run portable, but it does NOT activate a
        // workspace mount: no consensus-supplied cwd override, no workspace
        // env. the host's own scratch/persistent policy owns the cwd until the
        // provisioning wrapper lands (ADR ROL/M2 + W1).
        let Prepared {
            input,
            ctx,
            workspace,
        } = prepare(&v3_envelope_json(None), None).await.unwrap();
        assert_eq!(input, "GENERIC\n\nCONTRACT\n\nCONVERSATION");
        assert_eq!(ctx.agent_id.as_deref(), Some("bot"));
        assert_eq!(ctx.thread_key.as_deref(), Some("general#7"));
        assert!(
            ctx.portable,
            "v3 runs are portable and cannot resume host-local sessions"
        );
        assert!(
            ctx.workdir_override.is_none(),
            "portable activation is HELD: no consensus-supplied cwd is forced"
        );
        assert!(
            ctx.env.is_empty(),
            "no workspace env is injected until the mount is materialized"
        );
        assert!(ctx.path_entries.is_empty());
        // the pinned plan IS surfaced (so the pool CAN act on it when a
        // provisioner is wired) — surfacing it is not activating it.
        let plan = workspace.expect("a v3 envelope surfaces its portable plan");
        assert_eq!(plan.source_prefix, "/shared/agent-workspaces/bot");
        assert_eq!(plan.source_snapshot.as_deref(), Some("aa".repeat(32).as_str()));
        assert_eq!(plan.base_tools.len(), 3);
        // the C4 skills are surfaced as ro mounts (name -> mount_subpath).
        assert_eq!(plan.skills.len(), 1);
        assert_eq!(plan.skills[0].mount_subpath, "release");
        assert_eq!(plan.skills[0].source_prefix, "/shared/skills/release");
        assert_eq!(
            plan.skills[0].source_snapshot.as_deref(),
            Some("bb".repeat(32).as_str())
        );
    }

    #[tokio::test]
    async fn an_old_shape_v3_that_still_carries_mount_path_decodes_fine() {
        // the composer no longer emits mount_path (D7), but a mixed build must
        // never reject an in-flight envelope that still carries one — the extra
        // field is tolerated (decoded and ignored).
        let mut old_shape: serde_json::Value =
            serde_json::from_str(&v3_envelope_json(None)).unwrap();
        old_shape["workspace"]["mount_path"] = serde_json::json!("/tmp/ducktape-workspace");
        let Prepared {
            ctx, workspace, ..
        } = prepare(&old_shape.to_string(), None).await.unwrap();
        assert!(ctx.portable, "an old-shape v3 is still accepted + portable");
        let plan = workspace.expect("an old-shape v3 still surfaces its plan");
        assert_eq!(plan.source_prefix, "/shared/agent-workspaces/bot");
    }

    #[tokio::test]
    async fn a_v3_envelope_that_omits_or_breaks_the_portable_shape_fails_loudly() {
        // accept still means VALIDATE: a v3 marker with a missing/empty/wrong
        // portable block is a mixed-network signal, never silently downgraded.
        let base: serde_json::Value = serde_json::from_str(&v3_envelope_json(None)).unwrap();

        let mut missing = base.clone();
        missing["workspace"] = serde_json::Value::Null;
        let err = prepare(&missing.to_string(), None).await.unwrap_err();
        assert!(err.contains("missing workspace"), "got {err:?}");

        let mut empty_prefix = base.clone();
        empty_prefix["workspace"]["source_prefix"] = serde_json::json!("");
        let err = prepare(&empty_prefix.to_string(), None).await.unwrap_err();
        assert!(err.contains("source_prefix must not be empty"), "got {err:?}");

        let mut empty_tools = base.clone();
        empty_tools["base_tools"] = serde_json::json!([]);
        let err = prepare(&empty_tools.to_string(), None).await.unwrap_err();
        assert!(err.contains("base_tools must not be empty"), "got {err:?}");

        let mut bad_result = base.clone();
        bad_result["result_contract"]["ducktape_runner_result"] = serde_json::json!(99);
        let err = prepare(&bad_result.to_string(), None).await.unwrap_err();
        assert!(err.contains("runner result version 99"), "got {err:?}");

        // a present skill entry with no source is a mixed-network signal too.
        let mut empty_skill = base.clone();
        empty_skill["skills"][0]["source_prefix"] = serde_json::json!("");
        let err = prepare(&empty_skill.to_string(), None).await.unwrap_err();
        assert!(
            err.contains("skill entries must carry a name and source_prefix"),
            "got {err:?}"
        );
    }

    #[tokio::test]
    async fn v2_envelopes_remain_non_portable_for_legacy_in_flight_runs() {
        let Prepared {
            ctx, workspace, ..
        } = prepare(&envelope_json(None), None).await.unwrap();
        assert_eq!(ctx.agent_id.as_deref(), Some("bot"));
        assert!(!ctx.portable);
        assert!(ctx.workdir_override.is_none());
        assert!(ctx.env.is_empty());
        assert!(workspace.is_none(), "v2 runs carry no portable plan");
    }
}
