//! the package module's public wire surface — types only, no sdk dep.
//!
//! this is the cross-module vocabulary of the quack packaged-module system:
//! the install spec a capsule resolves into, the lifecycle ops the registry
//! accepts, the [`HarnessMsg`] contract every harness module handles from the
//! package module's origin, and the [`PackageActionQuery`]/[`PackageActionMsg`]
//! contract every action OWNER module serves (the ADR's action routing
//! standard, verbatim). a harness or an action owner depends on THIS crate for
//! the shapes, never on the registry impl.

use saga::SagaOrigin;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// the canonical module id the package registry is registered under.
pub const MODULE_PACKAGE: &str = "package";

// ---- write-time caps (consensus constants) ---------------------------------
// enforced by the module BEFORE staging, with rejection, so an oversized spec
// never enters the `root()` preimage. shared here so builders (the CLI, a
// harness test) can pre-validate.

/// longest tag-shaped identifier (package ids, logical ids, action tags,
/// capability tags, event/policy names), in bytes — the platform-wide rule.
pub const MAX_TAG_LEN: usize = 64;
/// version string byte length bound.
pub const MAX_VERSION_BYTES: usize = 64;
/// a manifest hash is always a full sha256 digest.
pub const MANIFEST_HASH_LEN: usize = 32;
/// installed package rows (tombstones included).
pub const MAX_PACKAGES: usize = 256;
/// logical -> module bindings per package.
pub const MAX_MODULE_BINDINGS: usize = 16;
/// prompt seeds per package.
pub const MAX_PROMPT_SEEDS: usize = 16;
/// agent seeds per package.
pub const MAX_AGENT_SEEDS: usize = 16;
/// action routes per package.
pub const MAX_ACTION_ROUTES: usize = 32;
/// engagement rules per package.
pub const MAX_ENGAGEMENT_RULES: usize = 32;
/// granted action tags per agent seed.
pub const MAX_ACTIONS_PER_AGENT: usize = 32;
/// total registered routes (builtin + every package's), network-wide.
pub const MAX_ROUTES: usize = 1024;
/// module-id byte length bound (binding targets).
pub const MAX_MODULE_ID_BYTES: usize = 128;
/// agent display-name byte length bound.
pub const MAX_DISPLAY_NAME_BYTES: usize = 128;

/// the ONE tag shape rule: non-empty, at most [`MAX_TAG_LEN`] bytes, charset
/// `[a-z0-9._-]`. a local mirror of `capability::validate_tag` (the interface
/// stays dependency-light; the rule is platform vocabulary, not module guts).
pub fn validate_tag(tag: &str) -> Result<(), String> {
    if tag.is_empty() {
        return Err("tag must be non-empty".into());
    }
    if tag.len() > MAX_TAG_LEN {
        return Err(format!(
            "tag exceeds {MAX_TAG_LEN} bytes: {} bytes",
            tag.len()
        ));
    }
    if !tag
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
    {
        return Err(format!(
            "tag has invalid characters (want [a-z0-9._-]): {tag:?}"
        ));
    }
    Ok(())
}

/// the quack lifecycle (ADR): `Available` is the off-chain state of an
/// uninstalled capsule, so the registry's rows start at `Installing`. v1
/// unplugs within one block, so `Unplugging` is wire vocabulary only — a
/// committed row is never observed in it.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PackageStatus {
    Installing,
    Active,
    Suspended,
    Unplugging,
    Inactive,
}

/// one logical -> concrete module binding. every manifest cross-reference
/// (`harness`, `actions[].owner`, `engagements[].source`) is by logical id;
/// the registry maps it to the network's module id at install.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ModuleBinding {
    pub logical: String,
    pub module_id: String,
}

/// one prompt to seed into the memory workspace at install: the content is
/// published inline at `path`, and `sha256` is the pin agents' `PromptRef`s
/// verify at compose time — install rejects a seed whose content does not
/// hash to it.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct PromptSeed {
    pub logical: String,
    /// absolute `/`-separated memory path (memory's canonical path rules).
    pub path: String,
    pub content: String,
    pub sha256: Vec<u8>,
}

/// one agent the harness registers from module origin on install. `prompt`
/// names a [`PromptSeed`] logical; `actions` are granted tags, each declared
/// in the spec's [`ActionRoute`] list.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct AgentSeed {
    pub agent_id: String,
    pub display_name: String,
    pub capability: String,
    /// the prompt seed's logical id.
    pub prompt: String,
    pub actions: Vec<String>,
    pub active: bool,
}

/// one open action tag and the logical id of the package module that owns it.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ActionRoute {
    pub tag: String,
    /// the owning module's logical id.
    pub owner: String,
}

/// one engagement rule: `source` (logical id) emits `event`; `agent` (a
/// declared [`AgentSeed`] id) engages under `policy`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct EngagementRule {
    pub source: String,
    pub event: String,
    pub agent: String,
    pub policy: String,
}

/// the manifest's uninstall posture. v1 accepts `pending_runs` of `"drain"` or
/// `"cancel"` and only the preserve-by-default `user_data: "preserve"`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct UninstallPolicy {
    pub pending_runs: String,
    pub user_data: String,
}

/// everything one install stages and hands to the harness: the resolved form
/// of a verified capsule manifest.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct InstallSpec {
    pub package: String,
    pub version: String,
    pub manifest_hash: Vec<u8>,
    pub modules: Vec<ModuleBinding>,
    /// the harness module's logical id (must be bound in `modules`).
    pub harness: String,
    pub prompts: Vec<PromptSeed>,
    pub agents: Vec<AgentSeed>,
    pub actions: Vec<ActionRoute>,
    pub engagements: Vec<EngagementRule>,
    pub uninstall: UninstallPolicy,
}

/// write intents the package registry accepts (its `execute` payload). every
/// arm rides its caller's block and MAY fail (registration posture).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PackageMsg {
    /// validate + stage a package row (`Installing`) and its action routes,
    /// then emit — same block — one memory publish per prompt seed and
    /// [`HarnessMsg::InstallPackage`] to the harness. requires an authenticated
    /// external or module origin (v1: any member may install).
    Install(InstallSpec),
    /// the harness's install ack: flips `Installing -> Active`. accepted ONLY
    /// from the recorded harness's module origin.
    MarkActive { package: String },
    /// `Active -> Suspended`; installer- or harness-origin-gated; emits
    /// [`HarnessMsg::SuspendPackage`].
    Suspend { package: String },
    /// `Suspended -> Active`; installer- or harness-origin-gated; emits
    /// [`HarnessMsg::ResumePackage`].
    Resume { package: String },
    /// removes the package's routes and tombstones the row (`Inactive`,
    /// audit-preserving); installer- or harness-origin-gated; emits
    /// [`HarnessMsg::UnplugPackage`].
    Unplug { package: String },
}

/// read requests the package registry serves via `Module::query`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PackageQuery {
    /// the module id owning `tag`, or `None` (unrouted actions are rejected
    /// by runs before any probe).
    ActionOwner { tag: String },
    /// one package row, or `None`.
    Get { package: String },
    /// every package row (tombstones included), sorted by package id.
    List,
    /// every registered tag owned by `module` (concrete id), sorted.
    RoutesForOwner { module: String },
}

/// one package row as served by queries: the consensus registry record.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct PackageView {
    pub package: String,
    pub version: String,
    pub manifest_hash: Vec<u8>,
    pub status: PackageStatus,
    /// logical id -> concrete module id.
    pub modules: BTreeMap<String, String>,
    /// the harness's concrete module id (mapped at install).
    pub harness: String,
    /// the install origin — the owner capability for lifecycle ops.
    pub installer: SagaOrigin,
    pub uninstall: UninstallPolicy,
    pub installed_at: u64,
    pub updated_at: u64,
}

/// replies to a [`PackageQuery`]. `Option` mirrors absence.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PackageReply {
    Owner(Option<String>),
    Package(Option<PackageView>),
    Packages(Vec<PackageView>),
    Routes(Vec<String>),
}

/// the harness contract (D4): lifecycle follow-ups a harness module handles
/// ONLY when `env.origin == Origin::Module(<package module id>)`. the install
/// arm MAY fail (it rides the installer's block); on success the harness
/// registers its agents + hooks and acks with [`PackageMsg::MarkActive`].
/// suspend/resume/unplug arms pause/resume/tombstone the harness's agents.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HarnessMsg {
    InstallPackage { package: String, spec: InstallSpec },
    SuspendPackage { package: String },
    ResumePackage { package: String },
    UnplugPackage { package: String },
}

/// the action owner's read-only validation surface (the ADR contract,
/// verbatim): `Probe` arrives via `Ctx::query`; the owner validates schema,
/// target existence, caps, authorship, idempotency against
/// staged-or-committed state. `run_context` is serde_json
/// `{ run_id, agent_id, package? }`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PackageActionQuery {
    Probe {
        action_id: String,
        tag: String,
        payload: Vec<u8>,
        run_context: Vec<u8>,
    },
}

/// the probe verdict: only `Accepted` actions are emitted as [`PackageActionMsg`].
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PackageActionReply {
    Accepted,
    Rejected { reason: String },
}

/// the accepted action's write intent, riding the delivery block as a
/// follow-up. the owner's `Apply` arm is NO-FAIL: decode-or-`Ok(())`,
/// re-probe cheaply, error rows + breadcrumbs on late conflict, always
/// `Ok(())` (the dispatch-receiver contract).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PackageActionMsg {
    Apply {
        action_id: String,
        tag: String,
        payload: Vec<u8>,
        run_context: Vec<u8>,
    },
}

pub fn encode_msg(m: &PackageMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}

pub fn decode_msg(b: &[u8]) -> Result<PackageMsg, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

pub fn encode_query(q: &PackageQuery) -> Vec<u8> {
    serde_json::to_vec(q).expect("serializable")
}

pub fn decode_query(b: &[u8]) -> Result<PackageQuery, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

pub fn encode_reply(r: &PackageReply) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}

pub fn decode_reply(b: &[u8]) -> Result<PackageReply, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

pub fn encode_harness_msg(m: &HarnessMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}

pub fn decode_harness_msg(b: &[u8]) -> Result<HarnessMsg, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

pub fn encode_action_query(q: &PackageActionQuery) -> Vec<u8> {
    serde_json::to_vec(q).expect("serializable")
}

pub fn decode_action_query(b: &[u8]) -> Result<PackageActionQuery, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

pub fn encode_action_reply(r: &PackageActionReply) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}

pub fn decode_action_reply(b: &[u8]) -> Result<PackageActionReply, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

pub fn encode_action_msg(m: &PackageActionMsg) -> Vec<u8> {
    serde_json::to_vec(m).expect("serializable")
}

pub fn decode_action_msg(b: &[u8]) -> Result<PackageActionMsg, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

#[cfg(test)]
mod interface_tests {
    use super::*;

    fn spec() -> InstallSpec {
        InstallSpec {
            package: "org.example.docs".into(),
            version: "1.0.0".into(),
            manifest_hash: vec![7u8; MANIFEST_HASH_LEN],
            modules: vec![ModuleBinding {
                logical: "harness".into(),
                module_id: "docs-harness".into(),
            }],
            harness: "harness".into(),
            prompts: vec![PromptSeed {
                logical: "editor".into(),
                path: "/packages/org.example.docs/prompts/editor.md".into(),
                content: "be terse".into(),
                sha256: vec![1u8; 32],
            }],
            agents: vec![AgentSeed {
                agent_id: "docs.editor".into(),
                display_name: "Docs Editor".into(),
                capability: "claude".into(),
                prompt: "editor".into(),
                actions: vec!["pages.comment.add".into()],
                active: true,
            }],
            actions: vec![ActionRoute {
                tag: "pages.comment.add".into(),
                owner: "harness".into(),
            }],
            engagements: vec![EngagementRule {
                source: "harness".into(),
                event: "comment_added".into(),
                agent: "docs.editor".into(),
                policy: "mention_or_assigned".into(),
            }],
            uninstall: UninstallPolicy {
                pending_runs: "drain".into(),
                user_data: "preserve".into(),
            },
        }
    }

    #[test]
    fn msg_and_harness_msg_round_trip() {
        let m = PackageMsg::Install(spec());
        assert_eq!(decode_msg(&encode_msg(&m)).unwrap(), m);
        let h = HarnessMsg::InstallPackage {
            package: "org.example.docs".into(),
            spec: spec(),
        };
        assert_eq!(decode_harness_msg(&encode_harness_msg(&h)).unwrap(), h);
    }

    #[test]
    fn action_contract_round_trips() {
        let q = PackageActionQuery::Probe {
            action_id: "a1".into(),
            tag: "pages.comment.add".into(),
            payload: b"{}".to_vec(),
            run_context: b"{}".to_vec(),
        };
        assert_eq!(decode_action_query(&encode_action_query(&q)).unwrap(), q);
        let r = PackageActionReply::Rejected {
            reason: "no such block".into(),
        };
        assert_eq!(decode_action_reply(&encode_action_reply(&r)).unwrap(), r);
        let m = PackageActionMsg::Apply {
            action_id: "a1".into(),
            tag: "pages.comment.add".into(),
            payload: b"{}".to_vec(),
            run_context: b"{}".to_vec(),
        };
        assert_eq!(decode_action_msg(&encode_action_msg(&m)).unwrap(), m);
    }

    #[test]
    fn tag_rule_matches_the_platform_shape() {
        for ok in ["tasks.create", "a", "org.ducktape.docs", "x-y_z.9"] {
            assert!(validate_tag(ok).is_ok(), "{ok} should pass");
        }
        for bad in ["", "UPPER", "sp ace", "emoji✨", &"x".repeat(65)] {
            assert!(validate_tag(bad).is_err(), "{bad:?} should fail");
        }
    }
}
