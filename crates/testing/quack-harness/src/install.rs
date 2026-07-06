//! install driving: the capsule -> `InstallSpec` mapping (the CLI's mapping,
//! shared here so `package test` and package authors' suites resolve a
//! manifest identically) and the post-install report the assertion kit
//! checks the ADR install checklist against.

use std::collections::BTreeMap;

use agent::{AgentQuery, AgentReply, AgentStatus, PromptRef};
use host::Host;
use memory::{Body, MemoryQuery, MemoryReply};
use package::{
    ActionRoute, AgentSeed, EngagementRule, InstallSpec, ModuleBinding, PackageQuery, PackageReply,
    PackageStatus, PromptSeed, UninstallPolicy,
};
use quack::Capsule;
use saga::SagaOrigin;
use sha2::{Digest, Sha256};

/// map a verified capsule into the [`InstallSpec`] wire shape.
///
/// the mapping decisions (owned here, mirrored by the CLI):
/// - every `[[modules]]` logical binds to `bindings[logical]`, falling back
///   to the manifest's `default_id`.
/// - `harness_logical` names the harness module — the manifest schema carries
///   no harness marker (a v1 gap), so the caller (a fixture's `harness`
///   field, the CLI's flag) supplies it; it must be a declared module.
/// - each prompt seeds the memory path `/packages/<package>/<capsule path>`
///   with the capsule file's utf-8 content; the pin is the manifest's
///   `sha256:` digest (already verified against the bytes).
/// - agent `status` maps `"active"`/`"paused"` to the seed's `active` flag;
///   anything else is rejected.
pub fn install_spec_from_capsule(
    capsule: &Capsule,
    harness_logical: &str,
    bindings: &BTreeMap<String, String>,
) -> Result<InstallSpec, String> {
    let toml = capsule
        .manifest_bytes()
        .ok_or("no quack.toml in the capsule")?;
    let manifest = quack::parse_manifest(toml).map_err(|e| format!("manifest: {e}"))?;
    quack::validate(&manifest).map_err(|e| format!("manifest: {e}"))?;
    quack::verify_digests(capsule, &manifest).map_err(|e| format!("content digests: {e}"))?;

    let modules: Vec<ModuleBinding> = manifest
        .modules
        .iter()
        .map(|entry| ModuleBinding {
            logical: entry.logical.clone(),
            module_id: bindings
                .get(&entry.logical)
                .unwrap_or(&entry.default_id)
                .clone(),
        })
        .collect();
    if !modules.iter().any(|b| b.logical == harness_logical) {
        return Err(format!(
            "harness logical {harness_logical:?} is not a declared [[modules]] entry"
        ));
    }

    let mut prompts = Vec::new();
    for entry in &manifest.prompts {
        let bytes = capsule
            .files
            .get(&entry.path)
            .expect("verify_digests checked every prompt file");
        let content = std::str::from_utf8(bytes)
            .map_err(|_| format!("prompt {} is not utf-8: {}", entry.logical, entry.path))?
            .to_string();
        prompts.push(PromptSeed {
            logical: entry.logical.clone(),
            path: format!("/packages/{}/{}", manifest.package, entry.path),
            content,
            sha256: parse_sha256_field(&entry.hash).ok_or_else(|| {
                format!(
                    "prompt {} hash field is malformed: {}",
                    entry.logical, entry.hash
                )
            })?,
        });
    }

    let mut agents = Vec::new();
    for entry in &manifest.agents {
        let active = match entry.status.as_str() {
            "active" => true,
            "paused" => false,
            other => {
                return Err(format!(
                    "agent {} has unknown status {other:?} (want \"active\" or \"paused\")",
                    entry.id
                ));
            }
        };
        agents.push(AgentSeed {
            agent_id: entry.id.clone(),
            display_name: entry.display_name.clone(),
            capability: entry.capability.clone(),
            prompt: entry.prompt.clone(),
            actions: entry.actions.clone(),
            active,
        });
    }

    Ok(InstallSpec {
        package: manifest.package.clone(),
        version: manifest.version.clone(),
        manifest_hash: quack::manifest_hash(toml).to_vec(),
        modules,
        harness: harness_logical.to_string(),
        prompts,
        agents,
        actions: manifest
            .actions
            .iter()
            .map(|a| ActionRoute {
                tag: a.tag.clone(),
                owner: a.owner.clone(),
            })
            .collect(),
        engagements: manifest
            .engagements
            .iter()
            .map(|e| EngagementRule {
                source: e.source.clone(),
                event: e.event.clone(),
                agent: e.agent.clone(),
                policy: e.policy.clone(),
            })
            .collect(),
        uninstall: UninstallPolicy {
            pending_runs: manifest.uninstall.pending_runs.clone(),
            user_data: manifest.uninstall.user_data.clone(),
        },
    })
}

fn parse_sha256_field(field: &str) -> Option<Vec<u8>> {
    let hex = field.strip_prefix("sha256:")?;
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
        .collect()
}

/// one prompt as actually seeded: the committed memory generation + pin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeededPrompt {
    pub logical: String,
    pub path: String,
    pub generation: u64,
    pub sha256: Vec<u8>,
}

/// one agent as actually registered: the committed registry record's owner,
/// status, prompt pin, and grants.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisteredAgent {
    pub agent_id: String,
    pub owner: SagaOrigin,
    pub status: AgentStatus,
    pub capability: String,
    pub prompt: Option<PromptRef>,
    pub allowed_actions: Vec<String>,
}

/// what one install actually committed — queried from the live registry
/// modules AFTER the install block, never echoed from the spec.
#[derive(Clone, Debug)]
pub struct InstallReport {
    pub package: String,
    pub version: String,
    pub manifest_hash: Vec<u8>,
    pub status: PackageStatus,
    /// the harness's CONCRETE module id (mapped at install).
    pub harness: String,
    /// logical id -> concrete module id.
    pub modules: BTreeMap<String, String>,
    pub prompts: Vec<SeededPrompt>,
    pub agents: Vec<RegisteredAgent>,
    /// action tag -> owning concrete module id.
    pub routes: BTreeMap<String, String>,
}

impl InstallReport {
    /// the install block flipped the row to `Active` (the harness acked).
    pub fn assert_active(&self) {
        assert_eq!(
            self.status,
            PackageStatus::Active,
            "package {} is {:?}, not Active",
            self.package,
            self.status
        );
    }

    /// the prompt at `path` was seeded and its committed pin is exactly
    /// `sha256(content)` — "install seeds prompt records with expected hashes".
    pub fn assert_prompt_seeded(&self, path: &str, content: &str) {
        let seeded = self
            .prompts
            .iter()
            .find(|p| p.path == path)
            .unwrap_or_else(|| {
                panic!(
                    "no prompt seeded at {path:?}; seeded: {:?}",
                    self.prompts
                        .iter()
                        .map(|p| p.path.as_str())
                        .collect::<Vec<_>>()
                )
            });
        let expected: Vec<u8> = Sha256::digest(content.as_bytes()).to_vec();
        assert_eq!(
            seeded.sha256, expected,
            "prompt {path:?} pin does not hash the expected content"
        );
    }

    /// the agent is owned by the harness module origin — package agents are
    /// never owned by an author-minted external key.
    pub fn assert_agent_owned_by_harness(&self, agent_id: &str) {
        let agent = self
            .agents
            .iter()
            .find(|a| a.agent_id == agent_id)
            .unwrap_or_else(|| panic!("agent {agent_id:?} was not registered"));
        assert_eq!(
            agent.owner,
            SagaOrigin::Module(self.harness.clone()),
            "agent {agent_id:?} is not owned by the harness module {:?}",
            self.harness
        );
    }

    /// the action tag routes to `owner` (a concrete module id).
    pub fn assert_route(&self, tag: &str, owner: &str) {
        assert_eq!(
            self.routes.get(tag).map(String::as_str),
            Some(owner),
            "action {tag:?} routes: {:?}",
            self.routes
        );
    }
}

/// query the committed registries for what `spec`'s install actually landed.
pub(crate) async fn build_report(host: &Host, spec: &InstallSpec) -> Result<InstallReport, String> {
    // the package row.
    let reply = host
        .query(
            "package",
            &package::encode_query(&PackageQuery::Get {
                package: spec.package.clone(),
            }),
        )
        .await
        .map_err(|e| format!("package query: {e}"))?;
    let view = match package::decode_reply(&reply).map_err(|e| format!("package reply: {e}"))? {
        PackageReply::Package(Some(view)) => view,
        PackageReply::Package(None) => {
            return Err(format!("package {} has no committed row", spec.package));
        }
        other => return Err(format!("unexpected package reply: {other:?}")),
    };

    // the seeded prompts, read back from memory at their committed latest
    // generation — the report records what a `PromptRef` would resolve.
    let mut prompts = Vec::new();
    for seed in &spec.prompts {
        let reply = host
            .query(
                "memory",
                &memory::encode_query(&MemoryQuery::Read {
                    path: seed.path.clone(),
                    generation: None,
                    snapshot: None,
                }),
            )
            .await
            .map_err(|e| format!("memory query: {e}"))?;
        let generation =
            match memory::decode_reply(&reply).map_err(|e| format!("memory reply: {e}"))? {
                MemoryReply::Read(Some(generation)) => generation,
                MemoryReply::Read(None) => {
                    return Err(format!(
                        "prompt {} was not seeded at {}",
                        seed.logical, seed.path
                    ));
                }
                other => return Err(format!("unexpected memory reply: {other:?}")),
            };
        let Body::Inline(content) = &generation.body else {
            return Err(format!("prompt {} body is not inline", seed.logical));
        };
        let digest: Vec<u8> = Sha256::digest(content.as_bytes()).to_vec();
        if digest != seed.sha256 {
            return Err(format!(
                "prompt {} committed content does not hash to its pin",
                seed.logical
            ));
        }
        prompts.push(SeededPrompt {
            logical: seed.logical.clone(),
            path: seed.path.clone(),
            generation: generation.generation,
            sha256: seed.sha256.clone(),
        });
    }

    // the registered agents.
    let mut agents = Vec::new();
    for seed in &spec.agents {
        let reply = host
            .query(
                "agent",
                &agent::encode_query(&AgentQuery::Agent {
                    agent_id: seed.agent_id.clone(),
                }),
            )
            .await
            .map_err(|e| format!("agent query: {e}"))?;
        let record = match agent::decode_reply(&reply).map_err(|e| format!("agent reply: {e}"))? {
            AgentReply::Agent(Some(record)) => record,
            AgentReply::Agent(None) => {
                return Err(format!("agent {} was not registered", seed.agent_id));
            }
            other => return Err(format!("unexpected agent reply: {other:?}")),
        };
        agents.push(RegisteredAgent {
            agent_id: record.agent_id,
            owner: record.owner,
            status: record.status,
            capability: record.capability,
            prompt: record.prompt,
            allowed_actions: record.allowed_actions,
        });
    }

    // the action routes.
    let mut routes = BTreeMap::new();
    for route in &spec.actions {
        let reply = host
            .query(
                "package",
                &package::encode_query(&PackageQuery::ActionOwner {
                    tag: route.tag.clone(),
                }),
            )
            .await
            .map_err(|e| format!("package query: {e}"))?;
        match package::decode_reply(&reply).map_err(|e| format!("package reply: {e}"))? {
            PackageReply::Owner(Some(owner)) => {
                routes.insert(route.tag.clone(), owner);
            }
            PackageReply::Owner(None) => {
                return Err(format!("action {} was not routed", route.tag));
            }
            other => return Err(format!("unexpected package reply: {other:?}")),
        }
    }

    Ok(InstallReport {
        package: view.package,
        version: view.version,
        manifest_hash: view.manifest_hash,
        status: view.status,
        harness: view.harness,
        modules: view.modules,
        prompts,
        agents,
        routes,
    })
}
