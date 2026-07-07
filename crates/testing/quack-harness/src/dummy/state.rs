//! the harness's canonical state: the committed/pending store, its strict
//! encode/decode, and the root/snapshot plumbing.

use std::collections::{BTreeMap, BTreeSet};

use sdk::{Error, ModuleId, StateRoot};
use sha2::{Digest, Sha256};

use super::DummyHarness;

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
    /// the engagement sources' CONCRETE module ids (sorted, deduped) — whose
    /// module-origin events this harness accepts, and where hooks live.
    pub(crate) sources: Vec<ModuleId>,
    /// registered agent ids, in seed order.
    pub(crate) agents: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Store {
    pub(crate) installed: Option<Installed>,
    /// the user data: note id -> text. survives suspend AND unplug.
    pub(crate) notes: BTreeMap<String, String>,
    /// idempotency keys of already-minted jobs (`<comment>\x1f<agent>`).
    pub(crate) minted: BTreeSet<String>,
}

impl DummyHarness {
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

// ---- snapshot tests --------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use sdk::{Module, Origin};

    use crate::dummy::ACTION_NOTE_ADD;
    use crate::dummy::testutil::*;

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
