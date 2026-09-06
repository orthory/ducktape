//! Model configuration belongs to runs; identity remains the account authority.
use super::*;
use capability::validate_tag;

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

/// a skill's `source_prefix` must be a SCOPED duckfs subtree, never a
/// namespace root: `resolve_skills` copies it verbatim into a run's
/// dispatch payload, and the provisioner's checkout runs one full checkout
/// per mount, so an unscoped prefix ("/", "/shared", "/home/<x>") makes every
/// run of the agent check out the whole namespace once per curated skill.
///
/// this is a local, minimal stand-in for `files::paths::canonical` (absolute,
/// `/`-separated, no empty/dot segments, at least 3 segments deep — the same
/// depth `library_skills` requires for `/shared/skills/<name>`): the agent
/// module is self-contained by design (see the crate doc), so it does not
/// depend on another module's crate for this shape check.
fn is_scoped_duckfs_prefix(prefix: &str) -> bool {
    if !prefix.starts_with('/') || prefix.contains('\0') {
        return false;
    }
    let segments: Vec<&str> = prefix.trim_start_matches('/').split('/').collect();
    let all_named = segments
        .iter()
        .all(|s| !s.is_empty() && *s != "." && *s != "..");
    all_named && segments.len() >= 3
}

// ---- the module -----------------------------------------------------------

impl RunsModule {
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

    /// a v4 skill ref must carry a name that is [`is_skill_mount_name`] (the
    /// SAME predicate the noded provisioner's `mount_dir_name` calls — one
    /// rule, not two that could drift), unique within the record, and a
    /// source_prefix that is a scoped duckfs subtree ([`is_scoped_duckfs_prefix`]),
    /// also unique within the record. a pinned snapshot, when present, must be
    /// non-empty. order is preserved verbatim (skills are an ordered override
    /// list).
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
        let mut names = BTreeSet::new();
        let mut prefixes = BTreeSet::new();
        for skill in skills {
            if !is_skill_mount_name(&skill.name) {
                return Err(Error::Module(format!(
                    "skill name {:?} is not a safe mount directory name (want \
                     [a-zA-Z0-9._-]+, at most {MAX_SKILL_NAME_BYTES} bytes, not \".\" or \"..\")",
                    skill.name
                )));
            }
            if !names.insert(skill.name.as_str()) {
                return Err(Error::Module(format!(
                    "duplicate skill name {:?}",
                    skill.name
                )));
            }
            if !is_scoped_duckfs_prefix(&skill.source_prefix) {
                return Err(Error::Module(format!(
                    "skill source_prefix {:?} is not a scoped duckfs subtree \
                     (want an absolute path at least 3 segments deep, e.g. \
                     /shared/skills/<name>)",
                    skill.source_prefix
                )));
            }
            if !prefixes.insert(skill.source_prefix.as_str()) {
                return Err(Error::Module(format!(
                    "duplicate skill source_prefix {:?}",
                    skill.source_prefix
                )));
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

    pub(super) fn model(&self, id: &str) -> Option<&ModelRecord> {
        match self.pending_models.get(id) {
            Some(record) => record.as_ref(),
            None => self.models.get(id),
        }
    }

    pub(super) fn model_records(&self) -> Vec<ModelRecord> {
        Self::visible_ids(&self.models, &self.pending_models)
            .into_iter()
            .filter_map(|id| self.model(&id).cloned())
            .collect()
    }

    pub(super) async fn account_control(
        &self,
        ctx: &dyn Ctx,
        account: sdk::AccountNumber,
    ) -> Result<identity::Control, Error> {
        let bytes = ctx
            .query(
                "identity",
                &identity::encode_query(&identity::IdentityQuery::Get { number: account }),
            )
            .await?;
        let identity::IdentityReply::Account(Some(view)) =
            identity::decode_reply(&bytes).map_err(Error::Module)?
        else {
            return Err(Error::Module("model account does not exist".into()));
        };
        Ok(view.control)
    }

    pub(super) async fn active_generation(
        &self,
        ctx: &dyn Ctx,
        account: u64,
    ) -> Result<u64, Error> {
        let identity::Control::Program {
            executor,
            generation,
            standing: identity::ProgramStanding::Active,
            ..
        } = self.account_control(ctx, account).await?
        else {
            return Err(Error::Module("program authority is not active".into()));
        };
        if executor != self.agent {
            return Err(Error::Module("program executor does not match".into()));
        }
        Ok(generation)
    }

    pub(super) async fn acting_account(ctx: &dyn Ctx) -> Result<sdk::AccountNumber, Error> {
        match &ctx.env().origin {
            Origin::Program(account) => Ok(*account),
            Origin::External(key) => {
                let bytes = ctx
                    .query(
                        "identity",
                        &identity::encode_query(&identity::IdentityQuery::OfKey {
                            key: key.clone(),
                        }),
                    )
                    .await?;
                let identity::IdentityReply::Account(Some(view)) =
                    identity::decode_reply(&bytes).map_err(Error::Module)?
                else {
                    return Err(Error::Module(
                        "model configuration requires an account".into(),
                    ));
                };
                Ok(view.number)
            }
            Origin::Module(_) | Origin::System => Err(Error::Module(
                "model configuration requires an account".into(),
            )),
        }
    }

    pub(super) async fn control_model(
        &self,
        ctx: &dyn Ctx,
        account: sdk::AccountNumber,
    ) -> Result<(), Error> {
        let actor = Self::acting_account(ctx).await?;
        let identity::Control::Program {
            controller,
            executor,
            ..
        } = self.account_control(ctx, account).await?
        else {
            return Err(Error::Module(
                "model requires a live program account".into(),
            ));
        };
        let may_configure = executor == self.agent && (actor == controller || actor == account);
        if !may_configure {
            return Err(Error::Module(
                "only the program or its current controller may configure its model".into(),
            ));
        }
        Ok(())
    }

    fn stage_model(&mut self, record: ModelRecord) -> Result<(), Error> {
        let bytes = sdk::wire::encode(&record);
        if bytes.len() > MAX_AGENT_RECORD_BYTES {
            return Err(Error::Module(format!(
                "model record exceeds {MAX_AGENT_RECORD_BYTES} bytes"
            )));
        }
        self.pending_models
            .insert(record.agent_id.clone(), Some(record));
        Ok(())
    }

    async fn controlled_model(&self, ctx: &dyn Ctx, id: &str) -> Result<ModelRecord, Error> {
        let Some(record) = self.model(id).cloned() else {
            return Err(Error::Module(format!("unknown model: {id}")));
        };
        self.control_model(ctx, record.account).await?;
        Ok(record)
    }

    pub(super) async fn configure_model(
        &mut self,
        ctx: &mut dyn Ctx,
        operation: ModelMsg,
    ) -> Result<(), Error> {
        match operation {
            ModelMsg::RegisterModel {
                account,
                agent_id,
                display_name,
                capability,
                allowed_actions,
                recipe_hash,
                caps,
                skills,
            } => {
                self.control_model(ctx, account).await?;
                validate_agent_id(&agent_id).map_err(Error::Module)?;
                Self::validate_non_empty("display_name", &display_name)?;
                validate_tag(&capability).map_err(Error::Module)?;
                if self.model(&agent_id).is_some() {
                    return Err(Error::Module(format!("model already exists: {agent_id}")));
                }
                let records = self.model_records();
                if records.len() >= MAX_REGISTERED_AGENTS {
                    return Err(Error::Module("model registry is full".into()));
                }
                let owner = canonical_origin(&ctx.env().origin)?;
                let owned = records
                    .iter()
                    .filter(|record| record.owner == owner)
                    .count();
                if owned >= MAX_AGENTS_PER_OWNER {
                    return Err(Error::Module("model owner allocation is full".into()));
                }
                let recipe_hash = recipe_hash.unwrap_or_default();
                Self::validate_recipe_hash(&recipe_hash)?;
                let skills = skills.unwrap_or_default();
                Self::validate_skills(&skills)?;
                let record = ModelRecord {
                    account,
                    agent_id: agent_id.clone(),
                    owner,
                    display_name,
                    capability: capability.clone(),
                    allowed_actions: Self::validate_actions(allowed_actions)?,
                    status: ModelStatus::Active,
                    role: ModelRole::default(),
                    created_at: ctx.env().consensus_time,
                    updated_at: ctx.env().consensus_time,
                    recipe_hash,
                    caps: Self::validate_caps(caps.unwrap_or_default())?,
                    skills,
                };
                self.stage_model(record)?;
                self.on_model_event(
                    ctx,
                    ModelEvent::Registered {
                        agent_id,
                        capability,
                    },
                )
            }
            ModelMsg::UpdateModel {
                agent_id,
                display_name,
                capability,
                allowed_actions,
                recipe_hash,
                caps,
                skills,
            } => {
                let mut record = self.controlled_model(ctx, &agent_id).await?;
                if let Some(name) = display_name {
                    Self::validate_non_empty("display_name", &name)?;
                    record.display_name = name;
                }
                if let Some(capability) = capability {
                    validate_tag(&capability).map_err(Error::Module)?;
                    if capability != record.capability {
                        self.on_model_event(
                            ctx,
                            ModelEvent::CapabilityChanged {
                                agent_id: agent_id.clone(),
                                capability: capability.clone(),
                            },
                        )?;
                    }
                    record.capability = capability;
                }
                if let Some(actions) = allowed_actions {
                    record.allowed_actions = Self::validate_actions(actions)?;
                }
                if let Some(hash) = recipe_hash {
                    Self::validate_recipe_hash(&hash)?;
                    record.recipe_hash = hash;
                }
                if let Some(caps) = caps {
                    record.caps = Self::validate_caps(caps)?;
                }
                if let Some(skills) = skills {
                    Self::validate_skills(&skills)?;
                    record.skills = skills;
                }
                record.updated_at = ctx.env().consensus_time;
                self.stage_model(record)
            }
            ModelMsg::PauseModel { agent_id } => {
                self.set_model_status(ctx, agent_id, ModelStatus::Paused)
                    .await
            }
            ModelMsg::ResumeModel { agent_id } => {
                self.set_model_status(ctx, agent_id, ModelStatus::Active)
                    .await
            }
            ModelMsg::DeregisterModel { agent_id } => {
                self.controlled_model(ctx, &agent_id).await?;
                self.pending_models.insert(agent_id.clone(), None);
                self.on_model_event(ctx, ModelEvent::Deregistered { agent_id })
            }
        }
    }

    async fn set_model_status(
        &mut self,
        ctx: &dyn Ctx,
        id: String,
        status: ModelStatus,
    ) -> Result<(), Error> {
        let mut record = self.controlled_model(ctx, &id).await?;
        record.status = status;
        record.updated_at = ctx.env().consensus_time;
        self.stage_model(record)
    }
}
