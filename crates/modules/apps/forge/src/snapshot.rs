//! the state-sync container — self-contained snapshot bytes for the whole
//! forge state (multi-branch repos + tracker), and the verify-then-mutate
//! install that replaces the namespace under a root gate.
//!
//! container:
//!
//! ```text
//! "FGv1"                                    # 4-byte magic
//! u32-LE(repo_count)
//! per BORN repo, sorted by name:            # born == at least one branch
//!   u32-LE(name_len) name
//!   u32-LE(ref_count)
//!   per branch, sorted by short name:
//!     u32-LE(branch_len) branch  [20-byte head oid]
//!   u32-LE(pending_count)                   # branches whose objects are absent
//!   per pending branch, sorted:
//!     u32-LE(branch_len) branch  [20-byte head oid] [32-byte pack digest]
//!   u32-LE(pack_len) pack                   # closure of the NON-pending heads
//! u32-LE(tracker_len) tracker-canonical-bytes
//! ```
//!
//! the pending section is what makes the container TOTAL. a committed head
//! whose objects this node does not hold is a state forge models on purpose —
//! the fork-safety invariant is that pack possession is per-node and the root
//! is not — so it must be serializable. carrying the catch-up map beside the
//! pack lands the receiver in exactly the sender's node-local state, still
//! retrying materialize, instead of stranding it on a head it can never
//! explain. the pending set is node-local, so two honest nodes at the same
//! root legitimately produce different container bytes; nothing compares them
//! (statesync verifies the ROOT, which covers refs + tracker only).

use std::collections::BTreeMap;

use git2::Oid;
use sdk::{Error, StateRoot};

use crate::codec::{self, Reader};
use crate::refs::{full_ref, norm_branch, open_or_init_repo, RepoState};
use crate::tracker::Tracker;
use crate::{compose_state_root, norm_repo, Forge, FORGE_SNAPSHOT_MAGIC};
use crate::git;

impl Forge {
    /// serialize the COMMITTED state into self-contained snapshot bytes (see
    /// the container layout above). only born branches are carried — they are
    /// exactly what contributes to `root()` — plus the tracker's canonical
    /// bytes. staged (this-block) state is deliberately excluded.
    pub fn snapshot(&self) -> Result<Vec<u8>, Error> {
        // PACKING IS THE EXPENSIVE PART AND THE CALLER IS A CLOCK. The node's
        // checkpoint calls this every `checkpoint_blocks` blocks on its select
        // loop; see [`SnapshotCache`] for the measurement and what it starved.
        let root = <Self as sdk::Module>::root(self);
        let pending = self.pending_key();
        let cached = self
            .snapshot_cache
            .borrow()
            .as_ref()
            .filter(|hit| hit.root == root && hit.pending == pending)
            .map(|hit| hit.bytes.clone());
        if let Some(bytes) = cached {
            return Ok(bytes);
        }

        let mut out = FORGE_SNAPSHOT_MAGIC.to_vec();
        let born: Vec<(&str, &RepoState)> = self
            .repos
            .iter()
            .filter(|(_, s)| !s.refs.is_empty())
            .map(|(n, s)| (n.as_str(), s))
            .collect();

        codec::put_u32(&mut out, born.len() as u32);
        for (name, state) in born {
            // pack the closure of the heads this node actually holds objects
            // for. a PENDING branch is by definition one whose objects never
            // arrived, so it contributes nothing to the pack and travels in
            // the pending section instead — packing it would fail, and that
            // failure used to abort the whole host capture (killing this
            // node's checkpointing and its ability to admit joiners) over
            // state forge is designed to tolerate.
            let repo = open_or_init_repo(&self.base, name)?;
            let heads: Vec<Oid> = state
                .refs
                .iter()
                .filter(|(branch, _)| !state.pending().contains_key(*branch))
                .map(|(_, oid)| *oid)
                .collect();
            let pack =
                git::pack_closure_many(&repo, &heads).map_err(|e| Error::Module(e.to_string()))?;
            codec::put_str(&mut out, name);
            codec::put_u32(&mut out, state.refs.len() as u32);
            for (branch, oid) in &state.refs {
                codec::put_str(&mut out, branch);
                out.extend_from_slice(oid.as_bytes());
            }
            crate::refs::put_pending(&mut out, state.pending());
            codec::put_bytes(&mut out, &pack);
        }
        codec::put_bytes(&mut out, &self.tracker.canonical_bytes());
        *self.snapshot_cache.borrow_mut() = Some(crate::SnapshotCache {
            root,
            pending,
            bytes: out.clone(),
        });
        Ok(out)
    }

    /// the node-local half of [`SnapshotCache`]'s key: which branches are still
    /// waiting on their objects, per repo. `root()` cannot carry this — the
    /// pending set is deliberately not consensus state — but it does change the
    /// container, so it has to be compared beside the root.
    fn pending_key(&self) -> Vec<(String, crate::refs::PendingMap)> {
        self.repos
            .iter()
            .map(|(name, state)| (name.clone(), state.pending().clone()))
            .collect()
    }

    /// replace this module's WHOLE state with snapshot bytes, gated on
    /// `expected`. the bytes are UNTRUSTED (a byzantine peer produced them);
    /// the order is verify-then-mutate:
    ///
    /// 1. PARSE the entire container with a bounds-checked reader — no write.
    /// 2. ROOT GATE: the composed root of the parsed branches + tracker must
    ///    equal `expected` before any byte reaches an odb.
    /// 3. INSTALL each repo's pack (libgit2 re-hashes every object) and
    ///    require the full closure of every head the pack CLAIMS to cover —
    ///    i.e. every non-pending branch — still moving no ref.
    /// 4. PUBLISH: full replacement — unbind every on-disk branch the snapshot
    ///    drops, move every branch whose objects arrived, rebuild the map with
    ///    the pending branches re-adopted as catch-up targets, swap the tracker
    ///    in and persist both.
    ///
    /// a PENDING branch is committed state (it is in the root gate of step 2)
    /// whose objects the sender did not hold either. its ref is deliberately
    /// left unmoved and its digest recorded, so this node retries materialize
    /// exactly as the sender does — never trusted content, just a target.
    ///
    /// on any `Err` before step 4 the committed refs and tracker — and so
    /// `root()` — are byte-identical to before the call.
    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        let body = bytes
            .strip_prefix(FORGE_SNAPSHOT_MAGIC.as_slice())
            .ok_or_else(|| {
                Error::Module("forge snapshot: missing the FGv1 container magic".into())
            })?;
        // ---- PHASE 1: parse (no writes) -------------------------------------
        let mut r = Reader::new(body);
        let count = r.u32()?;
        let mut parsed: BTreeMap<String, ParsedRepo> = BTreeMap::new();
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
            let pending = crate::refs::take_pending(&mut r)?;
            // a catch-up target for a branch this snapshot does not commit, or
            // for a different head than it commits, would leave the receiver
            // materializing toward state no root ever gated.
            for (branch, (head, _)) in &pending {
                if refs.get(branch) != Some(head) {
                    return Err(Error::Module(format!(
                        "forge snapshot: pending branch {branch} of {name} does not \
                         match the committed head"
                    )));
                }
            }
            let pack_len = r.u32()? as usize;
            let pack = r.take(pack_len)?;
            let repo = ParsedRepo {
                refs,
                pending,
                pack,
            };
            if parsed.insert(name.clone(), repo).is_some() {
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
        let entries = parsed.iter().map(|(n, repo)| (n.as_str(), &repo.refs));
        let composed = compose_state_root(entries, &tracker);
        if composed != expected {
            return Err(Error::Module(
                "snapshot root mismatch: composed state does not rehash to the expected root"
                    .into(),
            ));
        }

        // ---- PHASE 3: index packs + require closures, moving NO ref ---------
        for (name, parsed_repo) in &parsed {
            let repo = open_or_init_repo(&self.base, name)?;
            git::install_pack(&repo, parsed_repo.pack).map_err(|e| Error::Module(e.to_string()))?;
            for (branch, oid) in &parsed_repo.refs {
                if parsed_repo.pending.contains_key(branch) {
                    continue;
                }
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
            let keep = parsed.get(name).map(|repo| &repo.refs);
            let repo = open_or_init_repo(&self.base, name)?;
            for branch in state.refs.keys() {
                if keep.is_none_or(|refs| !refs.contains_key(branch)) {
                    git::delete_ref(&repo, &full_ref(branch))
                        .map_err(|e| Error::Module(e.to_string()))?;
                }
            }
        }

        let mut new_repos = BTreeMap::new();
        for (name, parsed_repo) in parsed {
            let repo = open_or_init_repo(&self.base, &name)?;
            for (branch, oid) in &parsed_repo.refs {
                // a pending branch's objects are absent: moving its ref would
                // dangle. `materialize` moves it once the pack arrives.
                if parsed_repo.pending.contains_key(branch) {
                    continue;
                }
                git::update_ref(&repo, &full_ref(branch), *oid)
                    .map_err(|e| Error::Module(e.to_string()))?;
            }
            let mut state = RepoState::with_refs(parsed_repo.refs);
            state.adopt_pending(parsed_repo.pending);
            new_repos.insert(name, state);
        }
        self.repos = new_repos;
        self.tracker = tracker;
        self.staged_tracker = None;
        self.persist_tracker()?;
        self.persist_pending()?;
        Ok(())
    }
}

/// one repo's parsed container section, before any byte reaches an odb.
struct ParsedRepo<'a> {
    refs: BTreeMap<String, Oid>,
    pending: crate::refs::PendingMap,
    pack: &'a [u8],
}
