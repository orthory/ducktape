//! The `quack.toml` manifest — schema v1 (native modules).
//!
//! The manifest is the authoritative package contract. Package-local ids
//! (`logical`) are mapped to concrete module ids at install; every
//! cross-reference here (`actions[].owner`, `agents[].prompt`,
//! `engagements[].source`/`.agent`) is by logical id and must resolve within
//! the manifest. v1 accepts only `kind = "native"` modules (code that ships in
//! the node binary, so `artifact`/`abi`/`hash` are omitted); `wasm` entries
//! parse but are rejected by [`validate`] until Wasm loading lands.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Longest well-formed tag/logical-id, in bytes — the platform-wide rule (same
/// value as `capability::MAX_TAG_LEN`).
pub const MAX_TAG_LEN: usize = 64;

/// Domain prefix for the manifest hash. The raw `quack.toml` bytes are the
/// canonical artifact (they ship verbatim in the capsule), so the hash commits
/// to the file itself — no canonical-TOML re-serialization.
const MANIFEST_NAMESPACE: &[u8] = b"ducktape:quack:manifest:v1:";

/// The `schema` value this crate understands.
pub const SCHEMA_V1: u32 = 1;

/// A parsed `quack.toml`. Field order mirrors the ADR §Decision example.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageManifest {
    pub schema: u32,
    pub package: String,
    pub version: String,
    /// The harness module's logical id — which `[[modules]]` entry owns the
    /// package's lifecycle (the `HarnessMsg` receiver). Optional in the
    /// schema: install tooling that gets no explicit harness falls back to
    /// this key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harness: Option<String>,
    /// The content pin of the capsule's `harness/golden.json` proof (a
    /// `sha256:<hex>` field). Optional: a package that ships no harness proof
    /// omits it. When a capsule carries a proof it MUST be pinned here — the
    /// enforcement lives in [`crate::verify_digests`], so the golden the
    /// recipient replays before activation is the one the signature commits to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub golden: Option<String>,
    pub requires: Requires,
    #[serde(default)]
    pub modules: Vec<ModuleEntry>,
    #[serde(default)]
    pub prompts: Vec<PromptEntry>,
    #[serde(default)]
    pub actions: Vec<ActionEntry>,
    #[serde(default)]
    pub agents: Vec<AgentEntry>,
    #[serde(default)]
    pub engagements: Vec<EngagementEntry>,
    pub install: InstallPolicy,
    pub uninstall: UninstallPolicy,
}

/// Network preconditions the package needs at install time.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Requires {
    pub protocol_min: u32,
    #[serde(default)]
    pub modules: Vec<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
}

/// How a module's code is delivered. v1 only accepts [`ModuleKind::Native`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModuleKind {
    Native,
    Wasm,
}

/// One `[[modules]]` entry. For `native` modules `artifact`/`abi`/`hash` are
/// omitted (the code is in the binary); a `wasm` entry would carry them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleEntry {
    pub logical: String,
    pub default_id: String,
    pub kind: ModuleKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abi: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
}

/// One `[[prompts]]` entry: a seed prompt file plus its content digest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptEntry {
    pub logical: String,
    pub path: String,
    pub hash: String,
}

/// One `[[actions]]` entry: an open action tag owned by a module, with an
/// optional JSON schema file describing its payload. A declared `schema` is
/// content-pinned by `schema_hash` (a `sha256:<hex>` field, like a prompt's
/// `hash`) — the two travel together or not at all (see [`validate`]), so a
/// schema can never be swapped under a valid signature.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionEntry {
    pub tag: String,
    pub owner: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_hash: Option<String>,
}

/// One `[[agents]]` entry: a package-owned agent skillset.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentEntry {
    pub id: String,
    pub display_name: String,
    pub prompt: String,
    pub capability: String,
    #[serde(default)]
    pub actions: Vec<String>,
    pub status: String,
}

/// One `[[engagements]]` entry: wire a source module's event to a package agent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngagementEntry {
    pub source: String,
    pub event: String,
    pub agent: String,
    pub policy: String,
}

/// The `[install]` lifecycle policy (ADR standard).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallPolicy {
    pub register_modules: bool,
    pub seed_state: bool,
    pub register_agents: bool,
    pub register_actions: bool,
    pub wire_hooks: bool,
    pub enable_jobs: bool,
    pub run_harness: bool,
}

/// The `[uninstall]` lifecycle policy (ADR standard).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UninstallPolicy {
    pub remove_hooks: bool,
    pub pause_agents: bool,
    pub unregister_actions: bool,
    pub pending_runs: String,
    pub user_data: String,
    pub package_state: String,
}

/// Everything that can go wrong parsing or validating a manifest.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ManifestError {
    #[error("quack.toml is not valid utf-8")]
    NotUtf8,
    #[error("quack.toml is not valid TOML: {0}")]
    Toml(String),
    #[error("unsupported schema {0} (this build understands schema {SCHEMA_V1})")]
    UnsupportedSchema(u32),
    #[error("wasm loading not yet supported (module {0:?} has kind = \"wasm\")")]
    WasmUnsupported(String),
    #[error("tag/id {value:?} is malformed (want non-empty [a-z0-9._-], <= {MAX_TAG_LEN} bytes)")]
    BadTag { value: String },
    #[error("duplicate logical id {0:?}")]
    DuplicateLogical(String),
    #[error("logical id {0:?} is declared in more than one namespace (a module and a prompt)")]
    CrossNamespaceLogical(String),
    #[error("action {tag:?} declares a schema but no schema_hash to pin it")]
    SchemaWithoutHash { tag: String },
    #[error("action {tag:?} declares a schema_hash but no schema file to pin")]
    HashWithoutSchema { tag: String },
    #[error("duplicate action tag {0:?}")]
    DuplicateAction(String),
    #[error("duplicate agent id {0:?}")]
    DuplicateAgent(String),
    #[error("action {tag:?} owner {owner:?} is not a declared module")]
    DanglingOwner { tag: String, owner: String },
    #[error("agent {agent:?} references undeclared prompt {prompt:?}")]
    DanglingPrompt { agent: String, prompt: String },
    #[error("agent {agent:?} grants undeclared action tag {tag:?}")]
    UndeclaredAction { agent: String, tag: String },
    #[error("engagement source {module:?} is not a declared module")]
    DanglingSource { module: String },
    #[error("harness {module:?} is not a declared module")]
    DanglingHarness { module: String },
    #[error("engagement references undeclared agent {agent:?}")]
    DanglingEngagementAgent { agent: String },
}

/// Parse raw `quack.toml` bytes into a [`PackageManifest`] (shape only — call
/// [`validate`] for the semantic checks).
pub fn parse_manifest(toml_bytes: &[u8]) -> Result<PackageManifest, ManifestError> {
    let text = std::str::from_utf8(toml_bytes).map_err(|_| ManifestError::NotUtf8)?;
    toml::from_str(text).map_err(|e| ManifestError::Toml(e.to_string()))
}

/// The v1 semantic gate: reject `wasm` modules, malformed tags/ids, duplicate
/// logical ids, and any cross-reference that does not resolve within the
/// manifest.
pub fn validate(m: &PackageManifest) -> Result<(), ManifestError> {
    if m.schema != SCHEMA_V1 {
        return Err(ManifestError::UnsupportedSchema(m.schema));
    }

    // capability requirements are platform tags too.
    for cap in &m.requires.capabilities {
        validate_tag(cap)?;
    }
    for module in &m.requires.modules {
        validate_tag(module)?;
    }

    // modules: native-only in v1; unique logical ids; well-formed ids.
    let mut module_ids = BTreeSet::new();
    for me in &m.modules {
        if me.kind == ModuleKind::Wasm {
            return Err(ManifestError::WasmUnsupported(me.logical.clone()));
        }
        validate_tag(&me.logical)?;
        validate_tag(&me.default_id)?;
        if !module_ids.insert(me.logical.as_str()) {
            return Err(ManifestError::DuplicateLogical(me.logical.clone()));
        }
    }

    // the harness key, when present, must name a declared module.
    if let Some(harness) = &m.harness {
        validate_tag(harness)?;
        if !module_ids.contains(harness.as_str()) {
            return Err(ManifestError::DanglingHarness {
                module: harness.clone(),
            });
        }
    }

    // prompts: unique logical ids; well-formed ids; no id shared with a module
    // (logical ids are one flat namespace across modules and prompts, so the
    // install mapping is never ambiguous).
    let mut prompt_ids = BTreeSet::new();
    for pe in &m.prompts {
        validate_tag(&pe.logical)?;
        if module_ids.contains(pe.logical.as_str()) {
            return Err(ManifestError::CrossNamespaceLogical(pe.logical.clone()));
        }
        if !prompt_ids.insert(pe.logical.as_str()) {
            return Err(ManifestError::DuplicateLogical(pe.logical.clone()));
        }
    }

    // actions: well-formed tags; unique; owner resolves to a declared module; a
    // declared schema and its pin travel together (a schema with no pin is the
    // unverifiable-payload hole; a pin with no schema is a dangling reference).
    let mut action_tags = BTreeSet::new();
    for ae in &m.actions {
        validate_tag(&ae.tag)?;
        if !action_tags.insert(ae.tag.as_str()) {
            return Err(ManifestError::DuplicateAction(ae.tag.clone()));
        }
        if !module_ids.contains(ae.owner.as_str()) {
            return Err(ManifestError::DanglingOwner {
                tag: ae.tag.clone(),
                owner: ae.owner.clone(),
            });
        }
        match (&ae.schema, &ae.schema_hash) {
            (Some(_), None) => {
                return Err(ManifestError::SchemaWithoutHash {
                    tag: ae.tag.clone(),
                });
            }
            (None, Some(_)) => {
                return Err(ManifestError::HashWithoutSchema {
                    tag: ae.tag.clone(),
                });
            }
            _ => {}
        }
    }

    // agents: unique ids; prompt + granted actions resolve; well-formed ids.
    let mut agent_ids = BTreeSet::new();
    for ag in &m.agents {
        validate_tag(&ag.id)?;
        validate_tag(&ag.capability)?;
        if !agent_ids.insert(ag.id.as_str()) {
            return Err(ManifestError::DuplicateAgent(ag.id.clone()));
        }
        if !prompt_ids.contains(ag.prompt.as_str()) {
            return Err(ManifestError::DanglingPrompt {
                agent: ag.id.clone(),
                prompt: ag.prompt.clone(),
            });
        }
        for tag in &ag.actions {
            validate_tag(tag)?;
            if !action_tags.contains(tag.as_str()) {
                return Err(ManifestError::UndeclaredAction {
                    agent: ag.id.clone(),
                    tag: tag.clone(),
                });
            }
        }
    }

    // engagements: source resolves to a module; agent resolves to an agent.
    for en in &m.engagements {
        if !module_ids.contains(en.source.as_str()) {
            return Err(ManifestError::DanglingSource {
                module: en.source.clone(),
            });
        }
        if !agent_ids.contains(en.agent.as_str()) {
            return Err(ManifestError::DanglingEngagementAgent {
                agent: en.agent.clone(),
            });
        }
    }

    Ok(())
}

/// The manifest hash: `sha256(b"ducktape:quack:manifest:v1:" ++ raw bytes)`.
/// Domain-separated so a manifest hash can never collide with a bare file
/// digest (which is what `hash = "sha256:..."` fields commit to).
pub fn manifest_hash(toml_bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(MANIFEST_NAMESPACE);
    hasher.update(toml_bytes);
    hasher.finalize().into()
}

/// The one tag/logical-id rule: non-empty, at most [`MAX_TAG_LEN`] bytes,
/// charset `[a-z0-9._-]` (mirrors `capability::validate_tag`).
pub fn validate_tag(tag: &str) -> Result<(), ManifestError> {
    let ok = !tag.is_empty()
        && tag.len() <= MAX_TAG_LEN
        && tag
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b));
    if ok {
        Ok(())
    } else {
        Err(ManifestError::BadTag {
            value: tag.to_string(),
        })
    }
}

// This inline test module pushes the file past the ~600-line soft cap; the
// production code above is ~360 lines. The tests stay inline (rather than an
// external `tests/` file) because they share the `GOOD` fixture and pin the
// domain-separation invariant against the private `MANIFEST_NAMESPACE` — moving
// them out would break both.
#[cfg(test)]
mod tests {
    use super::*;

    // a minimal, valid native manifest used as the base for mutation tests.
    const GOOD: &str = r#"
schema = 1
package = "org.ducktape.docs"
version = "0.1.0"

[requires]
protocol_min = 1
modules = ["agent", "runs"]
capabilities = ["codex"]

[[modules]]
logical = "pages"
default_id = "pages"
kind = "native"

[[modules]]
logical = "docs-harness"
default_id = "docs-harness"
kind = "native"

[[prompts]]
logical = "docs_editor_prompt"
path = "prompts/docs-editor.md"
hash = "sha256:aa"

[[actions]]
tag = "pages.comment.add"
owner = "docs-harness"
schema = "actions/pages.comment.add.schema.json"
schema_hash = "sha256:aa"

[[actions]]
tag = "pages.block.update_text"
owner = "docs-harness"
schema = "actions/pages.block.update_text.schema.json"
schema_hash = "sha256:bb"

[[agents]]
id = "docs.editor"
display_name = "Docs Editor"
prompt = "docs_editor_prompt"
capability = "codex"
actions = ["pages.comment.add", "pages.block.update_text"]
status = "active"

[[engagements]]
source = "pages"
event = "comment_added"
agent = "docs.editor"
policy = "mention_or_assigned"

[install]
register_modules = true
seed_state = true
register_agents = true
register_actions = true
wire_hooks = true
enable_jobs = true
run_harness = true

[uninstall]
remove_hooks = true
pause_agents = true
unregister_actions = true
pending_runs = "drain"
user_data = "preserve"
package_state = "tombstone"
"#;

    fn good() -> PackageManifest {
        parse_manifest(GOOD.as_bytes()).expect("fixture parses")
    }

    #[test]
    fn parses_native_manifest() {
        let m = good();
        assert_eq!(m.schema, 1);
        assert_eq!(m.package, "org.ducktape.docs");
        assert_eq!(m.version, "0.1.0");
        assert_eq!(m.requires.protocol_min, 1);
        assert_eq!(m.modules.len(), 2);
        assert_eq!(m.modules[0].kind, ModuleKind::Native);
        assert!(m.modules[0].artifact.is_none());
        assert!(m.modules[0].hash.is_none());
        assert_eq!(m.prompts.len(), 1);
        assert_eq!(m.actions.len(), 2);
        assert_eq!(m.agents.len(), 1);
        assert_eq!(m.agents[0].id, "docs.editor");
        assert_eq!(m.engagements.len(), 1);
        assert!(m.install.register_agents);
        assert_eq!(m.uninstall.pending_runs, "drain");
        validate(&m).expect("fixture validates");
    }

    #[test]
    fn harness_key_parses_and_defaults_to_none() {
        // the base fixture carries no harness key.
        assert_eq!(good().harness, None);
        // an explicit key parses and validates when it names a declared module.
        let with = GOOD.replace(
            "version = \"0.1.0\"",
            "version = \"0.1.0\"\nharness = \"docs-harness\"",
        );
        let m = parse_manifest(with.as_bytes()).expect("harness key parses");
        assert_eq!(m.harness.as_deref(), Some("docs-harness"));
        validate(&m).expect("a declared harness validates");
    }

    #[test]
    fn rejects_dangling_harness() {
        let mut m = good();
        m.harness = Some("ghost".into());
        assert_eq!(
            validate(&m),
            Err(ManifestError::DanglingHarness {
                module: "ghost".into(),
            })
        );
        // the tag shape rule applies to the harness key too.
        m.harness = Some("NOT A TAG".into());
        assert!(matches!(validate(&m), Err(ManifestError::BadTag { .. })));
    }

    #[test]
    fn rejects_wasm_kind() {
        let mut m = good();
        m.modules[0].kind = ModuleKind::Wasm;
        assert_eq!(
            validate(&m),
            Err(ManifestError::WasmUnsupported("pages".into()))
        );
    }

    #[test]
    fn rejects_bad_action_tag() {
        let mut m = good();
        m.actions[0].tag = "Pages Comment!".into();
        assert!(matches!(validate(&m), Err(ManifestError::BadTag { .. })));
    }

    #[test]
    fn rejects_action_schema_without_a_hash() {
        // a declared schema with no pin is the unverifiable-payload hole.
        let mut m = good();
        m.actions[0].schema_hash = None;
        assert_eq!(
            validate(&m),
            Err(ManifestError::SchemaWithoutHash {
                tag: "pages.comment.add".into(),
            })
        );
    }

    #[test]
    fn rejects_action_hash_without_a_schema() {
        // a pin with no schema file to check is a dangling reference.
        let mut m = good();
        m.actions[0].schema = None;
        assert_eq!(
            validate(&m),
            Err(ManifestError::HashWithoutSchema {
                tag: "pages.comment.add".into(),
            })
        );
    }

    #[test]
    fn accepts_actions_with_neither_schema_nor_hash() {
        // a plain action tag (no schema) is fine — the docs actions declare
        // schemas, so clear both to exercise the neither-branch.
        let mut m = good();
        for ae in &mut m.actions {
            ae.schema = None;
            ae.schema_hash = None;
        }
        validate(&m).expect("schema-less actions validate");
    }

    #[test]
    fn parses_schema_hash_and_golden_fields() {
        let with = GOOD.replace(
            "version = \"0.1.0\"",
            "version = \"0.1.0\"\ngolden = \"sha256:cc\"",
        );
        let m = parse_manifest(with.as_bytes()).expect("golden + schema_hash parse");
        assert_eq!(m.golden.as_deref(), Some("sha256:cc"));
        assert_eq!(m.actions[0].schema_hash.as_deref(), Some("sha256:aa"));
        validate(&m).expect("validates");
    }

    #[test]
    fn rejects_dangling_owner() {
        let mut m = good();
        m.actions[0].owner = "nope".into();
        assert_eq!(
            validate(&m),
            Err(ManifestError::DanglingOwner {
                tag: "pages.comment.add".into(),
                owner: "nope".into(),
            })
        );
    }

    #[test]
    fn rejects_dangling_prompt() {
        let mut m = good();
        m.agents[0].prompt = "ghost_prompt".into();
        assert_eq!(
            validate(&m),
            Err(ManifestError::DanglingPrompt {
                agent: "docs.editor".into(),
                prompt: "ghost_prompt".into(),
            })
        );
    }

    #[test]
    fn rejects_agent_action_not_declared() {
        let mut m = good();
        m.agents[0].actions.push("pages.thread.resolve".into());
        assert_eq!(
            validate(&m),
            Err(ManifestError::UndeclaredAction {
                agent: "docs.editor".into(),
                tag: "pages.thread.resolve".into(),
            })
        );
    }

    #[test]
    fn rejects_dangling_engagement_agent() {
        let mut m = good();
        m.engagements[0].agent = "ghost.agent".into();
        assert_eq!(
            validate(&m),
            Err(ManifestError::DanglingEngagementAgent {
                agent: "ghost.agent".into(),
            })
        );
    }

    #[test]
    fn rejects_dangling_engagement_source() {
        let mut m = good();
        m.engagements[0].source = "ghost".into();
        assert_eq!(
            validate(&m),
            Err(ManifestError::DanglingSource {
                module: "ghost".into(),
            })
        );
    }

    #[test]
    fn rejects_duplicate_logical() {
        let mut m = good();
        m.modules[1].logical = "pages".into();
        assert_eq!(
            validate(&m),
            Err(ManifestError::DuplicateLogical("pages".into()))
        );
    }

    #[test]
    fn rejects_cross_namespace_logical_collision() {
        // a prompt logical that shadows a module logical: distinct namespaces
        // sharing an id is operator confusion with no legitimate use, so it is
        // rejected at validation (not silently deduped per-namespace).
        let mut m = good();
        // shadow the "pages" module logical with a prompt of the same id; keep
        // the agent's prompt reference consistent so this isolates the
        // cross-namespace rule (not a dangling reference).
        m.prompts[0].logical = "pages".into();
        m.agents[0].prompt = "pages".into();
        assert_eq!(
            validate(&m),
            Err(ManifestError::CrossNamespaceLogical("pages".into()))
        );
    }

    #[test]
    fn rejects_duplicate_action_tag() {
        let mut m = good();
        m.actions[1].tag = "pages.comment.add".into();
        assert_eq!(
            validate(&m),
            Err(ManifestError::DuplicateAction("pages.comment.add".into()))
        );
    }

    #[test]
    fn validate_tag_rules() {
        for good in ["pages.comment.add", "codex", "a", "x-y_z.0"] {
            validate_tag(good).unwrap_or_else(|_| panic!("{good:?} should pass"));
        }
        for bad in ["", "UPPER", "has space", "emoji🦆", &"a".repeat(65)] {
            assert!(validate_tag(bad).is_err(), "{bad:?} should fail");
        }
    }

    #[test]
    fn manifest_hash_stable_and_domain_separated() {
        let bytes = GOOD.as_bytes();
        // stable across calls.
        assert_eq!(manifest_hash(bytes), manifest_hash(bytes));
        // domain-separated: not a bare sha256 of the bytes.
        let bare: [u8; 32] = Sha256::digest(bytes).into();
        assert_ne!(manifest_hash(bytes), bare);
        // exactly the namespaced preimage.
        let mut h = Sha256::new();
        h.update(MANIFEST_NAMESPACE);
        h.update(bytes);
        assert_eq!(manifest_hash(bytes), <[u8; 32]>::from(h.finalize()));
        // sensitive to content.
        assert_ne!(manifest_hash(bytes), manifest_hash(b"other"));
    }

    #[test]
    fn parses_on_disk_fixture() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../packages/docs/quack.toml"
        );
        let bytes = std::fs::read(path).expect("fixture on disk");
        let m = parse_manifest(&bytes).expect("fixture parses");
        validate(&m).expect("fixture validates");
        assert_eq!(m.package, "org.ducktape.docs");
        assert_eq!(m.harness.as_deref(), Some("docs-harness"));
        assert_eq!(m.actions.len(), 3);
        assert_eq!(
            m.modules
                .iter()
                .filter(|e| e.kind == ModuleKind::Native)
                .count(),
            2
        );
    }
}
