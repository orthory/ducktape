//! the agent module — the platform's agent registry, and nothing more.
//!
//! a pure state-machine module (in the app-hash) holding one map: agent id →
//! record (owner, capability tag, curated skills, granted actions, status). it is
//! 100% self-contained — no other module's interface crosses this crate
//! (`SagaOrigin` and the capability tag shape rule are platform vocabulary,
//! not module guts) — and it consumes no other module's events. everything
//! that ACTS on agents (engagement, composition, dispatch, response
//! delivery) lives in the runs module, which reads this registry by query.
//!
//! the one seam left is the registry hook: a registration (and a capability
//! change) emits an [`AgentEvent`] follow-up to a genesis-configured hook
//! target — the runs module — which answers by registering/retuning the
//! agent's dispatch-plane recipe IN THE SAME BLOCK. if the recipe cannot
//! land (a squatted id), that follow-up errors, the block aborts, and the
//! staged record vanishes with it: the agent and its recipe stay one atomic
//! unit (P2) without this crate referencing the dispatch plane. the hook
//! target is an opaque module id — config, not a reference.
//!
//! ## execute routing
//!
//! - `Origin::Module(saga)` → a dead-letter no-op: any submitter can point a
//!   saga trigger's `reply_to` at this module, and that callback must never
//!   abort the saga's terminal block (the callback-poison rule);
//! - anything else → an [`AgentMsg`] (registry admin). module origins are
//!   legitimate submitters — a module may own agents.
//!
//! `root()` folds in every field of the map, so any transition moves the
//! app-hash. a joiner rebuilds this module from a peer via
//! [`AgentModule::snapshot`] / [`AgentModule::install`]: the snapshot ships
//! the committed map in the exact canonical encoding `root()` hashes, and
//! install re-derives the root from the decoded temporaries before adopting
//! them — the consensus-agreed root, not the peer, is the trust anchor.

// the wire surface: this module's shared types, flattened at the crate root.
mod interface;
pub use interface::*;

use std::collections::{BTreeMap, BTreeSet};

use capability::validate_tag;
use saga::SagaOrigin;
use sdk::{Ctx, Error, Event, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle};
use sha2::{Digest, Sha256};

/// the canonical state form of an op origin (see [`SagaOrigin`]).
fn canonical_origin(origin: &Origin) -> SagaOrigin {
    match origin {
        Origin::External(key) => SagaOrigin::External(key.clone()),
        Origin::Module(module) => SagaOrigin::Module(module.clone()),
        Origin::System => SagaOrigin::System,
    }
}

/// one registered agent. the id is the map key, so it isn't repeated here.
#[derive(Clone, Debug, PartialEq, Eq)]
struct AgentState {
    /// the registration origin — the owner capability for every mutation.
    owner: SagaOrigin,
    display_name: String,
    /// the capability registry tag this agent's runs are dispatched on —
    /// WHAT the run needs; how it executes (binary, flags, model) is host
    /// policy in each provider's spec, invisible to consensus.
    capability: String,
    /// granted action names from the known vocabulary, deduped and sorted.
    allowed_actions: BTreeSet<String>,
    /// false = paused: the agent never engages new runs.
    active: bool,
    created_at: u64,
    updated_at: u64,
    /// runtime-identity tail. all three are empty/default when unset, and
    /// [`encode_committed`] always appends them to the canonical encoding.
    ///
    /// W4 recipe content-address: empty (unset) or exactly [`RECIPE_HASH_LEN`].
    recipe_hash: Vec<u8>,
    /// D3 resource caps — each list canonical sorted+deduped.
    caps: ResourceCaps,
    /// C4 ordered skill refs — the agent's SOUL (order is significant to the
    /// hash, and to the order `Always` bodies assemble in host-side).
    skills: Vec<SkillRef>,
    /// owner-assigned semantic role; general is the legacy default.
    role: AgentRole,
}

// ---- canonical encoding -------------------------------------------------------
// u64-le counts, sorted keys, every field in declaration order: u64-le length
// prefixes for byte strings, single-byte discriminants for enums, a 0/1 tag
// byte for options, u64-le integers. this is the exact preimage
// [`Module::root`] hashes, so a snapshot and the root that must authenticate
// it cannot drift. every entry ALWAYS carries the recipe_hash/caps/skills tail
// (empty/default when unset) — the runtime identity is part of the app-hash.

fn put_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(bytes);
}

fn put_origin(out: &mut Vec<u8>, origin: &SagaOrigin) {
    match origin {
        SagaOrigin::External(key) => {
            out.push(0);
            put_bytes(out, key);
        }
        SagaOrigin::Module(module) => {
            out.push(1);
            put_bytes(out, module.as_bytes());
        }
        SagaOrigin::System => out.push(2),
    }
}

/// a canonical string SET: a u64-le count then each entry length-prefixed.
/// callers pass an already sorted+deduped slice (the write path canonicalizes;
/// the decoder rejects a non-ascending list). part of the runtime-identity tail.
fn put_str_set(out: &mut Vec<u8>, items: &[String]) {
    out.extend_from_slice(&(items.len() as u64).to_le_bytes());
    for s in items {
        put_bytes(out, s.as_bytes());
    }
}

/// the D3 caps segment: seven canonical string sets in field order, then the
/// budget as u64-le (the field is u32; the decoder range-checks). part of the
/// runtime-identity tail.
fn put_caps(out: &mut Vec<u8>, c: &ResourceCaps) {
    put_str_set(out, &c.forge_read);
    put_str_set(out, &c.forge_push);
    put_str_set(out, &c.duckfs_read);
    put_str_set(out, &c.duckfs_write);
    put_str_set(out, &c.tools);
    put_str_set(out, &c.secrets);
    put_str_set(out, &c.pages_write);
    out.extend_from_slice(&(c.subagent_budget as u64).to_le_bytes());
}

/// the C4 skills segment: a u64-le count then each ref in ORDER (order is
/// significant) — name, source_prefix, a 0/1 option tag for the pinned
/// snapshot, then the load-mode discriminant. part of the runtime-identity tail.
///
/// the load mode is IN the preimage because it decides what the host inlines
/// into the agent's assembled context document: flipping a skill from on-demand
/// to always changes what the model is, so it must change the app-hash.
fn put_skills(out: &mut Vec<u8>, skills: &[SkillRef]) {
    out.extend_from_slice(&(skills.len() as u64).to_le_bytes());
    for s in skills {
        put_bytes(out, s.name.as_bytes());
        put_bytes(out, s.source_prefix.as_bytes());
        match &s.source_snapshot {
            Some(snap) => {
                out.push(1);
                put_bytes(out, snap.as_bytes());
            }
            None => out.push(0),
        }
        out.push(match s.load {
            LoadMode::Always => 0,
            LoadMode::OnDemand => 1,
        });
    }
}

fn encode_committed(agents: &BTreeMap<String, AgentState>) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(agents.len() as u64).to_le_bytes());
    for (id, a) in agents {
        put_bytes(&mut out, id.as_bytes());
        put_origin(&mut out, &a.owner);
        put_bytes(&mut out, a.display_name.as_bytes());
        put_bytes(&mut out, a.capability.as_bytes());
        out.extend_from_slice(&(a.allowed_actions.len() as u64).to_le_bytes());
        for action in &a.allowed_actions {
            put_bytes(&mut out, action.as_bytes());
        }
        out.push(if a.active { 0 } else { 1 });
        out.extend_from_slice(&a.created_at.to_le_bytes());
        out.extend_from_slice(&a.updated_at.to_le_bytes());
        // the runtime-identity tail — ALWAYS appended (empty/default when unset).
        put_bytes(&mut out, &a.recipe_hash);
        put_caps(&mut out, &a.caps);
        put_skills(&mut out, &a.skills);
    }
    // Sparse additive role tail. If every record is General, omit it entirely
    // so legacy snapshots and their roots remain byte-for-byte valid.
    let roles = agents
        .iter()
        .filter(|(_, agent)| agent.role != AgentRole::General)
        .collect::<Vec<_>>();
    if !roles.is_empty() {
        out.extend_from_slice(&(roles.len() as u64).to_le_bytes());
        for (id, agent) in roles {
            put_bytes(&mut out, id.as_bytes());
            out.push(match agent.role {
                AgentRole::General => 0,
                AgentRole::ProjectLibrarian => 1,
            });
        }
    }
    out
}

/// the state-based commitment over the committed map — shared by `root()`
/// and `install()` so the verification a snapshot must pass is definitionally
/// the same algorithm the live module answers with.
fn committed_root(agents: &BTreeMap<String, AgentState>) -> StateRoot {
    StateRoot(Sha256::digest(encode_committed(agents)).into())
}

// ---- canonical decoding (UNTRUSTED input) ---------------------------------
// bounds are validated against the remaining input BEFORE any allocation,
// keys must be strictly ascending (one encoding per state, uniqueness for
// free), unknown discriminants/tags and trailing bytes are rejected. never
// panics on malformed input.

/// pull `n` bytes off the front of `buf`, checked before any slicing.
fn take<'a>(buf: &mut &'a [u8], n: usize) -> Result<&'a [u8], String> {
    if n > buf.len() {
        return Err("snapshot truncated".into());
    }
    let (head, tail) = buf.split_at(n);
    *buf = tail;
    Ok(head)
}

fn take_u64(buf: &mut &[u8]) -> Result<u64, String> {
    Ok(u64::from_le_bytes(
        take(buf, 8)?.try_into().expect("8 bytes"),
    ))
}

/// a length prefix, validated against the remaining input before the caller
/// allocates anything of that size.
fn take_len(buf: &mut &[u8]) -> Result<usize, String> {
    let n = take_u64(buf)?;
    if n > buf.len() as u64 {
        return Err("snapshot length prefix exceeds input".into());
    }
    Ok(n as usize)
}

fn take_lp_bytes(buf: &mut &[u8]) -> Result<Vec<u8>, String> {
    let len = take_len(buf)?;
    Ok(take(buf, len)?.to_vec())
}

fn take_lp_string(buf: &mut &[u8]) -> Result<String, String> {
    let len = take_len(buf)?;
    Ok(std::str::from_utf8(take(buf, len)?)
        .map_err(|_| "snapshot string is not utf-8".to_string())?
        .to_owned())
}

fn take_origin(buf: &mut &[u8]) -> Result<SagaOrigin, String> {
    match take(buf, 1)?[0] {
        0 => Ok(SagaOrigin::External(take_lp_bytes(buf)?)),
        1 => Ok(SagaOrigin::Module(take_lp_string(buf)?)),
        2 => Ok(SagaOrigin::System),
        d => Err(format!("snapshot has unknown origin discriminant {d}")),
    }
}

/// a section count, bounded by what the remaining input could possibly hold
/// given each entry's minimum encoded size — rejected before the loop builds
/// anything.
fn take_count(buf: &mut &[u8], min_entry_bytes: u64, what: &str) -> Result<u64, String> {
    let count = take_u64(buf)?;
    if count
        .checked_mul(min_entry_bytes)
        .is_none_or(|need| need > buf.len() as u64)
    {
        return Err(format!("snapshot {what} count exceeds input"));
    }
    Ok(count)
}

/// enforce strictly-ascending map keys while inserting.
fn insert_ascending<V>(map: &mut BTreeMap<String, V>, key: String, value: V) -> Result<(), String> {
    if let Some((last, _)) = map.iter().next_back()
        && last.as_str() >= key.as_str()
    {
        return Err("snapshot keys not strictly ascending".into());
    }
    map.insert(key, value);
    Ok(())
}

fn contains_reserved_separator(value: &str) -> bool {
    value.contains(RESERVED_ID_SEPARATOR)
}

/// the longest agent id consensus admits — one DNS label (RFC 1035), which is
/// also what an RFC 5321 local part (64 bytes) holds verbatim.
pub const MAX_AGENT_ID_LEN: usize = 63;

/// an agent id must be a legal DNS label: lowercase ASCII `[a-z0-9-]`, 1..=63
/// bytes, no leading/trailing hyphen. the id IS the agent's address — forge
/// attributes its commits to `<agent_id>@agents.duck` (`agents` is reserved in
/// duckdns, see `RESERVED_ROOT_LABELS`), so an id that is not a label cannot
/// round-trip. deliberately a COPY of duckdns's `validate_handle` shape rule
/// rather than a call into it: two consensus modules must not share an
/// admission rule that either could silently move (duckdns's reserved-label
/// list is its own business — an agent may be called `net`). the tests pin the
/// two rules to the same shape.
pub fn validate_agent_id(agent_id: &str) -> Result<(), String> {
    if agent_id.is_empty() {
        return Err("agent_id must not be empty".into());
    }
    if agent_id.len() > MAX_AGENT_ID_LEN {
        return Err(format!(
            "agent_id exceeds {MAX_AGENT_ID_LEN} bytes: {} bytes",
            agent_id.len()
        ));
    }
    if agent_id.starts_with('-') || agent_id.ends_with('-') {
        return Err(format!(
            "agent_id must not start or end with a hyphen: {agent_id:?}"
        ));
    }
    if !agent_id
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err(format!(
            "agent_id must be a DNS label (lowercase [a-z0-9-]): {agent_id:?}"
        ));
    }
    Ok(())
}

/// decode a canonical string SET, enforcing strictly-ascending order so a
/// non-canonical (unsorted / duplicated) snapshot is rejected — the same
/// discipline `allowed_actions` uses. part of the runtime-identity tail.
fn take_str_set(buf: &mut &[u8]) -> Result<Vec<String>, String> {
    let n = take_count(buf, 8, "cap")?;
    let mut v: Vec<String> = Vec::new();
    for _ in 0..n {
        let s = take_lp_string(buf)?;
        if v.last().is_some_and(|last| last.as_str() >= s.as_str()) {
            return Err("snapshot caps not strictly ascending".into());
        }
        v.push(s);
    }
    Ok(v)
}

/// decode the D3 caps segment; the budget is range-checked to `u32` so a
/// byzantine value above `u32::MAX` is rejected, never truncated. part of the
/// runtime-identity tail.
fn take_caps(buf: &mut &[u8]) -> Result<ResourceCaps, String> {
    let forge_read = take_str_set(buf)?;
    let forge_push = take_str_set(buf)?;
    let duckfs_read = take_str_set(buf)?;
    let duckfs_write = take_str_set(buf)?;
    let tools = take_str_set(buf)?;
    let secrets = take_str_set(buf)?;
    let pages_write = take_str_set(buf)?;
    let subagent_budget = u32::try_from(take_u64(buf)?)
        .map_err(|_| "snapshot subagent_budget exceeds u32".to_string())?;
    Ok(ResourceCaps {
        forge_read,
        forge_push,
        duckfs_read,
        duckfs_write,
        tools,
        secrets,
        pages_write,
        subagent_budget,
    })
}

/// decode the C4 skills segment IN ORDER (skills are an ordered list, not a
/// set — no ascending check). an unknown option tag or load discriminant is
/// rejected: one valid encoding per state. part of the runtime-identity tail.
fn take_skills(buf: &mut &[u8]) -> Result<Vec<SkillRef>, String> {
    // per-entry minimum: a name prefix, a source_prefix prefix, the option tag
    // byte, and the load-mode byte.
    let n = take_count(buf, 8 + 8 + 1 + 1, "skill")?;
    let mut v: Vec<SkillRef> = Vec::new();
    for _ in 0..n {
        let name = take_lp_string(buf)?;
        let source_prefix = take_lp_string(buf)?;
        let source_snapshot = match take(buf, 1)?[0] {
            0 => None,
            1 => Some(take_lp_string(buf)?),
            d => return Err(format!("snapshot has unknown skill snapshot tag {d}")),
        };
        let load = match take(buf, 1)?[0] {
            0 => LoadMode::Always,
            1 => LoadMode::OnDemand,
            d => return Err(format!("snapshot has unknown skill load mode {d}")),
        };
        v.push(SkillRef {
            name,
            source_prefix,
            source_snapshot,
            load,
        });
    }
    Ok(v)
}

fn decode_committed(mut buf: &[u8]) -> Result<BTreeMap<String, AgentState>, String> {
    // per-entry minimum size: an agent costs its id prefix, one origin
    // discriminant, two length prefixes (display_name, capability), an action
    // count, a status byte, two u64s, and the ALWAYS-present runtime-identity
    // tail (a recipe_hash length prefix, seven cap-set counts, the budget u64,
    // and the skills count).
    const MIN_AGENT_BYTES: u64 = (8 + 1 + 8 + 8 + 8 + 1 + 8 + 8) + 8 + 7 * 8 + 8 + 8;

    let mut agents: BTreeMap<String, AgentState> = BTreeMap::new();
    let count = take_count(&mut buf, MIN_AGENT_BYTES, "agent")?;
    for _ in 0..count {
        let id = take_lp_string(&mut buf)?;
        if contains_reserved_separator(&id) {
            return Err("snapshot agent_id contains reserved unit separator".into());
        }
        let owner = take_origin(&mut buf)?;
        let display_name = take_lp_string(&mut buf)?;
        let capability = take_lp_string(&mut buf)?;
        let mut allowed_actions: BTreeSet<String> = BTreeSet::new();
        let actions = take_count(&mut buf, 8, "action")?;
        for _ in 0..actions {
            let action = take_lp_string(&mut buf)?;
            if let Some(last) = allowed_actions.iter().next_back()
                && last.as_str() >= action.as_str()
            {
                return Err("snapshot actions not strictly ascending".into());
            }
            allowed_actions.insert(action);
        }
        let active = match take(&mut buf, 1)?[0] {
            0 => true,
            1 => false,
            d => return Err(format!("snapshot has unknown agent status {d}")),
        };
        let created_at = take_u64(&mut buf)?;
        let updated_at = take_u64(&mut buf)?;
        // the runtime-identity tail — ALWAYS present (empty/default when unset).
        let recipe_hash = take_lp_bytes(&mut buf)?;
        let caps = take_caps(&mut buf)?;
        let skills = take_skills(&mut buf)?;
        insert_ascending(
            &mut agents,
            id,
            AgentState {
                owner,
                display_name,
                capability,
                allowed_actions,
                active,
                created_at,
                updated_at,
                recipe_hash,
                caps,
                skills,
                role: AgentRole::General,
            },
        )?;
    }

    // A missing tail is the legacy encoding. A present tail is a canonical,
    // strictly-ascending sparse map of non-default role assignments.
    if !buf.is_empty() {
        let role_count = take_count(&mut buf, 8 + 1, "agent role")?;
        if role_count == 0 {
            return Err("snapshot has empty agent role tail".into());
        }
        let mut last: Option<String> = None;
        for _ in 0..role_count {
            let id = take_lp_string(&mut buf)?;
            if last.as_ref().is_some_and(|previous| previous >= &id) {
                return Err("snapshot role keys not strictly ascending".into());
            }
            let role = match take(&mut buf, 1)?[0] {
                1 => AgentRole::ProjectLibrarian,
                d => return Err(format!("snapshot has unknown or default agent role {d}")),
            };
            let agent = agents
                .get_mut(&id)
                .ok_or_else(|| format!("snapshot role names unknown agent: {id}"))?;
            agent.role = role;
            last = Some(id);
        }
        if !buf.is_empty() {
            return Err("snapshot has trailing bytes".into());
        }
    }
    Ok(agents)
}

// ---- the module -----------------------------------------------------------

pub struct AgentModule {
    id: ModuleId,
    /// dead-letter routing only: a saga callback pointed here by a foreign
    /// trigger's `reply_to` must be swallowed, never abort its block.
    saga: ModuleId,
    /// the registry hook — the module notified (same block) when an agent
    /// registers or changes capability, so the agent's dispatch recipe stays
    /// in lockstep. an opaque id: this crate never decodes its interface.
    /// `None` (test-only) means no notifications — and no recipes.
    hook: Option<ModuleId>,
    /// Recovery-only execution mode for the exact protocol-v1 native
    /// registry. Its snapshot predates roles, so role-bearing snapshots remain
    /// unavailable while that workspace advertises the revision-1 fingerprint.
    legacy_v1: bool,
    /// committed state — what `root()` and the app-hash commit to.
    agents: BTreeMap<String, AgentState>,
    /// this block's staged writes, read ahead of committed state
    /// (read-your-writes) but merged in — and reflected in `root()` — only at
    /// `commit_block`. agents are upsert-only.
    pending_agents: BTreeMap<String, AgentState>,
}

impl AgentModule {
    /// wire the module. the ids must be pairwise distinct — the saga
    /// dead-letter is origin-routed, and a colliding hook id would collapse
    /// that namespace.
    pub fn new(id: impl Into<ModuleId>, saga: impl Into<ModuleId>, hook: Option<ModuleId>) -> Self {
        let id = id.into();
        let saga = saga.into();
        let mut ids = BTreeSet::from([id.clone(), saga.clone()]);
        let mut expected = 2;
        if let Some(module) = &hook {
            ids.insert(module.clone());
            expected += 1;
        }
        assert_eq!(
            ids.len(),
            expected,
            "agent collaborator module ids must be pairwise distinct"
        );
        Self {
            id,
            saga,
            hook,
            legacy_v1: false,
            agents: BTreeMap::new(),
            pending_agents: BTreeMap::new(),
        }
    }

    /// Preserve the exact protocol-v1 snapshot/root contract while rolling a
    /// newer node binary over an existing native workspace.
    pub fn with_legacy_v1_state(mut self) -> Self {
        self.legacy_v1 = true;
        self
    }

    // ---- staged-over-committed reads ---------------------------------------

    fn agent(&self, agent_id: &str) -> Option<&AgentState> {
        self.pending_agents
            .get(agent_id)
            .or_else(|| self.agents.get(agent_id))
    }

    fn visible_ids(&self) -> Vec<String> {
        self.pending_agents
            .keys()
            .chain(self.agents.keys())
            .cloned()
            .collect::<BTreeSet<String>>()
            .into_iter()
            .collect()
    }

    // ---- views ---------------------------------------------------------------

    fn agent_view(agent_id: &str, a: &AgentState) -> AgentRecord {
        AgentRecord {
            agent_id: agent_id.to_string(),
            owner: a.owner.clone(),
            display_name: a.display_name.clone(),
            capability: a.capability.clone(),
            allowed_actions: a.allowed_actions.iter().cloned().collect(),
            status: if a.active {
                AgentStatus::Active
            } else {
                AgentStatus::Paused
            },
            role: a.role,
            created_at: a.created_at,
            updated_at: a.updated_at,
            recipe_hash: a.recipe_hash.clone(),
            caps: a.caps.clone(),
            skills: a.skills.clone(),
        }
    }

    // ---- shared validation ----------------------------------------------------

    fn validate_non_empty(field: &str, value: &str) -> Result<(), Error> {
        if value.is_empty() {
            return Err(Error::Module(format!("{field} must not be empty")));
        }
        Ok(())
    }

    /// every granted action must come from the known vocabulary, so a grant
    /// always means something; duplicates collapse into the set.
    fn validate_actions(actions: Vec<String>) -> Result<BTreeSet<String>, Error> {
        let mut set = BTreeSet::new();
        for action in actions {
            if !KNOWN_ACTIONS.contains(&action.as_str()) {
                return Err(Error::Module(format!("unknown action: {action}")));
            }
            set.insert(action);
        }
        Ok(set)
    }

    /// a v4 recipe hash is empty (unset) or exactly [`RECIPE_HASH_LEN`] bytes.
    fn validate_recipe_hash(recipe_hash: &[u8]) -> Result<(), Error> {
        if !recipe_hash.is_empty() && recipe_hash.len() != RECIPE_HASH_LEN {
            return Err(Error::Module(format!(
                "recipe_hash must be empty or {RECIPE_HASH_LEN} bytes, got {}",
                recipe_hash.len()
            )));
        }
        Ok(())
    }

    /// canonicalize the D3 caps: reject empty entries, then sort+dedup every
    /// list so the committed encoding is canonical (the decoder enforces
    /// strictly-ascending, so a non-canonicalized write would produce a
    /// snapshot no joiner accepts). budget needs no normalization.
    fn validate_caps(mut caps: ResourceCaps) -> Result<ResourceCaps, Error> {
        for list in [
            &mut caps.forge_read,
            &mut caps.forge_push,
            &mut caps.duckfs_read,
            &mut caps.duckfs_write,
            &mut caps.tools,
            &mut caps.secrets,
            &mut caps.pages_write,
        ] {
            if list.iter().any(|s| s.is_empty()) {
                return Err(Error::Module("cap entries must be non-empty".into()));
            }
            list.sort();
            list.dedup();
        }
        Ok(caps)
    }

    /// a v4 skill ref must carry a non-empty name and source_prefix; a pinned
    /// snapshot, when present, must be non-empty. order is preserved verbatim
    /// (skills are an ordered override list).
    ///
    /// the COUNT is capped ([`MAX_SKILLS_PER_AGENT`]) for the same reason the
    /// record's bytes are: the list is replicated state, and it is also the run's
    /// context budget. curation is the whole point of the tier design — a
    /// 500-skill list is a library, and the library lives in duckfs, not in the
    /// record.
    fn validate_skills(skills: &[SkillRef]) -> Result<(), Error> {
        if skills.len() > MAX_SKILLS_PER_AGENT {
            return Err(Error::Module(format!(
                "an agent may curate at most {MAX_SKILLS_PER_AGENT} skills, got {}; leave the \
                 rest in the shared skill library",
                skills.len()
            )));
        }
        for skill in skills {
            if skill.name.is_empty() {
                return Err(Error::Module("skill name must not be empty".into()));
            }
            if skill.source_prefix.is_empty() {
                return Err(Error::Module(
                    "skill source_prefix must not be empty".into(),
                ));
            }
            if let Some(snapshot) = &skill.source_snapshot
                && snapshot.is_empty()
            {
                return Err(Error::Module(
                    "skill source_snapshot must not be empty when set".into(),
                ));
            }
        }
        Ok(())
    }

    /// registry entries live in the root preimage and every snapshot —
    /// size-gate them up front (the write-time-caps lesson).
    fn validate_record_size(agent_id: &str, state: &AgentState) -> Result<(), Error> {
        let bytes =
            serde_json::to_vec(&Self::agent_view(agent_id, state)).expect("record is serializable");
        if bytes.len() > MAX_AGENT_RECORD_BYTES {
            return Err(Error::Module(format!(
                "agent record too large: {} > {MAX_AGENT_RECORD_BYTES} bytes",
                bytes.len()
            )));
        }
        Ok(())
    }

    /// the owner capability: registration takes a non-empty external key or a
    /// module as the owner. the pre-consensus empty external default and the
    /// system origin (which any genesis path could wear) never own agents.
    fn admin_origin(origin: &Origin) -> Result<SagaOrigin, Error> {
        match origin {
            Origin::External(key) if key.is_empty() => Err(Error::Module(
                "agent admin ops require a non-empty submitter id".into(),
            )),
            Origin::System => Err(Error::Module(
                "agent admin ops require an external or module origin".into(),
            )),
            other => Ok(canonical_origin(other)),
        }
    }

    fn owned_agent(&self, ctx: &dyn Ctx, agent_id: &str) -> Result<&AgentState, Error> {
        let agent = self
            .agent(agent_id)
            .ok_or_else(|| Error::Module(format!("unknown agent: {agent_id}")))?;
        if agent.owner != canonical_origin(&ctx.env().origin) {
            return Err(Error::Module(format!(
                "only the owner may modify agent {agent_id}"
            )));
        }
        Ok(agent)
    }

    /// notify the registry hook — same block, so whatever the hook stages
    /// (the agent's dispatch recipe) commits or aborts WITH the registry
    /// write that caused it.
    fn emit_hook(&self, ctx: &mut dyn Ctx, event: &AgentEvent) {
        if let Some(hook) = &self.hook {
            ctx.emit_msg(Msg {
                target: hook.clone(),
                payload: encode_event(event),
            });
        }
    }

    /// an observability breadcrumb for the dead-letter arm.
    fn note(&self, ctx: &mut dyn Ctx, what: String) {
        ctx.emit_event(Event {
            source: self.id.clone(),
            payload: what.into_bytes(),
        });
    }

    // ---- registry admin ---------------------------------------------------------

    async fn on_admin(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        let now = ctx.env().consensus_time;
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            AgentMsg::RegisterAgent {
                agent_id,
                display_name,
                capability,
                allowed_actions,
                recipe_hash,
                caps,
                skills,
            } => {
                let owner = Self::admin_origin(&ctx.env().origin)?;
                // the label rule subsumes the old non-empty + reserved-separator
                // checks (\x1f is not in [a-z0-9-]) — see `validate_agent_id`.
                validate_agent_id(&agent_id).map_err(Error::Module)?;
                Self::validate_non_empty("display_name", &display_name)?;
                validate_tag(&capability).map_err(Error::Module)?;
                let allowed_actions = Self::validate_actions(allowed_actions)?;
                let recipe_hash = recipe_hash.unwrap_or_default();
                Self::validate_recipe_hash(&recipe_hash)?;
                let caps = Self::validate_caps(caps.unwrap_or_default())?;
                let skills = skills.unwrap_or_default();
                Self::validate_skills(&skills)?;
                if self.agent(&agent_id).is_some() {
                    return Err(Error::Module(format!("agent already exists: {agent_id}")));
                }
                let state = AgentState {
                    owner,
                    display_name,
                    capability: capability.clone(),
                    allowed_actions,
                    active: true,
                    created_at: now,
                    updated_at: now,
                    recipe_hash,
                    caps,
                    skills,
                    role: AgentRole::General,
                };
                Self::validate_record_size(&agent_id, &state)?;
                // the hook stages the agent's dispatch recipe in this same
                // block: a rejected recipe (squatted or oversized id) aborts
                // the block and the staged record with it (P2).
                self.emit_hook(
                    ctx,
                    &AgentEvent::Registered {
                        agent_id: agent_id.clone(),
                        capability,
                    },
                );
                self.pending_agents.insert(agent_id, state);
                Ok(())
            }
            AgentMsg::UpdateAgent {
                agent_id,
                display_name,
                capability,
                allowed_actions,
                recipe_hash,
                caps,
                skills,
            } => {
                let mut state = self.owned_agent(&*ctx, &agent_id)?.clone();
                if let Some(display_name) = display_name {
                    Self::validate_non_empty("display_name", &display_name)?;
                    state.display_name = display_name;
                }
                if let Some(capability) = capability {
                    validate_tag(&capability).map_err(Error::Module)?;
                    if capability != state.capability {
                        // the hook retunes the agent's dispatch recipe onto
                        // the new tag, atomically with the record change.
                        self.emit_hook(
                            ctx,
                            &AgentEvent::CapabilityChanged {
                                agent_id: agent_id.clone(),
                                capability: capability.clone(),
                            },
                        );
                    }
                    state.capability = capability;
                }
                if let Some(allowed_actions) = allowed_actions {
                    state.allowed_actions = Self::validate_actions(allowed_actions)?;
                }
                // runtime-identity fields: each Some overwrites, None keeps
                // the current value.
                if let Some(recipe_hash) = recipe_hash {
                    Self::validate_recipe_hash(&recipe_hash)?;
                    state.recipe_hash = recipe_hash;
                }
                if let Some(caps) = caps {
                    state.caps = Self::validate_caps(caps)?;
                }
                if let Some(skills) = skills {
                    Self::validate_skills(&skills)?;
                    state.skills = skills;
                }
                state.updated_at = now;
                Self::validate_record_size(&agent_id, &state)?;
                self.pending_agents.insert(agent_id, state);
                Ok(())
            }
            AgentMsg::PauseAgent { agent_id } => self.stage_active(ctx, agent_id, false, now),
            AgentMsg::ResumeAgent { agent_id } => self.stage_active(ctx, agent_id, true, now),
        }
    }

    fn stage_active(
        &mut self,
        ctx: &dyn Ctx,
        agent_id: String,
        active: bool,
        now: u64,
    ) -> Result<(), Error> {
        let state = self.owned_agent(ctx, &agent_id)?;
        if state.active == active {
            // idempotent: staging nothing keeps the root byte-identical.
            return Ok(());
        }
        let mut state = state.clone();
        state.active = active;
        state.updated_at = now;
        self.pending_agents.insert(agent_id, state);
        Ok(())
    }

    // ---- state-sync ---------------------------------------------------------
    // hand a joiner the committed continuation state as canonical bytes; the
    // consensus-agreed root — never the serving peer — decides whether they land.

    /// serialize the COMMITTED continuation state (never the staged overlay)
    /// into the canonical encoding `root()` commits to. deterministic across
    /// nodes — a plain body, no container magic.
    pub fn snapshot(&self) -> Vec<u8> {
        encode_committed(&self.agents)
    }

    /// adopt a peer's snapshot as own committed state — but only after the
    /// decoded temporaries re-derive `expected` via the exact `root()`
    /// algorithm, so a byzantine snapshot cannot land under an agreed root it
    /// doesn't match. all-or-nothing: on any Err this module (and its root)
    /// is byte-identical to before the call. on success the staged overlay is
    /// dropped — a snapshot describes a block boundary, and nothing
    /// half-applied may shadow it.
    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        let agents = decode_committed(bytes).map_err(Error::Module)?;
        if self.legacy_v1
            && agents
                .values()
                .any(|agent| agent.role != AgentRole::General)
        {
            return Err(Error::Module(
                "native-v1 agent snapshot contains a role tail".into(),
            ));
        }
        if committed_root(&agents) != expected {
            return Err(Error::Module(
                "snapshot does not match expected root".into(),
            ));
        }
        self.agents = agents;
        self.pending_agents.clear();
        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl Module for AgentModule {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn state_schema_revision(&self) -> u32 {
        if self.legacy_v1 {
            1
        } else {
            2
        }
    }

    /// state-based commitment: sha256 over the canonical committed encoding —
    /// a length-prefixed fold of every agent field in sorted-key order.
    /// sensitive to every field, so any transition moves the root. the
    /// preimage IS the snapshot encoding.
    fn root(&self) -> StateRoot {
        committed_root(&self.agents)
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::SnapshotBytes(self.snapshot()))
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        let origin = ctx.env().origin.clone();
        match origin {
            Origin::Module(module) if module == self.saga => {
                // dead letter: nothing here rides the saga, but any trigger's
                // reply_to can point a callback at this module — it must
                // never abort the saga's terminal block.
                self.note(ctx, "dropped a direct saga callback".into());
                Ok(())
            }
            // module origins fall through: a module is a legitimate agent
            // owner, so its submissions are admin ops like any other.
            _ => self.on_admin(ctx, msg).await,
        }
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            AgentQuery::Agents => {
                let agents = self
                    .visible_ids()
                    .into_iter()
                    .filter_map(|id| self.agent(&id).map(|a| Self::agent_view(&id, a)))
                    .collect();
                Ok(encode_reply(&AgentReply::Agents(agents)))
            }
            AgentQuery::Agent { agent_id } => Ok(encode_reply(&AgentReply::Agent(
                self.agent(&agent_id)
                    .map(|a| Self::agent_view(&agent_id, a)),
            ))),
        }
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        for (id, agent) in std::mem::take(&mut self.pending_agents) {
            self.agents.insert(id, agent);
        }
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.pending_agents.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ACTION_CHAT_POST, ACTION_TASKS_CREATE, decode_event, decode_reply, encode_msg,
        encode_query,
    };
    use futures::executor::block_on;
    use sdk::Env;

    /// a minimal `Ctx` that captures emitted msgs/effects/events — enough to
    /// unit-test `execute` in isolation (the host provides the real routing
    /// in integration).
    struct CaptureCtx {
        env: Env,
        msgs: Vec<Msg>,
        #[allow(dead_code)]
        events: Vec<Event>,
    }
    impl CaptureCtx {
        fn new() -> Self {
            Self {
                env: Env {
                    protocol_version: 0,
                    height: 0,
                    consensus_time: 0,
                    origin: Origin::System,
                    me: "agent".into(),
                },
                msgs: Vec::new(),
                events: Vec::new(),
            }
        }
        fn at(mut self, view: u64) -> Self {
            self.env.height = view;
            self.env.consensus_time = view;
            self
        }
        fn with_origin(mut self, origin: Origin) -> Self {
            self.env.origin = origin;
            self
        }
        /// decoded hook events emitted this dispatch.
        fn hook_events(&self) -> Vec<AgentEvent> {
            self.msgs
                .iter()
                .filter(|m| m.target == "runs")
                .map(|m| decode_event(&m.payload).expect("hook event"))
                .collect()
        }
    }
    #[async_trait::async_trait(?Send)]
    impl Ctx for CaptureCtx {
        fn env(&self) -> &Env {
            &self.env
        }
        fn module_root(&self, _target: &str) -> Option<StateRoot> {
            None
        }
        async fn query(&self, target: &str, _req: &[u8]) -> Result<Vec<u8>, Error> {
            Err(Error::UnknownModule(target.into()))
        }
        fn emit_msg(&mut self, msg: Msg) {
            self.msgs.push(msg);
        }
        fn emit_event(&mut self, ev: Event) {
            self.events.push(ev);
        }
    }

    // ---- fixtures -----------------------------------------------------------

    fn module() -> AgentModule {
        AgentModule::new("agent", "saga", Some("runs".into()))
    }

    fn user(byte: u8) -> Origin {
        Origin::External(vec![byte; 32])
    }

    #[test]
    fn agent_response_commit_message_is_optional_and_round_trips_exactly() {
        let legacy = decode_response(br#"{"reply_blocks":[],"actions":[]}"#).unwrap();
        assert_eq!(legacy.commit_message, None);
        assert!(legacy.delegations.is_empty());

        let message = "fix: exact subject\n\nExact body.";
        let response = AgentResponse {
            commit_message: Some(message.into()),
            ..AgentResponse::default()
        };
        assert_eq!(
            decode_response(&encode_response(&response))
                .unwrap()
                .commit_message
                .as_deref(),
            Some(message)
        );
    }

    fn register(agent_id: &str, actions: &[&str]) -> AgentMsg {
        AgentMsg::RegisterAgent {
            agent_id: agent_id.into(),
            display_name: agent_id.to_uppercase(),
            capability: "model-1".into(),
            allowed_actions: actions.iter().map(|s| s.to_string()).collect(),
            recipe_hash: None,
            caps: None,
            skills: None,
        }
    }

    /// a registration carrying the runtime-identity fields. `recipe_hash`
    /// empty => omitted (None); caps/skills always present so the op exercises
    /// the runtime-identity acceptance path.
    fn register_runtime(
        agent_id: &str,
        caps: ResourceCaps,
        skills: Vec<SkillRef>,
        recipe_hash: Vec<u8>,
    ) -> AgentMsg {
        AgentMsg::RegisterAgent {
            agent_id: agent_id.into(),
            display_name: agent_id.to_uppercase(),
            capability: "model-1".into(),
            allowed_actions: vec![],
            recipe_hash: (!recipe_hash.is_empty()).then_some(recipe_hash),
            caps: Some(caps),
            skills: Some(skills),
        }
    }

    /// an `AgentRecord` with the given caps and no skills — for the pure
    /// [`AgentRecord::permits`] gate and the keyless-D1 serde assertion.
    fn record_with_caps(caps: ResourceCaps) -> AgentRecord {
        AgentRecord {
            agent_id: "bot".into(),
            owner: SagaOrigin::External(vec![9; 32]),
            display_name: "BOT".into(),
            capability: "model-1".into(),
            allowed_actions: vec![],
            status: AgentStatus::Active,
            role: AgentRole::General,
            created_at: 0,
            updated_at: 0,
            recipe_hash: vec![],
            caps,
            skills: vec![],
        }
    }

    /// the fixed 2-agent registry the golden-hex + container-gating tests pin.
    fn build_fixture_registry() -> AgentModule {
        let mut m = module();
        let mut ctx = CaptureCtx::new().at(3).with_origin(user(9));
        exec(
            &mut m,
            &mut ctx,
            &admin(&register("alpha", &[ACTION_CHAT_POST, ACTION_TASKS_CREATE])),
        )
        .unwrap();
        exec(&mut m, &mut ctx, &admin(&register("beta", &[]))).unwrap();
        commit(&mut m);
        m
    }

    /// lowercase hex, for the golden-byte pin.
    fn hex(bytes: &[u8]) -> String {
        use std::fmt::Write as _;
        let mut s = String::with_capacity(bytes.len() * 2);
        for b in bytes {
            let _ = write!(s, "{b:02x}");
        }
        s
    }

    fn unhex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
            .collect()
    }

    fn admin(m: &AgentMsg) -> Msg {
        Msg {
            target: "agent".into(),
            payload: encode_msg(m),
        }
    }

    fn exec(m: &mut AgentModule, ctx: &mut CaptureCtx, op: &Msg) -> Result<(), Error> {
        block_on(m.execute(ctx, op))
    }

    fn commit(m: &mut AgentModule) {
        block_on(m.commit_block()).unwrap();
    }

    fn abort(m: &mut AgentModule) {
        block_on(m.abort_block()).unwrap();
    }

    fn get_agent(m: &AgentModule, agent_id: &str) -> Option<AgentRecord> {
        let reply = block_on(m.query(&encode_query(&AgentQuery::Agent {
            agent_id: agent_id.into(),
        })))
        .unwrap();
        match decode_reply(&reply).unwrap() {
            AgentReply::Agent(record) => record,
            other => panic!("unexpected reply: {other:?}"),
        }
    }

    fn list_agents(m: &AgentModule) -> Vec<AgentRecord> {
        let reply = block_on(m.query(&encode_query(&AgentQuery::Agents))).unwrap();
        match decode_reply(&reply).unwrap() {
            AgentReply::Agents(records) => records,
            other => panic!("unexpected reply: {other:?}"),
        }
    }

    // ---- registry admin -------------------------------------------------------

    #[test]
    fn register_validates_stages_an_active_agent_and_notifies_the_hook() {
        let mut m = module();
        let mut ctx = CaptureCtx::new().at(3).with_origin(user(9));
        exec(
            &mut m,
            &mut ctx,
            &admin(&register("bot", &[ACTION_CHAT_POST])),
        )
        .unwrap();
        commit(&mut m);

        let record = get_agent(&m, "bot").unwrap();
        assert_eq!(record.owner, SagaOrigin::External(vec![9; 32]));
        assert_eq!(record.status, AgentStatus::Active);
        assert_eq!(record.allowed_actions, vec![ACTION_CHAT_POST.to_string()]);
        assert_eq!(record.created_at, 3);

        // the hook is notified in the same block, so the agent's dispatch
        // recipe lands (or aborts) atomically with the record.
        assert_eq!(
            ctx.hook_events(),
            vec![AgentEvent::Registered {
                agent_id: "bot".into(),
                capability: "model-1".into(),
            }]
        );

        // duplicate registration is an error, even from the same owner.
        let err = exec(
            &mut m,
            &mut ctx,
            &admin(&register("bot", &[ACTION_CHAT_POST])),
        )
        .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        abort(&mut m);
    }

    /// the COUNT cap, exercised straight against the validator so the record's
    /// BYTE cap cannot be what rejects it: two rules that both fire is two rules
    /// that can drift, and this is the one the host-side assembler shares.
    #[test]
    fn a_curated_skill_list_over_the_count_cap_is_refused_loudly() {
        let skills: Vec<SkillRef> = (0..=MAX_SKILLS_PER_AGENT)
            .map(|i| SkillRef {
                name: format!("s{i}"),
                source_prefix: "/p".into(),
                source_snapshot: None,
                load: LoadMode::OnDemand,
            })
            .collect();
        let err = AgentModule::validate_skills(&skills).unwrap_err();
        assert!(
            matches!(&err, Error::Module(m) if m.contains(&MAX_SKILLS_PER_AGENT.to_string())),
            "the refusal must name the cap: {err:?}"
        );
        // one under is fine — the cap is a ceiling, not an off-by-one trap.
        assert!(AgentModule::validate_skills(&skills[..MAX_SKILLS_PER_AGENT]).is_ok());
    }

    #[test]
    fn register_rejects_bad_shapes_and_bad_origins() {
        let mut m = module();
        let root0 = m.root();
        let cases: Vec<(Origin, AgentMsg)> = vec![
            // the pre-consensus empty external default never owns an agent.
            (Origin::External(Vec::new()), register("a", &[])),
            // system is not an ownable origin (spec: external or module).
            (Origin::System, register("a", &[])),
            // a recipe hash that is neither empty nor exactly 32 bytes.
            (
                user(9),
                AgentMsg::RegisterAgent {
                    agent_id: "a".into(),
                    display_name: "A".into(),
                    capability: "m".into(),
                    allowed_actions: Vec::new(),
                    recipe_hash: Some(vec![7u8; 31]),
                    caps: None,
                    skills: None,
                },
            ),
            // a skill that names no source — an unmountable soul.
            (
                user(9),
                AgentMsg::RegisterAgent {
                    agent_id: "a".into(),
                    display_name: "A".into(),
                    capability: "m".into(),
                    allowed_actions: Vec::new(),
                    recipe_hash: None,
                    caps: None,
                    skills: Some(vec![SkillRef {
                        name: "persona".into(),
                        source_prefix: String::new(),
                        source_snapshot: None,
                        load: LoadMode::Always,
                    }]),
                },
            ),
            // an action outside the known vocabulary.
            (
                user(9),
                AgentMsg::RegisterAgent {
                    agent_id: "a".into(),
                    display_name: "A".into(),
                    capability: "m".into(),
                    allowed_actions: vec!["forge.push".into()],
                    recipe_hash: None,
                    caps: None,
                    skills: None,
                },
            ),
            // empty required fields.
            (
                user(9),
                AgentMsg::RegisterAgent {
                    agent_id: String::new(),
                    display_name: "A".into(),
                    capability: "m".into(),
                    allowed_actions: Vec::new(),
                    recipe_hash: None,
                    caps: None,
                    skills: None,
                },
            ),
            (
                user(9),
                AgentMsg::RegisterAgent {
                    agent_id: "bad\u{1f}id".into(),
                    display_name: "A".into(),
                    capability: "m".into(),
                    allowed_actions: Vec::new(),
                    recipe_hash: None,
                    caps: None,
                    skills: None,
                },
            ),
            (
                user(9),
                AgentMsg::RegisterAgent {
                    agent_id: "a".into(),
                    display_name: "A".into(),
                    capability: String::new(),
                    allowed_actions: Vec::new(),
                    recipe_hash: None,
                    caps: None,
                    skills: None,
                },
            ),
            // an oversized record is rejected before staging.
            (
                user(9),
                AgentMsg::RegisterAgent {
                    agent_id: "a".into(),
                    display_name: "x".repeat(MAX_AGENT_RECORD_BYTES),
                    capability: "m".into(),
                    allowed_actions: Vec::new(),
                    recipe_hash: None,
                    caps: None,
                    skills: None,
                },
            ),
        ];
        for (origin, op) in cases {
            let mut ctx = CaptureCtx::new().with_origin(origin.clone());
            let err = exec(&mut m, &mut ctx, &admin(&op)).unwrap_err();
            assert!(matches!(err, Error::Module(_)), "{origin:?} / {op:?}");
            assert!(
                ctx.hook_events().is_empty(),
                "a rejected register never notifies the hook: {op:?}"
            );
            abort(&mut m);
            assert_eq!(m.root(), root0, "a rejected register leaves no trace");
        }
    }

    #[test]
    fn update_pause_resume_are_owner_gated() {
        let mut m = module();
        let mut ctx = CaptureCtx::new().with_origin(user(9));
        exec(&mut m, &mut ctx, &admin(&register("bot", &[]))).unwrap();
        commit(&mut m);

        // a foreign origin can neither update nor pause.
        for op in [
            AgentMsg::UpdateAgent {
                agent_id: "bot".into(),
                display_name: Some("Stolen".into()),
                capability: None,
                allowed_actions: None,
                recipe_hash: None,
                caps: None,
                skills: None,
            },
            AgentMsg::PauseAgent {
                agent_id: "bot".into(),
            },
            AgentMsg::ResumeAgent {
                agent_id: "bot".into(),
            },
        ] {
            let mut ctx = CaptureCtx::new().with_origin(user(2));
            let err = exec(&mut m, &mut ctx, &admin(&op)).unwrap_err();
            assert!(matches!(err, Error::Module(_)));
            abort(&mut m);
        }

        // the owner updates fields selectively and toggles status.
        let mut ctx = CaptureCtx::new().at(5).with_origin(user(9));
        exec(
            &mut m,
            &mut ctx,
            &admin(&AgentMsg::UpdateAgent {
                agent_id: "bot".into(),
                display_name: None,
                capability: Some("model-2".into()),
                allowed_actions: Some(vec![ACTION_TASKS_CREATE.into()]),
                recipe_hash: None,
                caps: None,
                skills: None,
            }),
        )
        .unwrap();
        // a capability change notifies the hook, which retunes the agent's
        // dispatch recipe atomically.
        assert_eq!(
            ctx.hook_events(),
            vec![AgentEvent::CapabilityChanged {
                agent_id: "bot".into(),
                capability: "model-2".into(),
            }]
        );
        exec(
            &mut m,
            &mut ctx,
            &admin(&AgentMsg::PauseAgent {
                agent_id: "bot".into(),
            }),
        )
        .unwrap();
        commit(&mut m);
        let record = get_agent(&m, "bot").unwrap();
        assert_eq!(record.capability, "model-2");
        assert_eq!(record.display_name, "BOT", "unset fields keep their value");
        assert_eq!(
            record.allowed_actions,
            vec![ACTION_TASKS_CREATE.to_string()]
        );
        assert_eq!(record.status, AgentStatus::Paused);
        assert_eq!(record.updated_at, 5);

        // an update that keeps the capability does NOT notify the hook.
        let mut ctx = CaptureCtx::new().at(6).with_origin(user(9));
        exec(
            &mut m,
            &mut ctx,
            &admin(&AgentMsg::UpdateAgent {
                agent_id: "bot".into(),
                display_name: Some("Bot".into()),
                capability: Some("model-2".into()),
                allowed_actions: None,
                recipe_hash: None,
                caps: None,
                skills: None,
            }),
        )
        .unwrap();
        assert!(ctx.hook_events().is_empty(), "same capability, no retune");
        commit(&mut m);

        // pausing a paused agent stages nothing: root byte-identical.
        let paused_root = m.root();
        let mut ctx = CaptureCtx::new().at(7).with_origin(user(9));
        exec(
            &mut m,
            &mut ctx,
            &admin(&AgentMsg::PauseAgent {
                agent_id: "bot".into(),
            }),
        )
        .unwrap();
        commit(&mut m);
        assert_eq!(m.root(), paused_root, "an idempotent pause is a no-op");

        let mut ctx = CaptureCtx::new().at(8).with_origin(user(9));
        exec(
            &mut m,
            &mut ctx,
            &admin(&AgentMsg::ResumeAgent {
                agent_id: "bot".into(),
            }),
        )
        .unwrap();
        commit(&mut m);
        assert_eq!(get_agent(&m, "bot").unwrap().status, AgentStatus::Active);
    }

    #[test]
    fn a_direct_saga_callback_is_a_dead_letter() {
        let mut m = module();
        let root0 = m.root();
        let mut ctx = CaptureCtx::new().with_origin(Origin::Module("saga".into()));
        // arbitrary (non-AgentMsg) bytes: the callback-poison rule says this
        // must be swallowed, never abort the saga's terminal block.
        exec(
            &mut m,
            &mut ctx,
            &Msg {
                target: "agent".into(),
                payload: b"not an agent msg".to_vec(),
            },
        )
        .unwrap();
        assert!(!ctx.events.is_empty(), "the tombstone leaves a breadcrumb");
        commit(&mut m);
        assert_eq!(m.root(), root0);
    }

    #[test]
    fn a_module_origin_may_own_an_agent() {
        let mut m = module();
        let mut ctx = CaptureCtx::new().with_origin(Origin::Module("automations".into()));
        exec(&mut m, &mut ctx, &admin(&register("bot", &[]))).unwrap();
        commit(&mut m);
        assert_eq!(
            get_agent(&m, "bot").unwrap().owner,
            SagaOrigin::Module("automations".into())
        );
    }

    #[test]
    fn without_a_hook_registration_stages_but_notifies_nobody() {
        let mut m = AgentModule::new("agent", "saga", None);
        let mut ctx = CaptureCtx::new().with_origin(user(9));
        exec(&mut m, &mut ctx, &admin(&register("bot", &[]))).unwrap();
        assert!(ctx.msgs.is_empty(), "no hook, no follow-ups");
        commit(&mut m);
        assert!(get_agent(&m, "bot").is_some());
    }

    // ---- queries + state sync ---------------------------------------------------

    #[test]
    fn queries_list_agents_staged_over_committed() {
        let mut m = module();
        let mut ctx = CaptureCtx::new().with_origin(user(9));
        exec(&mut m, &mut ctx, &admin(&register("alpha", &[]))).unwrap();
        commit(&mut m);
        exec(&mut m, &mut ctx, &admin(&register("beta", &[]))).unwrap();

        // the staged agent is visible before the boundary (read-your-writes)…
        let ids: Vec<String> = list_agents(&m).into_iter().map(|a| a.agent_id).collect();
        assert_eq!(ids, vec!["alpha".to_string(), "beta".to_string()]);
        // …but not committed: an abort drops it.
        abort(&mut m);
        let ids: Vec<String> = list_agents(&m).into_iter().map(|a| a.agent_id).collect();
        assert_eq!(ids, vec!["alpha".to_string()]);
        assert!(get_agent(&m, "beta").is_none());
    }

    #[test]
    fn two_instances_replaying_the_same_ops_produce_identical_roots() {
        let ops = [
            (user(9), register("alpha", &[ACTION_CHAT_POST])),
            (user(9), register("beta", &[])),
            (
                user(9),
                AgentMsg::UpdateAgent {
                    agent_id: "alpha".into(),
                    display_name: None,
                    capability: Some("model-2".into()),
                    allowed_actions: None,
                    recipe_hash: None,
                    caps: None,
                    skills: None,
                },
            ),
            (
                user(9),
                AgentMsg::PauseAgent {
                    agent_id: "beta".into(),
                },
            ),
        ];
        let run = || {
            let mut m = module();
            for (i, (origin, op)) in ops.iter().enumerate() {
                let mut ctx = CaptureCtx::new().at(i as u64 + 1).with_origin(origin.clone());
                exec(&mut m, &mut ctx, &admin(op)).unwrap();
                commit(&mut m);
            }
            m
        };
        let (a, b) = (run(), run());
        assert_eq!(a.root(), b.root());
        assert_ne!(a.root(), module().root(), "state moved the root");
    }

    #[test]
    fn snapshots_install_only_under_their_own_root() {
        let mut m = module();
        let mut ctx = CaptureCtx::new().at(1).with_origin(user(9));
        exec(
            &mut m,
            &mut ctx,
            &admin(&register("alpha", &[ACTION_CHAT_POST])),
        )
        .unwrap();
        exec(&mut m, &mut ctx, &admin(&register("beta", &[]))).unwrap();
        commit(&mut m);

        let bytes = m.snapshot();
        let root = m.root();

        let mut joiner = module();
        joiner.install(&bytes, root).unwrap();
        assert_eq!(joiner.root(), root);
        assert_eq!(
            list_agents(&joiner).len(),
            2,
            "the joiner serves the installed registry"
        );

        // a snapshot under a foreign root is rejected, leaving no trace.
        let mut fresh = module();
        let before = fresh.root();
        assert!(fresh.install(&bytes, before).is_err());
        assert_eq!(fresh.root(), before);

        // truncated bytes never land.
        let mut fresh = module();
        assert!(fresh.install(&bytes[..bytes.len() - 1], root).is_err());
    }

    #[test]
    fn state_sync_handle_exposes_the_snapshot_bytes() {
        let mut m = module();
        let mut ctx = CaptureCtx::new().with_origin(user(9));
        exec(&mut m, &mut ctx, &admin(&register("alpha", &[]))).unwrap();
        commit(&mut m);
        match m.state_sync_handle().unwrap() {
            StateSyncHandle::SnapshotBytes(bytes) => assert_eq!(bytes, m.snapshot()),
            other => panic!("unexpected handle: {other:?}"),
        }
    }

    // ---- runtime identity ------------------------------------------------------

    /// the load-bearing determinism proof: a stable golden of the committed
    /// encoding over the fixed 2-agent fixture. the record ALWAYS carries the
    /// recipe_hash/caps/skills tail (empty/default here); if the encoding ever
    /// drifts, this fails loudly.
    ///
    /// re-pinned for the SOUL flag day: `prompt_hash` retired, so every agent
    /// loses its 8-byte length prefix and 32 pin bytes from the preimage (each
    /// skill entry also gains a load-mode byte, invisible here — the fixture
    /// mounts none). the app-hash moves; that is the flag day, declared.
    #[test]
    fn committed_bytes_match_the_golden() {
        const GOLDEN_HEX: &str = "02000000000000000500000000000000616c70686100200000000000000009090909090909090909090909090909090909090909090909090909090909090500000000000000414c50484107000000000000006d6f64656c2d3102000000000000000900000000000000636861742e706f73740c000000000000007461736b732e63726561746500030000000000000003000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000040000000000000062657461002000000000000000090909090909090909090909090909090909090909090909090909090909090904000000000000004245544107000000000000006d6f64656c2d31000000000000000000030000000000000003000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";
        let m = build_fixture_registry();
        assert_eq!(
            hex(&m.snapshot()),
            GOLDEN_HEX,
            "the committed bytes must never move"
        );
        assert_eq!(
            m.root(),
            StateRoot(Sha256::digest(unhex(GOLDEN_HEX)).into()),
            "root is sha256 of exactly the golden bytes"
        );
    }

    /// `root()` hashes exactly the snapshot bytes (there is no container magic),
    /// and a snapshot round-trips into a fresh joiner under the agreed root.
    #[test]
    fn snapshot_install_round_trips_and_root_hashes_the_snapshot() {
        let m = build_fixture_registry();
        let (bytes, root) = (m.snapshot(), m.root());
        assert_eq!(
            root,
            StateRoot(Sha256::digest(&bytes).into()),
            "root() hashes exactly the snapshot bytes"
        );
        let mut joiner = module();
        joiner.install(&bytes, root).unwrap();
        assert_eq!(joiner.root(), root);
        assert_eq!(joiner.snapshot(), bytes, "the joiner re-encodes identically");
    }

    /// register the runtime-identity fields, commit, snapshot -> install into a
    /// fresh joiner: roots and every field round-trip.
    #[test]
    fn round_trips_recipe_caps_and_skills() {
        let mut m = module();
        let mut ctx = CaptureCtx::new().at(3).with_origin(user(9));
        let caps = ResourceCaps {
            forge_read: vec!["repo-a".into()],
            forge_push: vec!["repo-a".into()],
            duckfs_read: vec!["src".into()],
            duckfs_write: vec!["out".into()],
            tools: vec!["bash".into()],
            secrets: vec!["vault://k".into()],
            pages_write: vec!["*".into(), "page-1".into()],
            subagent_budget: 2,
        };
        // one of each load mode: the persona (always) and a reference skill.
        let skills = vec![
            SkillRef {
                name: "fmt".into(),
                source_prefix: "/shared/skills/fmt".into(),
                source_snapshot: Some("aa".repeat(32)),
                load: LoadMode::Always,
            },
            SkillRef {
                name: "lint".into(),
                source_prefix: "/shared/skills/lint".into(),
                source_snapshot: None,
                load: LoadMode::OnDemand,
            },
        ];
        exec(
            &mut m,
            &mut ctx,
            &admin(&register_runtime(
                "bot",
                caps.clone(),
                skills.clone(),
                vec![9u8; RECIPE_HASH_LEN],
            )),
        )
        .unwrap();
        commit(&mut m);
        let rec = get_agent(&m, "bot").unwrap();
        assert_eq!(rec.caps, caps);
        assert_eq!(rec.skills, skills);
        assert_eq!(rec.recipe_hash, vec![9u8; RECIPE_HASH_LEN]);

        let (bytes, root) = (m.snapshot(), m.root());
        let mut joiner = module();
        joiner.install(&bytes, root).unwrap();
        assert_eq!(joiner.root(), root);
        let jrec = get_agent(&joiner, "bot").unwrap();
        assert_eq!(jrec.caps, caps);
        assert_eq!(jrec.skills, skills);
        assert_eq!(jrec.recipe_hash, vec![9u8; RECIPE_HASH_LEN]);
    }

    /// the soul is the curated skill set: an agent registers with NO prompt at
    /// all, and its persona is just a skill loaded `Always`. the load mode is in
    /// the preimage, so flipping one moves the root — what the model IS changed.
    #[test]
    fn the_load_mode_is_committed_state() {
        let skill = |name: &str, load| SkillRef {
            name: name.into(),
            source_prefix: format!("/shared/skills/{name}"),
            source_snapshot: Some("aa".repeat(32)),
            load,
        };
        let registry = |load| {
            let mut m = module();
            let mut ctx = CaptureCtx::new().at(3).with_origin(user(9));
            exec(
                &mut m,
                &mut ctx,
                &admin(&AgentMsg::RegisterAgent {
                    agent_id: "bot".into(),
                    display_name: "BOT".into(),
                    capability: "model-1".into(),
                    allowed_actions: vec![],
                    recipe_hash: None,
                    caps: None,
                    skills: Some(vec![
                        skill("persona", load),
                        skill("release", LoadMode::OnDemand),
                    ]),
                }),
            )
            .expect("an agent needs no prompt — only skills");
            commit(&mut m);
            m
        };

        // the mode round-trips through the record view…
        let m = registry(LoadMode::Always);
        let rec = get_agent(&m, "bot").unwrap();
        assert_eq!(rec.skills[0].load, LoadMode::Always);
        assert_eq!(rec.skills[1].load, LoadMode::OnDemand);

        // …and through snapshot → install, under the agreed root.
        let (bytes, root) = (m.snapshot(), m.root());
        let mut joiner = module();
        joiner.install(&bytes, root).unwrap();
        assert_eq!(joiner.root(), root);
        assert_eq!(get_agent(&joiner, "bot").unwrap().skills, rec.skills);

        // the SAME skills with one load mode flipped is a different app-hash:
        // an always-skill is inlined into the assembled context document and an
        // on-demand one is not, so the two agents do not think alike.
        assert_ne!(
            registry(LoadMode::OnDemand).root(),
            root,
            "the load mode is part of the committed state, not host policy"
        );
    }

    /// an omitted `load` decodes as the conservative mode — a skill a submitter
    /// said nothing about never silently becomes the agent's persona.
    #[test]
    fn an_unstated_load_mode_defaults_to_on_demand() {
        let msg: AgentMsg = decode_msg(
            br#"{"register_agent":{"agent_id":"bot","display_name":"BOT","capability":"model-1",
                 "allowed_actions":[],"skills":[{"name":"s","source_prefix":"/p"}]}}"#,
        )
        .expect("a registration without a load mode decodes");
        let AgentMsg::RegisterAgent { skills, .. } = msg else {
            panic!("expected a registration");
        };
        assert_eq!(skills.unwrap()[0].load, LoadMode::OnDemand);
    }

    /// a Register carrying the runtime-identity fields is accepted
    /// unconditionally — there is no version gate on the op fields.
    #[test]
    fn execute_accepts_runtime_fields_unconditionally() {
        let mut m = module();
        let caps = ResourceCaps {
            tools: vec!["bash".into()],
            subagent_budget: 1,
            ..Default::default()
        };
        let mut ctx = CaptureCtx::new().with_origin(user(9));
        exec(
            &mut m,
            &mut ctx,
            &admin(&register_runtime(
                "bot",
                caps.clone(),
                vec![],
                vec![9u8; RECIPE_HASH_LEN],
            )),
        )
        .unwrap();
        commit(&mut m);
        let rec = get_agent(&m, "bot").unwrap();
        assert_eq!(rec.caps, caps);
        assert_eq!(rec.recipe_hash, vec![9u8; RECIPE_HASH_LEN]);
    }

    /// the pure D3 cap gate: forge read != push, duckfs prefix containment (the
    /// `src`/`srcx` boundary), write-grants-read, tool/secret membership, and
    /// the budget ceiling. an empty record denies everything.
    #[test]
    fn permits_enforces_forge_duckfs_tool_secret_budget() {
        let rec = record_with_caps(ResourceCaps {
            forge_read: vec!["r".into()],
            duckfs_read: vec!["src".into()],
            duckfs_write: vec!["out".into()],
            tools: vec!["bash".into()],
            secrets: vec!["s".into()],
            subagent_budget: 1,
            ..Default::default()
        });
        assert!(rec.permits(&CapRequest::ForgeRead("r")));
        assert!(
            !rec.permits(&CapRequest::ForgePush("r")),
            "read is not push"
        );
        assert!(rec.permits(&CapRequest::DuckfsRead("src")));
        assert!(rec.permits(&CapRequest::DuckfsRead("src/lib.rs")));
        assert!(
            !rec.permits(&CapRequest::DuckfsRead("srcx")),
            "a sibling sharing a textual prefix is denied"
        );
        assert!(
            rec.permits(&CapRequest::DuckfsRead("out/x")),
            "a write grant also grants read"
        );
        assert!(rec.permits(&CapRequest::DuckfsWrite("out/x")));
        assert!(
            !rec.permits(&CapRequest::DuckfsWrite("src/x")),
            "a read grant does not grant write"
        );
        assert!(rec.permits(&CapRequest::Tool("bash")));
        assert!(!rec.permits(&CapRequest::Tool("rm")));
        assert!(rec.permits(&CapRequest::Secret("s")));
        assert!(!rec.permits(&CapRequest::Secret("t")));
        assert!(rec.permits(&CapRequest::SpawnSubagent));

        let root = record_with_caps(ResourceCaps {
            duckfs_read: vec!["/".into()],
            ..Default::default()
        });
        assert!(root.permits(&CapRequest::DuckfsRead("/shared/child")));
        assert!(!root.permits(&CapRequest::DuckfsRead("relative")));

        let empty = record_with_caps(ResourceCaps::default());
        assert!(!empty.permits(&CapRequest::SpawnSubagent));
        assert!(!empty.permits(&CapRequest::ForgeRead("r")));
    }

    #[test]
    fn a_run_scoped_call_intersects_authority_without_making_a_hierarchy() {
        let mut parent = record_with_caps(ResourceCaps {
            forge_read: vec!["docs".into()],
            forge_push: vec!["app".into()],
            duckfs_read: vec!["/shared/read".into()],
            duckfs_write: vec!["/shared/write".into()],
            tools: vec!["shell".into()],
            secrets: vec!["build-token".into()],
            pages_write: vec!["*".into()],
            subagent_budget: 2,
        });
        parent.allowed_actions = vec![ACTION_CHAT_POST.into(), ACTION_DUCKFS_WRITE_TEXT.into()];
        let mut child = parent.clone();
        child.agent_id = "child".into();
        child.allowed_actions = vec![ACTION_CHAT_POST.into()];
        child.skills = vec![SkillRef {
            name: "specialist".into(),
            source_prefix: "/shared/write/child/specialist".into(),
            source_snapshot: None,
            load: LoadMode::Always,
        }];
        child.caps = ResourceCaps {
            forge_read: vec!["app".into()],
            duckfs_read: vec!["/shared/write/child".into()],
            duckfs_write: vec!["/shared/write/child".into()],
            tools: vec!["shell".into()],
            secrets: vec!["build-token".into()],
            pages_write: vec!["one-page".into()],
            subagent_budget: 0,
            ..Default::default()
        };
        // Provider and owner are peer metadata, not a structural call gate.
        child.owner = SagaOrigin::External(vec![8; 32]);
        child.capability = "other-model".into();
        child.allowed_actions.push(ACTION_TASKS_CREATE.into());
        child.caps.forge_read.push("outside".into());
        child.caps.tools.push("other".into());
        child.caps.subagent_budget = 3;
        child.skills.push(SkillRef {
            name: "private".into(),
            source_prefix: "/unreadable/specialist".into(),
            source_snapshot: None,
            load: LoadMode::Always,
        });
        child.allowed_actions.sort();
        child.caps.forge_read.sort();
        child.caps.tools.sort();

        let scoped = parent.scoped_for_call(&child);
        assert_eq!(scoped.owner, child.owner);
        assert_eq!(scoped.capability, "other-model");
        assert_eq!(scoped.allowed_actions, vec![ACTION_CHAT_POST]);
        assert_eq!(scoped.caps.forge_read, vec!["app"]);
        assert_eq!(scoped.caps.duckfs_write, vec!["/shared/write/child"]);
        assert_eq!(scoped.caps.tools, vec!["shell"]);
        assert_eq!(scoped.caps.pages_write, vec!["one-page"]);
        assert_eq!(scoped.caps.subagent_budget, 2);
        assert_eq!(scoped.skills.len(), 1);
        assert_eq!(scoped.skills[0].name, "specialist");
    }

    #[test]
    fn historical_role_tail_round_trips_with_legacy_defaults() {
        let legacy: AgentRecord = serde_json::from_value(serde_json::json!({
            "agent_id": "bot",
            "owner": { "external": [9] },
            "display_name": "BOT",
            "capability": "model-1",
            "allowed_actions": [],
            "status": "active",
            "created_at": 0,
            "updated_at": 0
        }))
        .expect("legacy records omit role");
        assert_eq!(legacy.role, AgentRole::General);
        assert!(
            serde_json::to_value(&legacy).unwrap().get("role").is_none(),
            "the default role stays absent on the legacy JSON wire"
        );

        let mut m = module();
        let mut owner = CaptureCtx::new().at(3).with_origin(user(9));
        exec(&mut m, &mut owner, &admin(&register("bot", &[]))).unwrap();
        commit(&mut m);
        m.agents.get_mut("bot").unwrap().role = AgentRole::ProjectLibrarian;
        assert_eq!(
            get_agent(&m, "bot").unwrap().role,
            AgentRole::ProjectLibrarian
        );

        let (bytes, root) = (m.snapshot(), m.root());
        let mut joiner = module();
        joiner.install(&bytes, root).unwrap();
        assert_eq!(joiner.root(), root);
        assert_eq!(
            get_agent(&joiner, "bot").unwrap().role,
            AgentRole::ProjectLibrarian
        );
    }

    #[test]
    fn native_v1_mode_keeps_revision_one_and_rejects_role_tails() {
        let mut legacy = module().with_legacy_v1_state();
        assert_eq!(Module::state_schema_revision(&legacy), 1);
        assert_eq!(Module::state_schema_revision(&module()), 2);

        let mut owner = CaptureCtx::new().at(3).with_origin(user(9));
        exec(&mut legacy, &mut owner, &admin(&register("bot", &[]))).unwrap();
        commit(&mut legacy);
        let (legacy_bytes, legacy_root) = (legacy.snapshot(), legacy.root());

        let mut current = module();
        exec(&mut current, &mut owner, &admin(&register("bot", &[]))).unwrap();
        commit(&mut current);
        current.agents.get_mut("bot").unwrap().role = AgentRole::ProjectLibrarian;
        let err = legacy
            .install(&current.snapshot(), current.root())
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("native-v1 agent snapshot contains a role tail")
        );
        assert_eq!(legacy.snapshot(), legacy_bytes);
        assert_eq!(legacy.root(), legacy_root);
    }

    /// the library grant is an ORDINARY duckfs read cap — no special namespace,
    /// no second rule. the host assembler asks this question before it tells an
    /// agent the shared library exists, and the MCP tool plane gates the actual
    /// grep/read on `permits(DuckfsRead(..))`: the two must be the SAME rule, or
    /// the run's context document promises a door the tool plane then slams.
    #[test]
    fn library_readable_is_the_duckfs_read_cap_the_tool_plane_enforces() {
        let ungranted = record_with_caps(ResourceCaps::default());
        assert!(
            !ungranted.library_readable(),
            "the empty default denies everything, the library included"
        );

        let granted = record_with_caps(ResourceCaps {
            duckfs_read: vec![SKILL_LIBRARY_PREFIX.into()],
            ..Default::default()
        });
        assert!(granted.library_readable());
        // the grant the assembler reports is the grant the tool plane honors: the
        // agent is told to grep the library and read a skill under it, and BOTH
        // of those calls gate on the very same request.
        assert!(granted.permits(&CapRequest::DuckfsRead(SKILL_LIBRARY_PREFIX)));
        assert!(granted.permits(&CapRequest::DuckfsRead(&format!(
            "{SKILL_LIBRARY_PREFIX}/release/SKILL.md"
        ))));

        // a wider prefix contains the library (prefix containment, not equality).
        assert!(
            record_with_caps(ResourceCaps {
                duckfs_read: vec!["/shared".into()],
                ..Default::default()
            })
            .library_readable()
        );
        // a sibling that merely shares the text does not.
        assert!(
            !record_with_caps(ResourceCaps {
                duckfs_read: vec![format!("{SKILL_LIBRARY_PREFIX}-drafts")],
                ..Default::default()
            })
            .library_readable()
        );
        // a grant of ONE library skill is not a grant of the library: the agent
        // could not grep it, so it is not told it can.
        assert!(
            !record_with_caps(ResourceCaps {
                duckfs_read: vec![format!("{SKILL_LIBRARY_PREFIX}/release")],
                ..Default::default()
            })
            .library_readable()
        );
    }

    /// the pages_write matcher is exact-or-`"*"` — page ids are opaque, so a
    /// grant never implies a prefix (unlike duckfs) and only the literal
    /// wildcard entry grants every page.
    #[test]
    fn permits_pages_write_exact_or_wildcard() {
        let exact = record_with_caps(ResourceCaps {
            pages_write: vec!["page-1".into()],
            ..Default::default()
        });
        assert!(exact.permits(&CapRequest::PagesWrite("page-1")));
        assert!(!exact.permits(&CapRequest::PagesWrite("page-2")));
        assert!(
            !exact.permits(&CapRequest::PagesWrite("page-1/child")),
            "no prefix containment for pages"
        );

        let wild = record_with_caps(ResourceCaps {
            pages_write: vec!["*".into()],
            ..Default::default()
        });
        assert!(wild.permits(&CapRequest::PagesWrite("anything")));

        let empty = record_with_caps(ResourceCaps::default());
        assert!(!empty.permits(&CapRequest::PagesWrite("page-1")));
    }

    #[test]
    fn duckfs_write_text_is_a_known_action_with_a_duckfs_write_cap() {
        let action = AgentAction::DuckfsWriteText {
            path: "/shared/agents/qa-fixer/self-improvement/SKILL.md".into(),
            text: "lesson".into(),
            base_snapshot: None,
        };
        assert_eq!(action.vocabulary_name(), ACTION_DUCKFS_WRITE_TEXT);
        assert!(KNOWN_ACTIONS.contains(&ACTION_DUCKFS_WRITE_TEXT));

        let rec = record_with_caps(ResourceCaps {
            duckfs_write: vec!["/shared/agents/qa-fixer".into()],
            ..Default::default()
        });
        assert!(rec.permits(&CapRequest::DuckfsWrite(
            "/shared/agents/qa-fixer/self-improvement/SKILL.md"
        )));
        assert!(
            !rec.permits(&CapRequest::DuckfsWrite(
                "/shared/agents/qa-fixer-policy/SKILL.md"
            )),
            "a sibling sharing a textual prefix is denied"
        );
    }

    /// the two pages grants are in the vocabulary: a registration granting
    /// them is admitted (KNOWN_ACTIONS is the admission gate).
    #[test]
    fn register_accepts_the_pages_action_grants() {
        let mut m = module();
        let mut ctx = CaptureCtx::new().with_origin(user(9));
        exec(
            &mut m,
            &mut ctx,
            &admin(&register(
                "bot",
                &[ACTION_PAGES_COMMENT, ACTION_PAGES_SET_CHECKED],
            )),
        )
        .unwrap();
        commit(&mut m);
        let rec = get_agent(&m, "bot").unwrap();
        assert_eq!(
            rec.allowed_actions,
            vec![
                ACTION_PAGES_COMMENT.to_string(),
                ACTION_PAGES_SET_CHECKED.to_string()
            ]
        );
    }

    /// `MAX_AGENT_RECORD_BYTES` counts the runtime-identity fields — an oversized
    /// caps list is rejected before staging.
    #[test]
    fn record_size_gate_counts_runtime_fields() {
        let mut m = module();
        let mut ctx = CaptureCtx::new().with_origin(user(9));
        let huge = ResourceCaps {
            tools: vec!["x".repeat(MAX_AGENT_RECORD_BYTES)],
            ..Default::default()
        };
        let err = exec(
            &mut m,
            &mut ctx,
            &admin(&register_runtime("bot", huge, vec![], vec![])),
        )
        .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        abort(&mut m);
        assert!(get_agent(&m, "bot").is_none());
    }

    /// two instances replaying the same runtime-identity ops produce equal roots.
    #[test]
    fn replay_is_deterministic() {
        let caps = ResourceCaps {
            forge_push: vec!["a".into(), "b".into()],
            subagent_budget: 3,
            ..Default::default()
        };
        let skills = vec![SkillRef {
            name: "s".into(),
            source_prefix: "/p".into(),
            source_snapshot: Some("cc".repeat(32)),
            load: LoadMode::Always,
        }];
        let run = || {
            let mut m = module();
            let mut ctx = CaptureCtx::new().at(1).with_origin(user(9));
            exec(
                &mut m,
                &mut ctx,
                &admin(&register_runtime(
                    "alpha",
                    caps.clone(),
                    skills.clone(),
                    vec![9u8; RECIPE_HASH_LEN],
                )),
            )
            .unwrap();
            commit(&mut m);
            m
        };
        let (a, b) = (run(), run());
        assert_eq!(a.root(), b.root());
        let baseline = module();
        assert_ne!(a.root(), baseline.root(), "state moved the root");
    }

    /// D1 keyless: a serialized record carries NO key material — only opaque
    /// secret refs, nested under `caps`.
    #[test]
    fn keyless_d1_no_key_field() {
        let rec = record_with_caps(ResourceCaps {
            secrets: vec!["vault://k".into()],
            ..Default::default()
        });
        let j = serde_json::to_value(&rec).unwrap();
        assert!(j.get("key").is_none(), "no key material at the record root");
        assert!(
            j.get("secrets").is_none(),
            "secrets live inside caps, not at the record root"
        );
        assert_eq!(
            j["caps"]["secrets"][0],
            serde_json::json!("vault://k"),
            "secrets are opaque refs only"
        );
    }

    /// the id rule is a deliberate COPY of duckdns's handle shape rule (an
    /// agent id must be a DNS label because it IS the local part of
    /// `<id>@agents.duck`). pin the two together so tightening either one
    /// alone goes red. the ONE intended divergence: duckdns also rejects its
    /// reserved ROOT labels — an agent id is not a `.duck` handle, so an agent
    /// may be called `net` or `agents`.
    #[test]
    fn agent_id_shape_matches_the_duckdns_label_rule() {
        let too_long = "x".repeat(MAX_AGENT_ID_LEN + 1);
        let longest = "x".repeat(MAX_AGENT_ID_LEN);
        let cases = [
            "quackbot",
            "a",
            "9",
            "qa-luna",
            "a--b",
            "",
            "-lead",
            "trail-",
            "UPPER",
            "under_score",
            "dot.ted",
            "spa ce",
            "bad\u{1f}id",
            "quack/bot@example",
            too_long.as_str(),
            longest.as_str(),
        ];
        for id in cases {
            let admitted = validate_agent_id(id).is_ok();
            assert_eq!(
                admitted,
                duckdns::validate_handle_shape(id).is_ok(),
                "agent and duckdns disagree on the SHAPE of {id:?}"
            );
        }
        // duckdns's reserved ROOT labels are its ADMISSION policy, not part of
        // the shape — an agent id is not a `.duck` handle, so an agent may be
        // called `net` or `agents`.
        for label in duckdns::RESERVED_ROOT_LABELS {
            assert!(validate_agent_id(label).is_ok(), "{label}");
            assert!(duckdns::validate_handle_shape(label).is_ok(), "{label}");
            assert!(duckdns::validate_handle(label).is_err(), "{label}");
        }
    }

    /// THE TRAP THE DUCKDNS SIDE FELL INTO, PINNED SHUT HERE. `validate_agent_id`
    /// is an ADMISSION rule and must never reach `decode_committed`: agents
    /// registered before the rule existed hold ids it rejects (`"qa luna"`), and
    /// their bytes are in every committed snapshot. If decode enforced the rule,
    /// a node on the new binary could not install agent state (no state sync) and
    /// could not restore its own recovery checkpoint — a permanent brick with no
    /// migration path. A legacy agent DECODES. It simply cannot be re-registered.
    #[test]
    fn a_snapshot_holding_a_legacy_non_label_agent_id_still_installs() {
        // forge the bytes an old binary committed: register a same-length label
        // id and swap it in the canonical encoding (every length prefix stays
        // valid). `register` uppercases the display name, so the id is the only
        // occurrence of this window.
        const LEGACY: &str = "qa luna";
        const STAND_IN: &str = "qa-luna";
        assert_eq!(LEGACY.len(), STAND_IN.len());
        assert!(
            validate_agent_id(LEGACY).is_err(),
            "no live Register could ever admit it"
        );

        let mut old = module();
        let mut ctx = CaptureCtx::new().at(3).with_origin(user(9));
        exec(&mut old, &mut ctx, &admin(&register(STAND_IN, &[]))).unwrap();
        commit(&mut old);

        let mut bytes = old.snapshot();
        let at = bytes
            .windows(STAND_IN.len())
            .position(|w| w == STAND_IN.as_bytes())
            .expect("the stand-in id is in the canonical bytes");
        bytes[at..at + LEGACY.len()].copy_from_slice(LEGACY.as_bytes());
        let root = StateRoot(Sha256::digest(&bytes).into());

        let mut joiner = module();
        joiner
            .install(&bytes, root)
            .expect("a legacy non-label agent id must still DECODE");
        assert_eq!(joiner.root(), root);
        assert_eq!(joiner.snapshot(), bytes, "and re-encode identically");
        assert!(
            get_agent(&joiner, LEGACY).is_some(),
            "the legacy agent is intact and keeps working"
        );
    }
}
