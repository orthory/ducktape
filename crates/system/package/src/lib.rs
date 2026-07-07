//! the package registry module — the consensus half of the quack packaged-
//! module system: one row per installed package (lifecycle status, logical ->
//! module bindings, the recorded harness, the installer origin) plus the
//! tag -> owner action-route table every open action resolves through.
//!
//! like `memory`/`tasks`, this is a state-based (not qmdb-backed) module: it
//! stages all mutations into a pending working copy during `execute`, publishes
//! them at `commit_block`, discards them at `abort_block`, and computes
//! `root()` as a sha256 over a canonical byte encoding of the COMMITTED state.
//! `snapshot()`/`install()` use that exact preimage so a joiner can verify a
//! peer image against the expected root before adopting it.
//!
//! ## install choreography (design D3/D4)
//!
//! `Install` validates the whole spec BEFORE staging (registration posture: a
//! bad spec is a clean `Err` on the installer's own block), stages the row as
//! `Installing` plus its action routes, then emits — same block — one
//! `memory::Publish` per prompt seed and `HarnessMsg::InstallPackage` to the
//! harness. the harness registers its agents and hooks from module origin and
//! acks with `PackageMsg::MarkActive`, which this module accepts ONLY from the
//! recorded harness's module origin. atomicity is host-lent: any failing step
//! aborts the whole block, so a partial install cannot land.
//!
//! `Suspend`/`Resume`/`Unplug` are gated to the installer origin or the
//! harness origin and emit the matching [`HarnessMsg`]. `Unplug` removes the
//! package's routes but TOMBSTONES the row (`Inactive`) — the audit record
//! outlives the runtime entry points, and the id stays claimed.

// the wire surface: this module's shared types, flattened at the crate root.
mod interface;
pub use interface::*;

use std::collections::{BTreeMap, BTreeSet};

use memory::{MemoryMsg, PublishBody, encode_msg as memory_encode_msg};
use saga::SagaOrigin;
use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle};
use sha2::{Digest, Sha256};

/// the canonical state form of an op origin (the agent module's pattern).
fn canonical_origin(origin: &Origin) -> SagaOrigin {
    match origin {
        Origin::External(key) => SagaOrigin::External(key.clone()),
        Origin::Module(module) => SagaOrigin::Module(module.clone()),
        Origin::System => SagaOrigin::System,
    }
}

/// one installed package. the id is the map key, so it isn't repeated here.
#[derive(Clone, Debug, PartialEq, Eq)]
struct PackageRow {
    version: String,
    /// the verified capsule's manifest hash (always [`MANIFEST_HASH_LEN`] bytes).
    manifest_hash: Vec<u8>,
    status: PackageStatus,
    /// logical id -> concrete module id, as mapped at install.
    modules: BTreeMap<String, ModuleId>,
    /// the harness's concrete module id — the ONLY origin `MarkActive` accepts.
    harness: ModuleId,
    /// the install origin — the owner capability for lifecycle ops.
    installer: SagaOrigin,
    uninstall: UninstallPolicy,
    installed_at: u64,
    updated_at: u64,
}

/// one registered action route. the tag is the map key.
#[derive(Clone, Debug, PartialEq, Eq)]
struct RouteRow {
    /// the concrete module id that serves Probe/Apply for this tag.
    owner: ModuleId,
    /// the installing package, or `None` for a genesis-seeded builtin route.
    package: Option<String>,
    /// reserved: a pinned action payload schema (v1 installs carry none).
    schema_hash: Option<Vec<u8>>,
}

impl RouteRow {
    /// whether this route currently resolves through
    /// [`PackageQuery::ActionOwner`]: a builtin (`package: None`) route always
    /// does; an installed route does only while its package row is `Active`.
    /// this centralizes the suspend guarantee in the registry itself — an
    /// owner module that forgets to self-gate on phase can no longer serve a
    /// suspended or still-installing package's actions. owner modules should
    /// keep self-gating too (defense in depth), but this is now the backstop.
    fn is_live(&self, store: &Store) -> bool {
        match &self.package {
            None => true,
            Some(package) => store
                .packages
                .get(package)
                .is_some_and(|row| row.status == PackageStatus::Active),
        }
    }
}

/// the committed (or staged) state. cloning this is how a block stages: the
/// pending copy is mutated during `execute`; `commit_block` promotes it and
/// `abort_block` drops it, leaving `root()` byte-identical.
#[derive(Clone, Default)]
struct Store {
    packages: BTreeMap<String, PackageRow>,
    routes: BTreeMap<String, RouteRow>,
}

pub struct PackageModule {
    id: ModuleId,
    /// where prompt seeds are published.
    memory_module_id: ModuleId,
    /// what `root()` / `query` observe.
    committed: Store,
    /// the per-block working copy; `None` until the block's first mutation.
    pending: Option<Store>,
}

impl PackageModule {
    /// `builtin_routes` are `(tag, owner module id)` pairs seeded into
    /// committed state at genesis (built-in actions become action specs, per
    /// the ADR) — identical on every node, so they are part of the genesis
    /// root. a malformed builtin is a genesis-wiring bug: fail fast.
    pub fn new(
        id: impl Into<ModuleId>,
        memory_module_id: impl Into<ModuleId>,
        builtin_routes: Vec<(String, String)>,
    ) -> Self {
        let mut committed = Store::default();
        for (tag, owner) in builtin_routes {
            validate_tag(&tag).expect("builtin route tag");
            assert!(
                !owner.is_empty() && owner.len() <= MAX_MODULE_ID_BYTES,
                "builtin route owner module id"
            );
            let clobbered = committed.routes.insert(
                tag,
                RouteRow {
                    owner,
                    package: None,
                    schema_hash: None,
                },
            );
            assert!(clobbered.is_none(), "duplicate builtin route tag");
        }
        Self {
            id: id.into(),
            memory_module_id: memory_module_id.into(),
            committed,
            pending: None,
        }
    }

    /// the staged view — pending if this block already wrote, else committed.
    /// validation reads THIS (two installs in one block must collide).
    fn store(&self) -> &Store {
        self.pending.as_ref().unwrap_or(&self.committed)
    }

    /// the staged working copy — cloned from committed on the block's first write.
    fn store_mut(&mut self) -> &mut Store {
        if self.pending.is_none() {
            self.pending = Some(self.committed.clone());
        }
        self.pending.as_mut().expect("just populated")
    }

    // ---- mutations (staged) ------------------------------------------------

    fn install_package(&mut self, ctx: &mut dyn Ctx, spec: InstallSpec) -> Result<(), Error> {
        // v1 posture: any AUTHENTICATED member may install — a non-empty
        // external key or a module; never the empty pre-consensus origin or
        // system.
        let installer = match &ctx.env().origin {
            Origin::External(key) if key.is_empty() => {
                return Err(Error::Module("unauthenticated external origin".into()));
            }
            Origin::External(key) => SagaOrigin::External(key.clone()),
            Origin::Module(module) => SagaOrigin::Module(module.clone()),
            Origin::System => {
                return Err(Error::Module(
                    "install requires an external or module origin".into(),
                ));
            }
        };

        // validate EVERYTHING before staging, with rejection: a bad spec must
        // never enter the root preimage (the poison-value lesson), and a
        // rejected install must emit nothing.
        let (modules, harness) = validate_spec(self.store(), ctx, &spec)?;
        let height = ctx.env().height;

        // stage the row (Installing) and its routes.
        let store = self.store_mut();
        store.packages.insert(
            spec.package.clone(),
            PackageRow {
                version: spec.version.clone(),
                manifest_hash: spec.manifest_hash.clone(),
                status: PackageStatus::Installing,
                modules: modules.clone(),
                harness: harness.clone(),
                installer,
                uninstall: spec.uninstall.clone(),
                installed_at: height,
                updated_at: height,
            },
        );
        for route in &spec.actions {
            store.routes.insert(
                route.tag.clone(),
                RouteRow {
                    owner: modules[&route.owner].clone(),
                    package: Some(spec.package.clone()),
                    schema_hash: None,
                },
            );
        }

        // same-block follow-ups: one memory publish per prompt seed (kind-
        // tagged for workspace discovery), then the harness install hand-off.
        // the harness's ack (`MarkActive`) closes the loop; any failure along
        // the chain aborts the whole block, so a partial install cannot land.
        //
        // KNOWN DoS SHAPE (squat-to-cap, documented not gated — see decision
        // in task #207): prompt paths are predictable — a manifest fixes them
        // (typically `/packages/<package>/...`) before the package is ever
        // installed, and memory has no per-path ACL, so anyone can publish to
        // a path this install will later target. an attacker who front-runs a
        // known-upcoming package id and drives one of its prompt paths to
        // `memory::MAX_GENERATIONS_PER_PATH` live generations makes every
        // future `Install` naming that path fail here: the `memory::Publish`
        // follow-up rejects with "generation cap reached", which — atomicity
        // being host-lent — aborts the WHOLE block, so the install lands
        // nowhere (clean failure, no partial row, no trace; the same
        // structural guarantee `validate_spec` gives the rest of the spec).
        // recovery does not need a privileged actor: memory's `Delete` is
        // equally unauthenticated, so anyone (typically the would-be
        // installer) can `MemoryMsg::Delete` the squatted path — this frees
        // its live head, so the next `Install` attempt starts a fresh
        // generation range under the cap — and reinstall. a persistent
        // attacker can re-squat after every delete, so this is a nuisance
        // DoS on a specific predictable path, not a one-shot break; there is
        // no trivially-cheap, obviously-correct rejection for it (that would
        // mean the registry policing an entire OTHER module's namespace by
        // path convention alone), so it is documented here rather than
        // gated.
        for prompt in &spec.prompts {
            let meta: memory::Meta = [
                ("kind".to_string(), "prompt".to_string()),
                ("package".to_string(), spec.package.clone()),
            ]
            .into();
            ctx.emit_msg(Msg {
                target: self.memory_module_id.clone(),
                payload: memory_encode_msg(&MemoryMsg::Publish {
                    path: prompt.path.clone(),
                    body: PublishBody::Inline(prompt.content.clone()),
                    meta,
                }),
            });
        }
        let package = spec.package.clone();
        ctx.emit_msg(Msg {
            target: harness,
            payload: encode_harness_msg(&HarnessMsg::InstallPackage { package, spec }),
        });
        Ok(())
    }

    fn mark_active(&mut self, ctx: &mut dyn Ctx, package: String) -> Result<(), Error> {
        let height = ctx.env().height;
        let origin = ctx.env().origin.clone();
        // gate BEFORE touching `store_mut()`: a rejected op must leave no
        // trace, including the clone `store_mut()` takes on a block's first
        // write — reading the immutable `store()` view here means an unknown
        // package or a bad origin never stages a (no-op) pending copy.
        let row = self
            .store()
            .packages
            .get(&package)
            .ok_or_else(|| Error::Module(format!("unknown package: {package}")))?;
        // ONLY the recorded harness's module origin may activate — the ack
        // proves the harness's install arm ran in this very block.
        if origin != Origin::Module(row.harness.clone()) {
            return Err(Error::Module(format!(
                "MarkActive for {package} requires the {} module origin",
                row.harness
            )));
        }
        if row.status != PackageStatus::Installing {
            return Err(Error::Module(format!(
                "package {package} is not installing"
            )));
        }
        let store = self.store_mut();
        let row = store.packages.get_mut(&package).expect("checked above");
        row.status = PackageStatus::Active;
        row.updated_at = height;
        Ok(())
    }

    /// the shared suspend/resume/unplug entry: gate to the installer origin or
    /// the harness origin, check the expected current status, flip, bump
    /// `updated_at`, and hand the caller the harness id for its follow-up.
    fn lifecycle_op(
        &mut self,
        ctx: &mut dyn Ctx,
        package: &str,
        from: &[PackageStatus],
        to: PackageStatus,
    ) -> Result<ModuleId, Error> {
        let height = ctx.env().height;
        let origin = ctx.env().origin.clone();
        let caller = canonical_origin(&origin);
        // gate BEFORE touching `store_mut()` (same discipline as
        // `mark_active`): read the immutable `store()` view first so a
        // rejected op — unknown package, wrong caller, wrong status — never
        // stages a (no-op) pending clone.
        let row = self
            .store()
            .packages
            .get(package)
            .ok_or_else(|| Error::Module(format!("unknown package: {package}")))?;
        // the empty external origin can never match: install rejects it, so
        // no recorded installer is ever the empty key.
        if caller != row.installer && origin != Origin::Module(row.harness.clone()) {
            return Err(Error::Module(format!(
                "lifecycle ops on {package} are limited to its installer or harness"
            )));
        }
        if !from.contains(&row.status) {
            return Err(Error::Module(format!(
                "package {package} is {:?}, not {from:?}",
                row.status
            )));
        }
        let store = self.store_mut();
        let row = store.packages.get_mut(package).expect("checked above");
        row.status = to;
        row.updated_at = height;
        Ok(row.harness.clone())
    }

    fn suspend(&mut self, ctx: &mut dyn Ctx, package: String) -> Result<(), Error> {
        let harness = self.lifecycle_op(
            ctx,
            &package,
            &[PackageStatus::Active],
            PackageStatus::Suspended,
        )?;
        ctx.emit_msg(Msg {
            target: harness,
            payload: encode_harness_msg(&HarnessMsg::SuspendPackage { package }),
        });
        Ok(())
    }

    fn resume(&mut self, ctx: &mut dyn Ctx, package: String) -> Result<(), Error> {
        let harness = self.lifecycle_op(
            ctx,
            &package,
            &[PackageStatus::Suspended],
            PackageStatus::Active,
        )?;
        ctx.emit_msg(Msg {
            target: harness,
            payload: encode_harness_msg(&HarnessMsg::ResumePackage { package }),
        });
        Ok(())
    }

    fn unplug(&mut self, ctx: &mut dyn Ctx, package: String) -> Result<(), Error> {
        // unplug is the terminal op from any status a row can actually be
        // observed in, INCLUDING `Installing`: v1's install choreography acks
        // `MarkActive` in the same block, but a future harness (a no-fail
        // intake, or a wasm harness that can silently drop its InstallPackage
        // follow-up) could leave a row wedged in `Installing` forever with no
        // other recovery — `Suspend`/`Resume` both require a status this row
        // never reaches. allowing Unplug here is the escape hatch.
        let harness = self.lifecycle_op(
            ctx,
            &package,
            &[
                PackageStatus::Installing,
                PackageStatus::Active,
                PackageStatus::Suspended,
            ],
            PackageStatus::Inactive,
        )?;
        // remove the package's runtime entry points (its routes); the row
        // stays as an audit tombstone and the id stays claimed.
        let store = self.store_mut();
        store
            .routes
            .retain(|_, route| route.package.as_deref() != Some(package.as_str()));
        ctx.emit_msg(Msg {
            target: harness,
            payload: encode_harness_msg(&HarnessMsg::UnplugPackage { package }),
        });
        Ok(())
    }

    // ---- root / snapshot / install -----------------------------------------

    fn root_of(store: &Store) -> StateRoot {
        let mut h = Sha256::new();
        h.update(store.encode());
        StateRoot(h.finalize().into())
    }

    /// the exact `root()` preimage — the self-contained bytes a joiner installs.
    pub fn snapshot(&self) -> Vec<u8> {
        self.committed.encode()
    }

    /// adopt a peer image only after verifying it against `expected` (the
    /// consensus-committed root). the hash authenticates the BYTES (the
    /// snapshot is the exact root preimage), and the strict [`Store::decode`]
    /// behind it rejects execute-unreachable states outright.
    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        let mut h = Sha256::new();
        h.update(bytes);
        if StateRoot(h.finalize().into()) != expected {
            return Err(Error::Module("snapshot root mismatch".into()));
        }
        // the genesis-seeded builtin set: no execute path ever adds, renames,
        // or removes a `package: None` route, so THIS module's own current
        // committed view of it is exactly the genesis truth strict decode
        // checks the incoming bytes against (see `Store::decode`).
        let builtin_routes: BTreeMap<String, String> = self
            .committed
            .routes
            .iter()
            .filter(|(_, route)| route.package.is_none())
            .map(|(tag, route)| (tag.clone(), route.owner.clone()))
            .collect();
        self.committed = Store::decode(bytes, &self.id, &builtin_routes)?;
        self.pending = None;
        Ok(())
    }
}

/// validate an install spec against the staged store + registry, returning the
/// resolved `logical -> module id` map and the harness's concrete module id.
fn validate_spec(
    store: &Store,
    ctx: &dyn Ctx,
    spec: &InstallSpec,
) -> Result<(BTreeMap<String, ModuleId>, ModuleId), Error> {
    let fail = |what: String| Err::<(), Error>(Error::Module(what));

    validate_tag(&spec.package).map_err(Error::Module)?;
    if store.packages.contains_key(&spec.package) {
        // tombstones keep their id claimed — reinstalls are a v2 concern.
        fail(format!("package already installed: {}", spec.package))?;
    }
    if store.packages.len() >= MAX_PACKAGES {
        fail("package cap reached".into())?;
    }
    if spec.version.is_empty() || spec.version.len() > MAX_VERSION_BYTES {
        fail("version must be 1..=64 bytes".into())?;
    }
    if spec.manifest_hash.len() != MANIFEST_HASH_LEN {
        fail("manifest hash must be 32 bytes".into())?;
    }

    // bindings: every logical maps to a REGISTERED module. a dead binding
    // would leave routes (or the harness follow-up) pointing at nothing.
    if spec.modules.is_empty() || spec.modules.len() > MAX_MODULE_BINDINGS {
        fail("module bindings must be 1..=16".into())?;
    }
    let mut modules: BTreeMap<String, ModuleId> = BTreeMap::new();
    for binding in &spec.modules {
        validate_tag(&binding.logical).map_err(Error::Module)?;
        if binding.module_id.is_empty() || binding.module_id.len() > MAX_MODULE_ID_BYTES {
            fail(format!("module id is invalid: {}", binding.logical))?;
        }
        // never the registry itself: a HarnessMsg emitted back at this module
        // would be mis-decoded as a PackageMsg and poison the block.
        if binding.module_id == ctx.env().me {
            fail("a package may not bind the package registry".into())?;
        }
        if ctx.module_root(&binding.module_id).is_none() {
            fail(format!("unknown module id: {}", binding.module_id))?;
        }
        if modules
            .insert(binding.logical.clone(), binding.module_id.clone())
            .is_some()
        {
            fail(format!("duplicate binding logical: {}", binding.logical))?;
        }
    }
    let harness = modules
        .get(&spec.harness)
        .cloned()
        .ok_or_else(|| Error::Module(format!("harness is not bound: {}", spec.harness)))?;

    // prompt seeds: memory's path/body caps apply HERE so a bad seed is a
    // clean install rejection, not a follow-up failure deep in the block; the
    // pin must be the content's real sha256 or every PromptRef against it
    // would fail at compose time.
    if spec.prompts.len() > MAX_PROMPT_SEEDS {
        fail("prompt seed cap exceeded".into())?;
    }
    let mut prompt_logicals: BTreeSet<&str> = BTreeSet::new();
    let mut prompt_paths: BTreeSet<&str> = BTreeSet::new();
    for prompt in &spec.prompts {
        validate_tag(&prompt.logical).map_err(Error::Module)?;
        if !prompt_logicals.insert(&prompt.logical) {
            fail(format!("duplicate prompt logical: {}", prompt.logical))?;
        }
        validate_prompt_path(&prompt.path)?;
        if !prompt_paths.insert(&prompt.path) {
            fail(format!("duplicate prompt path: {}", prompt.path))?;
        }
        if prompt.content.len() > memory::MAX_BODY_BYTES {
            fail(format!("prompt content exceeds cap: {}", prompt.logical))?;
        }
        if prompt.sha256.len() != 32
            || Sha256::digest(prompt.content.as_bytes()).as_slice() != prompt.sha256
        {
            fail(format!(
                "prompt content does not hash to its pin: {}",
                prompt.logical
            ))?;
        }
    }

    // action routes: valid tag shape, owner bound, and UNROUTED — collisions
    // (builtin tags included) reject the install.
    if spec.actions.len() > MAX_ACTION_ROUTES {
        fail("action route cap exceeded".into())?;
    }
    if store.routes.len() + spec.actions.len() > MAX_ROUTES {
        fail("route table cap reached".into())?;
    }
    let mut tags: BTreeSet<&str> = BTreeSet::new();
    for route in &spec.actions {
        validate_tag(&route.tag).map_err(Error::Module)?;
        if !modules.contains_key(&route.owner) {
            fail(format!("action owner is not bound: {}", route.owner))?;
        }
        if store.routes.contains_key(&route.tag) || !tags.insert(&route.tag) {
            fail(format!("action tag already routed: {}", route.tag))?;
        }
    }

    // agent seeds: prompts by logical, actions from the declared set only.
    if spec.agents.len() > MAX_AGENT_SEEDS {
        fail("agent seed cap exceeded".into())?;
    }
    let mut agent_ids: BTreeSet<&str> = BTreeSet::new();
    for agent in &spec.agents {
        validate_tag(&agent.agent_id).map_err(Error::Module)?;
        if !agent_ids.insert(&agent.agent_id) {
            fail(format!("duplicate agent id: {}", agent.agent_id))?;
        }
        if agent.display_name.is_empty() || agent.display_name.len() > MAX_DISPLAY_NAME_BYTES {
            fail(format!("agent display name is invalid: {}", agent.agent_id))?;
        }
        validate_tag(&agent.capability).map_err(Error::Module)?;
        if !prompt_logicals.contains(agent.prompt.as_str()) {
            fail(format!("agent prompt is not seeded: {}", agent.agent_id))?;
        }
        if agent.actions.len() > MAX_ACTIONS_PER_AGENT {
            fail(format!("agent action cap exceeded: {}", agent.agent_id))?;
        }
        for action in &agent.actions {
            if !tags.contains(action.as_str()) {
                fail(format!(
                    "agent {} granted an undeclared action: {action}",
                    agent.agent_id
                ))?;
            }
        }
    }

    // engagement rules: source bound, agent declared, tag-shaped names.
    if spec.engagements.len() > MAX_ENGAGEMENT_RULES {
        fail("engagement rule cap exceeded".into())?;
    }
    for rule in &spec.engagements {
        if !modules.contains_key(&rule.source) {
            fail(format!("engagement source is not bound: {}", rule.source))?;
        }
        validate_tag(&rule.event).map_err(Error::Module)?;
        if !agent_ids.contains(rule.agent.as_str()) {
            fail(format!("engagement agent is not declared: {}", rule.agent))?;
        }
        validate_tag(&rule.policy).map_err(Error::Module)?;
    }

    validate_uninstall(&spec.uninstall)?;
    Ok((modules, harness))
}

/// v1 accepts drain-or-cancel for pending runs and ONLY the ADR's
/// preserve-by-default posture for user data.
fn validate_uninstall(policy: &UninstallPolicy) -> Result<(), Error> {
    if policy.pending_runs != "drain" && policy.pending_runs != "cancel" {
        return Err(Error::Module(
            "uninstall pending_runs must be \"drain\" or \"cancel\"".into(),
        ));
    }
    if policy.user_data != "preserve" {
        return Err(Error::Module(
            "uninstall user_data must be \"preserve\"".into(),
        ));
    }
    Ok(())
}

/// a prompt path must already be in memory's canonical file-path form —
/// absolute, `/`-separated, no empty/`.`/`..` segments, no trailing slash,
/// within memory's byte caps — so the seed publish can never bounce off the
/// memory module mid-block.
fn validate_prompt_path(path: &str) -> Result<(), Error> {
    let err = |what: &str| Error::Module(format!("prompt path {what}: {path}"));
    if path.len() > memory::MAX_PATH_BYTES {
        return Err(err("exceeds byte cap"));
    }
    let rest = path
        .strip_prefix('/')
        .ok_or_else(|| err("must be absolute"))?;
    if rest.is_empty() {
        return Err(err("must name a file"));
    }
    for segment in rest.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            return Err(err("has an invalid segment"));
        }
        if segment.len() > memory::MAX_SEGMENT_BYTES {
            return Err(err("has an oversized segment"));
        }
    }
    Ok(())
}

impl Store {
    fn view_of(&self, package: &str, row: &PackageRow) -> PackageView {
        PackageView {
            package: package.to_string(),
            version: row.version.clone(),
            manifest_hash: row.manifest_hash.clone(),
            status: row.status,
            modules: row.modules.clone(),
            harness: row.harness.clone(),
            installer: row.installer.clone(),
            uninstall: row.uninstall.clone(),
            installed_at: row.installed_at,
            updated_at: row.updated_at,
        }
    }

    // ---- canonical encode / decode (the root preimage) ---------------------
    // u64-le counts, sorted keys, every field in declaration order: u64-le
    // length prefixes for byte strings, single-byte discriminants for enums, a
    // 0/1 tag byte for options. no version byte — encoding changes are
    // flag-day (design principle: no backwards compatibility).

    fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(self.packages.len() as u64).to_le_bytes());
        for (id, row) in &self.packages {
            push_str(&mut out, id);
            push_str(&mut out, &row.version);
            push_bytes(&mut out, &row.manifest_hash);
            out.push(status_byte(row.status));
            out.extend_from_slice(&(row.modules.len() as u64).to_le_bytes());
            for (logical, module_id) in &row.modules {
                push_str(&mut out, logical);
                push_str(&mut out, module_id);
            }
            push_str(&mut out, &row.harness);
            push_origin(&mut out, &row.installer);
            push_str(&mut out, &row.uninstall.pending_runs);
            push_str(&mut out, &row.uninstall.user_data);
            out.extend_from_slice(&row.installed_at.to_le_bytes());
            out.extend_from_slice(&row.updated_at.to_le_bytes());
        }
        out.extend_from_slice(&(self.routes.len() as u64).to_le_bytes());
        for (tag, route) in &self.routes {
            push_str(&mut out, tag);
            push_str(&mut out, &route.owner);
            match &route.package {
                None => out.push(0),
                Some(package) => {
                    out.push(1);
                    push_str(&mut out, package);
                }
            }
            match &route.schema_hash {
                None => out.push(0),
                Some(hash) => {
                    out.push(1);
                    push_bytes(&mut out, hash);
                }
            }
        }
        out
    }

    /// strict decode: only canonical encodings of execute-reachable states are
    /// accepted. every section demands strictly-ascending unique keys (the
    /// only order [`Store::encode`] emits), tag-shaped ids, execute-time caps,
    /// a harness drawn from the row's own bindings, an authenticated installer
    /// (install admits no system or empty-key origin), no `Unplugging` rows
    /// (v1 unplugs within one block), and package routes that resolve to a
    /// live row and one of its bound modules (unplug removes routes with the
    /// tombstone). anything else is rejected — an honest validator can never
    /// have committed it.
    ///
    /// two more execute-unreachable shapes are rejected here: no row may bind
    /// `own_id` (the registry's own module id — `validate_spec` refuses this
    /// at install time so it can never be committed either), and every
    /// `package: None` route must be exactly one of `builtin_routes` (tag AND
    /// owner) — no execute path ever adds, renames, or drops a builtin route,
    /// so the `None`-routed set can never legitimately differ from genesis.
    fn decode(
        bytes: &[u8],
        own_id: &str,
        builtin_routes: &BTreeMap<String, String>,
    ) -> Result<Store, Error> {
        let mut off = 0usize;
        let mut store = Store::default();

        let package_count = read_count(bytes, &mut off)?;
        if package_count > MAX_PACKAGES as u64 {
            return Err(Error::Module("snapshot package count exceeds cap".into()));
        }
        for _ in 0..package_count {
            let id = read_string(bytes, &mut off)?;
            validate_tag(&id).map_err(Error::Module)?;
            if store
                .packages
                .last_key_value()
                .is_some_and(|(last, _)| last.as_str() >= id.as_str())
            {
                return Err(Error::Module(
                    "snapshot package ids not strictly ascending".into(),
                ));
            }
            let version = read_string(bytes, &mut off)?;
            if version.is_empty() || version.len() > MAX_VERSION_BYTES {
                return Err(Error::Module("snapshot version is invalid".into()));
            }
            let manifest_hash = read_bytes(bytes, &mut off)?;
            if manifest_hash.len() != MANIFEST_HASH_LEN {
                return Err(Error::Module("snapshot manifest hash is invalid".into()));
            }
            let status = read_status(bytes, &mut off)?;
            let binding_count = read_count(bytes, &mut off)?;
            if binding_count == 0 || binding_count > MAX_MODULE_BINDINGS as u64 {
                return Err(Error::Module("snapshot binding count is invalid".into()));
            }
            let mut modules: BTreeMap<String, ModuleId> = BTreeMap::new();
            for _ in 0..binding_count {
                let logical = read_string(bytes, &mut off)?;
                validate_tag(&logical).map_err(Error::Module)?;
                if modules
                    .last_key_value()
                    .is_some_and(|(last, _)| last.as_str() >= logical.as_str())
                {
                    return Err(Error::Module(
                        "snapshot bindings not strictly ascending".into(),
                    ));
                }
                let module_id = read_string(bytes, &mut off)?;
                if module_id.is_empty() || module_id.len() > MAX_MODULE_ID_BYTES {
                    return Err(Error::Module("snapshot module id is invalid".into()));
                }
                // `validate_spec` refuses a binding that names the registry
                // itself (a HarnessMsg looped back here would be mis-decoded
                // as a PackageMsg and poison the block), so no committed row
                // ever carries one.
                if module_id == own_id {
                    return Err(Error::Module(
                        "snapshot binding names the registry's own module id".into(),
                    ));
                }
                modules.insert(logical, module_id);
            }
            let harness = read_string(bytes, &mut off)?;
            if !modules.values().any(|m| *m == harness) {
                return Err(Error::Module("snapshot harness is not bound".into()));
            }
            let installer = read_origin(bytes, &mut off)?;
            let uninstall = UninstallPolicy {
                pending_runs: read_string(bytes, &mut off)?,
                user_data: read_string(bytes, &mut off)?,
            };
            validate_uninstall(&uninstall)?;
            let installed_at = read_u64(bytes, &mut off)?;
            let updated_at = read_u64(bytes, &mut off)?;
            if updated_at < installed_at {
                return Err(Error::Module("snapshot heights are inverted".into()));
            }
            store.packages.insert(
                id,
                PackageRow {
                    version,
                    manifest_hash,
                    status,
                    modules,
                    harness,
                    installer,
                    uninstall,
                    installed_at,
                    updated_at,
                },
            );
        }

        let route_count = read_count(bytes, &mut off)?;
        if route_count > MAX_ROUTES as u64 {
            return Err(Error::Module("snapshot route count exceeds cap".into()));
        }
        // every `package: None` route seen must be a genesis builtin (tag AND
        // owner); the count check after the loop (against
        // `builtin_routes.len()`) then forces exact set equality, since tags
        // are already unique and strictly ascending.
        let mut builtin_route_count: usize = 0;
        for _ in 0..route_count {
            let tag = read_string(bytes, &mut off)?;
            validate_tag(&tag).map_err(Error::Module)?;
            if store
                .routes
                .last_key_value()
                .is_some_and(|(last, _)| last.as_str() >= tag.as_str())
            {
                return Err(Error::Module(
                    "snapshot route tags not strictly ascending".into(),
                ));
            }
            let owner = read_string(bytes, &mut off)?;
            if owner.is_empty() || owner.len() > MAX_MODULE_ID_BYTES {
                return Err(Error::Module("snapshot route owner is invalid".into()));
            }
            let package = match read_byte(bytes, &mut off)? {
                0 => {
                    // no execute path ever inserts a package-less route
                    // beyond genesis, so it must be exactly one of the
                    // registry's own builtins (this module's committed view
                    // of them, per `PackageModule::install`) — not merely
                    // some plausible-looking tag/owner pair.
                    if builtin_routes.get(&tag) != Some(&owner) {
                        return Err(Error::Module(
                            "snapshot route is package-less but not a genesis builtin".into(),
                        ));
                    }
                    builtin_route_count += 1;
                    None
                }
                1 => {
                    let package = read_string(bytes, &mut off)?;
                    // a package route exists only between install and unplug:
                    // its row is live and its owner is one of the row's own
                    // bound modules.
                    let row = store.packages.get(&package).ok_or_else(|| {
                        Error::Module("snapshot route references a missing package".into())
                    })?;
                    if row.status == PackageStatus::Inactive {
                        return Err(Error::Module(
                            "snapshot route references a tombstoned package".into(),
                        ));
                    }
                    if !row.modules.values().any(|m| *m == owner) {
                        return Err(Error::Module(
                            "snapshot route owner is not bound by its package".into(),
                        ));
                    }
                    Some(package)
                }
                _ => {
                    return Err(Error::Module(
                        "snapshot route package tag is invalid".into(),
                    ));
                }
            };
            let schema_hash = match read_byte(bytes, &mut off)? {
                0 => None,
                1 => {
                    let hash = read_bytes(bytes, &mut off)?;
                    if hash.len() != MANIFEST_HASH_LEN {
                        return Err(Error::Module("snapshot schema hash is invalid".into()));
                    }
                    Some(hash)
                }
                _ => return Err(Error::Module("snapshot schema tag is invalid".into())),
            };
            store.routes.insert(
                tag,
                RouteRow {
                    owner,
                    package,
                    schema_hash,
                },
            );
        }
        if builtin_route_count != builtin_routes.len() {
            return Err(Error::Module(
                "snapshot builtin route set does not match genesis".into(),
            ));
        }

        if off != bytes.len() {
            return Err(Error::Module("snapshot has trailing bytes".into()));
        }
        Ok(store)
    }
}

#[async_trait::async_trait(?Send)]
impl Module for PackageModule {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn root(&self) -> StateRoot {
        Self::root_of(&self.committed)
    }

    /// advertise the snapshot lane: [`PackageModule::snapshot`] is the exact
    /// `root()` preimage and [`PackageModule::install`] verifies before
    /// adopting (memory/tasks pattern).
    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::SnapshotBytes(self.snapshot()))
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        // every arm is a registration/admin op riding its caller's block, so
        // decode-or-Err is the whole contract (no no-fail event intake here).
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            PackageMsg::Install(spec) => self.install_package(ctx, spec),
            PackageMsg::MarkActive { package } => self.mark_active(ctx, package),
            PackageMsg::Suspend { package } => self.suspend(ctx, package),
            PackageMsg::Resume { package } => self.resume(ctx, package),
            PackageMsg::Unplug { package } => self.unplug(ctx, package),
        }
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        let store = &self.committed;
        let reply = match decode_query(req).map_err(Error::Module)? {
            PackageQuery::ActionOwner { tag } => PackageReply::Owner(
                store
                    .routes
                    .get(&tag)
                    .filter(|route| route.is_live(store))
                    .map(|route| route.owner.clone()),
            ),
            PackageQuery::Get { package } => PackageReply::Package(
                store
                    .packages
                    .get(&package)
                    .map(|row| store.view_of(&package, row)),
            ),
            PackageQuery::List => PackageReply::Packages(
                store
                    .packages
                    .iter()
                    .map(|(id, row)| store.view_of(id, row))
                    .collect(),
            ),
            PackageQuery::RoutesForOwner { module } => PackageReply::Routes(
                store
                    .routes
                    .iter()
                    .filter(|(_, route)| route.owner == module)
                    .map(|(tag, _)| tag.clone())
                    .collect(),
            ),
        };
        Ok(encode_reply(&reply))
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        if let Some(pending) = self.pending.take() {
            self.committed = pending;
        }
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.pending = None;
        Ok(())
    }
}

// ---- canonical byte helpers --------------------------------------------------

fn status_byte(status: PackageStatus) -> u8 {
    match status {
        PackageStatus::Installing => 0,
        PackageStatus::Active => 1,
        PackageStatus::Suspended => 2,
        PackageStatus::Unplugging => 3,
        PackageStatus::Inactive => 4,
    }
}

fn read_status(bytes: &[u8], off: &mut usize) -> Result<PackageStatus, Error> {
    match read_byte(bytes, off)? {
        0 => Ok(PackageStatus::Installing),
        1 => Ok(PackageStatus::Active),
        2 => Ok(PackageStatus::Suspended),
        // 3 (Unplugging) is wire vocabulary only: v1 unplugs within one
        // block, so no honest validator ever commits a row in it.
        4 => Ok(PackageStatus::Inactive),
        _ => Err(Error::Module("snapshot status is invalid".into())),
    }
}

fn push_origin(out: &mut Vec<u8>, origin: &SagaOrigin) {
    match origin {
        SagaOrigin::External(key) => {
            out.push(0);
            push_bytes(out, key);
        }
        SagaOrigin::Module(module) => {
            out.push(1);
            push_str(out, module);
        }
        SagaOrigin::System => out.push(2),
    }
}

fn read_origin(bytes: &[u8], off: &mut usize) -> Result<SagaOrigin, Error> {
    match read_byte(bytes, off)? {
        0 => {
            let key = read_bytes(bytes, off)?;
            // install rejects the empty pre-consensus key, so a committed
            // installer never carries one.
            if key.is_empty() {
                return Err(Error::Module("snapshot installer key is empty".into()));
            }
            Ok(SagaOrigin::External(key))
        }
        1 => {
            let module = read_string(bytes, off)?;
            if module.is_empty() || module.len() > MAX_MODULE_ID_BYTES {
                return Err(Error::Module("snapshot installer module is invalid".into()));
            }
            Ok(SagaOrigin::Module(module))
        }
        // 2 (System) is execute-unreachable: install requires an external or
        // module origin.
        _ => Err(Error::Module("snapshot installer origin is invalid".into())),
    }
}

fn push_str(out: &mut Vec<u8>, value: &str) {
    push_bytes(out, value.as_bytes());
}

fn push_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(bytes);
}

/// a length-prefixed collection count, guarded so a corrupt count can never
/// make the decoder loop or allocate unboundedly (each entry costs >= 1 byte).
fn read_count(bytes: &[u8], off: &mut usize) -> Result<u64, Error> {
    let n = read_u64(bytes, off)?;
    if n > (bytes.len() - *off) as u64 {
        return Err(Error::Module("snapshot truncated".into()));
    }
    Ok(n)
}

fn read_byte(bytes: &[u8], off: &mut usize) -> Result<u8, Error> {
    let b = *bytes
        .get(*off)
        .ok_or_else(|| Error::Module("snapshot truncated".into()))?;
    *off += 1;
    Ok(b)
}

fn read_u64(bytes: &[u8], off: &mut usize) -> Result<u64, Error> {
    let end = off
        .checked_add(8)
        .filter(|&end| end <= bytes.len())
        .ok_or_else(|| Error::Module("snapshot truncated".into()))?;
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&bytes[*off..end]);
    *off = end;
    Ok(u64::from_le_bytes(buf))
}

fn read_bytes(bytes: &[u8], off: &mut usize) -> Result<Vec<u8>, Error> {
    let len = read_u64(bytes, off)?;
    let len = usize::try_from(len).map_err(|_| Error::Module("snapshot truncated".into()))?;
    if len > bytes.len() - *off {
        return Err(Error::Module("snapshot truncated".into()));
    }
    let value = bytes[*off..*off + len].to_vec();
    *off += len;
    Ok(value)
}

fn read_string(bytes: &[u8], off: &mut usize) -> Result<String, Error> {
    String::from_utf8(read_bytes(bytes, off)?)
        .map_err(|_| Error::Module("snapshot string is not utf-8".into()))
}

#[cfg(test)]
mod tests {
    //! white-box tests that reach `PackageModule::pending` directly — the
    //! "a rejected op leaves no trace" property `mark_active`/`lifecycle_op`
    //! now keep structurally (gate on the immutable `store()` view, take the
    //! mutable staged copy only once the op is known to proceed) is otherwise
    //! unobservable from outside the crate: every public read (`query`) always
    //! serves `committed`, and `commit_block`/`abort_block` are indifferent to
    //! whether `pending` held `None` or a no-op clone of `committed`. these
    //! tests assert the structural fact directly rather than not at all.
    use super::*;
    use futures::executor::block_on;

    struct Ctx0 {
        env: sdk::Env,
    }

    impl Ctx0 {
        fn new(origin: Origin) -> Self {
            Self {
                env: sdk::Env {
                    protocol_version: 0,
                    height: 1,
                    consensus_time: 1,
                    origin,
                    me: "package".into(),
                },
            }
        }
    }

    #[async_trait::async_trait(?Send)]
    impl Ctx for Ctx0 {
        fn env(&self) -> &sdk::Env {
            &self.env
        }
        fn module_root(&self, _target: &str) -> Option<StateRoot> {
            None
        }
        async fn query(&self, target: &str, _req: &[u8]) -> Result<Vec<u8>, Error> {
            Err(Error::UnknownModule(target.into()))
        }
        fn emit_msg(&mut self, _msg: Msg) {}
        fn emit_event(&mut self, _ev: sdk::Event) {}
        fn request_effect(&mut self, _eff: sdk::Effect) {}
    }

    fn exec(m: &mut PackageModule, ctx: &mut Ctx0, op: &PackageMsg) -> Result<(), Error> {
        let msg = Msg {
            target: "package".into(),
            payload: encode_msg(op),
        };
        block_on(m.execute(ctx, &msg))
    }

    #[test]
    fn rejected_mark_active_never_stages_a_pending_clone() {
        let mut m = PackageModule::new("package", "memory", Vec::new());
        let mut ctx = Ctx0::new(Origin::Module("nobody".into()));
        let err = exec(
            &mut m,
            &mut ctx,
            &PackageMsg::MarkActive {
                package: "ghost".into(),
            },
        );
        assert!(err.is_err(), "an unknown package must reject");
        assert!(
            m.pending.is_none(),
            "a rejected MarkActive must never clone committed into pending"
        );
    }

    #[test]
    fn rejected_lifecycle_op_never_stages_a_pending_clone() {
        let mut m = PackageModule::new("package", "memory", Vec::new());
        let mut ctx = Ctx0::new(Origin::External(b"nobody".to_vec()));
        for op in [
            PackageMsg::Suspend {
                package: "ghost".into(),
            },
            PackageMsg::Resume {
                package: "ghost".into(),
            },
            PackageMsg::Unplug {
                package: "ghost".into(),
            },
        ] {
            let err = exec(&mut m, &mut ctx, &op);
            assert!(err.is_err(), "{op:?} on an unknown package must reject");
            assert!(
                m.pending.is_none(),
                "a rejected {op:?} must never clone committed into pending"
            );
        }
    }
}
