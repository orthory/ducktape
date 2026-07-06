//! the agent module — the platform's agent registry, and nothing more.
//!
//! a pure state-machine module (in the app-hash) holding one map: agent id →
//! record (owner, capability tag, prompt pin, granted actions, status). it is
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
    /// sha256 of the prompt content (exactly [`PROMPT_HASH_LEN`] bytes).
    prompt_hash: Vec<u8>,
    /// the document module doc holding the prompt content; its canonical
    /// rendering must hash to `prompt_hash` (verified by the runs module at
    /// dispatch time).
    prompt_doc: Option<String>,
    /// granted action names from the known vocabulary, deduped and sorted.
    allowed_actions: BTreeSet<String>,
    /// false = paused: the agent never engages new runs.
    active: bool,
    created_at: u64,
    updated_at: u64,
}

// ---- canonical encoding -------------------------------------------------------
// u64-le counts, sorted keys, every field in declaration order: u64-le length
// prefixes for byte strings, single-byte discriminants for enums, a 0/1 tag
// byte for options, u64-le integers. this is the exact preimage
// [`Module::root`] hashes, so a snapshot and the root that must authenticate
// it cannot drift. no version byte — encoding changes are flag-day (design
// principle: no backwards compatibility).

fn put_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(bytes);
}

fn put_opt_string(out: &mut Vec<u8>, opt: &Option<String>) {
    match opt {
        None => out.push(0),
        Some(value) => {
            out.push(1);
            put_bytes(out, value.as_bytes());
        }
    }
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

fn encode_committed(agents: &BTreeMap<String, AgentState>) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(agents.len() as u64).to_le_bytes());
    for (id, a) in agents {
        put_bytes(&mut out, id.as_bytes());
        put_origin(&mut out, &a.owner);
        put_bytes(&mut out, a.display_name.as_bytes());
        put_bytes(&mut out, a.capability.as_bytes());
        put_bytes(&mut out, &a.prompt_hash);
        put_opt_string(&mut out, &a.prompt_doc);
        out.extend_from_slice(&(a.allowed_actions.len() as u64).to_le_bytes());
        for action in &a.allowed_actions {
            put_bytes(&mut out, action.as_bytes());
        }
        out.push(if a.active { 0 } else { 1 });
        out.extend_from_slice(&a.created_at.to_le_bytes());
        out.extend_from_slice(&a.updated_at.to_le_bytes());
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

fn take_opt_string(buf: &mut &[u8]) -> Result<Option<String>, String> {
    match take(buf, 1)?[0] {
        0 => Ok(None),
        1 => Ok(Some(take_lp_string(buf)?)),
        t => Err(format!("snapshot has unknown option tag {t}")),
    }
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
    if let Some((last, _)) = map.iter().next_back() {
        if last.as_str() >= key.as_str() {
            return Err("snapshot keys not strictly ascending".into());
        }
    }
    map.insert(key, value);
    Ok(())
}

fn contains_reserved_separator(value: &str) -> bool {
    value.contains(RESERVED_ID_SEPARATOR)
}

fn decode_committed(mut buf: &[u8]) -> Result<BTreeMap<String, AgentState>, String> {
    // per-entry minimum size: an agent costs its id prefix, one origin
    // discriminant, three length prefixes, a prompt-doc tag, an action
    // count, a status byte, and two u64s.
    const MIN_AGENT_BYTES: u64 = 8 + 1 + 8 + 8 + 8 + 1 + 8 + 1 + 8 + 8;

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
        let prompt_hash = take_lp_bytes(&mut buf)?;
        let prompt_doc = take_opt_string(&mut buf)?;
        let mut allowed_actions: BTreeSet<String> = BTreeSet::new();
        let actions = take_count(&mut buf, 8, "action")?;
        for _ in 0..actions {
            let action = take_lp_string(&mut buf)?;
            if let Some(last) = allowed_actions.iter().next_back() {
                if last.as_str() >= action.as_str() {
                    return Err("snapshot actions not strictly ascending".into());
                }
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
        insert_ascending(
            &mut agents,
            id,
            AgentState {
                owner,
                display_name,
                capability,
                prompt_hash,
                prompt_doc,
                allowed_actions,
                active,
                created_at,
                updated_at,
            },
        )?;
    }

    if !buf.is_empty() {
        return Err("snapshot has trailing bytes".into());
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
            agents: BTreeMap::new(),
            pending_agents: BTreeMap::new(),
        }
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
            prompt_hash: a.prompt_hash.clone(),
            prompt_doc: a.prompt_doc.clone(),
            allowed_actions: a.allowed_actions.iter().cloned().collect(),
            status: if a.active {
                AgentStatus::Active
            } else {
                AgentStatus::Paused
            },
            created_at: a.created_at,
            updated_at: a.updated_at,
        }
    }

    // ---- shared validation ----------------------------------------------------

    fn validate_non_empty(field: &str, value: &str) -> Result<(), Error> {
        if value.is_empty() {
            return Err(Error::Module(format!("{field} must not be empty")));
        }
        Ok(())
    }

    fn validate_prompt_hash(prompt_hash: &[u8]) -> Result<(), Error> {
        if prompt_hash.len() != PROMPT_HASH_LEN {
            return Err(Error::Module(format!(
                "prompt_hash must be exactly {PROMPT_HASH_LEN} bytes, got {}",
                prompt_hash.len()
            )));
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
                prompt_hash,
                prompt_doc,
                allowed_actions,
            } => {
                let owner = Self::admin_origin(&ctx.env().origin)?;
                Self::validate_non_empty("agent_id", &agent_id)?;
                if contains_reserved_separator(&agent_id) {
                    return Err(Error::Module(
                        "agent_id must not contain the reserved unit separator".into(),
                    ));
                }
                Self::validate_non_empty("display_name", &display_name)?;
                validate_tag(&capability).map_err(Error::Module)?;
                Self::validate_prompt_hash(&prompt_hash)?;
                if let Some(doc_id) = &prompt_doc {
                    Self::validate_non_empty("prompt_doc", doc_id)?;
                }
                let allowed_actions = Self::validate_actions(allowed_actions)?;
                if self.agent(&agent_id).is_some() {
                    return Err(Error::Module(format!("agent already exists: {agent_id}")));
                }
                let state = AgentState {
                    owner,
                    display_name,
                    capability: capability.clone(),
                    prompt_hash,
                    prompt_doc,
                    allowed_actions,
                    active: true,
                    created_at: now,
                    updated_at: now,
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
                prompt_hash,
                prompt_doc,
                allowed_actions,
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
                if let Some(prompt_hash) = prompt_hash {
                    Self::validate_prompt_hash(&prompt_hash)?;
                    state.prompt_hash = prompt_hash;
                }
                if let Some(doc_id) = prompt_doc {
                    Self::validate_non_empty("prompt_doc", &doc_id)?;
                    state.prompt_doc = Some(doc_id);
                }
                if let Some(allowed_actions) = allowed_actions {
                    state.allowed_actions = Self::validate_actions(allowed_actions)?;
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
    /// nodes.
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
    use sdk::{Effect, Env};

    /// a minimal `Ctx` that captures emitted msgs/effects/events — enough to
    /// unit-test `execute` in isolation (the host provides the real routing
    /// in integration).
    struct CaptureCtx {
        env: Env,
        msgs: Vec<Msg>,
        #[allow(dead_code)]
        effects: Vec<Effect>,
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
                effects: Vec::new(),
                events: Vec::new(),
            }
        }
        fn at(mut self, view: u64) -> Self {
            self.env.height = view;
            self.env.consensus_time = view;
            self
        }
        fn from_origin(mut self, origin: Origin) -> Self {
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
        fn request_effect(&mut self, eff: Effect) {
            self.effects.push(eff);
        }
    }

    // ---- fixtures -----------------------------------------------------------

    fn module() -> AgentModule {
        AgentModule::new("agent", "saga", Some("runs".into()))
    }

    fn user(byte: u8) -> Origin {
        Origin::External(vec![byte; 32])
    }

    fn register(agent_id: &str, actions: &[&str]) -> AgentMsg {
        AgentMsg::RegisterAgent {
            agent_id: agent_id.into(),
            display_name: agent_id.to_uppercase(),
            capability: "model-1".into(),
            prompt_hash: vec![7u8; PROMPT_HASH_LEN],
            prompt_doc: None,
            allowed_actions: actions.iter().map(|s| s.to_string()).collect(),
        }
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
        let mut ctx = CaptureCtx::new().at(3).from_origin(user(9));
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

    #[test]
    fn register_rejects_bad_shapes_and_bad_origins() {
        let mut m = module();
        let root0 = m.root();
        let cases: Vec<(Origin, AgentMsg)> = vec![
            // the pre-consensus empty external default never owns an agent.
            (Origin::External(Vec::new()), register("a", &[])),
            // system is not an ownable origin (spec: external or module).
            (Origin::System, register("a", &[])),
            // a prompt hash that is not exactly 32 bytes.
            (
                user(9),
                AgentMsg::RegisterAgent {
                    agent_id: "a".into(),
                    display_name: "A".into(),
                    capability: "m".into(),
                    prompt_hash: vec![7u8; 31],
                    prompt_doc: None,
                    allowed_actions: Vec::new(),
                },
            ),
            // an action outside the known vocabulary.
            (
                user(9),
                AgentMsg::RegisterAgent {
                    agent_id: "a".into(),
                    display_name: "A".into(),
                    capability: "m".into(),
                    prompt_hash: vec![7u8; 32],
                    prompt_doc: None,
                    allowed_actions: vec!["forge.push".into()],
                },
            ),
            // empty required fields.
            (
                user(9),
                AgentMsg::RegisterAgent {
                    agent_id: String::new(),
                    display_name: "A".into(),
                    capability: "m".into(),
                    prompt_hash: vec![7u8; 32],
                    prompt_doc: None,
                    allowed_actions: Vec::new(),
                },
            ),
            (
                user(9),
                AgentMsg::RegisterAgent {
                    agent_id: "bad\u{1f}id".into(),
                    display_name: "A".into(),
                    capability: "m".into(),
                    prompt_hash: vec![7u8; 32],
                    prompt_doc: None,
                    allowed_actions: Vec::new(),
                },
            ),
            (
                user(9),
                AgentMsg::RegisterAgent {
                    agent_id: "a".into(),
                    display_name: "A".into(),
                    capability: String::new(),
                    prompt_hash: vec![7u8; 32],
                    prompt_doc: None,
                    allowed_actions: Vec::new(),
                },
            ),
            // an oversized record is rejected before staging.
            (
                user(9),
                AgentMsg::RegisterAgent {
                    agent_id: "a".into(),
                    display_name: "x".repeat(MAX_AGENT_RECORD_BYTES),
                    capability: "m".into(),
                    prompt_hash: vec![7u8; 32],
                    prompt_doc: None,
                    allowed_actions: Vec::new(),
                },
            ),
        ];
        for (origin, op) in cases {
            let mut ctx = CaptureCtx::new().from_origin(origin.clone());
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
        let mut ctx = CaptureCtx::new().from_origin(user(9));
        exec(&mut m, &mut ctx, &admin(&register("bot", &[]))).unwrap();
        commit(&mut m);

        // a foreign origin can neither update nor pause.
        for op in [
            AgentMsg::UpdateAgent {
                agent_id: "bot".into(),
                display_name: Some("Stolen".into()),
                capability: None,
                prompt_hash: None,
                prompt_doc: None,
                allowed_actions: None,
            },
            AgentMsg::PauseAgent {
                agent_id: "bot".into(),
            },
            AgentMsg::ResumeAgent {
                agent_id: "bot".into(),
            },
        ] {
            let mut ctx = CaptureCtx::new().from_origin(user(2));
            let err = exec(&mut m, &mut ctx, &admin(&op)).unwrap_err();
            assert!(matches!(err, Error::Module(_)));
            abort(&mut m);
        }

        // the owner updates fields selectively and toggles status.
        let mut ctx = CaptureCtx::new().at(5).from_origin(user(9));
        exec(
            &mut m,
            &mut ctx,
            &admin(&AgentMsg::UpdateAgent {
                agent_id: "bot".into(),
                display_name: None,
                capability: Some("model-2".into()),
                prompt_hash: None,
                prompt_doc: None,
                allowed_actions: Some(vec![ACTION_TASKS_CREATE.into()]),
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
        let mut ctx = CaptureCtx::new().at(6).from_origin(user(9));
        exec(
            &mut m,
            &mut ctx,
            &admin(&AgentMsg::UpdateAgent {
                agent_id: "bot".into(),
                display_name: Some("Bot".into()),
                capability: Some("model-2".into()),
                prompt_hash: None,
                prompt_doc: None,
                allowed_actions: None,
            }),
        )
        .unwrap();
        assert!(ctx.hook_events().is_empty(), "same capability, no retune");
        commit(&mut m);

        // pausing a paused agent stages nothing: root byte-identical.
        let paused_root = m.root();
        let mut ctx = CaptureCtx::new().at(7).from_origin(user(9));
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

        let mut ctx = CaptureCtx::new().at(8).from_origin(user(9));
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
        let mut ctx = CaptureCtx::new().from_origin(Origin::Module("saga".into()));
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
        let mut ctx = CaptureCtx::new().from_origin(Origin::Module("automations".into()));
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
        let mut ctx = CaptureCtx::new().from_origin(user(9));
        exec(&mut m, &mut ctx, &admin(&register("bot", &[]))).unwrap();
        assert!(ctx.msgs.is_empty(), "no hook, no follow-ups");
        commit(&mut m);
        assert!(get_agent(&m, "bot").is_some());
    }

    // ---- queries + state sync ---------------------------------------------------

    #[test]
    fn queries_list_agents_staged_over_committed() {
        let mut m = module();
        let mut ctx = CaptureCtx::new().from_origin(user(9));
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
                    prompt_hash: None,
                    prompt_doc: Some("doc-1".into()),
                    allowed_actions: None,
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
                let mut ctx = CaptureCtx::new().at(i as u64 + 1).from_origin(origin.clone());
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
        let mut ctx = CaptureCtx::new().at(1).from_origin(user(9));
        exec(&mut m, &mut ctx, &admin(&register("alpha", &[ACTION_CHAT_POST]))).unwrap();
        exec(&mut m, &mut ctx, &admin(&register("beta", &[]))).unwrap();
        commit(&mut m);

        let bytes = m.snapshot();
        let root = m.root();

        let mut joiner = module();
        joiner.install(&bytes, root.clone()).unwrap();
        assert_eq!(joiner.root(), root);
        assert_eq!(
            list_agents(&joiner).len(),
            2,
            "the joiner serves the installed registry"
        );

        // a snapshot under a foreign root is rejected, leaving no trace.
        let mut fresh = module();
        let before = fresh.root();
        assert!(fresh.install(&bytes, before.clone()).is_err());
        assert_eq!(fresh.root(), before);

        // truncated bytes never land.
        let mut fresh = module();
        assert!(fresh.install(&bytes[..bytes.len() - 1], root).is_err());
    }

    #[test]
    fn state_sync_handle_exposes_the_snapshot_bytes() {
        let mut m = module();
        let mut ctx = CaptureCtx::new().from_origin(user(9));
        exec(&mut m, &mut ctx, &admin(&register("alpha", &[]))).unwrap();
        commit(&mut m);
        match m.state_sync_handle().unwrap() {
            StateSyncHandle::SnapshotBytes(bytes) => assert_eq!(bytes, m.snapshot()),
            other => panic!("unexpected handle: {other:?}"),
        }
    }
}
