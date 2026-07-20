//! the run envelope — the structured payload the runs module composes and
//! this worker assembles into the final model input.
//!
//! consensus commits a JSON envelope (marker `ducktape_run`) carrying the
//! agent's curated SKILL PINS (name + duckfs prefix/snapshot + load mode), a
//! thread-continuity key, generic fallback instructions, the strict output
//! contract, and the rendered conversation. the host assembles
//! `<instructions>\n\n[<context>\n\n]<contract>\n\n<conversation>` (the
//! bracketed section rides only when the envelope carries it).
//!
//! the agent's PERSONA is deliberately NOT in here any more: `prompt_hash` and
//! the blob-store prompt lane are retired (flag day). a persona is a skill now
//! — the skills already ride the envelope as content-addressed duckfs pins, and
//! the provisioner assembles the `always` ones into the run's context document
//! (see [`crate::soul`]). that also kills the trap where a present `prompt_hash`
//! silently dropped `instructions`: the fallback is now reached by the ABSENCE
//! of always-skills, a state you can see.
//!
//! a payload that is not a run envelope — or one that cannot be honored
//! (unknown version, malformed fields) — fails the run loudly: feeding a
//! half-understood payload is exactly the quiet corruption this format exists
//! to kill. the flat-string passthrough and the v2 tolerance are gone (flag day
//! — in-flight legacy runs are unsupported).

use capability_host::RunContext;
use serde::Deserialize;
use serde_json::Value;

use crate::provision::{PortablePlan, RoMount, Sink};
use crate::workspace_source::WireWorkspace;

/// the ONLY envelope version this worker assembles: the portable
/// duckfs/forge-workspace runner contract.
pub const RUN_ENVELOPE_VERSION: u64 = 3;
/// the runner-result wrapper version — the SINGLE owner across this crate. the
/// provisioning wrapper's [`crate::provision::assemble_runner_result`] stamps
/// it, the v3 accept slice validates the envelope requests it, and `runs`
/// reads it back as `u32 == 1`. never redeclare a second const.
pub const RUNNER_RESULT_VERSION: u64 = 1;

/// the wire shape shared by supported envelopes. field ORDER is the composer's
/// business (committed bytes); decoding here is by name. serde ignores unknown
/// fields on this tagged shape. `ducktape_run` is validated before this body is
/// deserialized, so the marker is intentionally absent here.
#[derive(Deserialize)]
struct WireEnvelope {
    agent_id: String,
    /// the run's CONSENSUS id — what `runs` resolves the run by (the id its
    /// pending map is keyed on, and the one its agent session lane binds). the
    /// host has no way to derive it: the pool's own `WorkspaceSpec.run_id` is
    /// `{saga_id}:{attempt}`, a host-local workspace-dir key that names no run
    /// in consensus. `Option` because it is an ADDITIVE field — an envelope
    /// composed before it existed carries no key, and such a run must still
    /// execute (it simply opens no session: the read-only tool plane).
    run_id: Option<String>,
    /// additive for v3: old in-flight envelopes fall back to `agent_id`, while
    /// newly composed runs carry the committed registry display name.
    #[serde(default)]
    agent_display_name: Option<String>,
    thread_key: Option<String>,
    instructions: String,
    contract: String,
    conversation: String,
    /// the deterministic forge item-context section (contract §1) — `None`
    /// (key absent) for every non-forge run. assembled into the provider
    /// input between the instructions and the contract; None-case assembly
    /// stays byte-identical.
    context: Option<String>,
    workspace: Option<WireWorkspace>,
    skills: Option<Vec<WireSkill>>,
    /// the committed `duckfs_read` verdict on the global skill library: the
    /// composer asks the agent's record (`agent::AgentRecord::library_readable`)
    /// and states the answer here, because the host has no consensus registry to
    /// ask. required (the composer always states it) — an envelope that never
    /// stated the grant cannot have earned it, and the paragraph it gates would
    /// only send the agent at a door the tool plane refuses.
    library_readable: bool,
    result_contract: Option<WireResultContract>,
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
    /// the committed load mode (`SkillRef::load`): `true` = this skill's full
    /// body is INLINED into the run's context document — the agent's persona
    /// lives here now that `prompt_hash` is retired. required: `false` =
    /// on-demand (indexed, not inlined), `true` = inlined.
    always: bool,
}

#[derive(Deserialize)]
struct WireResultContract {
    ducktape_runner_result: u64,
    /// the REQUESTED output sink (contract §1) — an ABSENT key is the Chain
    /// default, mirroring the composer's skip-serialization. the requested-Pr
    /// shape carries no title/body; [`Sink`]'s decode defaults them empty.
    #[serde(default)]
    sink: Sink,
}

/// the assembled provider input plus the per-run context and the pinned
/// workspace plan. The pool requires a provisioner before executing it.
#[derive(Debug)]
pub struct Prepared {
    pub input: String,
    pub ctx: RunContext,
    pub workspace: PortablePlan,
}

/// turn one dispatch payload into the provider's input and per-run context.
/// every payload MUST be a v3 run envelope; anything else — flat strings, v2
/// envelopes, unknown versions, malformed fields — is a loud `Err` that becomes
/// the saga result (NEVER a silent fallback: the pinned workspace and the
/// curated skills are the whole point).
///
pub fn prepare(input: &str) -> Result<Prepared, String> {
    let claimed = match serde_json::from_str::<Value>(input) {
        Ok(Value::Object(map)) if map.contains_key("ducktape_run") => Value::Object(map),
        _ => {
            return Err(
                "dispatch payload carries no ducktape_run envelope marker; legacy flat \
                 payloads are unsupported (flag day) — recompose the run"
                    .to_string(),
            );
        }
    };

    let version = claimed
        .get("ducktape_run")
        .and_then(Value::as_u64)
        .ok_or_else(|| "run envelope's ducktape_run marker is not an integer".to_string())?;
    if version != RUN_ENVELOPE_VERSION {
        return Err(format!(
            "run envelope version {version} is not supported by this worker \
             (understands {RUN_ENVELOPE_VERSION} only; v2/legacy envelopes are \
             unsupported); upgrade or recompose"
        ));
    }
    let envelope: WireEnvelope =
        serde_json::from_value(claimed).map_err(|e| format!("run envelope is malformed: {e}"))?;

    let agent_display_name = envelope
        .agent_display_name
        .unwrap_or_else(|| envelope.agent_id.clone());
    let mut ctx = RunContext {
        agent_id: Some(envelope.agent_id),
        thread_key: envelope.thread_key,
        ..RunContext::default()
    };
    let workspace = accept_portable_envelope(
        &mut ctx,
        envelope.run_id,
        envelope.workspace,
        envelope.skills,
        envelope.result_contract,
        agent_display_name,
        envelope.library_readable,
    )?;
    // reading order: system instructions → item context (forge runs only: what
    // this run is working ON) → output contract → conversation. every section is
    // byte-exact from its envelope field, joined with the same "\n\n" delimiter;
    // an absent OPTIONAL section contributes no delimiter either.
    //
    // what this session CAN do — the persona and the tool plane — is no longer a
    // section here: it is the run's CONTEXT DOCUMENT, assembled from the curated
    // skills by the provisioner and delivered through the door the capability
    // spec names (the executor's own auto-load file, or a prepend to this input).
    //
    // `instructions` is the generic fallback, and it is now UNCONDITIONAL: an
    // agent whose persona is an `always` skill gets that persona from its context
    // document instead. no field of this envelope can silently suppress another.
    let input = [
        Some(envelope.instructions.as_str()),
        envelope.context.as_deref(),
        Some(envelope.contract.as_str()),
        Some(envelope.conversation.as_str()),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join("\n\n");
    Ok(Prepared {
        input,
        ctx,
        workspace,
    })
}

/// ACCEPT a portable envelope and surface its pinned plan, without
/// ACTIVATING portable execution HERE.
///
/// this worker validates the portable shape and marks the run portable so no
/// host-local native session is resumed for it. it deliberately does NOT set
/// the child's working directory or inject workspace env: the envelope carries
/// SOURCE coordinates only (no `mount_path`, D7), and turning the plan into a
/// real mount is the pool's job via the injected provisioner (a
/// consensus-supplied host path like the constant `/workspace` is exactly the
/// unwritable cwd that turned live runs into `create_dir_all` failures, W1).
/// portable ACTIVATION — a per-run writable mount and its bindings — happens
/// in the pool through its required execution-time provisioner.
fn accept_portable_envelope(
    ctx: &mut RunContext,
    consensus_run_id: Option<String>,
    workspace: Option<WireWorkspace>,
    skills: Option<Vec<WireSkill>>,
    result_contract: Option<WireResultContract>,
    agent_display_name: String,
    library_readable: bool,
) -> Result<PortablePlan, String> {
    let workspace = workspace.ok_or_else(|| "v3 run envelope is missing workspace".to_string())?;
    // the tagged source block validates per variant (duckfs keeps its
    // non-empty-prefix rule; forge requires repo/commit/branch) with loud,
    // field-naming errors — see [`crate::workspace_source`].
    let source = workspace.validate()?;
    let result_contract =
        result_contract.ok_or_else(|| "v3 run envelope is missing result_contract".to_string())?;
    if result_contract.ducktape_runner_result != RUNNER_RESULT_VERSION {
        return Err(format!(
            "v3 run envelope requests runner result version {}, but this worker understands {RUNNER_RESULT_VERSION}",
            result_contract.ducktape_runner_result
        ));
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
    Ok(PortablePlan {
        source,
        // consensus decided this (the agent's duckfs_read caps); the host only
        // obeys — exactly like a skill's load mode.
        library_readable,
        // the id CONSENSUS knows this run by — carried through to the
        // provisioner, which is the only thing that can name the run back to
        // `runs`. absent on a pre-field envelope: no session, never a failure.
        consensus_run_id,
        agent_display_name,
        // the requested sink rides the plan so the pool can echo it on the
        // assembled RunnerResult; Chain (the default) when the key is absent.
        sink: result_contract.sink,
        // each skill becomes a ro mount: its name is the mount subpath the
        // wrapper materializes it under, and its load mode rides along so the
        // provisioner knows which bodies to inline into the context document.
        // ORDER IS CURATION ORDER and must survive: the assembled soul reads in
        // this order, and reordering an agent's persona is editing it.
        skills: skills
            .into_iter()
            .map(|s| RoMount {
                source_prefix: s.source_prefix,
                source_snapshot: s.source_snapshot,
                mount_subpath: s.name,
                always: s.always,
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// the CONSENSUS run id the composer stamps — see
    /// [`crate::provision::WorkspaceSpec::consensus_run_id`].
    const CONSENSUS_RUN_ID: &str = "chat\u{1f}general\u{1f}7\u{1f}bot";

    /// a duckfs-sourced v3 envelope — the byte shape the runs composer emits
    /// for every non-forge run. it carries NO prompt pin: the persona is a
    /// curated skill now (`always`), assembled into the run's context document
    /// by the provisioner, never resolved from a blob here.
    fn envelope_json() -> String {
        serde_json::json!({
            "ducktape_run": 3,
            "agent_id": "bot",
            "run_id": CONSENSUS_RUN_ID,
            "agent_display_name": "BOT",
            "thread_key": "general#7",
            "instructions": "GENERIC",
            "contract": "CONTRACT",
            "conversation": "CONVERSATION",
            "workspace": {
                "kind": "duckfs",
                "source_prefix": "/shared/agent-workspaces/bot",
                "source_snapshot": "aa".repeat(32)
            },
            "skills": [
                {"name":"release","source_prefix":"/shared/skills/release","source_snapshot": "bb".repeat(32), "always": false}
            ],
            "library_readable": false,
            "result_contract": {"ducktape_runner_result": 1}
        })
        .to_string()
    }

    /// a forge-sourced envelope — tagged forge workspace, `context` after
    /// `conversation`, requested-Pr sink WITHOUT title/body keys.
    fn forge_envelope_json() -> String {
        serde_json::json!({
            "ducktape_run": 3,
            "agent_id": "bot",
            "run_id": "chat\u{1f}forge:app:7\u{1f}2\u{1f}bot",
            "agent_display_name": "BOT",
            "thread_key": "forge:app:7#2",
            "instructions": "GENERIC",
            "contract": "CONTRACT",
            "conversation": "CONVERSATION",
            "context": "Forge item context — you are working this item as a session.\nrepo: app\nitem: issue #7 (open)",
            "workspace": {
                "kind": "forge",
                "repo": "app",
                "item_title": "Fix the gate",
                "commit": "d0".repeat(20),
                "branch": "agent/item-7",
                "branch_born": false,
                "forge_push": true
            },
            "skills": [],
            "library_readable": false,
            "result_contract": {
                "ducktape_runner_result": 1,
                "sink": {"mode":"pr","repo":"app","source_branch":"agent/item-7","target_branch":"main"}
            }
        })
        .to_string()
    }

    #[test]
    fn non_envelope_payloads_are_loud_errors_never_passthrough() {
        // FLAG DAY: the legacy flat-string passthrough is gone — anything
        // that is not a marker-carrying JSON object fails the run.
        for legacy in [
            "a plain rendered prompt",
            "",
            "{not json at all",
            "[1,2,3]",
            "\"just a string\"",
            r#"{"run_id":"r","agent_id":"a"}"#,
            "the ducktape_run marker is discussed here",
        ] {
            let err = prepare(legacy).unwrap_err();
            assert!(
                err.contains("no ducktape_run envelope marker"),
                "{legacy:?} must be rejected loudly, got {err:?}"
            );
        }
    }

    #[test]
    fn v2_envelopes_are_rejected() {
        // the second half of the flag day: the v2 tolerance is gone.
        let mut v: serde_json::Value = serde_json::from_str(&envelope_json()).unwrap();
        v["ducktape_run"] = serde_json::json!(2);
        let err = prepare(&v.to_string()).unwrap_err();
        assert!(err.contains("version 2"), "got {err:?}");
        assert!(err.contains("understands 3"), "got {err:?}");
    }

    #[test]
    fn an_envelope_assembles_instructions_contract_conversation() {
        let Prepared {
            input,
            ctx,
            workspace,
        } = prepare(&envelope_json()).unwrap();
        assert_eq!(input, "GENERIC\n\nCONTRACT\n\nCONVERSATION");
        assert_eq!(ctx.agent_id.as_deref(), Some("bot"));
        assert_eq!(workspace.agent_display_name, "BOT");
        assert_eq!(ctx.thread_key.as_deref(), Some("general#7"));
    }

    #[test]
    fn a_retired_prompt_hash_key_can_never_resurrect_the_blob_lane() {
        // the prompt blob is RETIRED (flag day). a stale composer's leftover
        // pin must be inert data — tolerated like any unknown field, never
        // resolved, and above all never able to suppress `instructions` (the
        // trap the old worker shipped: a present pin silently dropped them).
        let mut stale: serde_json::Value = serde_json::from_str(&envelope_json()).unwrap();
        stale["prompt_hash"] = serde_json::json!("07".repeat(32));
        let Prepared { input, .. } = prepare(&stale.to_string()).unwrap();
        assert_eq!(input, "GENERIC\n\nCONTRACT\n\nCONVERSATION");
    }

    #[test]
    fn claimed_but_broken_envelopes_are_loud_errors() {
        // an unknown version is a mixed-network signal, never model input.
        let err = prepare(r#"{"ducktape_run":99}"#).unwrap_err();
        assert!(err.contains("version 99"), "got {err:?}");

        // a non-integer marker.
        let err = prepare(r#"{"ducktape_run":"3"}"#).unwrap_err();
        assert!(err.contains("not an integer"), "got {err:?}");

        // v3 with required fields missing.
        let err = prepare(r#"{"ducktape_run":3,"agent_id":"bot"}"#).unwrap_err();
        assert!(err.contains("malformed"), "got {err:?}");
    }

    #[test]
    fn additive_fields_under_the_same_version_are_tolerated() {
        // a newer composer may add an OPTIONAL field without a flag day; the
        // worker must not kill in-flight runs over it.
        let mut v: serde_json::Value = serde_json::from_str(&envelope_json()).unwrap();
        v["a_future_field"] = serde_json::json!("x");
        let Prepared { input, .. } = prepare(&v.to_string()).unwrap();
        assert_eq!(input, "GENERIC\n\nCONTRACT\n\nCONVERSATION");
    }

    #[test]
    fn an_in_flight_envelope_without_a_display_name_uses_the_agent_id() {
        let mut v: serde_json::Value = serde_json::from_str(&envelope_json()).unwrap();
        v.as_object_mut().unwrap().remove("agent_display_name");
        let Prepared { workspace, .. } = prepare(&v.to_string()).unwrap();
        assert_eq!(workspace.agent_display_name, "bot");
    }

    #[test]
    fn job_envelopes_carry_no_thread_key() {
        let mut v: serde_json::Value = serde_json::from_str(&envelope_json()).unwrap();
        v["thread_key"] = serde_json::Value::Null;
        let Prepared { ctx, .. } = prepare(&v.to_string()).unwrap();
        assert_eq!(ctx.agent_id.as_deref(), Some("bot"));
        assert_eq!(ctx.thread_key, None);
    }

    #[test]
    fn envelopes_are_accepted_and_marked_portable_without_activating_a_mount() {
        // the worker ACCEPTS the envelope and marks the run portable, but it
        // does NOT activate a workspace mount: no consensus-supplied cwd
        // override, no workspace env. the pool activates the plan iff a
        // provisioner is wired (ADR ROL/M2 + W1).
        let Prepared {
            input,
            ctx,
            workspace,
        } = prepare(&envelope_json()).unwrap();
        assert_eq!(input, "GENERIC\n\nCONTRACT\n\nCONVERSATION");
        assert_eq!(ctx.agent_id.as_deref(), Some("bot"));
        assert_eq!(ctx.thread_key.as_deref(), Some("general#7"));
        assert!(
            ctx.portable,
            "runs are portable and cannot resume host-local sessions"
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
        assert!(
            ctx.context_doc.is_none(),
            "the soul is assembled from MATERIALIZED skills — the envelope alone \
             cannot carry it (the mounts do not exist yet)"
        );
        // the pinned plan IS surfaced (so the pool CAN act on it when a
        // provisioner is wired) — surfacing it is not activating it.
        assert_eq!(
            workspace.source,
            crate::workspace_source::WorkspaceSource::Duckfs {
                source_prefix: "/shared/agent-workspaces/bot".into(),
                source_snapshot: Some("aa".repeat(32)),
            }
        );
        assert_eq!(
            workspace.sink,
            crate::provision::Sink::Chain,
            "no requested sink key ⇒ Chain, the default"
        );
        // the C4 skills are surfaced as ro mounts (name -> mount_subpath).
        assert_eq!(workspace.skills.len(), 1);
        assert_eq!(workspace.skills[0].mount_subpath, "release");
        assert_eq!(workspace.skills[0].source_prefix, "/shared/skills/release");
        assert_eq!(
            workspace.skills[0].source_snapshot.as_deref(),
            Some("bb".repeat(32).as_str())
        );
    }

    #[test]
    fn the_load_mode_rides_each_skill_into_the_plan_in_curation_order() {
        // the soul's whole shape depends on these two bits: WHICH skills inline
        // (the persona) and in WHAT order (curation). `always:false` is
        // on-demand (indexed, not inlined); `always:true` inlines.
        let mut v: serde_json::Value = serde_json::from_str(&envelope_json()).unwrap();
        v["skills"] = serde_json::json!([
            {"name":"persona","source_prefix":"/shared/skills/persona","always":true},
            {"name":"release","source_prefix":"/shared/skills/release","always":false},
            {"name":"legacy","source_prefix":"/shared/skills/legacy","always":false},
        ]);
        let Prepared { workspace, .. } = prepare(&v.to_string()).unwrap();
        let modes: Vec<(&str, bool)> = workspace
            .skills
            .iter()
            .map(|s| (s.mount_subpath.as_str(), s.always))
            .collect();
        assert_eq!(
            modes,
            vec![("persona", true), ("release", false), ("legacy", false)],
            "curation order survives verbatim, and always:false is on-demand"
        );
    }

    /// the library grant crosses the wall as plain data: consensus decided it
    /// (the agent's `duckfs_read` caps), and the plan carries it to the
    /// assembler, which is what decides whether the run is ever TOLD the shared
    /// library exists. a `false` grant means the run is never pointed at the
    /// library — advertising a door the tool plane would refuse is the one
    /// outcome this field exists to prevent.
    #[test]
    fn the_library_read_grant_rides_the_envelope_into_the_plan() {
        let mut v: serde_json::Value = serde_json::from_str(&envelope_json()).unwrap();
        v["library_readable"] = serde_json::json!(true);
        let Prepared { workspace, .. } = prepare(&v.to_string()).unwrap();
        assert!(workspace.library_readable);

        // the composer stating `false` (an agent with no grant) rides through
        // as false.
        let Prepared { workspace, .. } = prepare(&envelope_json()).unwrap();
        assert!(
            !workspace.library_readable,
            "a false grant is no grant: the run is never pointed at the library"
        );
    }

    #[test]
    fn the_plan_carries_the_consensus_run_id_and_tolerates_its_absence() {
        // the id `runs` resolves the run by. it MUST survive the decode: the
        // pool has no other way to name the run — its own spec id is
        // `{saga_id}:{attempt}`, which resolves nothing in consensus — so a
        // dropped id here silently kills every mid-run write the run makes.
        let Prepared { workspace, .. } = prepare(&envelope_json()).unwrap();
        assert_eq!(
            workspace.consensus_run_id.as_deref(),
            Some(CONSENSUS_RUN_ID),
            "the run id crosses the envelope verbatim, separators and all"
        );

        // ADDITIVE: an envelope composed before the field existed still runs —
        // it simply names no run, so the provisioner opens no session (the
        // read-only tool plane) rather than binding to a run that isn't there.
        let mut legacy: serde_json::Value = serde_json::from_str(&envelope_json()).unwrap();
        legacy.as_object_mut().unwrap().remove("run_id");
        let Prepared {
            input, workspace, ..
        } = prepare(&legacy.to_string()).unwrap();
        assert_eq!(input, "GENERIC\n\nCONTRACT\n\nCONVERSATION");
        assert_eq!(workspace.consensus_run_id, None);
    }

    #[test]
    fn an_old_shape_envelope_that_still_carries_mount_path_decodes_fine() {
        // the composer no longer emits mount_path (D7), but an ADDITIVE field
        // inside the tagged workspace object must never reject an in-flight
        // envelope — the extra field is tolerated (decoded and ignored).
        let mut old_shape: serde_json::Value = serde_json::from_str(&envelope_json()).unwrap();
        old_shape["workspace"]["mount_path"] = serde_json::json!("/tmp/ducktape-workspace");
        let Prepared { ctx, workspace, .. } = prepare(&old_shape.to_string()).unwrap();
        assert!(
            ctx.portable,
            "an old-shape envelope is still accepted + portable"
        );
        assert!(
            matches!(
                &workspace.source,
                crate::workspace_source::WorkspaceSource::Duckfs { source_prefix, .. }
                    if source_prefix == "/shared/agent-workspaces/bot"
            ),
            "got {:?}",
            workspace.source
        );
    }

    #[test]
    fn a_forge_envelope_is_accepted_with_its_pinned_source_and_requested_sink() {
        // the worker half of the forge lane: the tagged forge source surfaces
        // on the plan, the item context lands in the assembled input
        // (instructions → context → contract → conversation), and the
        // requested Pr sink decodes with DEFAULT-EMPTY title/body (the
        // composer omits them; delivery derives them later — contract §1/§3).
        let Prepared {
            input,
            ctx,
            workspace,
        } = prepare(&forge_envelope_json()).unwrap();
        assert_eq!(
            input,
            "GENERIC\n\nForge item context — you are working this item as a session.\n\
             repo: app\nitem: issue #7 (open)\n\nCONTRACT\n\nCONVERSATION"
        );
        assert!(ctx.portable, "a forge run is portable");
        assert_eq!(ctx.thread_key.as_deref(), Some("forge:app:7#2"));
        assert_eq!(workspace.agent_display_name, "BOT");
        assert_eq!(
            workspace.source,
            crate::workspace_source::WorkspaceSource::Forge {
                repo: "app".into(),
                item_title: Some("Fix the gate".into()),
                commit: "d0".repeat(20),
                branch: "agent/item-7".into(),
                branch_born: false,
                forge_push: true,
            }
        );
        assert_eq!(
            workspace.sink,
            crate::provision::Sink::Pr {
                repo: "app".into(),
                source_branch: "agent/item-7".into(),
                target_branch: "main".into(),
                title: String::new(),
                body: String::new(),
            }
        );
        assert!(workspace.skills.is_empty());
    }

    #[test]
    fn context_reaches_the_provider_input_between_instructions_and_contract() {
        // the coordinator-decided reading order (M1 follow-up to contract §1):
        // system instructions → item context → output contract → conversation.
        // the section is byte-exact from the envelope field, joined with the
        // SAME "\n\n" delimiter the existing sections use.
        let mut with_context: serde_json::Value = serde_json::from_str(&envelope_json()).unwrap();
        with_context["context"] = serde_json::json!("Forge item context — repo: app");
        let Prepared { input, .. } = prepare(&with_context.to_string()).unwrap();
        assert_eq!(
            input,
            "GENERIC\n\nForge item context — repo: app\n\nCONTRACT\n\nCONVERSATION"
        );

        // the None case stays byte-identical — no stray delimiter: a
        // context-less envelope assembles exactly the pre-context bytes.
        let Prepared { input, .. } = prepare(&envelope_json()).unwrap();
        assert_eq!(input, "GENERIC\n\nCONTRACT\n\nCONVERSATION");
    }

    #[test]
    fn the_retired_runtime_section_is_no_longer_assembled_into_the_input() {
        // the runtime section (tool plane + skill listing) MOVED into the
        // assembled context document. a stale composer that still emits the key
        // must not have it spliced back into the prompt — one assembly, one
        // place, or the two drift.
        let mut with_runtime: serde_json::Value = serde_json::from_str(&envelope_json()).unwrap();
        with_runtime["runtime"] = serde_json::json!("TOOL PLANE");
        let Prepared { input, .. } = prepare(&with_runtime.to_string()).unwrap();
        assert_eq!(input, "GENERIC\n\nCONTRACT\n\nCONVERSATION");
        assert!(!input.contains("TOOL PLANE"));
    }

    #[test]
    fn the_flat_pre_forge_workspace_shape_is_rejected_loudly() {
        // the workspace block is a tagged enum (`kind` = duckfs | forge). the
        // old flat duckfs shape carries no `kind` — a mixed-binary signal that
        // must fail loudly, never decode.
        let mut flat: serde_json::Value = serde_json::from_str(&envelope_json()).unwrap();
        flat["workspace"] = serde_json::json!({
            "source_prefix": "/shared/agent-workspaces/bot",
            "source_snapshot": "aa".repeat(32)
        });
        let err = prepare(&flat.to_string()).unwrap_err();
        assert!(err.contains("malformed"), "got {err:?}");
        assert!(err.contains("kind"), "names the missing tag: {err:?}");
    }

    #[test]
    fn an_envelope_that_omits_or_breaks_the_portable_shape_fails_loudly() {
        // accept still means VALIDATE: a missing/empty/wrong portable block is
        // a mixed-network signal, never silently downgraded.
        let base: serde_json::Value = serde_json::from_str(&envelope_json()).unwrap();

        let mut missing = base.clone();
        missing["workspace"] = serde_json::Value::Null;
        let err = prepare(&missing.to_string()).unwrap_err();
        assert!(err.contains("missing workspace"), "got {err:?}");

        let mut empty_prefix = base.clone();
        empty_prefix["workspace"]["source_prefix"] = serde_json::json!("");
        let err = prepare(&empty_prefix.to_string()).unwrap_err();
        assert!(
            err.contains("source_prefix must not be empty"),
            "got {err:?}"
        );

        let mut no_contract = base.clone();
        no_contract["result_contract"] = serde_json::Value::Null;
        let err = prepare(&no_contract.to_string()).unwrap_err();
        assert!(err.contains("missing result_contract"), "got {err:?}");

        let mut bad_result = base.clone();
        bad_result["result_contract"]["ducktape_runner_result"] = serde_json::json!(99);
        let err = prepare(&bad_result.to_string()).unwrap_err();
        assert!(err.contains("runner result version 99"), "got {err:?}");

        // a present skill entry with no source is a mixed-network signal too.
        let mut empty_skill = base.clone();
        empty_skill["skills"][0]["source_prefix"] = serde_json::json!("");
        let err = prepare(&empty_skill.to_string()).unwrap_err();
        assert!(
            err.contains("skill entries must carry a name and source_prefix"),
            "got {err:?}"
        );

        // the forge variant validates its own coordinates per field …
        let forge_base: serde_json::Value = serde_json::from_str(&forge_envelope_json()).unwrap();
        for field in ["repo", "commit", "branch"] {
            let mut empty_field = forge_base.clone();
            empty_field["workspace"][field] = serde_json::json!("");
            let err = prepare(&empty_field.to_string()).unwrap_err();
            assert!(
                err.contains(&format!("workspace.{field} must not be empty")),
                "{field}: got {err:?}"
            );
        }
        // … and an OMITTED forge field fails the decode itself.
        let mut missing_commit = forge_base.clone();
        missing_commit["workspace"]
            .as_object_mut()
            .unwrap()
            .remove("commit");
        let err = prepare(&missing_commit.to_string()).unwrap_err();
        assert!(err.contains("malformed"), "got {err:?}");
    }
}
