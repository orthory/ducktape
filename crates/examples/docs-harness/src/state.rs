//! the harness's canonical state: the committed/pending store, its strict
//! encode/decode, and the root/snapshot plumbing.

use std::collections::BTreeSet;

use sdk::{Error, StateRoot};
use sha2::{Digest, Sha256};

use crate::{DocsHarness, FailureRow, MAX_FAILURE_ROWS};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
    Active,
    Suspended,
    Unplugged,
}

impl Phase {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Phase::Active => "active",
            Phase::Suspended => "suspended",
            Phase::Unplugged => "unplugged",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Installed {
    pub(crate) package: String,
    pub(crate) phase: Phase,
    /// registered agent ids, in seed order.
    pub(crate) agents: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Store {
    pub(crate) installed: Option<Installed>,
    /// idempotency keys of already-minted jobs (`<comment>\x1f<agent>`).
    ///
    /// ## growth is unbounded — deliberately
    ///
    /// this set grows ONE entry per real (comment, agent) engagement: every
    /// genuine mention a live network ever sees adds a key, for as long as
    /// the package stays installed, and NEVER shrinks. that is accepted, not
    /// an oversight — unlike [`FailureRow`]'s bounded, evictable log, EVERY
    /// entry here is load-bearing for as long as pages could still redeliver
    /// (or a hook replay could resubmit) the write that minted it: evicting
    /// an old key would let a redelivered `PageEvent` re-mint a duplicate job
    /// for a comment that already got one — the exact bug this set exists to
    /// prevent. there is no natural "this key is now safe to forget" horizon
    /// short of the package's own lifetime, so the set rides the package's
    /// full install-to-unplug span instead of a bounded window.
    ///
    /// `unplug` does NOT clear it: unplug tombstones the agents and drops
    /// the pages hook (mint-time inputs), but user data and the harness's
    /// own audit trail are preserve-by-default — `minted` freezes at
    /// whatever it held the moment unplug landed and stays exactly that from
    /// then on (no new writes reach it: the hook is gone, so `on_page_event`
    /// never fires again). that frozen set is itself useful post-mortem:
    /// "every (comment, agent) pair this installation ever engaged."
    pub(crate) minted: BTreeSet<String>,
    /// bounded error-row log (oldest evicted past [`MAX_FAILURE_ROWS`]).
    pub(crate) failures: Vec<FailureRow>,
}

impl DocsHarness {
    /// the staged view — pending if this block already wrote, else committed.
    pub(crate) fn store(&self) -> &Store {
        self.pending.as_ref().unwrap_or(&self.committed)
    }

    pub(crate) fn store_mut(&mut self) -> &mut Store {
        if self.pending.is_none() {
            self.pending = Some(self.committed.clone());
        }
        self.pending.as_mut().expect("just populated")
    }

    // ---- root / snapshot ---------------------------------------------------------

    pub(crate) fn root_of(store: &Store) -> StateRoot {
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
        self.applied_this_block.clear();
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
                out.extend_from_slice(&(installed.agents.len() as u64).to_le_bytes());
                for agent in &installed.agents {
                    push_str(&mut out, agent);
                }
            }
        }
        out.extend_from_slice(&(self.minted.len() as u64).to_le_bytes());
        for key in &self.minted {
            push_str(&mut out, key);
        }
        out.extend_from_slice(&(self.failures.len() as u64).to_le_bytes());
        for row in &self.failures {
            push_str(&mut out, &row.action_id);
            push_str(&mut out, &row.tag);
            push_str(&mut out, &row.reason);
        }
        out
    }

    /// strict decode: only canonical encodings of execute-reachable states
    /// are accepted (sorted unique keys, valid phase, bounded failure log,
    /// no trailing bytes).
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
                    agents,
                })
            }
            _ => return Err(Error::Module("snapshot installed tag is invalid".into())),
        };

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

        let failure_count = read_count(bytes, &mut off)?;
        if failure_count > MAX_FAILURE_ROWS as u64 {
            return Err(Error::Module("snapshot failure log exceeds its cap".into()));
        }
        let mut failures: Vec<FailureRow> = Vec::new();
        for _ in 0..failure_count {
            let action_id = read_string(bytes, &mut off)?;
            let tag = read_string(bytes, &mut off)?;
            let reason = read_string(bytes, &mut off)?;
            if tag.is_empty() || reason.is_empty() {
                return Err(Error::Module("snapshot failure row is invalid".into()));
            }
            failures.push(FailureRow {
                action_id,
                tag,
                reason,
            });
        }

        if off != bytes.len() {
            return Err(Error::Module("snapshot has trailing bytes".into()));
        }
        Ok(Store {
            installed,
            minted,
            failures,
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
