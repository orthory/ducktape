//! the state-sync container — self-contained snapshot bytes for the whole
//! forge state (multi-branch repos + tracker), and the verify-then-mutate
//! install that replaces the namespace under a root gate.
//!
//! container (baseline layout):
//!
//! ```text
//! u32-LE(repo_count)
//! per BORN repo, sorted by name:            # born == at least one branch
//!   u32-LE(name_len) name
//!   u32-LE(ref_count)
//!   per branch, sorted by short name:
//!     u32-LE(branch_len) branch  [20-byte head oid]
//!   u32-LE(pack_len) pack                   # ONE pack: closure of ALL heads
//! u32-LE(tracker_len) tracker-canonical-bytes
//! ```
//!
//! the v2 layout (the upgrade demonstrator) prefixes the identical body with
//! the `FGv2` magic and gates against the domain-separated v2 root.

use std::collections::BTreeMap;

use git2::Oid;
use sdk::{Error, StateRoot};

use crate::codec::{self, Reader};
use crate::refs::{full_ref, norm_branch, open_or_init_repo, RepoState};
use crate::tracker::Tracker;
use crate::{
    compose_state_root, compose_state_root_v2, forge_layout, norm_repo, Forge, ForgeLayout,
    FORGE_V2_SNAPSHOT_MAGIC,
};
use crate::git;

impl Forge {
    /// serialize the COMMITTED state into self-contained snapshot bytes (see
    /// the container layout above). only born branches are carried — they are
    /// exactly what contributes to `root()` — plus the tracker's canonical
    /// bytes. staged (this-block) state is deliberately excluded.
    pub fn snapshot(&self) -> Result<Vec<u8>, Error> {
        // SEAM (dual-path snapshot wire): the selected layout picks the
        // container format; `active_version` selects it (a snapshot has no
        // `Ctx`) and is NEVER serialized.
        match forge_layout(self.active_version) {
            ForgeLayout::MultiRepo => self.snapshot_body(),
            ForgeLayout::MultiRepoV2 => {
                let mut out = FORGE_V2_SNAPSHOT_MAGIC.to_vec();
                out.extend_from_slice(&self.snapshot_body()?);
                Ok(out)
            }
        }
    }

    fn snapshot_body(&self) -> Result<Vec<u8>, Error> {
        let born: Vec<(&str, &BTreeMap<String, Oid>)> = self
            .repos
            .iter()
            .filter(|(_, s)| !s.refs.is_empty())
            .map(|(n, s)| (n.as_str(), &s.refs))
            .collect();

        let mut out = Vec::new();
        codec::put_u32(&mut out, born.len() as u32);
        for (name, refs) in born {
            // every born head's objects live in the repo's odb (Commit built
            // them there, or materialize installed the pack); a still-pending
            // branch fails pack_closure_many here — a node can only SERVE
            // state it holds, same as phase 1.
            let repo = open_or_init_repo(&self.base, name)?;
            let heads: Vec<Oid> = refs.values().copied().collect();
            let pack =
                git::pack_closure_many(&repo, &heads).map_err(|e| Error::Module(e.to_string()))?;
            codec::put_str(&mut out, name);
            codec::put_u32(&mut out, refs.len() as u32);
            for (branch, oid) in refs {
                codec::put_str(&mut out, branch);
                out.extend_from_slice(oid.as_bytes());
            }
            codec::put_bytes(&mut out, &pack);
        }
        codec::put_bytes(&mut out, &self.tracker.canonical_bytes());
        Ok(out)
    }

    /// replace this module's WHOLE state with snapshot bytes, gated on
    /// `expected`. the bytes are UNTRUSTED (a byzantine peer produced them);
    /// the order is verify-then-mutate:
    ///
    /// 1. PARSE the entire container with a bounds-checked reader — no write.
    /// 2. ROOT GATE: the composed root of the parsed branches + tracker must
    ///    equal `expected` before any byte reaches an odb.
    /// 3. INSTALL each repo's pack (libgit2 re-hashes every object) and
    ///    require EVERY head's full closure — still moving no ref.
    /// 4. PUBLISH: full replacement — unbind every on-disk branch the snapshot
    ///    drops, move every snapshot branch, rebuild the map, swap the tracker
    ///    in and persist it.
    ///
    /// on any `Err` before step 4 the committed refs and tracker — and so
    /// `root()` — are byte-identical to before the call.
    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        match forge_layout(self.active_version) {
            ForgeLayout::MultiRepo => self.install_body(bytes, expected, ForgeLayout::MultiRepo),
            ForgeLayout::MultiRepoV2 => {
                let body = bytes
                    .strip_prefix(FORGE_V2_SNAPSHOT_MAGIC.as_slice())
                    .ok_or_else(|| {
                        Error::Module(
                            "forge snapshot: expected a v2 container (missing FGv2 magic)".into(),
                        )
                    })?;
                self.install_body(body, expected, ForgeLayout::MultiRepoV2)
            }
        }
    }

    fn install_body(
        &mut self,
        bytes: &[u8],
        expected: StateRoot,
        layout: ForgeLayout,
    ) -> Result<(), Error> {
        // ---- PHASE 1: parse (no writes) -------------------------------------
        let mut r = Reader::new(bytes);
        let count = r.u32()?;
        let mut parsed: BTreeMap<String, (BTreeMap<String, Oid>, &[u8])> = BTreeMap::new();
        for _ in 0..count {
            let name = norm_repo(&r.str_()?)?;
            let ref_count = r.u32()?;
            if ref_count == 0 {
                return Err(Error::Module(format!(
                    "forge snapshot: repo {name} carries no branches \
                     (unborn repos are not serialized)"
                )));
            }
            let mut refs = BTreeMap::new();
            for _ in 0..ref_count {
                let branch = r.str_()?;
                norm_branch(&branch)?;
                let oid = Oid::from_bytes(r.take(git::OID_RAW_LEN)?)
                    .map_err(|e| Error::Module(e.to_string()))?;
                if oid.is_zero() {
                    return Err(Error::Module(format!(
                        "forge snapshot: branch {branch} of {name} carries a zero oid"
                    )));
                }
                if refs.insert(branch, oid).is_some() {
                    return Err(Error::Module(format!(
                        "forge snapshot: duplicate branch in repo {name}"
                    )));
                }
            }
            let pack_len = r.u32()? as usize;
            let pack = r.take(pack_len)?;
            if parsed.insert(name.clone(), (refs, pack)).is_some() {
                return Err(Error::Module(format!(
                    "forge snapshot: duplicate repo {name}"
                )));
            }
        }
        let tracker_len = r.u32()? as usize;
        let tracker = Tracker::decode(r.take(tracker_len)?)?;
        if !r.done() {
            return Err(Error::Module(
                "forge snapshot: trailing bytes after the container".into(),
            ));
        }

        // ---- PHASE 2: root gate BEFORE any byte reaches an odb --------------
        let entries = parsed.iter().map(|(n, (refs, _))| (n.as_str(), refs));
        let composed = match layout {
            ForgeLayout::MultiRepo => compose_state_root(entries, &tracker),
            ForgeLayout::MultiRepoV2 => compose_state_root_v2(entries, &tracker),
        };
        if composed != expected {
            return Err(Error::Module(
                "snapshot root mismatch: composed state does not rehash to the expected root"
                    .into(),
            ));
        }

        // ---- PHASE 3: index packs + require closures, moving NO ref ---------
        for (name, (refs, pack)) in &parsed {
            let repo = open_or_init_repo(&self.base, name)?;
            git::install_pack(&repo, pack).map_err(|e| Error::Module(e.to_string()))?;
            for oid in refs.values() {
                git::verify_closure(&repo, *oid).map_err(|e| Error::Module(e.to_string()))?;
            }
        }

        // ---- PHASE 4: publish (full replacement) ----------------------------
        // unbind every currently-committed branch the snapshot drops (durably,
        // so a restart re-adopt can't resurrect it) — dropped repos AND dropped
        // branches of surviving repos.
        for (name, state) in &self.repos {
            if state.refs.is_empty() {
                continue;
            }
            let keep = parsed.get(name).map(|(refs, _)| refs);
            let repo = open_or_init_repo(&self.base, name)?;
            for branch in state.refs.keys() {
                if keep.is_none_or(|refs| !refs.contains_key(branch)) {
                    git::delete_ref(&repo, &full_ref(branch))
                        .map_err(|e| Error::Module(e.to_string()))?;
                }
            }
        }

        let mut new_repos = BTreeMap::new();
        for (name, (refs, _)) in parsed {
            let repo = open_or_init_repo(&self.base, &name)?;
            for (branch, oid) in &refs {
                git::update_ref(&repo, &full_ref(branch), *oid)
                    .map_err(|e| Error::Module(e.to_string()))?;
            }
            new_repos.insert(name, RepoState::with_refs(refs));
        }
        self.repos = new_repos;
        self.tracker = tracker;
        self.staged_tracker = None;
        self.persist_tracker()?;
        Ok(())
    }
}
