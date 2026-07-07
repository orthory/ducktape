//! restart recovery — the durable half of the ordered lane.
//!
//! a validator's app state is split across substrates with very different
//! persistence: qmdb modules and forge commit to disk per block, the rest are
//! in-memory canonical-bytes, and the consensus engine journals votes but not
//! payloads. this crate makes the COMPOSED state recoverable: a write-ahead
//! op journal (every finalized frame's bytes land on disk before they mutate
//! state), a periodic checkpoint of the in-memory modules, and a boot-time
//! replay that rolls every module forward from its own position to the
//! journal tip.
//!
//! ## the position model — root equality, not op counters
//!
//! every sealed block records the FULL module-root vector after it settled
//! ([`node::BlockSeal`]). on boot each module's live root is compared against
//! those seals: a module whose root equals a seal's entry has that block (and
//! everything before it) applied. this works because the disk substrates'
//! roots never repeat — a qmdb root is an op-log commitment (every commit
//! moves it) and a git head oid commits to its parent chain — while the
//! in-memory modules' roots are pure state commitments restored exactly at
//! the checkpoint. so per-block skip/apply decisions reduce to comparing the
//! CHANGED modules (seal roots vs the previous seal's) against the live host.
//!
//! ## crash windows
//!
//! - crash between a frame's WAL record and its seal: at most one unsealed
//!   [`Record::Block`] can exist at the journal tip (each pre-apply record is
//!   synced, which also makes every earlier append durable). boot ROLLS it
//!   FORWARD: if the live roots still equal the pre-block vector the frame
//!   re-applies; if they moved, the apply completed before the crash and the
//!   block is sealed from the observed roots.
//! - a TORN SEALED block: a block whose commit spans substrates with different
//!   durability — a qmdb store commits to disk PER BLOCK, while the in-memory
//!   cohort only persists at the periodic checkpoint. a crash (or a hard kill)
//!   after the disk commit but before the next checkpoint leaves the disk
//!   substrate at the block's POST root and the in-memory cohort restored to
//!   its PRE root from the checkpoint. this is no longer "all-or-nothing":
//!   recovery replays such a block by re-running the sealed frame and
//!   committing ONLY the still-at-pre modules (a pure state re-commitment,
//!   deterministic and idempotent) while ABORTING the already-durable ones —
//!   re-committing a qmdb store would MOVE its op-log root and fork us. the
//!   per-module root compare (live vs sealed pre/post) decides the partition;
//!   a changed module at NEITHER pre nor post is genuine damage and still
//!   fail-stops as [`Error::Torn`], and the recomposed-vs-sealed app-hash
//!   check is the final backstop.
//! - a block that touches several DISK substrates could in principle crash
//!   BETWEEN their commits — the classic multi-store atomicity limit; today no
//!   block commits to more than one disk substrate (ops target one module; the
//!   only cross-module dispatch, governance -> valset, stays in the in-memory
//!   cohort). recovery refuses this EXPLICITLY: only a per-block-durable disk
//!   substrate can be at its post-root after a crash (the in-memory cohort
//!   always rolls back to the checkpoint), so two-or-more changed modules at
//!   post means two-or-more disk substrates committed — a changed set spanning
//!   >1 disk substrate at mixed roots. selective replay's premise (only the
//!   in-memory cohort rolled back) no longer holds and its single-frame
//!   re-execution could read a partially-committed world, so boot fail-stops
//!   with [`Error::Torn`] at the point it detects this rather than healing and
//!   leaning on the after-the-fact per-module verify. (a residual case — two
//!   disk substrates where exactly ONE committed, so only one is at post — is
//!   indistinguishable by root alone from the healthy single-disk torn block;
//!   selective replay reconciles it and the per-module post-root verify is the
//!   backstop that fail-stops rather than forks if the re-execution diverges.)
//!   the exact fix if a block ever legitimately commits >1 disk substrate: a
//!   per-commit height cursor in qmdb's commit metadata slot.
//! - crash between the engine journaling a finalization and the drain: the
//!   frame's bytes are already durable here — locally-submitted frames are
//!   pinned at submit time ([`Record::Pinned`]), before the engine can ever
//!   propose their digest. boot seeds the consensus content store from these
//!   records so the re-reported finalization resolves and applies.

use std::collections::{BTreeMap, BTreeSet};
use std::num::{NonZeroU16, NonZeroU64, NonZeroUsize};

use commonware_codec::RangeCfg;
use commonware_runtime::BufferPooler;
use commonware_runtime::buffer::paged::CacheRef;
use commonware_storage::journal::contiguous::{Reader as _, variable};
use commonware_storage::metadata;
use commonware_utils::sequence::U64;
use futures::{StreamExt as _, pin_mut};

use host::{BlockContext, DispatchRecord, Host, SubmitError};
use node::{BlockSeal, BlockSink, Disposition, decode_frame};
use sdk::{ModuleId, StateRoot, UpgradeCoords};

/// runtime bounds every store here needs (same alias the storage crate uses:
/// `Storage + Clock + Metrics`).
pub use commonware_storage::Context;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// a storage-layer fault (journal/metadata io). fail-stop territory.
    #[error("recovery storage: {0}")]
    Storage(String),
    /// a record or manifest that does not decode — the journal is damaged.
    #[error("recovery journal corrupt: {0}")]
    Corrupt(String),
    /// the on-disk substrates disagree with the journal in a way replay cannot
    /// reconcile (a module neither at a block's pre- nor post-roots).
    #[error("recovery state torn: {0}")]
    Torn(String),
    /// replay reached the tip but the recomposed state does not match what was
    /// sealed — the recovered node would fork, so it must not start.
    #[error("recovery verification failed: {0}")]
    Verify(String),
    /// the caller asked for records below the retained checkpoint/journal
    /// suffix boundary. a statesync joiner must refetch a fresher manifest.
    #[error("recovery journal range pruned after {after_height}; retained from {retained_start}")]
    RangePruned {
        after_height: u64,
        retained_start: u64,
    },
}

impl From<Error> for node::Error {
    fn from(e: Error) -> Self {
        node::Error::Journal(e.to_string())
    }
}

// ============================================================================
// wire — hand-rolled little-endian records (statesync-wire discipline:
// length-prefixed, bounds-checked, no partial reads).
// ============================================================================

const MAX_LEN: usize = 1 << 21; // 2 MiB: > the p2p frame cap + framing.

fn put_u64(out: &mut Vec<u8>, v: u64) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_bytes(out: &mut Vec<u8>, b: &[u8]) {
    put_u64(out, b.len() as u64);
    out.extend_from_slice(b);
}

fn put_root(out: &mut Vec<u8>, r: &StateRoot) {
    out.extend_from_slice(&r.0);
}

struct Cursor<'a> {
    buf: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, at: 0 }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self
            .at
            .checked_add(n)
            .filter(|e| *e <= self.buf.len())
            .ok_or_else(|| Error::Corrupt("record truncated".into()))?;
        let s = &self.buf[self.at..end];
        self.at = end;
        Ok(s)
    }

    fn u64(&mut self) -> Result<u64, Error> {
        let b = self.take(8)?;
        Ok(u64::from_le_bytes(b.try_into().expect("8 bytes")))
    }

    fn u32(&mut self) -> Result<u32, Error> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes(b.try_into().expect("4 bytes")))
    }

    /// true once every byte has been consumed — the tolerant-tail probe: an
    /// old-format checkpoint ends here, a new-format one still has its version
    /// fields to read.
    fn at_end(&self) -> bool {
        self.at == self.buf.len()
    }

    fn bytes(&mut self) -> Result<Vec<u8>, Error> {
        let len = self.u64()? as usize;
        if len > MAX_LEN {
            return Err(Error::Corrupt(format!(
                "length {len} exceeds the record cap"
            )));
        }
        Ok(self.take(len)?.to_vec())
    }

    fn root(&mut self) -> Result<StateRoot, Error> {
        let b = self.take(32)?;
        Ok(StateRoot(b.try_into().expect("32 bytes")))
    }

    fn done(&self) -> Result<(), Error> {
        if self.at == self.buf.len() {
            Ok(())
        } else {
            Err(Error::Corrupt("trailing bytes after record".into()))
        }
    }
}

fn put_roots(out: &mut Vec<u8>, roots: &[(ModuleId, StateRoot)]) {
    put_u64(out, roots.len() as u64);
    for (id, root) in roots {
        put_bytes(out, id.as_bytes());
        put_root(out, root);
    }
}

fn get_roots(c: &mut Cursor) -> Result<Vec<(ModuleId, StateRoot)>, Error> {
    let n = c.u64()? as usize;
    if n > 4096 {
        return Err(Error::Corrupt(format!(
            "{n} module roots exceeds sanity cap"
        )));
    }
    let mut roots = Vec::with_capacity(n);
    for _ in 0..n {
        let id = String::from_utf8(c.bytes()?)
            .map_err(|_| Error::Corrupt("module id is not utf-8".into()))?;
        roots.push((id, c.root()?));
    }
    Ok(roots)
}

fn put_keys(out: &mut Vec<u8>, keys: &[Vec<u8>]) {
    put_u64(out, keys.len() as u64);
    for k in keys {
        put_bytes(out, k);
    }
}

fn get_keys(c: &mut Cursor) -> Result<Vec<Vec<u8>>, Error> {
    let n = c.u64()? as usize;
    if n > 4096 {
        return Err(Error::Corrupt(format!(
            "{n} participant keys exceeds sanity cap"
        )));
    }
    let mut keys = Vec::with_capacity(n);
    for _ in 0..n {
        keys.push(c.bytes()?);
    }
    Ok(keys)
}

// ============================================================================
// records — the op journal's entries.
// ============================================================================

const TAG_PINNED: u8 = 1;
const TAG_BLOCK: u8 = 2;
const TAG_SEAL: u8 = 3;
const TAG_CUTOVER: u8 = 4;

const DISP_APPLIED: u8 = 0;
const DISP_REJECTED: u8 = 1;

/// one op-journal entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Record {
    /// a locally-submitted frame's bytes, durable BEFORE the consensus engine
    /// may propose their digest (closes the finalized-before-drained window).
    Pinned { frame: Vec<u8> },
    /// WAL: a finalized frame about to be applied at `height`.
    Block { height: u64, frame: Vec<u8> },
    /// a settled block: how it landed plus the post-block replay positions.
    Seal {
        height: u64,
        disposition: Disposition,
        roots: Vec<(ModuleId, StateRoot)>,
        app_hash: StateRoot,
    },
    /// an epoch cutover: the new epoch, its app-height base, and the engine
    /// participant set it was spawned over. the set rides the record so a
    /// restart in the window between the cutover and the next checkpoint
    /// respawns the engine with the EPOCH'S set, never the instantaneous
    /// valset projection (which may already stage the next change).
    Cutover {
        epoch: u64,
        view_base: u64,
        participants: Vec<Vec<u8>>,
        /// the epoch's RESIDENT set (transport standing, no quorum seat) —
        /// empty on records written before the staged-admission tier.
        residents: Vec<Vec<u8>>,
    },
}

impl Record {
    fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        match self {
            Record::Pinned { frame } => {
                out.push(TAG_PINNED);
                put_bytes(&mut out, frame);
            }
            Record::Block { height, frame } => {
                out.push(TAG_BLOCK);
                put_u64(&mut out, *height);
                put_bytes(&mut out, frame);
            }
            Record::Seal {
                height,
                disposition,
                roots,
                app_hash,
            } => {
                out.push(TAG_SEAL);
                put_u64(&mut out, *height);
                // discarded frames are never journaled — the tag is two-valued.
                out.push(match disposition {
                    Disposition::Applied => DISP_APPLIED,
                    Disposition::Rejected => DISP_REJECTED,
                    Disposition::Discarded => unreachable!("discarded frames are not sealed"),
                });
                put_roots(&mut out, roots);
                put_root(&mut out, app_hash);
            }
            Record::Cutover {
                epoch,
                view_base,
                participants,
                residents,
            } => {
                out.push(TAG_CUTOVER);
                put_u64(&mut out, *epoch);
                put_u64(&mut out, *view_base);
                put_keys(&mut out, participants);
                put_keys(&mut out, residents);
            }
        }
        out
    }

    fn decode(bytes: &[u8]) -> Result<Self, Error> {
        let mut c = Cursor::new(bytes);
        let tag = c.take(1)?[0];
        let record = match tag {
            TAG_PINNED => Record::Pinned { frame: c.bytes()? },
            TAG_BLOCK => Record::Block {
                height: c.u64()?,
                frame: c.bytes()?,
            },
            TAG_SEAL => {
                let height = c.u64()?;
                let disposition = match c.take(1)?[0] {
                    DISP_APPLIED => Disposition::Applied,
                    DISP_REJECTED => Disposition::Rejected,
                    d => return Err(Error::Corrupt(format!("unknown disposition {d}"))),
                };
                let roots = get_roots(&mut c)?;
                let app_hash = c.root()?;
                Record::Seal {
                    height,
                    disposition,
                    roots,
                    app_hash,
                }
            }
            TAG_CUTOVER => {
                let epoch = c.u64()?;
                let view_base = c.u64()?;
                let participants = get_keys(&mut c)?;
                // ADDITIVE tail: a record written before the resident tier
                // ends here — map the missing set to empty, like the
                // manifest's version tail.
                let residents = if c.at_end() { Vec::new() } else { get_keys(&mut c)? };
                Record::Cutover {
                    epoch,
                    view_base,
                    participants,
                    residents,
                }
            }
            t => return Err(Error::Corrupt(format!("unknown record tag {t}"))),
        };
        c.done()?;
        Ok(record)
    }
}

// ============================================================================
// the checkpoint manifest.
// ============================================================================

/// the periodic checkpoint: everything a boot needs BESIDES the disk
/// substrates (which recover themselves) and the journal suffix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifest {
    /// the sealed height this checkpoint captures; `None` = genesis, nothing
    /// applied yet. an explicit option — heights start at the engine's first
    /// view, and some orderers stamp that as 0, so 0 cannot double as a
    /// sentinel.
    pub height: Option<u64>,
    /// the consensus epoch whose engine was live at `height`.
    pub epoch: u64,
    /// that epoch's app-height base (`app_height = view_base + engine view`).
    pub view_base: u64,
    /// the epoch's ENGINE PARTICIPANT SET (raw public-key bytes) — what the
    /// live engine was spawned over. a restart respawns with exactly this
    /// set: the checkpointed valset snapshot may already hold a membership
    /// change whose cutover had not happened yet.
    pub participants: Vec<Vec<u8>>,
    /// the epoch's RESIDENT set (transport standing, no quorum seat) — same
    /// epoch-scoped discipline as `participants`. empty on checkpoints
    /// written before the staged-admission tier (tolerant decode).
    pub residents: Vec<Vec<u8>>,
    /// an epoch cutover armed but not yet crossed at checkpoint time (the
    /// ordered lane's discard-ceiling view). a restart re-arms the same
    /// deterministic boundary its peers are converging on.
    pub pending_cutover_view: Option<u64>,
    /// the composed app-hash at `height`.
    pub app_hash: StateRoot,
    /// every module's root at `height` — the replay baseline.
    pub roots: Vec<(ModuleId, StateRoot)>,
    /// canonical snapshot bytes for the modules that do NOT persist
    /// themselves (the in-memory cohort), keyed by module id. the caller
    /// decides the set; this crate stores bytes.
    pub snapshots: Vec<(ModuleId, Vec<u8>)>,
    /// the op-journal position at which this manifest was written; everything
    /// below the PREVIOUS manifest's position is prunable once the persisted
    /// finalization floor has passed its height.
    pub oplog_pos: u64,
    /// the node's next local submit sequence at checkpoint time — restored so
    /// a restarted node advances past every sequence it may already have
    /// framed (the exactly-once digest gate does not survive the process, so
    /// a reused (origin, seq, payload) triple would re-apply).
    pub next_seq: u64,
    /// the agreed protocol version active at this boundary. an UNAUTHENTICATED
    /// preflight hint — the authority stays the replayed upgrade-module state,
    /// confirmed by the final app-hash compose. defaults to 0 on an on-disk
    /// checkpoint written before this field existed (tolerant decode).
    pub current_version: u32,
    /// the single upgrade armed but not yet activated at checkpoint time, if
    /// any — the version analogue of `pending_cutover_view`.
    pub pending_upgrade: Option<UpgradeCoords>,
    /// the highest protocol version any block at or after this boundary needs
    /// (`pending.to_version` once `height >= pending.activation_height`, else
    /// `current_version`). the boot preflight compares the local build's
    /// `MAX_PROTOCOL_VERSION` against this and refuses EARLY when it is lower.
    pub required_min_version: u32,
}

impl Manifest {
    fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        match self.height {
            Some(h) => {
                out.push(1);
                put_u64(&mut out, h);
            }
            None => out.push(0),
        }
        put_u64(&mut out, self.epoch);
        put_u64(&mut out, self.view_base);
        put_keys(&mut out, &self.participants);
        match self.pending_cutover_view {
            Some(v) => {
                out.push(1);
                put_u64(&mut out, v);
            }
            None => out.push(0),
        }
        put_root(&mut out, &self.app_hash);
        put_roots(&mut out, &self.roots);
        put_u64(&mut out, self.snapshots.len() as u64);
        for (id, bytes) in &self.snapshots {
            put_bytes(&mut out, id.as_bytes());
            put_bytes(&mut out, bytes);
        }
        put_u64(&mut out, self.oplog_pos);
        put_u64(&mut out, self.next_seq);
        // --- ADDITIVE version tail (see decode's tolerant read) ---
        put_u32(&mut out, self.current_version);
        match &self.pending_upgrade {
            Some(u) => {
                out.push(1);
                put_bytes(&mut out, u.name.as_bytes());
                put_u64(&mut out, u.activation_height);
                put_u32(&mut out, u.to_version);
            }
            None => out.push(0),
        }
        put_u32(&mut out, self.required_min_version);
        // --- ADDITIVE resident tail (after the version tail, same tolerant
        // discipline) ---
        put_keys(&mut out, &self.residents);
        out
    }

    fn decode(bytes: &[u8]) -> Result<Self, Error> {
        let mut c = Cursor::new(bytes);
        let height = match c.take(1)?[0] {
            0 => None,
            1 => Some(c.u64()?),
            t => return Err(Error::Corrupt(format!("bad height tag {t}"))),
        };
        let epoch = c.u64()?;
        let view_base = c.u64()?;
        let participants = get_keys(&mut c)?;
        let pending_cutover_view = match c.take(1)?[0] {
            0 => None,
            1 => Some(c.u64()?),
            t => return Err(Error::Corrupt(format!("bad pending-cutover tag {t}"))),
        };
        let app_hash = c.root()?;
        let roots = get_roots(&mut c)?;
        let n = c.u64()? as usize;
        if n > 4096 {
            return Err(Error::Corrupt(format!("{n} snapshots exceeds sanity cap")));
        }
        let mut snapshots = Vec::with_capacity(n);
        for _ in 0..n {
            let id = String::from_utf8(c.bytes()?)
                .map_err(|_| Error::Corrupt("module id is not utf-8".into()))?;
            snapshots.push((id, c.bytes()?));
        }
        let oplog_pos = c.u64()?;
        let next_seq = c.u64()?;
        // ADDITIVE version tail. a checkpoint written by an older binary ends
        // here; map the missing tail to baseline defaults rather than tripping
        // the no-trailing-bytes check, so a new binary boots an old on-disk
        // checkpoint. a present-but-malformed tail still fails loud.
        let (current_version, pending_upgrade, required_min_version) = if c.at_end() {
            (0, None, 0)
        } else {
            let current_version = c.u32()?;
            let pending_upgrade = match c.take(1)?[0] {
                0 => None,
                1 => {
                    let name = String::from_utf8(c.bytes()?)
                        .map_err(|_| Error::Corrupt("upgrade name is not utf-8".into()))?;
                    let activation_height = c.u64()?;
                    let to_version = c.u32()?;
                    Some(UpgradeCoords {
                        name,
                        activation_height,
                        to_version,
                    })
                }
                t => return Err(Error::Corrupt(format!("bad pending-upgrade tag {t}"))),
            };
            let required_min_version = c.u32()?;
            (current_version, pending_upgrade, required_min_version)
        };
        // ADDITIVE resident tail — absent on checkpoints written before the
        // staged-admission tier.
        let residents = if c.at_end() { Vec::new() } else { get_keys(&mut c)? };
        c.done()?;
        Ok(Self {
            height,
            epoch,
            view_base,
            participants,
            residents,
            pending_cutover_view,
            app_hash,
            roots,
            snapshots,
            oplog_pos,
            next_seq,
            current_version,
            pending_upgrade,
            required_min_version,
        })
    }

    /// look up a stored snapshot by module id.
    pub fn snapshot(&self, id: &str) -> Option<&[u8]> {
        self.snapshots
            .iter()
            .find(|(m, _)| m == id)
            .map(|(_, b)| b.as_slice())
    }

    /// look up a module's checkpointed root.
    pub fn root(&self, id: &str) -> Option<StateRoot> {
        self.roots.iter().find(|(m, _)| m == id).map(|(_, r)| *r)
    }

    /// build a checkpoint manifest from a live host at a settled boundary.
    /// reuses the statesync surface: every module that reports
    /// [`sdk::StateSyncHandle::SnapshotBytes`] gets its bytes stored (the
    /// in-memory cohort); disk-backed modules recover themselves and
    /// contribute only their root. a local checkpoint IS a statesync capture
    /// of your past self.
    #[allow(clippy::too_many_arguments)]
    pub fn capture(
        host: &Host,
        height: Option<u64>,
        epoch: u64,
        view_base: u64,
        participants: Vec<Vec<u8>>,
        residents: Vec<Vec<u8>>,
        pending_cutover_view: Option<u64>,
        current_version: u32,
        pending_upgrade: Option<UpgradeCoords>,
        oplog_pos: u64,
        next_seq: u64,
    ) -> Result<Self, Error> {
        let snapshot = host
            .capture_finalized_snapshot(host::FinalizedBlock {
                // the capture only verifies the app-hash; a genesis manifest
                // has no boundary yet and 0 is a placeholder, not a height.
                height: height.unwrap_or(0),
                app_hash: host.app_hash(),
            })
            .map_err(|e| Error::Storage(format!("checkpoint capture: {e}")))?;
        let roots = snapshot
            .modules
            .iter()
            .map(|m| (m.id.clone(), m.root))
            .collect();
        let snapshots = snapshot
            .modules
            .into_iter()
            .filter_map(|m| match m.state_sync {
                sdk::StateSyncHandle::SnapshotBytes(bytes) => Some((m.id, bytes)),
                _ => None,
            })
            .collect();
        let required_min_version = sdk::required_min_version(
            current_version,
            pending_upgrade.as_ref(),
            // genesis has no boundary yet; 0 is a placeholder height there.
            height.unwrap_or(0),
        );
        Ok(Self {
            height,
            epoch,
            view_base,
            participants,
            residents,
            pending_cutover_view,
            app_hash: host.app_hash(),
            roots,
            snapshots,
            oplog_pos,
            next_seq,
            current_version,
            pending_upgrade,
            required_min_version,
        })
    }

    /// this boundary's required minimum protocol version — the fence the boot
    /// preflight checks the local build against.
    pub fn required_min_version(&self) -> u32 {
        self.required_min_version
    }

    /// boot preflight: fail loud when the local build's `max_supported`
    /// protocol version is below this boundary's `required_min_version`,
    /// turning an opaque post-replay app-hash mismatch into an early,
    /// actionable "height needs binary vX" refusal. NOT yet wired into the
    /// live resume path (that invocation is a later phase).
    pub fn preflight(&self, max_supported: u32) -> Result<(), sdk::UnsupportedVersion> {
        sdk::check_required_version(self.required_min_version, max_supported)
    }
}

/// the persisted finalization floor: the newest certificate whose view the
/// app had FULLY drained when it was recorded — safe to respawn the engine on
/// (`Floor::Finalized`) because nothing at or below it is missing from state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FloorCert {
    pub epoch: u64,
    /// the app height of the certificate's view (`view_base + view`).
    pub height: u64,
    /// the scheme-encoded finalization certificate.
    pub cert: Vec<u8>,
}

impl FloorCert {
    fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        put_u64(&mut out, self.epoch);
        put_u64(&mut out, self.height);
        put_bytes(&mut out, &self.cert);
        out
    }

    fn decode(bytes: &[u8]) -> Result<Self, Error> {
        let mut c = Cursor::new(bytes);
        let out = Self {
            epoch: c.u64()?,
            height: c.u64()?,
            cert: c.bytes()?,
        };
        c.done()?;
        Ok(out)
    }
}

// ============================================================================
// the store — op journal + manifest + floor cert, all under storage_dir.
// ============================================================================

const PARTITION_OPLOG: &str = "recovery-oplog";
const PARTITION_MANIFEST: &str = "recovery-manifest";
const PARTITION_CERT: &str = "recovery-cert";
const KEY: u64 = 0;

type OpJournal<E> = variable::Journal<E, Vec<u8>>;
type Meta<E> = metadata::Metadata<E, U64, Vec<u8>>;

/// the durable recovery store. implements [`node::BlockSink`] (the ordered
/// lane journals through it live) and drives boot-time replay.
pub struct Recovery<E>
where
    E: Context + BufferPooler + commonware_runtime::Supervisor,
{
    journal: OpJournal<E>,
    manifest_store: Meta<E>,
    cert_store: Meta<E>,
}

fn storage_err(e: impl std::fmt::Display) -> Error {
    Error::Storage(e.to_string())
}

/// one paired recovery-journal block and seal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JournalFrame {
    pub height: u64,
    pub frame: Vec<u8>,
    pub disposition: Disposition,
    pub roots: Vec<(ModuleId, StateRoot)>,
    pub app_hash: StateRoot,
}

impl<E> Recovery<E>
where
    E: Context + BufferPooler + commonware_runtime::Supervisor,
{
    /// open (or create) the recovery store under `context`'s storage root.
    pub async fn open(context: E) -> Result<Self, Error> {
        let page_cache = CacheRef::from_pooler(
            &context,
            NonZeroU16::new(128).expect("nonzero"),
            NonZeroUsize::new(64).expect("nonzero"),
        );
        let journal = OpJournal::<E>::init(
            context.child("oplog"),
            variable::Config {
                partition: PARTITION_OPLOG.into(),
                items_per_section: NonZeroU64::new(64).expect("nonzero"),
                write_buffer: NonZeroUsize::new(1024).expect("nonzero"),
                compression: None,
                codec_config: (RangeCfg::from(0..=MAX_LEN), ()),
                page_cache,
            },
        )
        .await
        .map_err(storage_err)?;
        let manifest_store = Meta::<E>::init(
            context.child("manifest"),
            metadata::Config {
                partition: PARTITION_MANIFEST.into(),
                codec_config: (RangeCfg::from(0..=usize::MAX), ()),
            },
        )
        .await
        .map_err(storage_err)?;
        let cert_store = Meta::<E>::init(
            context.child("cert"),
            metadata::Config {
                partition: PARTITION_CERT.into(),
                codec_config: (RangeCfg::from(0..=MAX_LEN), ()),
            },
        )
        .await
        .map_err(storage_err)?;
        Ok(Self {
            journal,
            manifest_store,
            cert_store,
        })
    }

    /// the persisted checkpoint, if any. `None` means this storage dir has
    /// never run with recovery — a fresh genesis boot.
    pub fn manifest(&self) -> Result<Option<Manifest>, Error> {
        self.manifest_store
            .get(&U64::new(KEY))
            .map(|b| Manifest::decode(b))
            .transpose()
    }

    /// true when the op journal holds any records (fresh-boot guard: a
    /// journal without a manifest is damaged state, not a fresh dir).
    pub async fn journal_is_empty(&self) -> bool {
        self.journal.size().await == 0
    }

    /// atomically persist a checkpoint manifest. syncs the op journal first
    /// so the manifest can never be newer than the journal it summarizes.
    pub async fn write_manifest(&mut self, manifest: &Manifest) -> Result<(), Error> {
        self.journal.sync().await.map_err(storage_err)?;
        self.manifest_store
            .put_sync(U64::new(KEY), manifest.encode())
            .await
            .map_err(storage_err)
    }

    /// the current op-journal append position (recorded into manifests).
    pub async fn oplog_pos(&self) -> u64 {
        self.journal.size().await
    }

    /// force every buffered journal append durable — the graceful-shutdown
    /// barrier (a crash instead simply leaves the tail to roll forward).
    pub async fn sync(&mut self) -> Result<(), Error> {
        self.journal.sync().await.map_err(storage_err)
    }

    /// the persisted finalization floor, if any.
    pub fn floor_cert(&self) -> Result<Option<FloorCert>, Error> {
        self.cert_store
            .get(&U64::new(KEY))
            .map(|b| FloorCert::decode(b))
            .transpose()
    }

    /// persist the finalization floor (see [`FloorCert`]). callers only write
    /// this when the ordered lane has fully drained everything at or below
    /// the certificate's view.
    pub async fn write_floor_cert(&mut self, cert: &FloorCert) -> Result<(), Error> {
        self.cert_store
            .put_sync(U64::new(KEY), cert.encode())
            .await
            .map_err(storage_err)
    }

    /// prune op-journal records below `pos` (a PREVIOUS manifest's
    /// `oplog_pos`). callers gate this on the persisted floor cert having
    /// passed that manifest's height — pruned frames must never be needed to
    /// resolve a re-reported finalization.
    pub async fn prune_oplog(&mut self, pos: u64) -> Result<(), Error> {
        self.journal
            .prune(pos)
            .await
            .map(|_| ())
            .map_err(storage_err)
    }

    /// read sealed recovery frames in `(after_height, up_to_height]`.
    ///
    /// This is the durable equivalent of a restart's replay suffix. The local
    /// checkpoint height is the retained suffix boundary: asking below it is a
    /// pruned-range condition, even if old journal bytes have not yet been
    /// physically removed.
    pub async fn read_finalized_frames(
        &self,
        after_height: u64,
        up_to_height: u64,
    ) -> Result<Vec<JournalFrame>, Error> {
        if after_height > up_to_height {
            return Err(Error::Corrupt(format!(
                "invalid frame range ({after_height}, {up_to_height}]"
            )));
        }
        if let Some(retained_start) = self.manifest()?.and_then(|m| m.height) {
            if after_height < retained_start {
                return Err(Error::RangePruned {
                    after_height,
                    retained_start,
                });
            }
        }
        if after_height == up_to_height {
            return Ok(Vec::new());
        }

        let mut records: Vec<Record> = Vec::new();
        {
            let reader = self.journal.reader().await;
            let bounds = reader.bounds();
            let stream = reader
                .replay(NonZeroUsize::new(1 << 16).expect("nonzero"), bounds.start)
                .await
                .map_err(storage_err)?;
            pin_mut!(stream);
            while let Some(item) = stream.next().await {
                let (_pos, bytes) = item.map_err(storage_err)?;
                records.push(Record::decode(&bytes)?);
            }
        }

        let mut out = Vec::new();
        let mut pending: Option<(u64, Vec<u8>)> = None;
        for record in records {
            match record {
                Record::Block { height, frame } => {
                    if height <= after_height {
                        continue;
                    }
                    if height > up_to_height {
                        break;
                    }
                    if let Some((prev, _)) = pending {
                        return Err(Error::Corrupt(format!(
                            "block {height} appeared before block {prev} was sealed"
                        )));
                    }
                    pending = Some((height, frame));
                }
                Record::Seal {
                    height,
                    disposition,
                    roots,
                    app_hash,
                } => {
                    if height <= after_height {
                        continue;
                    }
                    if height > up_to_height {
                        break;
                    }
                    let Some((block_height, frame)) = pending.take() else {
                        return Err(Error::Corrupt(format!(
                            "seal at height {height} without its block record"
                        )));
                    };
                    if block_height != height {
                        return Err(Error::Corrupt(format!(
                            "seal height {height} does not match block {block_height}"
                        )));
                    }
                    out.push(JournalFrame {
                        height,
                        frame,
                        disposition,
                        roots,
                        app_hash,
                    });
                }
                Record::Pinned { .. } | Record::Cutover { .. } => {}
            }
        }
        if let Some((height, _)) = pending {
            return Err(Error::Corrupt(format!(
                "block {height} in requested range is missing its seal"
            )));
        }
        Ok(out)
    }
}

// the live sink: append (and where required, sync) records as the ordered
// lane drives it. sync policy: `pin` and `pre_apply` are barriers (their
// records must be durable before the engine/host may act); `seal` is a plain
// append — the NEXT pre-apply's sync makes it durable before another block
// can apply, and a lost trailing seal is exactly the roll-forward case.
impl<E> BlockSink for Recovery<E>
where
    E: Context + BufferPooler + commonware_runtime::Supervisor,
{
    fn pin(&mut self, frame: &[u8]) -> impl std::future::Future<Output = Result<(), node::Error>> {
        let record = Record::Pinned {
            frame: frame.to_vec(),
        }
        .encode();
        async move {
            self.journal.append(&record).await.map_err(storage_err)?;
            self.journal.sync().await.map_err(storage_err)?;
            Ok(())
        }
    }

    fn pre_apply(
        &mut self,
        height: u64,
        frame: &[u8],
    ) -> impl std::future::Future<Output = Result<(), node::Error>> {
        let record = Record::Block {
            height,
            frame: frame.to_vec(),
        }
        .encode();
        async move {
            self.journal.append(&record).await.map_err(storage_err)?;
            self.journal.sync().await.map_err(storage_err)?;
            Ok(())
        }
    }

    fn seal(
        &mut self,
        seal: &BlockSeal,
    ) -> impl std::future::Future<Output = Result<(), node::Error>> {
        let record = Record::Seal {
            height: seal.height,
            disposition: seal.disposition,
            roots: seal.roots.clone(),
            app_hash: seal.app_hash,
        }
        .encode();
        async move {
            self.journal.append(&record).await.map_err(storage_err)?;
            Ok(())
        }
    }

    fn cutover(
        &mut self,
        epoch: u64,
        view_base: u64,
        participants: &[Vec<u8>],
        residents: &[Vec<u8>],
    ) -> impl std::future::Future<Output = Result<(), node::Error>> {
        let record = Record::Cutover {
            epoch,
            view_base,
            participants: participants.to_vec(),
            residents: residents.to_vec(),
        }
        .encode();
        async move {
            self.journal.append(&record).await.map_err(storage_err)?;
            self.journal.sync().await.map_err(storage_err)?;
            Ok(())
        }
    }
}

// ============================================================================
// boot-time replay.
// ============================================================================

/// one re-executed sealed block, as replay hands it to a [`ReplaySink`]: the
/// SEALED frame bytes, how the block landed, the composed app-hash it left
/// behind (the seal's recorded value), and the deterministic dispatch trace
/// (empty for a rejected block). the frame + disposition + app-hash ride
/// along because a node-layer observer (the explorer's blocks database)
/// derives its row from the frame's content, not the dispatch trace — the
/// trace alone cannot reproduce it.
pub struct FoldedBlock<'a> {
    pub height: u64,
    pub frame: &'a [u8],
    pub disposition: Disposition,
    pub app_hash: StateRoot,
    pub dispatches: &'a [DispatchRecord],
}

/// observer of every sealed block the journal replay walks, in height order —
/// the seam a derived tier (the per-module read-model index) folds from, so a
/// restart re-derives exactly what the live drain would have fed it.
///
/// two shapes, because replay cannot always reproduce a block's content:
/// - a RE-EXECUTED block surfaces its sealed frame and the deterministic
///   dispatch trace consensus applied ([`ReplaySink::folded_block`]; the
///   trace is empty for a rejected block);
/// - a block replay SKIPS — its state already durable, or root-idempotent —
///   has no reproducible trace ([`ReplaySink::opaque_block`]). the observer
///   decides what an unreproducible height means for its tier (the index
///   stops folding and lets its from-state rebuild repair the gap).
///
/// observation is best-effort by design: sink calls return nothing and MUST
/// not fail — recovery's own verification never depends on them.
pub trait ReplaySink {
    fn folded_block(&mut self, block: &FoldedBlock<'_>);
    fn opaque_block(&mut self, height: u64);
}

/// what a completed recovery hands back to the boot path.
#[derive(Debug)]
pub struct Recovered {
    /// the journal tip: the last sealed height (`None` = nothing applied) and
    /// the verified composed app-hash there.
    pub height: Option<u64>,
    pub app_hash: StateRoot,
    /// the consensus epoch to respawn and its app-height base.
    pub epoch: u64,
    pub view_base: u64,
    /// the epoch's engine participant set: the manifest's, superseded by any
    /// newer [`Record::Cutover`] the journal retained.
    pub participants: Vec<Vec<u8>>,
    /// the epoch's resident set, recovered with the same precedence.
    pub residents: Vec<Vec<u8>>,
    /// every retained frame's bytes (pins and blocks) — the boot path seeds
    /// the consensus content store with these so re-reported finalizations
    /// resolve locally instead of wedging the ordered gate.
    pub frames: Vec<Vec<u8>>,
    /// every post-checkpoint sealed block's `(height, post-block roots)`, in
    /// order — the boot path scans these to re-derive a cutover that was
    /// armed by a block ABOVE the checkpoint (the checkpoint itself records
    /// one armed at or below it via `pending_cutover_view`).
    pub blocks: Vec<(u64, Vec<(ModuleId, StateRoot)>)>,
    /// replay accounting, for the boot log line.
    pub applied: usize,
    pub skipped: usize,
    pub rolled_forward: bool,
}

impl<E> Recovery<E>
where
    E: Context + BufferPooler + commonware_runtime::Supervisor,
{
    /// roll `host` forward from the checkpoint to the journal tip.
    ///
    /// the caller has already installed `manifest`'s snapshots into the
    /// in-memory modules and reopened the disk substrates at their own
    /// positions; this replays the journal suffix, skipping blocks a module
    /// already contains (root equality) and re-applying the rest, verifying
    /// each re-applied block against its sealed roots and the final state
    /// against the tip's sealed app-hash.
    pub async fn recover(
        &mut self,
        host: &mut Host,
        manifest: &Manifest,
    ) -> Result<Recovered, Error> {
        self.recover_with_sink(host, manifest, None).await
    }

    /// [`Recovery::recover`], additionally reporting every sealed block the
    /// replay walks to `sink` (see [`ReplaySink`]). recovery's own behavior
    /// and verification are identical with or without an observer.
    pub async fn recover_with_sink(
        &mut self,
        host: &mut Host,
        manifest: &Manifest,
        mut sink: Option<&mut dyn ReplaySink>,
    ) -> Result<Recovered, Error> {
        let mut expected: BTreeMap<ModuleId, StateRoot> = manifest.roots.iter().cloned().collect();
        let mut tip_height: Option<u64> = manifest.height;
        let mut tip_hash = manifest.app_hash;
        let mut epoch = manifest.epoch;
        let mut view_base = manifest.view_base;
        let mut participants = manifest.participants.clone();
        let mut residents = manifest.residents.clone();
        let mut frames: Vec<Vec<u8>> = Vec::new();
        let mut blocks: Vec<(u64, Vec<(ModuleId, StateRoot)>)> = Vec::new();
        let mut pending: Option<(u64, Vec<u8>)> = None;
        let mut applied = 0usize;
        let mut skipped = 0usize;

        // decode the retained journal into records first: replay borrows the
        // reader, and applying blocks needs `&mut host` with no borrow held.
        let mut records: Vec<Record> = Vec::new();
        {
            let reader = self.journal.reader().await;
            let bounds = reader.bounds();
            let stream = reader
                .replay(NonZeroUsize::new(1 << 16).expect("nonzero"), bounds.start)
                .await
                .map_err(storage_err)?;
            pin_mut!(stream);
            while let Some(item) = stream.next().await {
                let (_pos, bytes) = item.map_err(storage_err)?;
                records.push(Record::decode(&bytes)?);
            }
        }

        for record in records {
            match record {
                Record::Pinned { frame } => frames.push(frame),
                Record::Cutover {
                    epoch: e,
                    view_base: b,
                    participants: p,
                    residents: o,
                } => {
                    // monotone: a stale record retained from below the
                    // checkpoint must not regress the manifest's values.
                    if e > epoch {
                        epoch = e;
                        view_base = b;
                        participants = p;
                        residents = o;
                    }
                }
                Record::Block { height, frame } => {
                    if manifest.height.is_some_and(|h| height <= h) {
                        continue; // pre-checkpoint remnant, not yet pruned.
                    }
                    if let Some((h, _)) = pending {
                        return Err(Error::Corrupt(format!(
                            "two unsealed blocks ({h} then {height}) — the WAL never leaves \
                             more than one apply in flight"
                        )));
                    }
                    frames.push(frame.clone());
                    pending = Some((height, frame));
                }
                Record::Seal {
                    height,
                    disposition,
                    roots,
                    app_hash,
                } => {
                    if manifest.height.is_some_and(|h| height <= h) {
                        continue;
                    }
                    let Some((block_height, frame)) = pending.take() else {
                        return Err(Error::Corrupt(format!(
                            "seal at height {height} without its block record"
                        )));
                    };
                    if block_height != height {
                        return Err(Error::Corrupt(format!(
                            "seal height {height} does not match its block record {block_height}"
                        )));
                    }
                    // VERSION-AWARE REPLAY (fork-critical): restore the boundary
                    // version for THIS height BEFORE any `root()` read below — both
                    // the at_pre/at_post branch decision AND the post-apply
                    // verification branch a dual-path module's `root()` on
                    // `active_version`, so it must equal what the live node held at
                    // `height` or replay recomputes a different root across H.
                    // derived the SAME pure way `Host::effective_version` does, from
                    // the replayed committed upgrade state.
                    // the block's OWN effective version — the value the live node
                    // dispatched and sealed under. this is what `apply_block` stamps
                    // and what the post-apply verification (and the at_post
                    // already-applied check) must read `root()` under.
                    let protocol_version = host.effective_version(height).await;
                    // the modules this block CHANGED: the replay unit. a pure
                    // comparison of RECORDED root values — version-independent.
                    let changed: Vec<(ModuleId, StateRoot)> = roots
                        .iter()
                        .filter(|(id, root)| expected.get(id) != Some(root))
                        .cloned()
                        .collect();
                    if changed.is_empty() {
                        // a rejected or root-idempotent block: nothing durable
                        // to redo (re-applying could MOVE a history-committed
                        // root and fork us).
                        skipped += 1;
                        if let Some(sink) = sink.as_deref_mut() {
                            match disposition {
                                // a rejected block never had content anywhere.
                                Disposition::Rejected => sink.folded_block(&FoldedBlock {
                                    height,
                                    frame: &frame,
                                    disposition,
                                    app_hash,
                                    dispatches: &[],
                                }),
                                // an applied block whose ops moved no root:
                                // its trace existed at runtime but is not
                                // re-executed here — unreproducible.
                                _ => sink.opaque_block(height),
                            }
                        }
                    } else {
                        // the PRE-apply state was sealed under the PREVIOUS height's
                        // effective version — it differs from this block's version
                        // ONLY at an activation boundary H, where a dual-path
                        // `root()` switches algorithm. check "still at the pre-block
                        // roots?" under it; check "already at the sealed post-roots?"
                        // (a disk substrate that applied H before the crash) under
                        // this block's version. at the boundary the two checks need
                        // DIFFERENT selectors — evaluate each under its own.
                        let pre_version =
                            host.effective_version(height.saturating_sub(1)).await;
                        host.set_active_version(pre_version);
                        let at_pre = changed
                            .iter()
                            .all(|(id, _)| host.module_root(id) == expected.get(id).copied());
                        host.set_active_version(protocol_version);
                        let at_post = changed
                            .iter()
                            .all(|(id, root)| host.module_root(id) == Some(*root));
                        if at_post {
                            skipped += 1; // a disk substrate already holds it.
                            if let Some(sink) = sink.as_deref_mut() {
                                sink.opaque_block(height);
                            }
                        } else if at_pre {
                            let (_, dispatches) =
                                apply_block(host, height, &frame, protocol_version, Some(disposition))
                                    .await?;
                            if let Some(sink) = sink.as_deref_mut() {
                                sink.folded_block(&FoldedBlock {
                                    height,
                                    frame: &frame,
                                    disposition,
                                    app_hash,
                                    dispatches: &dispatches,
                                });
                            }
                            for (id, root) in &changed {
                                let live = host.module_root(id);
                                if live != Some(*root) {
                                    return Err(Error::Verify(format!(
                                        "replayed block {height} left module {id} at \
                                         {live:?}, sealed root was {root:?}"
                                    )));
                                }
                            }
                            applied += 1;
                        } else {
                            // a TORN block: some changed modules are at their
                            // pre-root (rolled back to the checkpoint — the
                            // in-memory cohort), others already at their sealed
                            // post-root (per-block-durable disk substrates that
                            // committed before the crash). re-run the sealed
                            // frame but commit ONLY the still-at-pre cohort and
                            // ABORT the already-durable ones — re-committing a
                            // qmdb store would move its op-log root and fork us.
                            let mut commit_only: BTreeSet<ModuleId> = BTreeSet::new();
                            let mut already_post = 0usize;
                            for (id, root) in &changed {
                                // classify each changed module under the SAME split
                                // version selectors the bulk at_pre/at_post checks
                                // above use: "still at pre" is read under the
                                // PREVIOUS height's version, "already at post" under
                                // THIS block's. a dual-path module's root() switches
                                // format at an activation boundary, so a single
                                // selector would misclassify a rolled-back in-memory
                                // module as neither-pre-nor-post (false Torn brick).
                                host.set_active_version(pre_version);
                                let at_pre_root =
                                    host.module_root(id) == expected.get(id).copied();
                                host.set_active_version(protocol_version);
                                let at_post_root = host.module_root(id) == Some(*root);
                                if at_pre_root {
                                    commit_only.insert(id.clone());
                                } else if at_post_root {
                                    already_post += 1;
                                }
                            }
                            // re-execute and verify under this block's version,
                            // matching the at_pre replay path above.
                            host.set_active_version(protocol_version);
                            // MULTI-DISK atomicity limit — fail-stop EXPLICITLY.
                            // only a per-block-durable disk substrate can be at
                            // its POST root after a crash (the in-memory cohort
                            // always rolls back to the checkpoint pre-root), so
                            // `already_post` counts exactly the disk substrates
                            // that committed before the crash. two or more of
                            // them is a changed set spanning >1 disk substrate at
                            // mixed roots: the classic multi-store atomicity zone
                            // whose sealed state may not be internally consistent
                            // and whose single-frame re-execution can read a
                            // partially-committed world. selective replay's
                            // assumption (only the in-memory cohort was rolled
                            // back) no longer holds, so we refuse it up front
                            // rather than heal-and-hope the per-module verify
                            // catches a divergence. no block commits to >1 disk
                            // substrate today, so this is unreachable in prod;
                            // the real fix if that changes is a per-commit height
                            // cursor in the disk substrate's commit metadata.
                            if already_post >= 2 {
                                return Err(Error::Torn(format!(
                                    "block {height}: changed set spans {already_post} \
                                     per-block-durable disk substrates at mixed roots — the \
                                     multi-store atomicity limit; single-frame selective replay \
                                     cannot safely reconcile this. wipe app state and re-sync \
                                     (keep the consensus journal)"
                                )));
                            }
                            // a changed module at NEITHER pre nor post is
                            // genuine damage — still fail-stop. so is a torn
                            // block with nothing left to re-commit (it should
                            // have been caught by the all-at-post fast path).
                            if commit_only.len() + already_post != changed.len()
                                || commit_only.is_empty()
                            {
                                return Err(Error::Torn(format!(
                                    "block {height}: touched modules are neither all-applied \
                                     nor all-unapplied and cannot be reconciled by selective \
                                     replay — wipe app state and re-sync (keep the consensus \
                                     journal)"
                                )));
                            }
                            let (_, dispatches) = apply_block_committing(
                                host,
                                height,
                                &frame,
                                protocol_version,
                                Some(disposition),
                                &commit_only,
                            )
                            .await?;
                            if let Some(sink) = sink.as_deref_mut() {
                                // the dispatch trace is the full deterministic
                                // re-execution; only the COMMIT scope was
                                // selective.
                                sink.folded_block(&FoldedBlock {
                                    height,
                                    frame: &frame,
                                    disposition,
                                    app_hash,
                                    dispatches: &dispatches,
                                });
                            }
                            // every changed module — the re-committed cohort AND
                            // the already-durable ones left untouched — must now
                            // stand at its sealed post-root.
                            for (id, root) in &changed {
                                let live = host.module_root(id);
                                if live != Some(*root) {
                                    return Err(Error::Verify(format!(
                                        "torn-block replay {height} left module {id} at \
                                         {live:?}, sealed root was {root:?}"
                                    )));
                                }
                            }
                            applied += 1;
                        }
                    }
                    blocks.push((height, roots.clone()));
                    for (id, root) in roots {
                        expected.insert(id, root);
                    }
                    tip_height = Some(height);
                    tip_hash = app_hash;
                }
            }
        }

        // a trailing unsealed block: the crash hit between its WAL record and
        // its seal. roll it forward (or recognize it as already applied) and
        // seal it NOW from the observed outcome.
        let mut rolled_forward = false;
        if let Some((height, frame)) = pending {
            // version-aware replay: same split-selector restore as the sealed path
            // above — the at_pre comparison reads `root()` under the PREVIOUS
            // height's version, the roll-forward + seal under this block's version.
            let protocol_version = host.effective_version(height).await;
            let pre_version = host.effective_version(height.saturating_sub(1)).await;
            host.set_active_version(pre_version);
            let at_pre = host
                .module_roots()
                .iter()
                .all(|(id, root)| expected.get(id) == Some(root));
            host.set_active_version(protocol_version);
            let disposition = if at_pre {
                let (disposition, dispatches) =
                    apply_block(host, height, &frame, protocol_version, None).await?;
                if let Some(sink) = sink.as_deref_mut() {
                    sink.folded_block(&FoldedBlock {
                        height,
                        frame: &frame,
                        disposition,
                        // the roll-forward seals from the observed outcome
                        // below; this is that same post-block boundary.
                        app_hash: host.app_hash(),
                        dispatches: &dispatches,
                    });
                }
                disposition
            } else {
                // the apply completed before the crash; the roots that moved
                // are its outcome. (single-disk-substrate blocks make this
                // exact — see the crate doc on the multi-store limit.)
                if let Some(sink) = sink.as_deref_mut() {
                    sink.opaque_block(height);
                }
                Disposition::Applied
            };
            let seal = BlockSeal {
                height,
                disposition,
                roots: host.module_roots(),
                app_hash: host.app_hash(),
            };
            BlockSink::seal(self, &seal)
                .await
                .map_err(|e| Error::Storage(e.to_string()))?;
            self.journal.sync().await.map_err(storage_err)?;
            blocks.push((height, seal.roots.clone()));
            tip_height = Some(height);
            tip_hash = host.app_hash();
            rolled_forward = true;
        }

        // final belt-and-suspenders: drive every dual-path module's branch
        // selector to the TIP's effective version before the app-hash check, so a
        // replay whose tail blocks were all skipped (disk substrates already held
        // them) or a genesis boot with no journal suffix still evaluates `root()`
        // at the correct boundary version. no-op below an activation boundary.
        if let Some(h) = tip_height {
            let v = host.effective_version(h).await;
            host.set_active_version(v);
        }
        // THE verification: the recomposed state must be byte-identical to
        // what consensus sealed at the tip. anything else means the recovered
        // node would fork — refuse to start.
        let live = host.app_hash();
        if live != tip_hash {
            return Err(Error::Verify(format!(
                "recomposed app_hash {live:?} != sealed tip {tip_hash:?} at height {tip_height:?}"
            )));
        }

        Ok(Recovered {
            height: tip_height,
            app_hash: tip_hash,
            epoch,
            view_base,
            participants,
            residents,
            frames,
            blocks,
            applied,
            skipped,
            rolled_forward,
        })
    }
}

/// re-apply one journaled frame through the host at its original block
/// coordinate. when `expect` is given, the outcome must reproduce the sealed
/// disposition (the drain is deterministic — anything else is divergence).
/// alongside the disposition, hands back the block's dispatch trace (empty
/// for a rejected block) so a [`ReplaySink`] can fold what the drain would
/// have fed it live.
async fn apply_block(
    host: &mut Host,
    height: u64,
    frame: &[u8],
    protocol_version: u32,
    expect: Option<Disposition>,
) -> Result<(Disposition, Vec<DispatchRecord>), Error> {
    let mut dispatches = Vec::new();
    let outcome = match decode_frame(frame) {
        Ok((origin, msg)) => {
            // `protocol_version` is the PURE `effective_version(height)` over the
            // REPLAYED committed upgrade state — the identical value the live node
            // stamped for this block (never the old hardcoded baseline, which would
            // fork a dual-path module's v2 `root()` at/after an activation boundary
            // H). the caller has already driven every dual-path module's non-hashed
            // `active_version` to this same version so `root()` recomputes the
            // boundary's format. inert (baseline) until the upgrade module is
            // registered and armed.
            let ctx = BlockContext {
                protocol_version,
                height,
                consensus_time: height,
                origin,
            };
            match host.submit_at(ctx, msg).await {
                Ok(outcome) => {
                    dispatches = outcome.dispatches;
                    Disposition::Applied
                }
                Err(SubmitError::Rejected(_)) => Disposition::Rejected,
                Err(SubmitError::Fatal(f)) => {
                    return Err(Error::Torn(format!("boundary fault during replay: {f}")));
                }
            }
        }
        // a frame that never decoded was a deterministic no-op at runtime too.
        Err(_) => Disposition::Rejected,
    };
    if let Some(expect) = expect {
        if outcome != expect {
            return Err(Error::Verify(format!(
                "replayed block {height} landed as {outcome:?}, sealed as {expect:?}"
            )));
        }
    }
    Ok((outcome, dispatches))
}

/// re-apply one journaled frame like [`apply_block`], but commit ONLY the
/// modules in `commit_only` at the block boundary and abort the rest (see
/// [`Host::submit_at_committing`]). used to heal a TORN block whose disk
/// substrates are already durable at their sealed post-root: replay re-commits
/// only the in-memory cohort that was rolled back to the checkpoint.
async fn apply_block_committing(
    host: &mut Host,
    height: u64,
    frame: &[u8],
    protocol_version: u32,
    expect: Option<Disposition>,
    commit_only: &BTreeSet<ModuleId>,
) -> Result<(Disposition, Vec<DispatchRecord>), Error> {
    let mut dispatches = Vec::new();
    let outcome = match decode_frame(frame) {
        Ok((origin, msg)) => {
            // stamp the block's effective version exactly like `apply_block`, so a
            // dual-path module re-executes the sealed frame under the SAME version
            // the live node did (fork-critical at an activation boundary).
            let ctx = BlockContext {
                protocol_version,
                height,
                consensus_time: height,
                origin,
            };
            match host.submit_at_committing(ctx, msg, commit_only).await {
                Ok(outcome) => {
                    dispatches = outcome.dispatches;
                    Disposition::Applied
                }
                Err(SubmitError::Rejected(_)) => Disposition::Rejected,
                Err(SubmitError::Fatal(f)) => {
                    return Err(Error::Torn(format!("boundary fault during torn replay: {f}")));
                }
            }
        }
        Err(_) => Disposition::Rejected,
    };
    if let Some(expect) = expect {
        if outcome != expect {
            return Err(Error::Verify(format!(
                "torn-block replay {height} landed as {outcome:?}, sealed as {expect:?}"
            )));
        }
    }
    Ok((outcome, dispatches))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roots(pairs: &[(&str, u8)]) -> Vec<(ModuleId, StateRoot)> {
        pairs
            .iter()
            .map(|(id, fill)| (id.to_string(), StateRoot([*fill; 32])))
            .collect()
    }

    #[test]
    fn record_roundtrip() {
        let records = vec![
            Record::Pinned {
                frame: b"frame-bytes".to_vec(),
            },
            Record::Block {
                height: 7,
                frame: vec![0, 1, 2],
            },
            Record::Seal {
                height: 7,
                disposition: Disposition::Applied,
                roots: roots(&[("directory", 3), ("kv", 9)]),
                app_hash: StateRoot([5; 32]),
            },
            Record::Seal {
                height: 8,
                disposition: Disposition::Rejected,
                roots: vec![],
                app_hash: StateRoot([6; 32]),
            },
            Record::Cutover {
                epoch: 2,
                view_base: 40,
                participants: vec![vec![7u8; 32], vec![8u8; 32]],
                residents: vec![vec![9u8; 32]],
            },
        ];
        for r in records {
            let decoded = Record::decode(&r.encode()).expect("roundtrip");
            assert_eq!(decoded, r);
        }
    }

    #[test]
    fn record_rejects_damage() {
        let good = Record::Block {
            height: 7,
            frame: vec![0, 1, 2],
        }
        .encode();
        // truncation
        assert!(Record::decode(&good[..good.len() - 1]).is_err());
        // trailing garbage
        let mut long = good.clone();
        long.push(0);
        assert!(Record::decode(&long).is_err());
        // unknown tag
        let mut bad = good;
        bad[0] = 99;
        assert!(Record::decode(&bad).is_err());
    }

    fn sample_manifest() -> Manifest {
        Manifest {
            height: Some(42),
            epoch: 1,
            view_base: 30,
            participants: vec![vec![7u8; 32], vec![8u8; 32]],
            residents: vec![vec![9u8; 32]],
            pending_cutover_view: Some(15),
            app_hash: StateRoot([1; 32]),
            roots: roots(&[("directory", 2), ("valset", 3)]),
            snapshots: vec![
                ("directory".into(), b"dir-bytes".to_vec()),
                ("valset".into(), vec![]),
            ],
            oplog_pos: 17,
            next_seq: 5,
            current_version: 0,
            pending_upgrade: None,
            required_min_version: 0,
        }
    }

    #[test]
    fn manifest_roundtrip() {
        // defaults for the version tail.
        let m = sample_manifest();
        let decoded = Manifest::decode(&m.encode()).expect("roundtrip");
        assert_eq!(decoded, m);
        assert_eq!(decoded.snapshot("directory"), Some(b"dir-bytes".as_ref()));
        assert_eq!(decoded.root("valset"), Some(StateRoot([3; 32])));

        // non-default version tail, with a pending upgrade set.
        let m = Manifest {
            current_version: 3,
            pending_upgrade: Some(UpgradeCoords {
                name: "v4".into(),
                activation_height: 100,
                to_version: 4,
            }),
            required_min_version: 3,
            ..sample_manifest()
        };
        assert_eq!(Manifest::decode(&m.encode()).expect("roundtrip"), m);
    }

    #[test]
    fn manifest_decode_tolerates_old_format() {
        // an on-disk checkpoint written before the version fields existed: the
        // encoding truncated at `next_seq`. a new binary must map BOTH missing
        // tails (version fields, resident set) to baseline defaults.
        let m = Manifest {
            residents: vec![],
            ..sample_manifest()
        };
        let full = m.encode();
        // the tails are `u32 current + 1-byte pending tag(None) + u32 required`
        // = 9 bytes, then the empty resident keys = 8 bytes; drop both to
        // simulate the pre-version format.
        let old = &full[..full.len() - 17];
        let decoded = Manifest::decode(old).expect("old format decodes");
        assert_eq!(decoded.current_version, 0);
        assert_eq!(decoded.pending_upgrade, None);
        assert_eq!(decoded.required_min_version, 0);
        // everything before the tails survives unchanged.
        assert_eq!(decoded, m);

        // the MIDDLE format — version tail present, resident tail absent (a
        // checkpoint from the version era, before the staged-admission tier):
        // residents default to empty, version fields survive.
        let middle = &full[..full.len() - 8];
        let decoded = Manifest::decode(middle).expect("middle format decodes");
        assert_eq!(decoded, m);
    }

    #[test]
    fn manifest_decode_rejects_truncated_version_tail() {
        // a present-but-malformed tail (one byte into the version fields) must
        // fail loud, never silently default. the resident tail sits after the
        // version tail, so to tear inside the version fields drop the whole
        // (empty) resident section — 8 bytes — plus half the required_min u32.
        let m = Manifest {
            residents: vec![],
            ..sample_manifest()
        };
        let full = m.encode();
        let torn = &full[..full.len() - 8 - 4];
        assert!(Manifest::decode(torn).is_err());
        // and a torn RESIDENT tail fails loud too.
        let full = sample_manifest().encode();
        let torn = &full[..full.len() - 4]; // inside the resident key bytes.
        assert!(Manifest::decode(torn).is_err());
    }

    #[test]
    fn required_min_version_fencepost_via_capture() {
        // the derivation the capture path uses, exercised through the shared
        // sdk fence: below activation -> current_version, at/after -> to_version.
        let pending = UpgradeCoords {
            name: "v2".into(),
            activation_height: 50,
            to_version: 2,
        };
        assert_eq!(sdk::required_min_version(1, Some(&pending), 49), 1);
        assert_eq!(sdk::required_min_version(1, Some(&pending), 50), 2);
        assert_eq!(sdk::required_min_version(1, None, 50), 1);
    }

    #[test]
    fn preflight_gates_on_required_min() {
        let m = Manifest {
            required_min_version: 3,
            ..sample_manifest()
        };
        // a build that supports >= the required min boots.
        assert!(m.preflight(3).is_ok());
        assert!(m.preflight(4).is_ok());
        // an under-versioned build refuses loud.
        let err = m.preflight(2).expect_err("under-versioned build");
        assert_eq!(err.required_min, 3);
        assert_eq!(err.max_supported, 2);
    }

    #[test]
    fn floor_cert_roundtrip() {
        let c = FloorCert {
            epoch: 3,
            height: 99,
            cert: b"certificate".to_vec(),
        };
        assert_eq!(FloorCert::decode(&c.encode()).expect("roundtrip"), c);
    }
}
