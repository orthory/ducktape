//! replicated team vaults — opaque ciphertext with owner/reader bookkeeping.
//!
//! ## trust model (the part that must never blur)
//!
//! replicated state is readable by every validator, so secret VALUES are
//! opaque ciphertext the CLIENT produced (encrypted to the vault's readers
//! before submission) — the cryptographic envelope is the read barrier, the
//! module never sees plaintext. what the module DOES enforce is WRITE
//! integrity: every op's `Origin::External(pubkey)` is authenticated by the
//! ordered lane's frame-signature verification, and only a vault's OWNERS may
//! rotate secrets or membership. the reader list is recipient bookkeeping —
//! it tells clients whom to encrypt for and gives rotation a worklist.
//!
//! state model mirrors governance/tasks: execute STAGES whole-vault copies
//! into a pending overlay, `commit_block` publishes, `abort_block` discards;
//! `root()` is sha256 over the canonical encoding of COMMITTED vaults, and
//! `snapshot`/`install` ship exactly that preimage (verify-then-adopt, strict
//! bounds-checked decode of untrusted bytes).

// the wire surface: this module's shared types, flattened at the crate root.
mod interface;
pub use interface::*;

use std::collections::{BTreeMap, BTreeSet};

use sdk::codec;
use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot};
use sha2::{Digest, Sha256};

/// per-secret ciphertext ceiling: a vault holds credentials, not blobs. keeps
/// one hostile put from ballooning every validator's replicated state.
const MAX_CIPHERTEXT_LEN: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Vault {
    name: String,
    created_at: u64,
    owners: BTreeSet<Vec<u8>>,
    readers: BTreeSet<Vec<u8>>,
    secrets: BTreeMap<String, SecretEntry>,
}

pub struct Vaults {
    id: ModuleId,
    /// committed vaults — what `root()` commits to.
    vaults: BTreeMap<String, Vault>,
    /// this block's staged writes (whole-vault granularity).
    pending: BTreeMap<String, Vault>,
}

impl Vaults {
    pub fn new(id: impl Into<ModuleId>) -> Self {
        Self {
            id: id.into(),
            vaults: BTreeMap::new(),
            pending: BTreeMap::new(),
        }
    }

    fn get(&self, id: &str) -> Option<&Vault> {
        self.pending.get(id).or_else(|| self.vaults.get(id))
    }

    /// the AUTHENTICATED submitter. vault ops are user actions: module and
    /// system origins are refused so no module can quietly become an owner, and
    /// the pre-consensus empty external default is refused so it cannot become
    /// one either — an empty owner would let any unauthenticated caller pass
    /// `require_owner`. mirrors chat/agent's rejection of an empty external id.
    fn external_origin(ctx: &dyn Ctx) -> Result<Vec<u8>, Error> {
        match &ctx.env().origin {
            Origin::External(key) if key.is_empty() => Err(Error::Module(
                "vault ops require a non-empty external submitter".into(),
            )),
            Origin::External(key) => Ok(key.clone()),
            other => Err(Error::Module(format!(
                "vault ops require an external submitter, got {other:?}"
            ))),
        }
    }

    fn require_owner(vault: &Vault, who: &[u8]) -> Result<(), Error> {
        if vault.owners.contains(who) {
            Ok(())
        } else {
            Err(Error::Module("submitter is not a vault owner".into()))
        }
    }

    fn view_of(id: &str, v: &Vault) -> VaultView {
        VaultView {
            vault_id: id.to_string(),
            name: v.name.clone(),
            created_at: v.created_at,
            owners: v.owners.iter().cloned().collect(),
            readers: v.readers.iter().cloned().collect(),
            secret_names: v.secrets.keys().cloned().collect(),
        }
    }

    // ---- canonical state bytes ----------------------------------------------

    fn encode_state(vaults: &BTreeMap<String, Vault>) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(vaults.len() as u64).to_le_bytes());
        for (id, v) in vaults {
            codec::push_bytes(&mut out, id.as_bytes());
            codec::push_bytes(&mut out, v.name.as_bytes());
            out.extend_from_slice(&v.created_at.to_le_bytes());
            push_key_set(&mut out, &v.owners);
            push_key_set(&mut out, &v.readers);
            out.extend_from_slice(&(v.secrets.len() as u64).to_le_bytes());
            for (name, s) in &v.secrets {
                codec::push_bytes(&mut out, name.as_bytes());
                codec::push_bytes(&mut out, &s.ciphertext);
                out.extend_from_slice(&s.version.to_le_bytes());
                out.extend_from_slice(&s.created_at.to_le_bytes());
                out.extend_from_slice(&s.updated_at.to_le_bytes());
            }
        }
        out
    }

    fn root_of(vaults: &BTreeMap<String, Vault>) -> StateRoot {
        if vaults.is_empty() {
            return StateRoot::ZERO;
        }
        let mut h = Sha256::new();
        h.update(Self::encode_state(vaults));
        StateRoot(h.finalize().into())
    }

    /// canonical bytes of COMMITTED state — the exact preimage of `root()`.
    pub fn snapshot(&self) -> Vec<u8> {
        Self::encode_state(&self.vaults)
    }

    /// verify-then-adopt a peer snapshot; any error leaves every layer intact.
    pub fn install(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
        let decoded = decode_state(bytes)?;
        sdk::verify_snapshot_root(Self::root_of(&decoded), expected)?;
        self.vaults = decoded;
        self.pending.clear();
        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl Module for Vaults {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn root(&self) -> StateRoot {
        Self::root_of(&self.vaults)
    }

    fn snapshot_bytes(&self) -> Option<Vec<u8>> {
        Some(self.snapshot())
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        let who = Self::external_origin(ctx)?;
        let now = ctx.env().consensus_time;
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            VaultMsg::CreateVault { vault_id, name } => {
                sdk::require_non_empty("vault_id", &vault_id)?;
                sdk::require_non_empty("name", &name)?;
                if self.get(&vault_id).is_some() {
                    return Err(Error::Module(format!("vault already exists: {vault_id}")));
                }
                let mut owners = BTreeSet::new();
                owners.insert(who.clone());
                let mut readers = BTreeSet::new();
                readers.insert(who);
                self.pending.insert(
                    vault_id,
                    Vault {
                        name,
                        created_at: now,
                        owners,
                        readers,
                        secrets: BTreeMap::new(),
                    },
                );
                Ok(())
            }
            VaultMsg::AddOwner { vault_id, key } => {
                self.stage_membership(&vault_id, &who, |v| {
                    v.owners.insert(key.clone());
                    // an owner can always be encrypted-for: owners are readers.
                    v.readers.insert(key.clone());
                    Ok(())
                })
            }
            VaultMsg::RemoveOwner { vault_id, key } => {
                self.stage_membership(&vault_id, &who, |v| {
                    if v.owners.len() == 1 && v.owners.contains(&key) {
                        return Err(Error::Module("a vault must keep at least one owner".into()));
                    }
                    v.owners.remove(&key);
                    Ok(())
                })
            }
            VaultMsg::AddReader { vault_id, key } => self.stage_membership(&vault_id, &who, |v| {
                v.readers.insert(key.clone());
                Ok(())
            }),
            VaultMsg::RemoveReader { vault_id, key } => {
                self.stage_membership(&vault_id, &who, |v| {
                    if v.owners.contains(&key) {
                        return Err(Error::Module(
                            "owners are always readers; remove ownership first".into(),
                        ));
                    }
                    v.readers.remove(&key);
                    Ok(())
                })
            }
            VaultMsg::PutSecret {
                vault_id,
                name,
                ciphertext,
            } => {
                sdk::require_non_empty("secret name", &name)?;
                if ciphertext.is_empty() {
                    return Err(Error::Module("ciphertext must not be empty".into()));
                }
                if ciphertext.len() > MAX_CIPHERTEXT_LEN {
                    return Err(Error::Module(format!(
                        "ciphertext exceeds the {MAX_CIPHERTEXT_LEN}-byte ceiling"
                    )));
                }
                self.stage_membership(&vault_id, &who, |v| {
                    let entry = v.secrets.get(&name);
                    let (version, created_at) = match entry {
                        Some(e) => (e.version + 1, e.created_at),
                        None => (1, now),
                    };
                    v.secrets.insert(
                        name.clone(),
                        SecretEntry {
                            ciphertext: ciphertext.clone(),
                            version,
                            created_at,
                            updated_at: now,
                        },
                    );
                    Ok(())
                })
            }
            VaultMsg::DeleteSecret { vault_id, name } => {
                self.stage_membership(&vault_id, &who, |v| {
                    if v.secrets.remove(&name).is_none() {
                        return Err(Error::Module(format!("no such secret: {name}")));
                    }
                    Ok(())
                })
            }
        }
    }

    /// read projection — committed plus this block's staged changes. secret
    /// ciphertext is served to anyone who asks: replicated state is not
    /// confidential, the client-side envelope is (see the module doc).
    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        match decode_query(req).map_err(Error::Module)? {
            VaultQuery::Vaults => {
                let mut merged = self.vaults.clone();
                for (id, v) in &self.pending {
                    merged.insert(id.clone(), v.clone());
                }
                let views = merged.iter().map(|(id, v)| Self::view_of(id, v)).collect();
                Ok(encode_reply(&VaultReply::Vaults(views)))
            }
            VaultQuery::Vault { vault_id } => Ok(encode_reply(&VaultReply::Vault(
                self.get(&vault_id).map(|v| Self::view_of(&vault_id, v)),
            ))),
            VaultQuery::Secret { vault_id, name } => Ok(encode_reply(&VaultReply::Secret(
                self.get(&vault_id)
                    .and_then(|v| v.secrets.get(&name).cloned()),
            ))),
        }
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        for (id, v) in std::mem::take(&mut self.pending) {
            self.vaults.insert(id, v);
        }
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.pending.clear();
        Ok(())
    }
}

impl Vaults {
    /// stage an owner-gated mutation of one vault (whole-vault copy overlay).
    fn stage_membership(
        &mut self,
        vault_id: &str,
        who: &[u8],
        mutate: impl FnOnce(&mut Vault) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let mut vault = self
            .get(vault_id)
            .cloned()
            .ok_or_else(|| Error::Module(format!("no such vault: {vault_id}")))?;
        Self::require_owner(&vault, who)?;
        mutate(&mut vault)?;
        self.pending.insert(vault_id.to_string(), vault);
        Ok(())
    }
}

fn push_key_set(out: &mut Vec<u8>, keys: &BTreeSet<Vec<u8>>) {
    out.extend_from_slice(&(keys.len() as u64).to_le_bytes());
    for k in keys {
        codec::push_bytes(out, k);
    }
}

// ---- strict snapshot decode (untrusted bytes) -------------------------------
// primitives over the shared `sdk::codec::Cursor` (u64-le counts, u64-le
// length prefixes) — every accessor bounds-checks before it reads.

fn take_key_set(cur: &mut codec::Cursor, what: &str) -> Result<BTreeSet<Vec<u8>>, Error> {
    let count = cur.u64(what)?;
    cur.bound(count, 8, what)?;
    let mut set = BTreeSet::new();
    let mut prev: Option<Vec<u8>> = None;
    for _ in 0..count {
        let key = cur.bytes(what)?.to_vec();
        if prev.as_deref().is_some_and(|p| p >= key.as_slice()) {
            return Err(Error::Module(
                "snapshot keys must be strictly increasing".into(),
            ));
        }
        prev = Some(key.clone());
        set.insert(key);
    }
    Ok(set)
}

fn decode_state(bytes: &[u8]) -> Result<BTreeMap<String, Vault>, Error> {
    let mut cur = codec::Cursor::new(bytes);
    let count = cur.u64("vault count")?;
    cur.bound(count, 8, "vault")?;
    let mut vaults = BTreeMap::new();
    let mut prev_id: Option<String> = None;
    for _ in 0..count {
        let id = cur.string("vault id")?;
        if prev_id.as_deref().is_some_and(|p| p >= id.as_str()) {
            return Err(Error::Module(
                "snapshot vault ids must be strictly increasing".into(),
            ));
        }
        let name = cur.string("vault name")?;
        let created_at = cur.u64("vault created_at")?;
        let owners = take_key_set(&mut cur, "vault owners")?;
        let readers = take_key_set(&mut cur, "vault readers")?;
        let secret_count = cur.u64("secret count")?;
        cur.bound(secret_count, 8, "secret")?;
        let mut secrets = BTreeMap::new();
        let mut prev_name: Option<String> = None;
        for _ in 0..secret_count {
            let sname = cur.string("secret name")?;
            if prev_name.as_deref().is_some_and(|p| p >= sname.as_str()) {
                return Err(Error::Module(
                    "snapshot secret names must be strictly increasing".into(),
                ));
            }
            let ciphertext = cur.bytes("secret ciphertext")?.to_vec();
            let version = cur.u64("secret version")?;
            let s_created = cur.u64("secret created_at")?;
            let s_updated = cur.u64("secret updated_at")?;
            prev_name = Some(sname.clone());
            secrets.insert(
                sname,
                SecretEntry {
                    ciphertext,
                    version,
                    created_at: s_created,
                    updated_at: s_updated,
                },
            );
        }
        prev_id = Some(id.clone());
        vaults.insert(
            id,
            Vault {
                name,
                created_at,
                owners,
                readers,
                secrets,
            },
        );
    }
    cur.finish("snapshot")?;
    Ok(vaults)
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;
    use host::{Host, SubmitError};
    use crate::{decode_reply, encode_msg, encode_query, VaultQuery, VaultReply};

    /// the noded surface applies client bytes verbatim as `Origin::External`, so
    /// an empty origin (also `Host::submit`'s pre-consensus default) must not
    /// mint a vault owned by `[]` — an empty owner any later empty-origin caller
    /// would pass `require_owner` for. the op is a clean deterministic rejection.
    #[test]
    fn empty_external_origin_creates_no_vault() {
        block_on(async {
            let mut host = Host::genesis(vec![Box::new(Vaults::new("vaults"))]).expect("genesis");
            let app0 = host.root_hash();

            // Host::submit uses the default Origin::External(vec![]).
            let err = host
                .submit(Msg {
                    target: "vaults".into(),
                    payload: encode_msg(&VaultMsg::CreateVault {
                        vault_id: "infra".into(),
                        name: "Infra".into(),
                    }),
                })
                .await
                .expect_err("empty-origin create must be refused");
            assert!(
                matches!(err, SubmitError::Rejected(Error::Module(ref m)) if m.contains("non-empty external submitter")),
                "got {err:?}"
            );

            // no empty-owner vault landed, and the root hash is untouched.
            let reply = host
                .query("vaults", &encode_query(&VaultQuery::Vaults))
                .await
                .expect("query");
            let VaultReply::Vaults(views) = decode_reply(&reply).expect("decode") else {
                panic!("vaults reply");
            };
            assert!(views.is_empty(), "no vault must exist");
            assert_eq!(host.root_hash(), app0, "refused create leaves no trace");
        });
    }
}
