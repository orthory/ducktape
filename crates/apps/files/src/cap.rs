//! typed fs capability over the module-injected interface: reads are host-routed
//! committed-state queries; writes are emitted intents that come back as
//! follow-up ops under the EMITTING module's origin, so `/home/<module-id>/**`
//! authority applies naturally.
//!
//! # the two halves
//!
//! a module never touches duckfs state directly — it only holds an `sdk::Ctx`. so
//! [`FsCap`] is pure sugar over that ctx: reads ride [`Ctx::query`] (the host
//! routes them to the fs module's committed, dispatch-start read projection) and
//! decode the crate's own wire replies; writes ride [`Ctx::emit_msg`] (each is a
//! [`FilesMsg`] the host re-dispatches as a FOLLOW-UP op after the current
//! `execute` returns — never a reentrant mutating call). because the follow-up
//! runs under the emitter's `Origin::Module(id)`, a write into `/home/<id>/**`
//! carries exactly the authority the fs module gates on, with no extra plumbing.
//!
//! # reads see committed state, writes are not yet visible
//!
//! [`Ctx::query`] serves committed state as of the start of dispatch, so a read
//! never observes a write this same `execute` emitted (the follow-up has not run
//! yet). a mutation flow that must build on its own prior write therefore commits
//! across blocks, threading [`FsCap::refs`] head into the next
//! [`FsCap::commit`]'s base — not by reading back within one dispatch.
//!
//! # wasm note
//!
//! [`FsCap`] speaks only the crate's wire types and the `sdk::Ctx` surface, so it
//! moves unchanged onto a future ctx-shim when the pure core compiles to wasm —
//! the shim supplies the same `query`/`emit_msg` seam this wraps today.

use std::collections::BTreeMap;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

use duckfs_core::*;
use sdk::{Ctx, Error, Msg};

/// a typed duckfs handle bound to one dispatch's `sdk::Ctx`. reads borrow the ctx
/// shared (`&self`); write intents borrow it mutably (`&mut self`) because they
/// push onto the emit queue.
pub struct FsCap<'a> {
    ctx: &'a mut dyn Ctx,
    target: String,
}

impl<'a> FsCap<'a> {
    /// bind to the standard "files" module id.
    pub fn new(ctx: &'a mut dyn Ctx) -> Self {
        Self::with_target(ctx, "files")
    }

    /// bind to a nonstandard fs module id (tests, a renamed deployment).
    pub fn with_target(ctx: &'a mut dyn Ctx, target: impl Into<String>) -> Self {
        Self {
            ctx,
            target: target.into(),
        }
    }

    // ---- reads (async — they ride Ctx::query) -------------------------------

    /// route one query to the fs module and decode its reply. a transport error
    /// bubbles as-is; a decode failure maps to [`Error::Module`].
    async fn ask(&self, q: &FilesQuery) -> Result<FilesReply, Error> {
        let bytes = self.ctx.query(&self.target, &encode_query(q)).await?;
        decode_reply(&bytes).map_err(Error::Module)
    }

    /// stat one path (defaulting to head, or an explicit committed snapshot).
    /// `None` = the path does not exist there.
    pub async fn stat(
        &self,
        path: &str,
        snapshot: Option<&str>,
    ) -> Result<Option<EntryInfo>, Error> {
        match self
            .ask(&FilesQuery::Stat {
                path: path.into(),
                snapshot: snapshot.map(Into::into),
            })
            .await?
        {
            FilesReply::Stat(entry) => Ok(entry),
            other => Err(unexpected(&other)),
        }
    }

    /// list one directory, paged: `after` is the last name of the prior page and
    /// the returned `Option<String>` is the cursor for the next (None at the end).
    pub async fn ls(
        &self,
        path: &str,
        snapshot: Option<&str>,
        after: Option<&str>,
        limit: u64,
    ) -> Result<(Vec<EntryInfo>, Option<String>), Error> {
        match self
            .ask(&FilesQuery::Ls {
                path: path.into(),
                snapshot: snapshot.map(Into::into),
                after: after.map(Into::into),
                limit,
            })
            .await?
        {
            FilesReply::Ls { entries, next } => Ok((entries, next)),
            other => Err(unexpected(&other)),
        }
    }

    /// read a whole file by looping [`FilesQuery::Read`] pages until eof. each
    /// page asks for [`MAX_READ_BYTES`] (the module's per-read clamp), so a page
    /// short of that bound still advances by exactly the bytes returned; the loop
    /// terminates the instant the module reports eof (`offset + returned ==
    /// size`), which lands cleanly even at an exact page boundary. a non-eof page
    /// that returns nothing is impossible from the real module (a positive `len`
    /// short of eof always yields ≥1 byte) but is broken out of anyway so a
    /// malformed peer can never spin the loop.
    pub async fn read_all(&self, path: &str, snapshot: Option<&str>) -> Result<Vec<u8>, Error> {
        let mut out = Vec::new();
        let mut offset = 0u64;
        loop {
            let (b64, eof) = match self
                .ask(&FilesQuery::Read {
                    path: path.into(),
                    snapshot: snapshot.map(Into::into),
                    offset,
                    len: MAX_READ_BYTES,
                })
                .await?
            {
                FilesReply::Read { b64, eof } => (b64, eof),
                other => return Err(unexpected(&other)),
            };
            let page = STANDARD
                .decode(b64.as_bytes())
                .map_err(|e| Error::Module(format!("files: read page b64: {e}")))?;
            let advanced = !page.is_empty();
            offset += page.len() as u64;
            out.extend_from_slice(&page);
            if eof || !advanced {
                break;
            }
        }
        Ok(out)
    }

    /// grep the first page of hits under `prefix`. one call is bounded (the
    /// module caps hits per call and scans a byte budget); callers wanting the
    /// full sweep drive [`FilesQuery::Grep`] with the returned cursor directly.
    pub async fn grep(
        &self,
        pattern: &str,
        prefix: &str,
        snapshot: Option<&str>,
    ) -> Result<Vec<GrepHit>, Error> {
        match self
            .ask(&FilesQuery::Grep {
                pattern: pattern.into(),
                prefix: prefix.into(),
                snapshot: snapshot.map(Into::into),
                cursor: None,
                limit: MAX_PAGE,
            })
            .await?
        {
            FilesReply::Grep { hits, .. } => Ok(hits),
            other => Err(unexpected(&other)),
        }
    }

    /// the refs cell: head snapshot, pins, and the committed window length.
    pub async fn refs(&self) -> Result<RefsInfo, Error> {
        match self.ask(&FilesQuery::Refs {}).await? {
            FilesReply::Refs(info) => Ok(info),
            other => Err(unexpected(&other)),
        }
    }

    // ---- write intents (sync — they ride emit_msg) --------------------------

    /// emit one [`FilesMsg`] as a follow-up op at the fs module.
    fn emit(&mut self, m: &FilesMsg) {
        self.ctx.emit_msg(Msg {
            target: self.target.clone(),
            payload: encode_msg(m),
        });
    }

    /// emit an atomic multi-path commit. `base` is the CAS base snapshot (`None`
    /// = the empty tree, i.e. a first/create-only commit); a mutation of an
    /// existing path must pass the live head (see [`FsCap::refs`]).
    pub fn commit(&mut self, base: Option<String>, message: &str, changes: Vec<Change>) {
        self.emit(&FilesMsg::Commit {
            base_snapshot: base,
            message: message.into(),
            changes,
        });
    }

    /// CREATE-ONLY sugar: a one-file inline commit with `base = None`. this only
    /// succeeds while the path is absent both at the empty base and at head
    /// (per-path CAS), so it is for fresh files. a mutation flow must instead
    /// [`FsCap::refs`] the head and [`FsCap::commit`] with `base = head`.
    pub fn put_inline(&mut self, path: &str, bytes: &[u8], message: &str) {
        self.commit(
            None,
            message,
            vec![Change::Put {
                path: path.into(),
                exec: false,
                meta: BTreeMap::new(),
                content: Content::Inline {
                    b64: STANDARD.encode(bytes),
                },
            }],
        );
    }

    /// pin a committed snapshot under `name` (protects it from window expiry).
    pub fn pin(&mut self, snapshot: &str, name: &str) {
        self.emit(&FilesMsg::Pin {
            snapshot: snapshot.into(),
            name: name.into(),
        });
    }

    /// register a watch: commits touching `prefix` fan out a notification op to
    /// `module_id` (decode it with [`decode_notify`]).
    pub fn watch(&mut self, prefix: &str, module_id: &str) {
        self.emit(&FilesMsg::Watch {
            prefix: prefix.into(),
            module_id: module_id.into(),
        });
    }
}

/// a query answered with the wrong reply variant — a routing/version skew, not a
/// module-domain rejection, but still surfaced through the same channel.
fn unexpected(reply: &FilesReply) -> Error {
    Error::Module(format!("files: unexpected reply variant {reply:?}"))
}

/// a decoded duckfs watch notification arriving at a module's `execute`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notify {
    pub prefix: String,
    pub path: String,
    pub snapshot: String,
}

/// decode a duckfs watch notification — the `{"duckfs_notify": {"prefix": ..,
/// "path": .., "snapshot": ..}}` shape the fs module emits on a watched commit.
/// returns `None` for ANY foreign payload (a sibling module's op, arbitrary
/// json, non-json bytes) and NEVER errors: a module probes every incoming op
/// with this, so a non-match must be a quiet miss, not a failure.
pub fn decode_notify(payload: &[u8]) -> Option<Notify> {
    let v: serde_json::Value = serde_json::from_slice(payload).ok()?;
    let n = v.get("duckfs_notify")?;
    Some(Notify {
        prefix: n.get("prefix")?.as_str()?.to_string(),
        path: n.get("path")?.as_str()?.to_string(),
        snapshot: n.get("snapshot")?.as_str()?.to_string(),
    })
}
