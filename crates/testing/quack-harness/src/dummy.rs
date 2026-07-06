//! the framework's reference package: the smallest COMPLETE harness module.
//!
//! `DummyHarness` implements the whole harness contract (design D4) against a
//! trivial domain — a keyed note pad — so the framework can prove itself
//! end-to-end without the real `docs-harness`, and a package author has a
//! template to copy:
//!
//! - `HarnessMsg` arms, accepted ONLY from the package module's origin:
//!   install registers a `PageMsg::RegisterHook` on every engagement source,
//!   registers each agent seed from module origin (owner = this harness) with
//!   a `PromptRef` pinned to the seeded memory path, and acks with
//!   `PackageMsg::MarkActive`; suspend/resume/unplug pause/resume/tombstone
//!   the agents (unplug also unregisters the hooks, preserving user data).
//! - `PageEvent` intake (no-fail) from the recorded sources: a comment
//!   mentioning `@<agent_id>` mints ONE idempotent `JobsMsg::Submit`
//!   (`kind = "agent/<agent_id>"`), probe-before-emit against the jobs board.
//! - the action-owner contract for `dummy.note.add` / `dummy.note.set_text`:
//!   `Probe` validates against staged-or-committed state; `Apply` is no-fail
//!   (decode-or-drop, re-check, breadcrumb on late conflict).
//!
//! ## prompt generation pin
//!
//! the install block publishes each prompt seed into memory BEFORE this
//! module's install arm runs, but queries observe committed state only — so
//! the harness pins generation 1, the committed generation a FRESH package
//! path lands on. a pre-published path would make the pin miss at compose
//! time, which fails the RUN deterministically (never the block) — the ADR's
//! prompt-pin rule, not a harness concern.

use std::collections::{BTreeMap, BTreeSet};

use agent::{AgentMsg, PromptRef, RENDERER_MEMORY_GENERATION, encode_msg as agent_encode_msg};
use jobs::{JobsMsg, JobsQuery, JobsReply, encode_msg as jobs_encode_msg};
use package::{
    HarnessMsg, InstallSpec, PackageActionMsg, PackageActionQuery, PackageActionReply, PackageMsg,
    decode_action_msg, decode_action_query, decode_harness_msg, encode_action_reply,
    encode_msg as package_encode_msg,
};
use pages::{PageEvent, PageMsg, decode_page_event, encode_msg as pages_encode_msg};
use sdk::{Ctx, Error, Event, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// create a note: `{ "note_id": ..., "text": ... }`.
pub const ACTION_NOTE_ADD: &str = "dummy.note.add";
/// replace an EXISTING note's text: `{ "note_id": ..., "text": ... }`.
pub const ACTION_NOTE_SET_TEXT: &str = "dummy.note.set_text";

/// the memory generation a fresh package prompt path commits at (see the
/// module docs on the pin assumption).
const FIRST_GENERATION: u64 = 1;

const MAX_NOTE_ID_BYTES: usize = 64;
const MAX_NOTE_TEXT_BYTES: usize = 4096;
const MAX_NOTES: usize = 1024;

// ---- wire surface ----------------------------------------------------------------

/// one note, as served by [`DummyQuery::Notes`].
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub note_id: String,
    pub text: String,
}

/// the harness's committed lifecycle view.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DummyStatus {
    pub package: String,
    /// `"active"`, `"suspended"`, or `"unplugged"`.
    pub phase: String,
    pub agents: Vec<String>,
    /// how many jobs this harness has minted over its life.
    pub minted: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DummyQuery {
    Notes,
    Status,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DummyReply {
    Notes(Vec<Note>),
    Status(Option<DummyStatus>),
}

pub fn encode_query(q: &DummyQuery) -> Vec<u8> {
    serde_json::to_vec(q).expect("serializable")
}
pub fn decode_query(b: &[u8]) -> Result<DummyQuery, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}
pub fn encode_reply(r: &DummyReply) -> Vec<u8> {
    serde_json::to_vec(r).expect("serializable")
}
pub fn decode_reply(b: &[u8]) -> Result<DummyReply, String> {
    serde_json::from_slice(b).map_err(|e| e.to_string())
}

/// both note actions share one payload schema; unknown fields reject at probe.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct NotePayload {
    note_id: String,
    text: String,
}

// ---- state -----------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Active,
    Suspended,
    Unplugged,
}

impl Phase {
    fn as_str(self) -> &'static str {
        match self {
            Phase::Active => "active",
            Phase::Suspended => "suspended",
            Phase::Unplugged => "unplugged",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Installed {
    package: String,
    phase: Phase,
    /// the engagement sources' CONCRETE module ids (sorted, deduped) — whose
    /// module-origin events this harness accepts, and where hooks live.
    sources: Vec<ModuleId>,
    /// registered agent ids, in seed order.
    agents: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Store {
    installed: Option<Installed>,
    /// the user data: note id -> text. survives suspend AND unplug.
    notes: BTreeMap<String, String>,
    /// idempotency keys of already-minted jobs (`<comment>\x1f<agent>`).
    minted: BTreeSet<String>,
}

/// what one probe/apply validated — shared so the verdict and the apply-time
/// re-check cannot drift (the tasks-module idiom).
enum ValidatedNote {
    Add { note_id: String, text: String },
    SetText { note_id: String, text: String },
}

pub struct DummyHarness {
    id: ModuleId,
    /// the package registry module (the only origin `HarnessMsg` is accepted
    /// from, and the `MarkActive` ack target).
    package: ModuleId,
    /// the agent registry (agent seeds register here, harness-owned).
    agent: ModuleId,
    /// the jobs board (engagements mint here).
    jobs: ModuleId,
    /// the memory workspace (prompt seeds live here; `PromptRef.module`).
    memory: ModuleId,
    committed: Store,
    pending: Option<Store>,
}

impl DummyHarness {
    pub fn new(
        id: impl Into<ModuleId>,
        package: impl Into<ModuleId>,
        agent: impl Into<ModuleId>,
        jobs: impl Into<ModuleId>,
        memory: impl Into<ModuleId>,
    ) -> Self {
        Self {
            id: id.into(),
            package: package.into(),
            agent: agent.into(),
            jobs: jobs.into(),
            memory: memory.into(),
            committed: Store::default(),
            pending: None,
        }
    }

    /// the staged view — pending if this block already wrote, else committed.
    fn store(&self) -> &Store {
        self.pending.as_ref().unwrap_or(&self.committed)
    }

    fn store_mut(&mut self) -> &mut Store {
        if self.pending.is_none() {
            self.pending = Some(self.committed.clone());
        }
        self.pending.as_mut().expect("just populated")
    }

    fn breadcrumb(&self, ctx: &mut dyn Ctx, what: String) {
        ctx.emit_event(Event {
            source: self.id.clone(),
            payload: what.into_bytes(),
        });
    }

    // ---- the harness contract (origin == package module) ----------------------

    fn install(
        &mut self,
        ctx: &mut dyn Ctx,
        package: String,
        spec: InstallSpec,
    ) -> Result<(), Error> {
        if self.store().installed.is_some() {
            return Err(Error::Module(
                "dummy-harness already hosts a package".into(),
            ));
        }
        let bindings: BTreeMap<&str, &str> = spec
            .modules
            .iter()
            .map(|b| (b.logical.as_str(), b.module_id.as_str()))
            .collect();

        // hook every engagement source (the registry validated the bindings).
        let mut sources: BTreeSet<ModuleId> = BTreeSet::new();
        for rule in &spec.engagements {
            let source = bindings.get(rule.source.as_str()).ok_or_else(|| {
                Error::Module(format!("engagement source is not bound: {}", rule.source))
            })?;
            sources.insert((*source).to_string());
        }
        for source in &sources {
            ctx.emit_msg(Msg {
                target: source.clone(),
                payload: pages_encode_msg(&PageMsg::RegisterHook {}),
            });
        }

        // register each agent seed FROM THIS MODULE'S ORIGIN (harness-owned),
        // its prompt pinned to the seed path at the fresh generation.
        for seed in &spec.agents {
            let prompt = spec
                .prompts
                .iter()
                .find(|p| p.logical == seed.prompt)
                .ok_or_else(|| {
                    Error::Module(format!("agent prompt is not seeded: {}", seed.agent_id))
                })?;
            ctx.emit_msg(Msg {
                target: self.agent.clone(),
                payload: agent_encode_msg(&AgentMsg::RegisterAgent {
                    agent_id: seed.agent_id.clone(),
                    display_name: seed.display_name.clone(),
                    capability: seed.capability.clone(),
                    prompt: Some(PromptRef {
                        module: self.memory.clone(),
                        target: format!("{}@{FIRST_GENERATION}", prompt.path),
                        renderer: RENDERER_MEMORY_GENERATION.into(),
                        sha256: prompt.sha256.clone(),
                    }),
                    allowed_actions: seed.actions.clone(),
                }),
            });
            if !seed.active {
                ctx.emit_msg(Msg {
                    target: self.agent.clone(),
                    payload: agent_encode_msg(&AgentMsg::PauseAgent {
                        agent_id: seed.agent_id.clone(),
                    }),
                });
            }
        }

        self.store_mut().installed = Some(Installed {
            package: package.clone(),
            phase: Phase::Active,
            sources: sources.into_iter().collect(),
            agents: spec.agents.iter().map(|a| a.agent_id.clone()).collect(),
        });

        // the ack that flips Installing -> Active — LAST, so it rides behind
        // every registration this arm staged.
        ctx.emit_msg(Msg {
            target: self.package.clone(),
            payload: package_encode_msg(&PackageMsg::MarkActive { package }),
        });
        Ok(())
    }

    /// the shared lifecycle transition: check the recorded package + expected
    /// phase, flip, and hand back the agents for the caller's follow-ups.
    fn transition(&mut self, package: &str, from: &[Phase], to: Phase) -> Result<Installed, Error> {
        let installed = self
            .store()
            .installed
            .clone()
            .ok_or_else(|| Error::Module("dummy-harness hosts no package".into()))?;
        if installed.package != package {
            return Err(Error::Module(format!(
                "dummy-harness hosts {:?}, not {package:?}",
                installed.package
            )));
        }
        if !from.contains(&installed.phase) {
            return Err(Error::Module(format!(
                "package {package} is {:?}, not {from:?}",
                installed.phase.as_str()
            )));
        }
        let store = self.store_mut();
        let record = store.installed.as_mut().expect("checked above");
        record.phase = to;
        Ok(record.clone())
    }

    fn suspend(&mut self, ctx: &mut dyn Ctx, package: String) -> Result<(), Error> {
        let record = self.transition(&package, &[Phase::Active], Phase::Suspended)?;
        for agent_id in &record.agents {
            ctx.emit_msg(Msg {
                target: self.agent.clone(),
                payload: agent_encode_msg(&AgentMsg::PauseAgent {
                    agent_id: agent_id.clone(),
                }),
            });
        }
        Ok(())
    }

    fn resume(&mut self, ctx: &mut dyn Ctx, package: String) -> Result<(), Error> {
        let record = self.transition(&package, &[Phase::Suspended], Phase::Active)?;
        for agent_id in &record.agents {
            ctx.emit_msg(Msg {
                target: self.agent.clone(),
                payload: agent_encode_msg(&AgentMsg::ResumeAgent {
                    agent_id: agent_id.clone(),
                }),
            });
        }
        Ok(())
    }

    fn unplug(&mut self, ctx: &mut dyn Ctx, package: String) -> Result<(), Error> {
        let record = self.transition(
            &package,
            &[Phase::Active, Phase::Suspended],
            Phase::Unplugged,
        )?;
        // tombstone the agents (terminal, audit-preserving) and drop the
        // hooks; the notes — user data — stay untouched (preserve-by-default).
        for agent_id in &record.agents {
            ctx.emit_msg(Msg {
                target: self.agent.clone(),
                payload: agent_encode_msg(&AgentMsg::TombstoneAgent {
                    agent_id: agent_id.clone(),
                }),
            });
        }
        for source in &record.sources {
            ctx.emit_msg(Msg {
                target: source.clone(),
                payload: pages_encode_msg(&PageMsg::UnregisterHook {}),
            });
        }
        Ok(())
    }

    // ---- engagement intake (origin == a recorded source; NO-FAIL) ---------------

    async fn on_page_event(&mut self, ctx: &mut dyn Ctx, event: PageEvent) {
        let Some(installed) = self.store().installed.clone() else {
            return; // unreachable: the hook exists only while installed.
        };
        if installed.phase != Phase::Active {
            return; // suspended/unplugged packages mint nothing.
        }
        let PageEvent::CommentAdded {
            comment_id, text, ..
        } = event
        else {
            return; // only comments engage the note taker.
        };
        for agent_id in &installed.agents {
            if !text.contains(&format!("@{agent_id}")) {
                continue;
            }
            // idempotency: one job per (comment, agent), across redeliveries.
            let key = format!("{comment_id}\u{1f}{agent_id}");
            if self.store().minted.contains(&key) {
                self.breadcrumb(
                    ctx,
                    format!("comment {comment_id} already minted a job for {agent_id}"),
                );
                continue;
            }
            // probe-before-emit: a squatted job id would make the Submit
            // follow-up abort the COMMENTER's block (this arm is no-fail).
            let job_id = format!("dummy:{agent_id}:{comment_id}");
            match self.job_exists(ctx, &job_id).await {
                Ok(false) => {
                    self.store_mut().minted.insert(key);
                    ctx.emit_msg(Msg {
                        target: self.jobs.clone(),
                        payload: jobs_encode_msg(&JobsMsg::Submit {
                            job_id,
                            kind: format!("agent/{agent_id}"),
                            spec: text.clone(),
                        }),
                    });
                }
                Ok(true) => {
                    self.breadcrumb(ctx, format!("job id already taken: {job_id}"));
                }
                Err(e) => {
                    self.breadcrumb(ctx, format!("jobs probe failed for {job_id}: {e}"));
                }
            }
        }
    }

    async fn job_exists(&self, ctx: &dyn Ctx, job_id: &str) -> Result<bool, String> {
        let reply = ctx
            .query(
                &self.jobs,
                &jobs::encode_query(&JobsQuery::Get {
                    job_id: job_id.into(),
                }),
            )
            .await
            .map_err(|e| e.to_string())?;
        match jobs::decode_reply(&reply) {
            Ok(JobsReply::Job(job)) => Ok(job.is_some()),
            Ok(other) => Err(format!("unexpected jobs reply: {other:?}")),
            Err(e) => Err(e),
        }
    }

    // ---- the action-owner contract ---------------------------------------------

    /// validate one owned action against STAGED-OR-COMMITTED state — the
    /// read-only half of `Probe` and the cheap re-check `Apply` runs.
    fn validate_action(&self, tag: &str, payload: &[u8]) -> Result<ValidatedNote, String> {
        let active = self
            .store()
            .installed
            .as_ref()
            .is_some_and(|i| i.phase == Phase::Active);
        if !active {
            return Err("the dummy package is not active".into());
        }
        let note: NotePayload =
            serde_json::from_slice(payload).map_err(|e| format!("malformed {tag} payload: {e}"))?;
        if note.note_id.is_empty() || note.note_id.len() > MAX_NOTE_ID_BYTES {
            return Err("note_id must be 1..=64 bytes".into());
        }
        if note.text.is_empty() || note.text.len() > MAX_NOTE_TEXT_BYTES {
            return Err(format!("text must be 1..={MAX_NOTE_TEXT_BYTES} bytes"));
        }
        match tag {
            ACTION_NOTE_ADD => {
                if self.store().notes.contains_key(&note.note_id) {
                    return Err(format!("note already exists: {}", note.note_id));
                }
                if self.store().notes.len() >= MAX_NOTES {
                    return Err("note cap reached".into());
                }
                Ok(ValidatedNote::Add {
                    note_id: note.note_id,
                    text: note.text,
                })
            }
            ACTION_NOTE_SET_TEXT => {
                if !self.store().notes.contains_key(&note.note_id) {
                    return Err(format!("unknown note: {}", note.note_id));
                }
                Ok(ValidatedNote::SetText {
                    note_id: note.note_id,
                    text: note.text,
                })
            }
            other => Err(format!("dummy-harness does not own action tag: {other}")),
        }
    }

    /// NO-FAIL: an accepted action's `Apply` rides the runs module's delivery
    /// block — decode-or-drop, re-check, breadcrumb on late conflict.
    fn apply_action(&mut self, ctx: &mut dyn Ctx, apply: &PackageActionMsg) {
        let PackageActionMsg::Apply {
            action_id,
            tag,
            payload,
            ..
        } = apply;
        match self.validate_action(tag, payload) {
            Ok(ValidatedNote::Add { note_id, text })
            | Ok(ValidatedNote::SetText { note_id, text }) => {
                self.store_mut().notes.insert(note_id, text);
            }
            Err(reason) => {
                self.breadcrumb(ctx, format!("action {action_id} ({tag}) dropped: {reason}"));
            }
        }
    }

    // ---- root / snapshot ---------------------------------------------------------

    fn root_of(store: &Store) -> StateRoot {
        StateRoot(Sha256::digest(store.encode()).into())
    }

    /// the exact `root()` preimage (the platform snapshot-bytes convention).
    pub fn snapshot(&self) -> Vec<u8> {
        self.committed.encode()
    }

    /// verify-then-adopt a peer image (the memory/tasks pattern).
    pub fn install_snapshot(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        let store = Store::decode(bytes)?;
        if Self::root_of(&store) != expected {
            return Err(Error::Module("snapshot root mismatch".into()));
        }
        self.committed = store;
        self.pending = None;
        Ok(())
    }
}

// ---- canonical encode / decode ------------------------------------------------------

impl Store {
    // u64-le counts, length-prefixed strings, single tag/phase bytes, sorted
    // keys — the state-based module encoding discipline.
    fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        match &self.installed {
            None => out.push(0),
            Some(installed) => {
                out.push(1);
                push_str(&mut out, &installed.package);
                out.push(match installed.phase {
                    Phase::Active => 0,
                    Phase::Suspended => 1,
                    Phase::Unplugged => 2,
                });
                out.extend_from_slice(&(installed.sources.len() as u64).to_le_bytes());
                for source in &installed.sources {
                    push_str(&mut out, source);
                }
                out.extend_from_slice(&(installed.agents.len() as u64).to_le_bytes());
                for agent in &installed.agents {
                    push_str(&mut out, agent);
                }
            }
        }
        out.extend_from_slice(&(self.notes.len() as u64).to_le_bytes());
        for (note_id, text) in &self.notes {
            push_str(&mut out, note_id);
            push_str(&mut out, text);
        }
        out.extend_from_slice(&(self.minted.len() as u64).to_le_bytes());
        for key in &self.minted {
            push_str(&mut out, key);
        }
        out
    }

    /// strict decode: only canonical encodings of execute-reachable states
    /// are accepted (sorted unique keys, valid phase, no trailing bytes).
    fn decode(bytes: &[u8]) -> Result<Store, Error> {
        let mut off = 0usize;
        let installed = match read_byte(bytes, &mut off)? {
            0 => None,
            1 => {
                let package = read_string(bytes, &mut off)?;
                if package.is_empty() {
                    return Err(Error::Module("snapshot package id is empty".into()));
                }
                let phase = match read_byte(bytes, &mut off)? {
                    0 => Phase::Active,
                    1 => Phase::Suspended,
                    2 => Phase::Unplugged,
                    _ => return Err(Error::Module("snapshot phase is invalid".into())),
                };
                let source_count = read_count(bytes, &mut off)?;
                let mut sources: Vec<String> = Vec::new();
                for _ in 0..source_count {
                    let source = read_string(bytes, &mut off)?;
                    if source.is_empty() || sources.last().is_some_and(|last| *last >= source) {
                        return Err(Error::Module(
                            "snapshot sources not strictly ascending".into(),
                        ));
                    }
                    sources.push(source);
                }
                let agent_count = read_count(bytes, &mut off)?;
                let mut agents: Vec<String> = Vec::new();
                for _ in 0..agent_count {
                    let agent = read_string(bytes, &mut off)?;
                    if agent.is_empty() || agents.contains(&agent) {
                        return Err(Error::Module("snapshot agents are invalid".into()));
                    }
                    agents.push(agent);
                }
                Some(Installed {
                    package,
                    phase,
                    sources,
                    agents,
                })
            }
            _ => return Err(Error::Module("snapshot installed tag is invalid".into())),
        };

        let note_count = read_count(bytes, &mut off)?;
        let mut notes: BTreeMap<String, String> = BTreeMap::new();
        for _ in 0..note_count {
            let note_id = read_string(bytes, &mut off)?;
            if note_id.is_empty()
                || notes
                    .last_key_value()
                    .is_some_and(|(last, _)| *last >= note_id)
            {
                return Err(Error::Module(
                    "snapshot notes not strictly ascending".into(),
                ));
            }
            let text = read_string(bytes, &mut off)?;
            notes.insert(note_id, text);
        }

        let minted_count = read_count(bytes, &mut off)?;
        let mut minted: BTreeSet<String> = BTreeSet::new();
        for _ in 0..minted_count {
            let key = read_string(bytes, &mut off)?;
            if key.is_empty() || minted.last().is_some_and(|last| *last >= key) {
                return Err(Error::Module(
                    "snapshot minted keys not strictly ascending".into(),
                ));
            }
            minted.insert(key);
        }

        if off != bytes.len() {
            return Err(Error::Module("snapshot has trailing bytes".into()));
        }
        Ok(Store {
            installed,
            notes,
            minted,
        })
    }
}

fn push_str(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(&(value.len() as u64).to_le_bytes());
    out.extend_from_slice(value.as_bytes());
}

fn read_byte(bytes: &[u8], off: &mut usize) -> Result<u8, Error> {
    let b = *bytes
        .get(*off)
        .ok_or_else(|| Error::Module("snapshot truncated".into()))?;
    *off += 1;
    Ok(b)
}

fn read_count(bytes: &[u8], off: &mut usize) -> Result<u64, Error> {
    let end = off
        .checked_add(8)
        .filter(|&end| end <= bytes.len())
        .ok_or_else(|| Error::Module("snapshot truncated".into()))?;
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&bytes[*off..end]);
    *off = end;
    let n = u64::from_le_bytes(buf);
    if n > (bytes.len() - *off) as u64 {
        return Err(Error::Module("snapshot truncated".into()));
    }
    Ok(n)
}

fn read_string(bytes: &[u8], off: &mut usize) -> Result<String, Error> {
    let len = read_count(bytes, off)? as usize;
    let value = std::str::from_utf8(&bytes[*off..*off + len])
        .map_err(|_| Error::Module("snapshot string is not utf-8".into()))?
        .to_owned();
    *off += len;
    Ok(value)
}

// ---- the module seam ------------------------------------------------------------

#[async_trait::async_trait(?Send)]
impl Module for DummyHarness {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn root(&self) -> StateRoot {
        Self::root_of(&self.committed)
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::SnapshotBytes(self.snapshot()))
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        let origin = ctx.env().origin.clone();

        // the harness contract — ONLY the package module's origin. a
        // HarnessMsg-shaped payload from anyone else falls through to the
        // rejection below: it is never acted on.
        if origin == Origin::Module(self.package.clone()) {
            return match decode_harness_msg(&msg.payload).map_err(Error::Module)? {
                HarnessMsg::InstallPackage { package, spec } => self.install(ctx, package, spec),
                HarnessMsg::SuspendPackage { package } => self.suspend(ctx, package),
                HarnessMsg::ResumePackage { package } => self.resume(ctx, package),
                HarnessMsg::UnplugPackage { package } => self.unplug(ctx, package),
            };
        }

        if let Origin::Module(emitter) = &origin {
            // engagement intake from a recorded source — NO-FAIL: this rides
            // the WRITER's block (a commenter must never be aborted by us).
            let recorded = self
                .store()
                .installed
                .as_ref()
                .is_some_and(|i| i.sources.contains(emitter));
            if recorded {
                match decode_page_event(&msg.payload) {
                    Ok(event) => self.on_page_event(ctx, event).await,
                    Err(e) => self.breadcrumb(ctx, format!("dropped undecodable page event: {e}")),
                }
                return Ok(());
            }
            // an accepted action's Apply, riding the delivery block — NO-FAIL.
            if let Ok(apply) = decode_action_msg(&msg.payload) {
                self.apply_action(ctx, &apply);
                return Ok(());
            }
        }

        Err(Error::Module(
            "dummy-harness accepts HarnessMsg from the package module, PageEvents from its \
             recorded sources, and package-action Applies from module origins"
                .into(),
        ))
    }

    /// external reads serve COMMITTED state only.
    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        let reply = match decode_query(req).map_err(Error::Module)? {
            DummyQuery::Notes => DummyReply::Notes(
                self.committed
                    .notes
                    .iter()
                    .map(|(note_id, text)| Note {
                        note_id: note_id.clone(),
                        text: text.clone(),
                    })
                    .collect(),
            ),
            DummyQuery::Status => {
                DummyReply::Status(self.committed.installed.as_ref().map(|i| DummyStatus {
                    package: i.package.clone(),
                    phase: i.phase.as_str().into(),
                    agents: i.agents.clone(),
                    minted: self.committed.minted.len() as u64,
                }))
            }
        };
        Ok(encode_reply(&reply))
    }

    /// the action owner's `Probe` rides the same query lane (the wire shapes
    /// are disjoint, so decode picks); the verdict reads STAGED-or-committed
    /// state — which is exactly why a probe can never see sibling actions
    /// from its own response: their applies have not staged yet.
    async fn query_with(&self, _ctx: &dyn Ctx, req: &[u8]) -> Result<Vec<u8>, Error> {
        if let Ok(PackageActionQuery::Probe { tag, payload, .. }) = decode_action_query(req) {
            let verdict = match self.validate_action(&tag, &payload) {
                Ok(_) => PackageActionReply::Accepted,
                Err(reason) => PackageActionReply::Rejected { reason },
            };
            return Ok(encode_action_reply(&verdict));
        }
        self.query(req).await
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

// ---- module-level tests --------------------------------------------------------
// the seams the end-to-end framework suite cannot reach through real modules:
// literal redelivery of one event (pages never redelivers), malformed events
// from a "source", and snapshot tamper rejection.

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;
    use package::{
        ActionRoute, AgentSeed, EngagementRule, MANIFEST_HASH_LEN, ModuleBinding, PromptSeed,
        UninstallPolicy, encode_action_msg, encode_action_query, encode_harness_msg,
    };

    const HARNESS: &str = "dummy-harness";
    const PKG: &str = "org.example.dummy";

    struct TestCtx {
        env: sdk::Env,
        emitted: Vec<Msg>,
        events: Vec<String>,
    }

    impl TestCtx {
        fn at(origin: Origin) -> Self {
            Self {
                env: sdk::Env {
                    protocol_version: 0,
                    height: 1,
                    consensus_time: 1,
                    origin,
                    me: HARNESS.into(),
                },
                emitted: Vec::new(),
                events: Vec::new(),
            }
        }
    }

    #[async_trait::async_trait(?Send)]
    impl Ctx for TestCtx {
        fn env(&self) -> &sdk::Env {
            &self.env
        }
        fn module_root(&self, _target: &str) -> Option<StateRoot> {
            Some(StateRoot::ZERO)
        }
        async fn query(&self, target: &str, _req: &[u8]) -> Result<Vec<u8>, Error> {
            // the jobs probe-before-emit: an empty board.
            if target == "jobs" {
                return Ok(jobs::encode_reply(&JobsReply::Job(None)));
            }
            Err(Error::UnknownModule(target.into()))
        }
        fn emit_msg(&mut self, msg: Msg) {
            self.emitted.push(msg);
        }
        fn emit_event(&mut self, ev: Event) {
            self.events
                .push(String::from_utf8_lossy(&ev.payload).into_owned());
        }
        fn request_effect(&mut self, _eff: sdk::Effect) {}
    }

    fn spec() -> InstallSpec {
        let content = "be a dummy";
        InstallSpec {
            package: PKG.into(),
            version: "0.1.0".into(),
            manifest_hash: vec![7u8; MANIFEST_HASH_LEN],
            modules: vec![
                ModuleBinding {
                    logical: "pages".into(),
                    module_id: "pages".into(),
                },
                ModuleBinding {
                    logical: "dummy-harness".into(),
                    module_id: HARNESS.into(),
                },
            ],
            harness: "dummy-harness".into(),
            prompts: vec![PromptSeed {
                logical: "p".into(),
                path: "/packages/org.example.dummy/prompts/dummy.md".into(),
                content: content.into(),
                sha256: Sha256::digest(content.as_bytes()).to_vec(),
            }],
            agents: vec![AgentSeed {
                agent_id: "dummy.note-taker".into(),
                display_name: "Dummy".into(),
                capability: "mock-llm-1".into(),
                prompt: "p".into(),
                actions: vec![ACTION_NOTE_ADD.into()],
                active: true,
            }],
            actions: vec![ActionRoute {
                tag: ACTION_NOTE_ADD.into(),
                owner: "dummy-harness".into(),
            }],
            engagements: vec![EngagementRule {
                source: "pages".into(),
                event: "comment_added".into(),
                agent: "dummy.note-taker".into(),
                policy: "mention".into(),
            }],
            uninstall: UninstallPolicy {
                pending_runs: "drain".into(),
                user_data: "preserve".into(),
            },
        }
    }

    fn module() -> DummyHarness {
        DummyHarness::new(HARNESS, "package", "agent", "jobs", "memory")
    }

    fn package_origin() -> Origin {
        Origin::Module("package".into())
    }

    fn exec(m: &mut DummyHarness, ctx: &mut TestCtx, payload: Vec<u8>) -> Result<(), Error> {
        block_on(m.execute(
            ctx,
            &Msg {
                target: HARNESS.into(),
                payload,
            },
        ))
    }

    fn commit(m: &mut DummyHarness) {
        block_on(m.commit_block()).unwrap();
    }

    fn installed(m: &mut DummyHarness) {
        let mut ctx = TestCtx::at(package_origin());
        exec(
            m,
            &mut ctx,
            encode_harness_msg(&HarnessMsg::InstallPackage {
                package: PKG.into(),
                spec: spec(),
            }),
        )
        .unwrap();
        commit(m);
    }

    fn comment_event(comment_id: &str, text: &str) -> Vec<u8> {
        pages::encode_page_event(&PageEvent::CommentAdded {
            page_id: "p1".into(),
            target: "p1".into(),
            thread_id: "t1".into(),
            comment_id: comment_id.into(),
            author: pages::AuthorRef::User(vec![7; 32]),
            text: text.into(),
        })
    }

    fn apply(tag: &str, payload: serde_json::Value) -> Vec<u8> {
        encode_action_msg(&PackageActionMsg::Apply {
            action_id: "a1".into(),
            tag: tag.into(),
            payload: serde_json::to_vec(&payload).unwrap(),
            run_context: b"{}".to_vec(),
        })
    }

    #[test]
    fn install_registers_hook_and_agents_then_acks_last() {
        let mut m = module();
        let mut ctx = TestCtx::at(package_origin());
        exec(
            &mut m,
            &mut ctx,
            encode_harness_msg(&HarnessMsg::InstallPackage {
                package: PKG.into(),
                spec: spec(),
            }),
        )
        .unwrap();

        assert_eq!(ctx.emitted.len(), 3);
        assert_eq!(ctx.emitted[0].target, "pages");
        assert_eq!(
            pages::decode_msg(&ctx.emitted[0].payload).unwrap(),
            PageMsg::RegisterHook {}
        );
        assert_eq!(ctx.emitted[1].target, "agent");
        match agent::decode_msg(&ctx.emitted[1].payload).unwrap() {
            AgentMsg::RegisterAgent {
                agent_id, prompt, ..
            } => {
                assert_eq!(agent_id, "dummy.note-taker");
                let prompt = prompt.expect("prompt pinned");
                assert_eq!(prompt.module, "memory");
                assert_eq!(
                    prompt.target,
                    "/packages/org.example.dummy/prompts/dummy.md@1"
                );
            }
            other => panic!("expected RegisterAgent, got {other:?}"),
        }
        assert_eq!(ctx.emitted[2].target, "package");
        assert_eq!(
            package::decode_msg(&ctx.emitted[2].payload).unwrap(),
            PackageMsg::MarkActive {
                package: PKG.into()
            }
        );

        // a second install rejects.
        commit(&mut m);
        let mut again = TestCtx::at(package_origin());
        assert!(
            exec(
                &mut m,
                &mut again,
                encode_harness_msg(&HarnessMsg::InstallPackage {
                    package: "org.example.other".into(),
                    spec: spec(),
                }),
            )
            .is_err()
        );
    }

    #[test]
    fn harness_msgs_from_non_package_origins_are_never_acted_on() {
        let mut m = module();
        let payload = encode_harness_msg(&HarnessMsg::InstallPackage {
            package: PKG.into(),
            spec: spec(),
        });
        for origin in [
            Origin::External(b"mallory".to_vec()),
            Origin::Module("pages".into()),
            Origin::System,
        ] {
            let mut ctx = TestCtx::at(origin.clone());
            assert!(
                exec(&mut m, &mut ctx, payload.clone()).is_err(),
                "{origin:?} must not drive the harness"
            );
            assert!(ctx.emitted.is_empty(), "{origin:?} must emit nothing");
        }
        commit(&mut m);
        assert_eq!(m.committed.installed, None, "no state landed");
    }

    #[test]
    fn a_mention_comment_mints_one_job_idempotently() {
        let mut m = module();
        installed(&mut m);

        let event = comment_event("c1", "hey @dummy.note-taker note this");
        let mut ctx = TestCtx::at(Origin::Module("pages".into()));
        exec(&mut m, &mut ctx, event.clone()).unwrap();
        assert_eq!(ctx.emitted.len(), 1);
        assert_eq!(ctx.emitted[0].target, "jobs");
        match jobs::decode_msg(&ctx.emitted[0].payload).unwrap() {
            JobsMsg::Submit { job_id, kind, .. } => {
                assert_eq!(job_id, "dummy:dummy.note-taker:c1");
                assert_eq!(kind, "agent/dummy.note-taker");
            }
            other => panic!("expected Submit, got {other:?}"),
        }
        commit(&mut m);

        // literal redelivery of the same event: nothing mints again.
        let mut again = TestCtx::at(Origin::Module("pages".into()));
        exec(&mut m, &mut again, event).unwrap();
        assert!(again.emitted.is_empty(), "redelivery must not re-mint");
        assert!(again.events.iter().any(|e| e.contains("already minted")));

        // a non-mention comment engages nothing.
        let mut quiet = TestCtx::at(Origin::Module("pages".into()));
        exec(&mut m, &mut quiet, comment_event("c2", "no robots here")).unwrap();
        assert!(quiet.emitted.is_empty());
    }

    #[test]
    fn a_malformed_page_event_from_a_source_is_a_no_op_observation() {
        let mut m = module();
        installed(&mut m);
        let root = m.root();
        let mut ctx = TestCtx::at(Origin::Module("pages".into()));
        exec(&mut m, &mut ctx, b"not a page event".to_vec())
            .expect("a malformed event must NOT abort the writer's block");
        assert!(ctx.emitted.is_empty());
        assert!(ctx.events.iter().any(|e| e.contains("undecodable")));
        commit(&mut m);
        assert_eq!(m.root(), root, "nothing staged");
    }

    #[test]
    fn suspend_pauses_and_stops_minting_and_unplug_tombstones_preserving_notes() {
        let mut m = module();
        installed(&mut m);

        // a note lands (user data).
        let mut ctx = TestCtx::at(Origin::Module("runs".into()));
        exec(
            &mut m,
            &mut ctx,
            apply(
                ACTION_NOTE_ADD,
                serde_json::json!({"note_id": "n1", "text": "kept"}),
            ),
        )
        .unwrap();
        commit(&mut m);

        // suspend: agents pause, minting stops.
        let mut ctx = TestCtx::at(package_origin());
        exec(
            &mut m,
            &mut ctx,
            encode_harness_msg(&HarnessMsg::SuspendPackage {
                package: PKG.into(),
            }),
        )
        .unwrap();
        assert_eq!(ctx.emitted.len(), 1);
        assert!(matches!(
            agent::decode_msg(&ctx.emitted[0].payload).unwrap(),
            AgentMsg::PauseAgent { .. }
        ));
        commit(&mut m);
        let mut quiet = TestCtx::at(Origin::Module("pages".into()));
        exec(
            &mut m,
            &mut quiet,
            comment_event("c9", "@dummy.note-taker anyone?"),
        )
        .unwrap();
        assert!(
            quiet.emitted.is_empty(),
            "a suspended package mints nothing"
        );

        // unplug (from suspended): tombstones + unregisters, notes preserved.
        let mut ctx = TestCtx::at(package_origin());
        exec(
            &mut m,
            &mut ctx,
            encode_harness_msg(&HarnessMsg::UnplugPackage {
                package: PKG.into(),
            }),
        )
        .unwrap();
        assert_eq!(ctx.emitted.len(), 2);
        assert!(matches!(
            agent::decode_msg(&ctx.emitted[0].payload).unwrap(),
            AgentMsg::TombstoneAgent { .. }
        ));
        assert_eq!(
            pages::decode_msg(&ctx.emitted[1].payload).unwrap(),
            PageMsg::UnregisterHook {}
        );
        commit(&mut m);
        assert_eq!(
            m.committed.notes.get("n1").map(String::as_str),
            Some("kept"),
            "user data preserved"
        );

        // and no further lifecycle op is accepted.
        let mut again = TestCtx::at(package_origin());
        assert!(
            exec(
                &mut m,
                &mut again,
                encode_harness_msg(&HarnessMsg::ResumePackage {
                    package: PKG.into(),
                }),
            )
            .is_err()
        );
    }

    #[test]
    fn apply_is_no_fail_and_probe_shares_its_validation() {
        let mut m = module();
        installed(&mut m);

        // probe: accept a fresh create, reject set_text on a missing note.
        let probe = |m: &DummyHarness, tag: &str, payload: serde_json::Value| {
            let ctx = TestCtx::at(Origin::Module("runs".into()));
            let req = encode_action_query(&PackageActionQuery::Probe {
                action_id: "a1".into(),
                tag: tag.into(),
                payload: serde_json::to_vec(&payload).unwrap(),
                run_context: b"{}".to_vec(),
            });
            package::decode_action_reply(&block_on(m.query_with(&ctx, &req)).unwrap()).unwrap()
        };
        assert_eq!(
            probe(
                &m,
                ACTION_NOTE_ADD,
                serde_json::json!({"note_id": "n1", "text": "x"})
            ),
            PackageActionReply::Accepted
        );
        assert!(matches!(
            probe(
                &m,
                ACTION_NOTE_SET_TEXT,
                serde_json::json!({"note_id": "n1", "text": "x"})
            ),
            PackageActionReply::Rejected { .. }
        ));

        // apply the create, then a LATE-CONFLICT duplicate: breadcrumb, Ok.
        let mut ctx = TestCtx::at(Origin::Module("runs".into()));
        exec(
            &mut m,
            &mut ctx,
            apply(
                ACTION_NOTE_ADD,
                serde_json::json!({"note_id": "n1", "text": "x"}),
            ),
        )
        .unwrap();
        let mut dup = TestCtx::at(Origin::Module("runs".into()));
        exec(
            &mut m,
            &mut dup,
            apply(
                ACTION_NOTE_ADD,
                serde_json::json!({"note_id": "n1", "text": "y"}),
            ),
        )
        .expect("a late conflict must not abort the delivery block");
        assert!(dup.events.iter().any(|e| e.contains("already exists")));

        // malformed payload: breadcrumb, Ok, nothing staged.
        let mut bad = TestCtx::at(Origin::Module("runs".into()));
        exec(
            &mut m,
            &mut bad,
            apply(ACTION_NOTE_ADD, serde_json::json!({"bogus": true})),
        )
        .expect("a malformed apply must not abort the delivery block");
        assert!(bad.events.iter().any(|e| e.contains("dropped")));

        commit(&mut m);
        assert_eq!(m.committed.notes.get("n1").map(String::as_str), Some("x"));
    }

    #[test]
    fn snapshot_round_trips_and_rejects_tampered_bytes() {
        let mut m = module();
        installed(&mut m);
        let mut ctx = TestCtx::at(Origin::Module("runs".into()));
        exec(
            &mut m,
            &mut ctx,
            apply(
                ACTION_NOTE_ADD,
                serde_json::json!({"note_id": "n1", "text": "x"}),
            ),
        )
        .unwrap();
        commit(&mut m);

        let root = m.root();
        let bytes = m.snapshot();
        assert_eq!(
            StateRoot(Sha256::digest(&bytes).into()),
            root,
            "the snapshot is the exact root preimage"
        );

        let mut fresh = module();
        fresh.install_snapshot(&bytes, root).expect("install");
        assert_eq!(fresh.root(), root);
        assert_eq!(fresh.committed, m.committed);

        // tampered bytes reject against the honest root.
        let mut tampered = bytes.clone();
        let last = tampered.len() - 1;
        tampered[last] ^= 0xff;
        assert!(module().install_snapshot(&tampered, root).is_err());
        // honest bytes against a wrong root reject too.
        assert!(module().install_snapshot(&bytes, StateRoot::ZERO).is_err());
    }
}
