//! qmdb-backed agent module — the platform's agent registry, and nothing more.
//!
//! pure logic over a host-injected [`sdk::MerkleStore`]: the HOST constructs
//! the concrete store (qmdb today — `statesync::qmdb::QmdbStore`) and hands it
//! to [`AgentModule::new`], so this crate never names a storage crate. the
//! store is used for what it is — hash-addressable authenticated state, one
//! logical record per agent, every read a point lookup — plus ONE enumeration
//! record the registry cannot do without: the roster (the sorted id list).
//! the roster is not scan machinery for a human surface — CONSENSUS consumes
//! it (the runs module's `All`/`RoundRobin` engagement domain reads every
//! active agent inside `execute()`), and consensus can never depend on the
//! unverifiable derived tier, so the enumeration stays canonical. it is
//! bounded by [`MAX_REGISTERED_AGENTS`]. there is no index guest: every read
//! this module serves is dispatch-consumed (runs' registry reads, the MCP
//! read plane through the same canonical queries).
//!
//! the module is 100% self-contained — no other module's interface crosses
//! this crate (`SagaOrigin` and the capability tag shape rule are platform
//! vocabulary, not module guts) — and it consumes no other module's events.
//! everything that ACTS on agents (engagement, composition, dispatch,
//! response delivery) lives in the runs module, which reads this registry by
//! query.
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
//! writes are staged during a block and flushed to the store in one batch at
//! `commit_block`; the module root IS the store's merkle root. sync belongs
//! to the store, not this module: a joiner rebuilds the concrete store from a
//! peer (`QmdbStore::sync_from`) and wraps a fresh `AgentModule` around it —
//! this module only forwards the trait's serve surface.

// the wire surface: this module's shared types, flattened at the crate root.
mod interface;
pub use interface::*;

// the wasm-guest port: the store-backed dispatch shell that adapts this
// module to the ducktape:module world. compiled only by the guest-builder's
// synthesized wasm32 cdylib workspace (feature `guest`), never by the native
// build.
#[cfg(feature = "guest")]
mod guest;

use std::collections::BTreeSet;

use capability::validate_tag;
use saga::SagaOrigin;
use sdk::{
    Ctx, Error, Event, MerkleStore, Module, ModuleId, Msg, Origin, ResolverSyncTarget, StagedStore,
    StateRoot, StateSyncHandle,
};
use borsh::{BorshDeserialize, BorshSerialize};

/// the canonical state form of an op origin (see [`SagaOrigin`]).
fn canonical_origin(origin: &Origin) -> SagaOrigin {
    match origin {
        Origin::External(key) => SagaOrigin::External(key.clone()),
        Origin::Module(module) => SagaOrigin::Module(module.clone()),
        Origin::System => SagaOrigin::System,
    }
}

/// per-agent record key: prefix + 0 + id (the single-component shape chat
/// uses). safe because both key literals are fixed and neither is the other
/// followed by a 0 byte.
fn agent_key(agent_id: &str) -> Vec<u8> {
    let mut key = Vec::with_capacity(5 + 1 + agent_id.len());
    key.extend_from_slice(b"agent");
    key.push(0);
    key.extend_from_slice(agent_id.as_bytes());
    key
}

/// the roster record's whole key. collides with no `agent\0...` key.
const ROSTER_KEY: &[u8] = b"roster";

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

// ---- the module -----------------------------------------------------------

/// storage-backed agent registry.
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
    /// the host-injected authenticated store plus this block's staging overlay
    /// (read-your-writes, folded into `root()` at `commit_block`). store key
    /// is `sha256(logical_key)`, owned by [`StagedStore`].
    staged: StagedStore,
}

impl AgentModule {
    /// wrap the host-constructed store under module identity `id`. the ids
    /// must be pairwise distinct — the saga dead-letter is origin-routed, and
    /// a colliding hook id would collapse that namespace.
    pub fn new(
        id: impl Into<ModuleId>,
        store: Box<dyn MerkleStore>,
        saga: impl Into<ModuleId>,
        hook: Option<ModuleId>,
    ) -> Self {
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
            staged: StagedStore::new(store),
        }
    }

    // ---- staged-over-committed reads ---------------------------------------

    async fn load<T>(&self, key: &[u8]) -> Result<Option<T>, Error>
    where
        T: BorshDeserialize,
    {
        match self.staged.get(key).await? {
            Some(bytes) => Ok(Some(
                borsh::from_slice(&bytes).map_err(|e| Error::Module(e.to_string()))?,
            )),
            None => Ok(None),
        }
    }

    fn store<T>(&mut self, key: Vec<u8>, value: &T)
    where
        T: BorshSerialize,
    {
        self.staged.stage(
            key,
            borsh::to_vec(value).expect("agent value is serializable"),
        );
    }

    /// stage a value only if its serialized size fits `cap` — the write-time
    /// guard against poison values (the qmdb codec cap is decode-only).
    fn store_bounded<T>(
        &mut self,
        key: Vec<u8>,
        value: &T,
        cap: usize,
        what: &str,
    ) -> Result<(), Error>
    where
        T: BorshSerialize,
    {
        let bytes = borsh::to_vec(value).expect("agent value is serializable");
        if bytes.len() > cap {
            return Err(Error::Module(format!(
                "{what} record too large: {} > {cap} bytes",
                bytes.len()
            )));
        }
        self.staged.stage(key, bytes);
        Ok(())
    }

    async fn agent(&self, agent_id: &str) -> Result<Option<AgentRecord>, Error> {
        self.load(&agent_key(agent_id)).await
    }

    /// the registry roster — every registered id, sorted. the ONE enumeration
    /// record: consensus itself consumes it (runs' engagement domain), so it
    /// stays canonical, bounded by [`MAX_REGISTERED_AGENTS`].
    async fn roster(&self) -> Result<Vec<String>, Error> {
        Ok(self.load(ROSTER_KEY).await?.unwrap_or_default())
    }

    /// how many roster entries `owner` already holds — the per-owner cap
    /// check. no secondary index: the roster is already bounded by
    /// [`MAX_REGISTERED_AGENTS`], so a full scan at registration time (the
    /// only caller) stays cheap and needs no extra replicated state.
    async fn count_owned(&self, roster: &[String], owner: &SagaOrigin) -> Result<usize, Error> {
        let mut count = 0;
        for agent_id in roster {
            let Some(record) = self.agent(agent_id).await? else {
                continue;
            };
            if &record.owner == owner {
                count += 1;
            }
        }
        Ok(count)
    }

    // ---- shared validation ----------------------------------------------------

    fn validate_non_empty(field: &str, value: &str) -> Result<(), Error> {
        if value.is_empty() {
            return Err(Error::Module(format!("{field} must not be empty")));
        }
        Ok(())
    }

    /// every granted action must come from the known vocabulary, so a grant
    /// always means something; duplicates collapse into the sorted set.
    fn validate_actions(actions: Vec<String>) -> Result<Vec<String>, Error> {
        let mut set = BTreeSet::new();
        for action in actions {
            if !KNOWN_ACTIONS.contains(&action.as_str()) {
                return Err(Error::Module(format!("unknown action: {action}")));
            }
            set.insert(action);
        }
        Ok(set.into_iter().collect())
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
    /// list so the committed record is canonical — one valid byte encoding
    /// per state, and `permits` reads the same shape everywhere. budget needs
    /// no normalization.
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

    async fn owned_agent(&self, ctx: &dyn Ctx, agent_id: &str) -> Result<AgentRecord, Error> {
        let Some(record) = self.agent(agent_id).await? else {
            return Err(Error::Module(format!("unknown agent: {agent_id}")));
        };
        if record.owner != canonical_origin(&ctx.env().origin) {
            return Err(Error::Module(format!(
                "only the owner may modify agent {agent_id}"
            )));
        }
        Ok(record)
    }

    /// the governance module's follow-up after a passing proposal — the ONE
    /// non-owner actor allowed to reach into another account's registration,
    /// the same escape hatch a squatted-roster incident needs to recover
    /// without a hard fork.
    fn is_governance(origin: &Origin) -> bool {
        matches!(origin, Origin::Module(module) if module == "governance")
    }

    /// deregistration's authority check: the recorded owner, or governance —
    /// [`Self::owned_agent`] with the governance escape hatch added.
    async fn deregistrable_agent(&self, ctx: &dyn Ctx, agent_id: &str) -> Result<AgentRecord, Error> {
        let origin = &ctx.env().origin;
        let Some(record) = self.agent(agent_id).await? else {
            return Err(Error::Module(format!("unknown agent: {agent_id}")));
        };
        let is_owner = record.owner == canonical_origin(origin);
        if !is_owner && !Self::is_governance(origin) {
            return Err(Error::Module(format!(
                "only the owner or governance may deregister agent {agent_id}"
            )));
        }
        Ok(record)
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
                // the roster is the ONE existence authority: record and roster
                // are staged (and commit or abort) together, so membership in
                // one is membership in both.
                let mut roster = self.roster().await?;
                let position = match roster.binary_search(&agent_id) {
                    Ok(_) => {
                        return Err(Error::Module(format!("agent already exists: {agent_id}")));
                    }
                    Err(position) => position,
                };
                if roster.len() >= MAX_REGISTERED_AGENTS {
                    return Err(Error::Module(format!(
                        "agent registry is full: {MAX_REGISTERED_AGENTS} agents"
                    )));
                }
                let owner_agents = self.count_owned(&roster, &owner).await?;
                if owner_agents >= MAX_AGENTS_PER_OWNER {
                    return Err(Error::Module(format!(
                        "owner already registered {MAX_AGENTS_PER_OWNER} agents: the per-owner cap"
                    )));
                }
                roster.insert(position, agent_id.clone());
                let record = AgentRecord {
                    agent_id: agent_id.clone(),
                    owner,
                    display_name,
                    capability: capability.clone(),
                    allowed_actions,
                    status: AgentStatus::Active,
                    role: AgentRole::default(),
                    created_at: now,
                    updated_at: now,
                    recipe_hash,
                    caps,
                    skills,
                };
                self.store_bounded(
                    agent_key(&agent_id),
                    &record,
                    MAX_AGENT_RECORD_BYTES,
                    "agent",
                )?;
                self.store(ROSTER_KEY.to_vec(), &roster);
                // the hook stages the agent's dispatch recipe in this same
                // block: a rejected recipe (squatted or oversized id) aborts
                // the block and the staged record with it (P2).
                self.emit_hook(
                    ctx,
                    &AgentEvent::Registered {
                        agent_id,
                        capability,
                    },
                );
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
                let mut record = self.owned_agent(&*ctx, &agent_id).await?;
                if let Some(display_name) = display_name {
                    Self::validate_non_empty("display_name", &display_name)?;
                    record.display_name = display_name;
                }
                if let Some(capability) = capability {
                    validate_tag(&capability).map_err(Error::Module)?;
                    if capability != record.capability {
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
                    record.capability = capability;
                }
                if let Some(allowed_actions) = allowed_actions {
                    record.allowed_actions = Self::validate_actions(allowed_actions)?;
                }
                // runtime-identity fields: each Some overwrites, None keeps
                // the current value.
                if let Some(recipe_hash) = recipe_hash {
                    Self::validate_recipe_hash(&recipe_hash)?;
                    record.recipe_hash = recipe_hash;
                }
                if let Some(caps) = caps {
                    record.caps = Self::validate_caps(caps)?;
                }
                if let Some(skills) = skills {
                    Self::validate_skills(&skills)?;
                    record.skills = skills;
                }
                record.updated_at = now;
                self.store_bounded(
                    agent_key(&agent_id),
                    &record,
                    MAX_AGENT_RECORD_BYTES,
                    "agent",
                )
            }
            AgentMsg::PauseAgent { agent_id } => {
                self.stage_status(ctx, agent_id, AgentStatus::Paused, now).await
            }
            AgentMsg::ResumeAgent { agent_id } => {
                self.stage_status(ctx, agent_id, AgentStatus::Active, now).await
            }
            AgentMsg::DeregisterAgent { agent_id } => self.on_deregister(ctx, agent_id).await,
        }
    }

    /// remove `agent_id` from the registry: the record and its roster slot
    /// commit or abort together, same as registration. notifies the hook so
    /// the dispatch-plane recipe is retired in the same block — the recipe
    /// owner is `Origin::Module("runs")` (the hook target itself emits
    /// `RegisterRecipe`), so runs' own `RemoveRecipe` follow-up is always
    /// authorized regardless of who deregistered the agent here.
    async fn on_deregister(&mut self, ctx: &mut dyn Ctx, agent_id: String) -> Result<(), Error> {
        self.deregistrable_agent(ctx, &agent_id).await?;
        let mut roster = self.roster().await?;
        let Ok(position) = roster.binary_search(&agent_id) else {
            // the roster is the one existence authority (see RegisterAgent):
            // an id with a record but no roster slot cannot happen.
            return Err(Error::Module(format!("unknown agent: {agent_id}")));
        };
        roster.remove(position);
        self.store(ROSTER_KEY.to_vec(), &roster);
        self.staged.delete(agent_key(&agent_id));
        self.emit_hook(ctx, &AgentEvent::Deregistered { agent_id });
        Ok(())
    }

    async fn stage_status(
        &mut self,
        ctx: &dyn Ctx,
        agent_id: String,
        status: AgentStatus,
        now: u64,
    ) -> Result<(), Error> {
        let mut record = self.owned_agent(ctx, &agent_id).await?;
        if record.status == status {
            // idempotent: staging nothing keeps the op log — and the root —
            // byte-identical to no write at all.
            return Ok(());
        }
        record.status = status;
        record.updated_at = now;
        self.store_bounded(
            agent_key(&agent_id),
            &record,
            MAX_AGENT_RECORD_BYTES,
            "agent",
        )
    }
}

#[async_trait::async_trait(?Send)]
impl Module for AgentModule {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    /// the store's merkle root over all committed records, verbatim — the
    /// staged overlay is invisible here until `commit_block`.
    fn root(&self) -> StateRoot {
        self.staged.root()
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        self.staged.state_sync_handle()
    }

    /// the network state-sync serve lane: answers the shared qmdb wire requests
    /// (historical proof-carrying op ranges) from committed state. read-only;
    /// the joiner's sync engine merkle-verifies every batch.
    async fn serve_sync(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        self.staged.serve_sync(req).await
    }

    async fn resolver_sync_target(&self) -> Result<ResolverSyncTarget, Error> {
        self.staged.sync_target().await
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
                // roster order IS the reply order (sorted by id). a rostered
                // id without a record is a store bug — loud, never skipped.
                let mut agents = Vec::new();
                for agent_id in self.roster().await? {
                    let Some(record) = self.agent(&agent_id).await? else {
                        return Err(Error::Module(format!("missing agent record: {agent_id}")));
                    };
                    agents.push(record);
                }
                Ok(encode_reply(&AgentReply::Agents(agents)))
            }
            AgentQuery::Agent { agent_id } => Ok(encode_reply(&AgentReply::Agent(
                self.agent(&agent_id).await?,
            ))),
        }
    }

    /// publish the block's staged writes in ONE store batch. no-op (and no
    /// root movement) if nothing was staged.
    async fn commit_block(&mut self) -> Result<(), Error> {
        self.staged.commit().await
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.staged.abort();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;
    use sdk::Env;
    use sdk_testkit::{MemStore, TestCtx};

    // ---- fixtures -----------------------------------------------------------

    fn module() -> AgentModule {
        AgentModule::new("agent", Box::new(MemStore::new()), "saga", Some("runs".into()))
    }

    fn user(byte: u8) -> Origin {
        Origin::External(vec![byte; 32])
    }

    fn ctx_at(height: u64, origin: Origin) -> TestCtx {
        TestCtx::with_env(Env {
            height,
            consensus_time: height,
            origin,
            me: "agent".into(),
        })
    }

    /// decoded hook events emitted this dispatch.
    fn hook_events(ctx: &TestCtx) -> Vec<AgentEvent> {
        ctx.msgs()
            .iter()
            .filter(|m| m.target == "runs")
            .map(|m| decode_event(&m.payload).expect("hook event"))
            .collect()
    }

    #[test]
    fn agent_response_commit_message_is_optional_and_round_trips_exactly() {
        let clean = decode_response(br#"{"reply_blocks":[],"actions":[]}"#).unwrap();
        assert_eq!(clean.commit_message, None);

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

    fn admin(m: &AgentMsg) -> Msg {
        Msg {
            target: "agent".into(),
            payload: encode_msg(m),
        }
    }

    fn exec(m: &mut AgentModule, ctx: &mut TestCtx, op: &Msg) -> Result<(), Error> {
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
        let mut ctx = ctx_at(3, user(9));
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
            hook_events(&ctx),
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
            // an oversized record is rejected before staging lands.
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
            let mut ctx = ctx_at(0, origin.clone());
            let err = exec(&mut m, &mut ctx, &admin(&op)).unwrap_err();
            assert!(matches!(err, Error::Module(_)), "{origin:?} / {op:?}");
            assert!(
                hook_events(&ctx).is_empty(),
                "a rejected register never notifies the hook: {op:?}"
            );
            abort(&mut m);
            assert_eq!(m.root(), root0, "a rejected register leaves no trace");
        }
    }

    #[test]
    fn update_pause_resume_are_owner_gated() {
        let mut m = module();
        let mut ctx = ctx_at(0, user(9));
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
            let mut ctx = ctx_at(0, user(2));
            let err = exec(&mut m, &mut ctx, &admin(&op)).unwrap_err();
            assert!(matches!(err, Error::Module(_)));
            abort(&mut m);
        }

        // the owner updates fields selectively and toggles status.
        let mut ctx = ctx_at(5, user(9));
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
            hook_events(&ctx),
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
        let mut ctx = ctx_at(6, user(9));
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
        assert!(hook_events(&ctx).is_empty(), "same capability, no retune");
        commit(&mut m);

        // pausing a paused agent stages nothing: root byte-identical.
        let paused_root = m.root();
        let mut ctx = ctx_at(7, user(9));
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

        let mut ctx = ctx_at(8, user(9));
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

    /// a stranger may not deregister someone else's agent; the owner may,
    /// and doing so frees the roster slot and retires the dispatch recipe.
    #[test]
    fn deregister_is_owner_gated_and_frees_the_slot() {
        let mut m = module();
        let mut ctx = ctx_at(0, user(9));
        exec(&mut m, &mut ctx, &admin(&register("bot", &[]))).unwrap();
        commit(&mut m);

        let mut stranger_ctx = ctx_at(1, user(2));
        let err = exec(
            &mut m,
            &mut stranger_ctx,
            &admin(&AgentMsg::DeregisterAgent {
                agent_id: "bot".into(),
            }),
        )
        .unwrap_err();
        assert!(matches!(err, Error::Module(_)));
        abort(&mut m);
        assert!(get_agent(&m, "bot").is_some(), "the refused op staged nothing");

        let mut owner_ctx = ctx_at(2, user(9));
        exec(
            &mut m,
            &mut owner_ctx,
            &admin(&AgentMsg::DeregisterAgent {
                agent_id: "bot".into(),
            }),
        )
        .unwrap();
        assert_eq!(
            hook_events(&owner_ctx),
            vec![AgentEvent::Deregistered {
                agent_id: "bot".into(),
            }]
        );
        commit(&mut m);
        assert!(get_agent(&m, "bot").is_none());
        assert!(list_agents(&m).is_empty(), "the roster slot is freed");

        // the freed slot is reusable — a fresh registration under the same
        // id lands cleanly.
        let mut ctx = ctx_at(3, user(1));
        exec(&mut m, &mut ctx, &admin(&register("bot", &[]))).unwrap();
        commit(&mut m);
        assert_eq!(get_agent(&m, "bot").unwrap().owner, canonical_origin(&user(1)));
    }

    /// governance may deregister an agent it does not own — the escape hatch
    /// a squatted-roster incident needs.
    #[test]
    fn governance_may_deregister_any_agent() {
        let mut m = module();
        let mut ctx = ctx_at(0, user(9));
        exec(&mut m, &mut ctx, &admin(&register("bot", &[]))).unwrap();
        commit(&mut m);

        let mut governance_ctx = ctx_at(1, Origin::Module("governance".into()));
        exec(
            &mut m,
            &mut governance_ctx,
            &admin(&AgentMsg::DeregisterAgent {
                agent_id: "bot".into(),
            }),
        )
        .unwrap();
        commit(&mut m);
        assert!(get_agent(&m, "bot").is_none());
    }

    /// the per-owner cap: one owner may not fill the registry alone, and the
    /// refusal names the cap. deregistering one of the owner's agents frees
    /// a slot the SAME owner can immediately reuse.
    #[test]
    fn the_per_owner_cap_refuses_the_next_registration_and_a_deregister_frees_it() {
        let mut m = module();
        for i in 0..MAX_AGENTS_PER_OWNER {
            let mut ctx = ctx_at(0, user(9));
            exec(&mut m, &mut ctx, &admin(&register(&format!("a{i:04}"), &[]))).unwrap();
        }
        commit(&mut m);

        let mut ctx = ctx_at(0, user(9));
        let err = exec(&mut m, &mut ctx, &admin(&register("overflow", &[]))).unwrap_err();
        assert!(
            matches!(&err, Error::Module(msg) if msg.contains(&MAX_AGENTS_PER_OWNER.to_string())),
            "the refusal must name the per-owner cap: {err:?}"
        );
        abort(&mut m);

        exec(
            &mut m,
            &mut ctx,
            &admin(&AgentMsg::DeregisterAgent {
                agent_id: "a0000".into(),
            }),
        )
        .unwrap();
        commit(&mut m);
        exec(&mut m, &mut ctx, &admin(&register("overflow", &[]))).unwrap();
        commit(&mut m);
        assert_eq!(list_agents(&m).len(), MAX_AGENTS_PER_OWNER);
    }

    #[test]
    fn a_direct_saga_callback_is_a_dead_letter() {
        let mut m = module();
        let root0 = m.root();
        let mut ctx = ctx_at(0, Origin::Module("saga".into()));
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
        assert!(!ctx.events().is_empty(), "the tombstone leaves a breadcrumb");
        commit(&mut m);
        assert_eq!(m.root(), root0);
    }

    #[test]
    fn a_module_origin_may_own_an_agent() {
        let mut m = module();
        let mut ctx = ctx_at(0, Origin::Module("automations".into()));
        exec(&mut m, &mut ctx, &admin(&register("bot", &[]))).unwrap();
        commit(&mut m);
        assert_eq!(
            get_agent(&m, "bot").unwrap().owner,
            SagaOrigin::Module("automations".into())
        );
    }

    #[test]
    fn without_a_hook_registration_stages_but_notifies_nobody() {
        let mut m = AgentModule::new("agent", Box::new(MemStore::new()), "saga", None);
        let mut ctx = ctx_at(0, user(9));
        exec(&mut m, &mut ctx, &admin(&register("bot", &[]))).unwrap();
        assert!(ctx.msgs().is_empty(), "no hook, no follow-ups");
        commit(&mut m);
        assert!(get_agent(&m, "bot").is_some());
    }

    // ---- queries + state sync ---------------------------------------------------

    #[test]
    fn queries_list_agents_staged_over_committed() {
        let mut m = module();
        let mut ctx = ctx_at(0, user(9));
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
    fn the_listing_is_sorted_by_id_regardless_of_registration_order() {
        let mut m = module();
        let mut ctx = ctx_at(0, user(9));
        exec(&mut m, &mut ctx, &admin(&register("beta", &[]))).unwrap();
        exec(&mut m, &mut ctx, &admin(&register("alpha", &[]))).unwrap();
        commit(&mut m);
        let ids: Vec<String> = list_agents(&m).into_iter().map(|a| a.agent_id).collect();
        assert_eq!(ids, vec!["alpha".to_string(), "beta".to_string()]);
    }

    /// the registry COUNT cap: the roster is one replicated record and the
    /// `All` engagement domain, so registration `MAX_REGISTERED_AGENTS + 1`
    /// is refused loudly — and the refusal names the cap. spread the fill
    /// across `MAX_REGISTERED_AGENTS / MAX_AGENTS_PER_OWNER` distinct owners
    /// so the per-owner cap (a separate test) never fires first.
    #[test]
    fn the_registry_count_cap_refuses_the_next_registration() {
        let mut m = module();
        let owners = MAX_REGISTERED_AGENTS / MAX_AGENTS_PER_OWNER;
        for i in 0..MAX_REGISTERED_AGENTS {
            let mut ctx = ctx_at(0, user((i % owners) as u8));
            exec(&mut m, &mut ctx, &admin(&register(&format!("a{i:04}"), &[]))).unwrap();
        }
        let mut ctx = ctx_at(0, user(owners as u8));
        let err = exec(&mut m, &mut ctx, &admin(&register("overflow", &[]))).unwrap_err();
        assert!(
            matches!(&err, Error::Module(msg) if msg.contains(&MAX_REGISTERED_AGENTS.to_string())),
            "the refusal must name the cap: {err:?}"
        );
        commit(&mut m);
        assert_eq!(list_agents(&m).len(), MAX_REGISTERED_AGENTS);
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
                let mut ctx = ctx_at(i as u64 + 1, origin.clone());
                exec(&mut m, &mut ctx, &admin(op)).unwrap();
                commit(&mut m);
            }
            m
        };
        let (a, b) = (run(), run());
        assert_eq!(a.root(), b.root());
        assert_ne!(a.root(), module().root(), "state moved the root");
    }

    /// the module rides the store's resolver sync lane — the manifest capture
    /// path branches on this handle, so agent joins/restores like the other
    /// qmdb modules (never as checkpoint snapshot bytes).
    #[test]
    fn state_sync_handle_is_resolver_backed() {
        let m = module();
        match m.state_sync_handle().unwrap() {
            StateSyncHandle::ResolverBacked { backend, .. } => assert_eq!(backend, "qmdb"),
            other => panic!("unexpected handle: {other:?}"),
        }
    }

    // ---- runtime identity ------------------------------------------------------

    /// register the runtime-identity fields, commit: every field round-trips
    /// through the committed record.
    #[test]
    fn round_trips_recipe_caps_and_skills() {
        let mut m = module();
        let mut ctx = ctx_at(3, user(9));
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
    }

    /// the soul is the curated skill set: an agent registers with NO prompt at
    /// all, and its persona is just a skill loaded `Always`. the load mode is
    /// committed state, so flipping one moves the root — what the model IS
    /// changed.
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
            let mut ctx = ctx_at(3, user(9));
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

        // …and the SAME skills with one load mode flipped is a different
        // root: an always-skill is inlined into the assembled context
        // document and an on-demand one is not, so the two agents do not
        // think alike.
        assert_ne!(
            registry(LoadMode::OnDemand).root(),
            m.root(),
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
        let mut ctx = ctx_at(0, user(9));
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
    fn role_defaults_to_general() {
        let record: AgentRecord = serde_json::from_value(serde_json::json!({
            "agent_id": "bot",
            "owner": { "external": [9] },
            "display_name": "BOT",
            "capability": "model-1",
            "allowed_actions": [],
            "status": "active",
            "created_at": 0,
            "updated_at": 0
        }))
        .expect("a record without a role decodes to General");
        assert_eq!(record.role, AgentRole::General);
        assert!(
            serde_json::to_value(&record).unwrap().get("role").is_none(),
            "the default role stays absent on the JSON wire"
        );
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
        let mut ctx = ctx_at(0, user(9));
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
    /// caps list is rejected before staging lands.
    #[test]
    fn record_size_gate_counts_runtime_fields() {
        let mut m = module();
        let mut ctx = ctx_at(0, user(9));
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
            let mut ctx = ctx_at(1, user(9));
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
}
