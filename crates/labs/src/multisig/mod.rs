//! M-of-N multisig coordination for external-chain (EVM) transactions.
//!
//! Named `multisig`, not `vault`: `crates/modules/apps/vaults` already exists and is a
//! different thing (client-sealed team secrets). The product surface still
//! calls this a Vault.
//!
//! ## what consensus is FOR here (and what it is not)
//!
//! Ethereum's `ecrecover` is the verifier of record for Safe owner signatures.
//! This module never re-implements a curve, never holds a key, and never
//! decides whether a transaction is *valid* — the Safe contract does that when
//! `execTransaction` runs. What consensus supplies is the thing Ducktape is
//! actually good at and Ethereum is not: a **total order and a
//! non-equivocating record** of who approved what. Concretely it gives us
//!
//! - one agreed-upon SafeTx hash per proposal, COMPUTED here rather than taken
//!   from the proposer, so nobody can show owners one transaction and collect
//!   signatures over another;
//! - a replicated, tamper-evident approval set, so the M-th owner does not have
//!   to trust a coordinator service to have kept the first M-1 signatures
//!   honestly (this is the piece Safe's centralized Transaction Service
//!   provides, and the piece we get for free);
//! - deterministic, byte-exact `execTransaction` calldata as COMMITTED state,
//!   so the oracle that broadcasts it transmits bytes it did not
//!   choose.
//!
//! ## determinism
//!
//! Every op is a pure function of committed state and op bytes: no clock beyond
//! `consensus_time`, no RNG, no I/O. All chain contact is quarantined in the
//! oracle, which re-enters through the validator-gated `RecordChainState` /
//! `RecordExecution` ops.
//!
//! State model mirrors `vaults`/`governance`: `execute` STAGES whole-vault
//! copies into a pending overlay, `commit_block` publishes, `abort_block`
//! discards; `root()` is sha256 over the canonical encoding of COMMITTED
//! vaults, and `snapshot`/`install` ship exactly that preimage.

mod interface;
pub use interface::*;

pub mod key;
pub mod safe;

use std::collections::{BTreeMap, BTreeSet};

use alloy_primitives::{Address, B256, U256};
use safe::{SafeTx, exec_transaction_calldata, pack_signatures, recover_owner, safe_tx_hash};
use sdk::{Ctx, Error, Event, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle};
use sha2::{Digest, Sha256};
use valset::{ValsetQuery, decode_reply as valset_decode_reply, encode_query as valset_encode_query};

/// Calldata ceiling for one proposal. A Safe transaction carries a contract
/// call, not a payload — this keeps one hostile proposal from ballooning every
/// validator's replicated state.
pub const MAX_DATA_LEN: usize = 32 * 1024;

/// How far past the chain's current nonce a proposal may reach. Safe nonces are
/// strictly sequential, so a proposal at `chain_nonce + k` cannot execute until
/// every nonce below it has. Without this bound, one owner could mint proposals
/// at arbitrary nonces and pin unbounded state.
pub const MAX_NONCE_LOOKAHEAD: u64 = 32;

/// Unexecuted proposals retained per vault.
pub const MAX_PENDING_PROPOSALS: usize = 64;

/// The maximum owners a mirrored Safe may declare.
pub const MAX_OWNERS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Proposal {
    nonce: u64,
    to: Address,
    value: U256,
    data: Vec<u8>,
    proposer: Address,
    /// owner -> their 65-byte signature over `safe_tx_hash`. A BTreeMap keyed by
    /// owner is what makes an approval idempotent AND makes double-counting one
    /// owner impossible.
    approvals: BTreeMap<Address, Vec<u8>>,
    executed: Option<ExecutionView>,
    created_at: u64,
}

impl Proposal {
    fn safe_tx(&self) -> SafeTx {
        SafeTx::call(self.to, self.value, self.data.clone(), self.nonce)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Vault {
    chain_id: u64,
    safe_address: Address,
    owners: BTreeSet<Address>,
    threshold: u8,
    backend: Backend,
    chain_nonce: u64,
    drifted: bool,
    created_at: u64,
    /// keyed by SafeTx hash — the natural identity of a proposal, since it is
    /// exactly what every owner signed.
    proposals: BTreeMap<B256, Proposal>,
    /// attribution only: owner address -> the account key that claimed it.
    bindings: BTreeMap<Address, Vec<u8>>,
}

pub struct Multisig {
    id: ModuleId,
    valset_id: ModuleId,
    vaults: BTreeMap<String, Vault>,
    pending: BTreeMap<String, Vault>,
}

impl Multisig {
    pub fn new(id: impl Into<ModuleId>, valset_id: impl Into<ModuleId>) -> Self {
        Self {
            id: id.into(),
            valset_id: valset_id.into(),
            vaults: BTreeMap::new(),
            pending: BTreeMap::new(),
        }
    }

    fn get(&self, id: &str) -> Option<&Vault> {
        self.pending.get(id).or_else(|| self.vaults.get(id))
    }

    /// The AUTHENTICATED submitter. Module and system origins are refused so no
    /// module can quietly bind an owner address, and the pre-consensus empty
    /// external default is refused so it cannot either (mirrors `vaults`).
    fn external_origin(ctx: &dyn Ctx) -> Result<Vec<u8>, Error> {
        match &ctx.env().origin {
            Origin::External(key) if key.is_empty() => Err(Error::Module(
                "multisig: ops require a non-empty external submitter".into(),
            )),
            Origin::External(key) => Ok(key.clone()),
            other => Err(Error::Module(format!(
                "multisig: ops require an external submitter, got {other:?}"
            ))),
        }
    }

    /// Chain facts are asserted by whichever node ran the RPC call, and nothing
    /// in consensus can check them against the real chain. Restricting them to
    /// the validator set at least keeps the assertion inside the quorum that
    /// already runs the network.
    ///
    /// This bounds the damage rather than eliminating it: a byzantine validator
    /// can stall a vault (freeze it as drifted, or hold `chain_nonce` back) but
    /// CANNOT move funds — spending still requires M real owner signatures over
    /// a hash this module computed, and the Safe re-checks all of them on-chain.
    /// A quorum attestation would close the stalling gap; it is not needed to
    /// make theft impossible.
    // ponytail: first-writer-wins among validators. Upgrade to an M-of-N
    // attestation if a byzantine validator stalling a vault becomes real.
    async fn require_validator(&self, ctx: &dyn Ctx, who: &[u8]) -> Result<(), Error> {
        let reply = valset_decode_reply(
            &ctx.query(&self.valset_id, &valset_encode_query(&ValsetQuery::Validators))
                .await?,
        )
        .map_err(Error::Module)?;
        let is_validator = match reply {
            valset::ValsetReply::Validators(keys) => keys.iter().any(|k| k == who),
            other => {
                return Err(Error::Module(format!(
                    "multisig: valset answered Validators with {other:?}"
                )));
            }
        };
        if is_validator {
            Ok(())
        } else {
            Err(Error::Module(
                "multisig: chain facts may only be recorded by a validator".into(),
            ))
        }
    }

    fn vault_mut(&mut self, vault_id: &str) -> Result<Vault, Error> {
        self.get(vault_id)
            .cloned()
            .ok_or_else(|| Error::Module(format!("multisig: no such vault: {vault_id}")))
    }

    fn executable_of(vault_id: &str, v: &Vault, hash: &B256, p: &Proposal) -> ExecutableView {
        let approvals = p
            .approvals
            .iter()
            .map(|(owner, sig)| {
                let mut raw = [0u8; 65];
                raw.copy_from_slice(sig);
                (*owner, raw)
            })
            .collect();
        ExecutableView {
            safe_tx_hash: hash.to_vec(),
            vault_id: vault_id.to_string(),
            chain_id: v.chain_id,
            safe_address: v.safe_address.to_vec(),
            nonce: p.nonce,
            calldata: exec_transaction_calldata(&p.safe_tx(), &pack_signatures(approvals)),
        }
    }

    fn proposal_view(vault_id: &str, v: &Vault, hash: &B256, p: &Proposal) -> ProposalView {
        ProposalView {
            safe_tx_hash: hash.to_vec(),
            vault_id: vault_id.to_string(),
            nonce: p.nonce,
            to: p.to.to_vec(),
            value: p.value.to_be_bytes::<32>().to_vec(),
            data: p.data.clone(),
            proposer: p.proposer.to_vec(),
            approvals: p.approvals.keys().map(|a| a.to_vec()).collect(),
            threshold: v.threshold,
            executed: p.executed.clone(),
            created_at: p.created_at,
        }
    }

    fn view_of(id: &str, v: &Vault) -> VaultView {
        VaultView {
            vault_id: id.to_string(),
            chain_id: v.chain_id,
            safe_address: v.safe_address.to_vec(),
            owners: v.owners.iter().map(|a| a.to_vec()).collect(),
            threshold: v.threshold,
            backend: v.backend,
            chain_nonce: v.chain_nonce,
            drifted: v.drifted,
            created_at: v.created_at,
        }
    }

    // ---- canonical state bytes ---------------------------------------------

    fn encode_state(vaults: &BTreeMap<String, Vault>) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&(vaults.len() as u64).to_le_bytes());
        for (id, v) in vaults {
            push_bytes(&mut out, id.as_bytes());
            out.extend_from_slice(&v.chain_id.to_le_bytes());
            out.extend_from_slice(v.safe_address.as_slice());
            out.extend_from_slice(&(v.owners.len() as u64).to_le_bytes());
            for owner in &v.owners {
                out.extend_from_slice(owner.as_slice());
            }
            out.push(v.threshold);
            out.push(match v.backend {
                Backend::Safe => 0,
                Backend::ThresholdEcdsa => 1,
            });
            out.extend_from_slice(&v.chain_nonce.to_le_bytes());
            out.push(v.drifted as u8);
            out.extend_from_slice(&v.created_at.to_le_bytes());
            out.extend_from_slice(&(v.proposals.len() as u64).to_le_bytes());
            for (hash, p) in &v.proposals {
                out.extend_from_slice(hash.as_slice());
                out.extend_from_slice(&p.nonce.to_le_bytes());
                out.extend_from_slice(p.to.as_slice());
                out.extend_from_slice(&p.value.to_be_bytes::<32>());
                push_bytes(&mut out, &p.data);
                out.extend_from_slice(p.proposer.as_slice());
                out.extend_from_slice(&(p.approvals.len() as u64).to_le_bytes());
                for (owner, sig) in &p.approvals {
                    out.extend_from_slice(owner.as_slice());
                    push_bytes(&mut out, sig);
                }
                match &p.executed {
                    None => out.push(0),
                    Some(e) => {
                        out.push(1);
                        push_bytes(&mut out, &e.chain_tx_hash);
                        out.push(e.success as u8);
                    }
                }
                out.extend_from_slice(&p.created_at.to_le_bytes());
            }
            out.extend_from_slice(&(v.bindings.len() as u64).to_le_bytes());
            for (addr, account) in &v.bindings {
                out.extend_from_slice(addr.as_slice());
                push_bytes(&mut out, account);
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
        if Self::root_of(&decoded) != expected {
            return Err(Error::Module("multisig: snapshot root mismatch".into()));
        }
        self.vaults = decoded;
        self.pending.clear();
        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl Module for Multisig {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn root(&self) -> StateRoot {
        Self::root_of(&self.vaults)
    }

    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::SnapshotBytes(self.snapshot()))
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        let who = Self::external_origin(ctx)?;
        let now = ctx.env().consensus_time;
        match decode_msg(&msg.payload).map_err(Error::Module)? {
            MultisigMsg::RegisterVault {
                vault_id,
                chain_id,
                safe_address,
                owners,
                threshold,
                signature,
            } => {
                sdk::require_non_empty("vault_id", &vault_id)?;
                if self.get(&vault_id).is_some() {
                    return Err(Error::Module(format!(
                        "multisig: vault already exists: {vault_id}"
                    )));
                }
                if owners.is_empty() || owners.len() > MAX_OWNERS {
                    return Err(Error::Module(format!(
                        "multisig: a vault needs 1..={MAX_OWNERS} owners"
                    )));
                }
                if threshold == 0 || usize::from(threshold) > owners.len() {
                    return Err(Error::Module(
                        "multisig: threshold must be within 1..=owners".into(),
                    ));
                }
                let safe = address_of(&safe_address)?;
                let owner_set: BTreeSet<Address> = owners
                    .iter()
                    .map(|o| address_of(o))
                    .collect::<Result<_, _>>()?;
                if owner_set.len() != owners.len() {
                    return Err(Error::Module("multisig: duplicate owner address".into()));
                }

                // Possession: whoever registers must hold one of the declared
                // owner keys, or anyone could spam records against Safes they
                // have nothing to do with.
                let preimage =
                    register_preimage(&vault_id, chain_id, &safe_address, &owners, threshold);
                let signer = recover_prehashed(&preimage, &signature)?;
                if !owner_set.contains(&signer) {
                    return Err(Error::Module(
                        "multisig: registration must be signed by a declared owner".into(),
                    ));
                }

                self.pending.insert(
                    vault_id,
                    Vault {
                        chain_id,
                        safe_address: safe,
                        owners: owner_set,
                        threshold,
                        backend: Backend::Safe,
                        // Zero until the oracle reports the real one. Proposals
                        // are still bounded by MAX_NONCE_LOOKAHEAD.
                        chain_nonce: 0,
                        drifted: false,
                        created_at: now,
                        proposals: BTreeMap::new(),
                        bindings: BTreeMap::new(),
                    },
                );
                Ok(())
            }

            MultisigMsg::BindOwnerEoa {
                vault_id,
                address,
                possession,
            } => {
                let mut vault = self.vault_mut(&vault_id)?;
                let addr = address_of(&address)?;
                if !vault.owners.contains(&addr) {
                    return Err(Error::Module(
                        "multisig: address is not an owner of this vault".into(),
                    ));
                }
                let preimage = bind_preimage(&vault_id, &address, &who);
                if recover_prehashed(&preimage, &possession)? != addr {
                    return Err(Error::Module(
                        "multisig: possession proof does not recover to the claimed address".into(),
                    ));
                }
                vault.bindings.insert(addr, who);
                self.pending.insert(vault_id, vault);
                Ok(())
            }

            MultisigMsg::ProposeTx {
                vault_id,
                nonce,
                to,
                value,
                data,
                signature,
            } => {
                let mut vault = self.vault_mut(&vault_id)?;
                if vault.drifted {
                    return Err(Error::Module(
                        "multisig: vault mirror disagrees with the chain; re-register it before proposing".into(),
                    ));
                }
                if data.len() > MAX_DATA_LEN {
                    return Err(Error::Module(format!(
                        "multisig: calldata exceeds the {MAX_DATA_LEN}-byte ceiling"
                    )));
                }
                // A Safe nonce is strictly sequential: below the chain's nonce
                // it can never execute, and far above it can only pin state.
                if nonce < vault.chain_nonce {
                    return Err(Error::Module(format!(
                        "multisig: nonce {nonce} is below the chain nonce {}",
                        vault.chain_nonce
                    )));
                }
                if nonce > vault.chain_nonce + MAX_NONCE_LOOKAHEAD {
                    return Err(Error::Module(format!(
                        "multisig: nonce {nonce} is more than {MAX_NONCE_LOOKAHEAD} past the chain nonce"
                    )));
                }
                let pending = vault
                    .proposals
                    .values()
                    .filter(|p| p.executed.is_none())
                    .count();
                if pending >= MAX_PENDING_PROPOSALS {
                    return Err(Error::Module(format!(
                        "multisig: vault already holds {MAX_PENDING_PROPOSALS} unexecuted proposals"
                    )));
                }

                let tx = SafeTx::call(address_of(&to)?, u256_of(&value)?, data, nonce);
                tx.validate().map_err(Error::Module)?;
                // COMPUTED here, never taken from the proposer: this is what
                // stops a proposer showing owners one transaction and having
                // them sign another.
                let hash = safe_tx_hash(vault.chain_id, vault.safe_address, &tx);

                let proposer = recover_owner(hash, &signature_of(&signature)?)
                    .map_err(|e| Error::Module(format!("multisig: {e}")))?;
                if !vault.owners.contains(&proposer) {
                    return Err(Error::Module(
                        "multisig: proposal must be signed by a current owner".into(),
                    ));
                }
                if vault.proposals.contains_key(&hash) {
                    return Err(Error::Module(
                        "multisig: an identical proposal already exists".into(),
                    ));
                }

                let mut approvals = BTreeMap::new();
                approvals.insert(proposer, signature);
                let proposal = Proposal {
                    nonce,
                    to: tx.to,
                    value: tx.value,
                    data: tx.data,
                    proposer,
                    approvals,
                    executed: None,
                    created_at: now,
                };
                // A 1-of-N vault is executable the instant it is proposed.
                if proposal.approvals.len() >= usize::from(vault.threshold) {
                    emit_executable(ctx, &self.id, &vault_id, &vault, &hash, &proposal);
                }
                vault.proposals.insert(hash, proposal);
                self.pending.insert(vault_id, vault);
                Ok(())
            }

            MultisigMsg::Approve {
                vault_id,
                safe_tx_hash: hash_bytes,
                signature,
            } => {
                let mut vault = self.vault_mut(&vault_id)?;
                let hash = b256_of(&hash_bytes)?;
                let sig = signature_of(&signature)?;

                let signer = recover_owner(hash, &sig)
                    .map_err(|e| Error::Module(format!("multisig: {e}")))?;
                if !vault.owners.contains(&signer) {
                    return Err(Error::Module(
                        "multisig: approval must be signed by a current owner".into(),
                    ));
                }

                let Some(proposal) = vault.proposals.get_mut(&hash) else {
                    return Err(Error::Module("multisig: no such proposal".into()));
                };
                if proposal.executed.is_some() {
                    return Err(Error::Module("multisig: proposal already executed".into()));
                }
                // Keyed by owner: re-approving is a no-op, and one owner can
                // never be counted twice toward the threshold.
                if proposal.approvals.insert(signer, signature).is_some() {
                    return Ok(());
                }

                if proposal.approvals.len() >= usize::from(vault.threshold) {
                    let proposal = proposal.clone();
                    emit_executable(ctx, &self.id, &vault_id, &vault, &hash, &proposal);
                }
                self.pending.insert(vault_id, vault);
                Ok(())
            }

            MultisigMsg::RecordChainState {
                vault_id,
                nonce,
                owners,
                threshold,
            } => {
                self.require_validator(ctx, &who).await?;
                let mut vault = self.vault_mut(&vault_id)?;
                let chain_owners: BTreeSet<Address> = owners
                    .iter()
                    .map(|o| address_of(o))
                    .collect::<Result<_, _>>()?;

                // The Safe is authoritative; the mirror is not. Disagreement
                // freezes new proposals rather than silently reconciling a
                // money path — an owner removed on-chain must not keep
                // approving here, and an owner added on-chain must not be
                // invisible to the threshold count.
                vault.drifted = chain_owners != vault.owners || threshold != vault.threshold;
                vault.chain_nonce = nonce;

                // Anything at or below the chain nonce can never execute again.
                vault
                    .proposals
                    .retain(|_, p| p.executed.is_some() || p.nonce >= nonce);
                self.pending.insert(vault_id, vault);
                Ok(())
            }

            MultisigMsg::RecordExecution {
                vault_id,
                safe_tx_hash: hash_bytes,
                chain_tx_hash,
                success,
            } => {
                self.require_validator(ctx, &who).await?;
                let mut vault = self.vault_mut(&vault_id)?;
                let hash = b256_of(&hash_bytes)?;
                if chain_tx_hash.len() != 32 {
                    return Err(Error::Module(
                        "multisig: chain tx hash must be 32 bytes".into(),
                    ));
                }
                let Some(proposal) = vault.proposals.get_mut(&hash) else {
                    return Err(Error::Module("multisig: no such proposal".into()));
                };
                if proposal.executed.is_some() {
                    return Ok(());
                }
                proposal.executed = Some(ExecutionView {
                    chain_tx_hash,
                    success,
                });
                self.pending.insert(vault_id, vault);
                Ok(())
            }
        }
    }

    async fn query(&self, req: &[u8]) -> Result<Vec<u8>, Error> {
        let merged = {
            let mut m = self.vaults.clone();
            for (id, v) in &self.pending {
                m.insert(id.clone(), v.clone());
            }
            m
        };
        match decode_query(req).map_err(Error::Module)? {
            MultisigQuery::Vaults => Ok(encode_reply(&MultisigReply::Vaults(
                merged.iter().map(|(id, v)| Self::view_of(id, v)).collect(),
            ))),
            MultisigQuery::Vault { vault_id } => Ok(encode_reply(&MultisigReply::Vault(
                merged.get(&vault_id).map(|v| Self::view_of(&vault_id, v)),
            ))),
            MultisigQuery::Proposals { vault_id } => {
                let views = merged
                    .get(&vault_id)
                    .map(|v| {
                        v.proposals
                            .iter()
                            .map(|(h, p)| Self::proposal_view(&vault_id, v, h, p))
                            .collect()
                    })
                    .unwrap_or_default();
                Ok(encode_reply(&MultisigReply::Proposals(views)))
            }
            MultisigQuery::Executable { vault_id } => {
                let views = merged
                    .get(&vault_id)
                    .map(|v| {
                        v.proposals
                            .iter()
                            .filter(|(_, p)| {
                                p.executed.is_none()
                                    && p.approvals.len() >= usize::from(v.threshold)
                            })
                            .map(|(h, p)| Self::executable_of(&vault_id, v, h, p))
                            .collect()
                    })
                    .unwrap_or_default();
                Ok(encode_reply(&MultisigReply::Executable(views)))
            }
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

fn emit_executable(
    ctx: &mut dyn Ctx,
    id: &ModuleId,
    vault_id: &str,
    vault: &Vault,
    hash: &B256,
    proposal: &Proposal,
) {
    ctx.emit_event(Event {
        source: id.clone(),
        payload: encode_event(&MultisigEvent::Executable(Multisig::executable_of(
            vault_id, vault, hash, proposal,
        ))),
    });
}

// ---- byte-field validation (untrusted op bytes) -----------------------------

fn address_of(bytes: &[u8]) -> Result<Address, Error> {
    let raw: [u8; 20] = bytes
        .try_into()
        .map_err(|_| Error::Module(format!("multisig: address must be 20 bytes, got {}", bytes.len())))?;
    Ok(Address::from(raw))
}

fn b256_of(bytes: &[u8]) -> Result<B256, Error> {
    let raw: [u8; 32] = bytes
        .try_into()
        .map_err(|_| Error::Module(format!("multisig: hash must be 32 bytes, got {}", bytes.len())))?;
    Ok(B256::from(raw))
}

fn u256_of(bytes: &[u8]) -> Result<U256, Error> {
    let raw: [u8; 32] = bytes.try_into().map_err(|_| {
        Error::Module(format!(
            "multisig: uint256 must be 32 bytes, got {}",
            bytes.len()
        ))
    })?;
    Ok(U256::from_be_bytes(raw))
}

fn signature_of(bytes: &[u8]) -> Result<[u8; 65], Error> {
    bytes.try_into().map_err(|_| {
        Error::Module(format!(
            "multisig: signature must be 65 bytes, got {}",
            bytes.len()
        ))
    })
}

/// Recover the signer of a module preimage (registration, binding). The
/// preimage is keccak-hashed here so the signer signs a fixed 32-byte digest,
/// exactly as for a SafeTx hash — one signing shape across the module.
fn recover_prehashed(preimage: &[u8], signature: &[u8]) -> Result<Address, Error> {
    let sig = signature_of(signature)?;
    recover_owner(alloy_primitives::keccak256(preimage), &sig)
        .map_err(|e| Error::Module(format!("multisig: {e}")))
}

fn push_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(bytes);
}

// ---- strict snapshot decode (untrusted bytes) -------------------------------

fn take_u64(buf: &mut &[u8]) -> Result<u64, Error> {
    let Some((head, rest)) = buf.split_first_chunk::<8>() else {
        return Err(Error::Module("multisig: snapshot truncated".into()));
    };
    *buf = rest;
    Ok(u64::from_le_bytes(*head))
}

fn take_u8(buf: &mut &[u8]) -> Result<u8, Error> {
    let Some((head, rest)) = buf.split_first() else {
        return Err(Error::Module("multisig: snapshot truncated".into()));
    };
    *buf = rest;
    Ok(*head)
}

fn take_array<const N: usize>(buf: &mut &[u8]) -> Result<[u8; N], Error> {
    let Some((head, rest)) = buf.split_first_chunk::<N>() else {
        return Err(Error::Module("multisig: snapshot truncated".into()));
    };
    *buf = rest;
    Ok(*head)
}

fn take_vec(buf: &mut &[u8]) -> Result<Vec<u8>, Error> {
    let len = take_u64(buf)? as usize;
    if buf.len() < len {
        return Err(Error::Module("multisig: snapshot truncated".into()));
    }
    let (head, rest) = buf.split_at(len);
    *buf = rest;
    Ok(head.to_vec())
}

fn take_string(buf: &mut &[u8]) -> Result<String, Error> {
    String::from_utf8(take_vec(buf)?)
        .map_err(|_| Error::Module("multisig: snapshot holds invalid utf-8".into()))
}

fn take_bool(buf: &mut &[u8]) -> Result<bool, Error> {
    match take_u8(buf)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(Error::Module("multisig: snapshot holds a non-boolean".into())),
    }
}

fn decode_state(bytes: &[u8]) -> Result<BTreeMap<String, Vault>, Error> {
    let mut buf = bytes;
    let count = take_u64(&mut buf)?;
    let mut vaults = BTreeMap::new();
    for _ in 0..count {
        let id = take_string(&mut buf)?;
        let chain_id = take_u64(&mut buf)?;
        let safe_address = Address::from(take_array::<20>(&mut buf)?);
        let owner_count = take_u64(&mut buf)?;
        if owner_count as usize > MAX_OWNERS {
            return Err(Error::Module("multisig: snapshot owner count exceeds the cap".into()));
        }
        let mut owners = BTreeSet::new();
        for _ in 0..owner_count {
            owners.insert(Address::from(take_array::<20>(&mut buf)?));
        }
        let threshold = take_u8(&mut buf)?;
        let backend = match take_u8(&mut buf)? {
            0 => Backend::Safe,
            1 => Backend::ThresholdEcdsa,
            other => {
                return Err(Error::Module(format!(
                    "multisig: snapshot holds an unknown backend tag {other}"
                )));
            }
        };
        let chain_nonce = take_u64(&mut buf)?;
        let drifted = take_bool(&mut buf)?;
        let created_at = take_u64(&mut buf)?;

        let proposal_count = take_u64(&mut buf)?;
        let mut proposals = BTreeMap::new();
        for _ in 0..proposal_count {
            let hash = B256::from(take_array::<32>(&mut buf)?);
            let nonce = take_u64(&mut buf)?;
            let to = Address::from(take_array::<20>(&mut buf)?);
            let value = U256::from_be_bytes(take_array::<32>(&mut buf)?);
            let data = take_vec(&mut buf)?;
            if data.len() > MAX_DATA_LEN {
                return Err(Error::Module(
                    "multisig: snapshot proposal exceeds the calldata ceiling".into(),
                ));
            }
            let proposer = Address::from(take_array::<20>(&mut buf)?);
            let approval_count = take_u64(&mut buf)?;
            if approval_count as usize > MAX_OWNERS {
                return Err(Error::Module(
                    "multisig: snapshot approval count exceeds the owner cap".into(),
                ));
            }
            let mut approvals = BTreeMap::new();
            for _ in 0..approval_count {
                let owner = Address::from(take_array::<20>(&mut buf)?);
                let sig = take_vec(&mut buf)?;
                if sig.len() != 65 {
                    return Err(Error::Module(
                        "multisig: snapshot holds a malformed signature".into(),
                    ));
                }
                approvals.insert(owner, sig);
            }
            let executed = match take_u8(&mut buf)? {
                0 => None,
                1 => {
                    let chain_tx_hash = take_vec(&mut buf)?;
                    if chain_tx_hash.len() != 32 {
                        return Err(Error::Module(
                            "multisig: snapshot holds a malformed chain tx hash".into(),
                        ));
                    }
                    let success = take_bool(&mut buf)?;
                    Some(ExecutionView {
                        chain_tx_hash,
                        success,
                    })
                }
                _ => {
                    return Err(Error::Module(
                        "multisig: snapshot holds a non-boolean execution tag".into(),
                    ));
                }
            };
            let created_at = take_u64(&mut buf)?;
            proposals.insert(
                hash,
                Proposal {
                    nonce,
                    to,
                    value,
                    data,
                    proposer,
                    approvals,
                    executed,
                    created_at,
                },
            );
        }

        let binding_count = take_u64(&mut buf)?;
        let mut bindings = BTreeMap::new();
        for _ in 0..binding_count {
            let addr = Address::from(take_array::<20>(&mut buf)?);
            bindings.insert(addr, take_vec(&mut buf)?);
        }

        vaults.insert(
            id,
            Vault {
                chain_id,
                safe_address,
                owners,
                threshold,
                backend,
                chain_nonce,
                drifted,
                created_at,
                proposals,
                bindings,
            },
        );
    }
    if !buf.is_empty() {
        return Err(Error::Module(
            "multisig: snapshot has trailing bytes".into(),
        ));
    }
    Ok(vaults)
}
