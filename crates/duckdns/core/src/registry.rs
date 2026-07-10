//! Pure replicated account-name registry. The adapter supplies the AccountId
//! derived from the authenticated submitting node; this core owns no node or
//! service discovery state.

use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};

use crate::codec::{Reader, push_bytes, push_string, push_u64};
use crate::{
    DuckDnsName, HandleRegistration, MAX_LABEL_LEN, MAX_QUERY_LIMIT, ResolvedAccount,
    validate_handle,
};

const MAX_ACCOUNT_ID_LEN: usize = 1024;
/// Version 2 deliberately drops the v1 node/service declaration section.
const STATE_FORMAT_VERSION: u8 = 2;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct State {
    /// Human aliases indexed by handle. State validation enforces the inverse
    /// one-handle-per-account invariant without committing a duplicate index.
    handles: BTreeMap<String, Vec<u8>>,
}

/// SDK-free DuckDNS state machine with block staging.
pub struct Registry {
    committed: State,
    pending: Option<State>,
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

impl Registry {
    pub fn new() -> Self {
        Self {
            committed: State::default(),
            pending: None,
        }
    }

    fn effective(&self) -> &State {
        self.pending.as_ref().unwrap_or(&self.committed)
    }

    pub fn resolve(&self, name: &DuckDnsName) -> Result<Option<ResolvedAccount>, String> {
        name.validate()?;
        Ok(self
            .effective()
            .handles
            .get(&name.handle)
            .cloned()
            .map(|account_id| ResolvedAccount { account_id }))
    }

    pub fn registrations(&self, from: u64, limit: u64) -> Result<Vec<HandleRegistration>, String> {
        if limit > MAX_QUERY_LIMIT {
            return Err(format!(
                "duckdns: registration query limit {limit} exceeds {MAX_QUERY_LIMIT}"
            ));
        }
        let from = usize::try_from(from)
            .map_err(|_| "duckdns: registration query offset overflows usize".to_string())?;
        let limit =
            usize::try_from(limit).expect("MAX_QUERY_LIMIT always fits usize on supported targets");
        Ok(self
            .effective()
            .handles
            .iter()
            .skip(from)
            .take(limit)
            .map(|(handle, account_id)| HandleRegistration {
                handle: handle.clone(),
                account_id: account_id.clone(),
            })
            .collect())
    }

    /// Declaratively replace one account's optional handle. Renames are atomic;
    /// `None` unregisters the name without changing Identity.
    pub fn set_handle(&mut self, account: &[u8], handle: Option<String>) -> Result<(), String> {
        validate_account(account)?;
        if let Some(handle) = &handle {
            validate_handle(handle)?;
            if let Some(owner) = self.effective().handles.get(handle)
                && owner != account
            {
                return Err(format!(
                    "duckdns: handle {handle:?} is already claimed by another account"
                ));
            }
        }

        let current = self
            .effective()
            .handles
            .iter()
            .find_map(|(current, owner)| (owner == account).then_some(current));
        if current.map(String::as_str) == handle.as_deref() {
            return Ok(());
        }

        let mut next = self.effective().clone();
        next.handles.retain(|_, owner| owner != account);
        if let Some(handle) = handle {
            next.handles.insert(handle, account.to_vec());
        }
        validate_state(&next)?;
        self.pending = Some(next);
        Ok(())
    }

    pub fn commit(&mut self) {
        if let Some(next) = self.pending.take() {
            self.committed = next;
        }
    }

    pub fn abort(&mut self) {
        self.pending = None;
    }

    pub fn root_bytes(&self) -> [u8; 32] {
        root_of(&self.committed)
    }

    pub fn snapshot(&self) -> Vec<u8> {
        encode_state(&self.committed)
    }

    pub fn install(&mut self, bytes: &[u8], expected: [u8; 32]) -> Result<(), String> {
        let decoded = decode_state(bytes)?;
        let actual = root_of(&decoded);
        if actual != expected {
            return Err(format!(
                "duckdns: registry snapshot root mismatch: decoded {}, expected {}",
                hex(&actual),
                hex(&expected)
            ));
        }
        self.committed = decoded;
        self.pending = None;
        Ok(())
    }
}

fn validate_account(account: &[u8]) -> Result<(), String> {
    if account.is_empty() || account.len() > MAX_ACCOUNT_ID_LEN {
        return Err(format!(
            "duckdns: account id must be 1..={MAX_ACCOUNT_ID_LEN} bytes, got {}",
            account.len()
        ));
    }
    Ok(())
}

fn validate_state(state: &State) -> Result<(), String> {
    let mut accounts = BTreeSet::new();
    for (handle, owner) in &state.handles {
        validate_handle(handle)?;
        validate_account(owner)?;
        if !accounts.insert(owner) {
            return Err("duckdns: an account may register at most one handle".into());
        }
    }
    Ok(())
}

fn encode_state(state: &State) -> Vec<u8> {
    let mut out = vec![STATE_FORMAT_VERSION];
    push_u64(&mut out, state.handles.len());
    for (handle, owner) in &state.handles {
        push_string(&mut out, handle);
        push_bytes(&mut out, owner);
    }
    out
}

fn decode_state(bytes: &[u8]) -> Result<State, String> {
    let mut reader = Reader::new(bytes);
    let version = reader.u8()?;
    if version != STATE_FORMAT_VERSION {
        return Err(format!(
            "duckdns: unsupported naming snapshot version {version}"
        ));
    }
    let handle_count = reader.count("handle", 16)?;
    let mut handles = BTreeMap::new();
    let mut previous_handle: Option<String> = None;
    for _ in 0..handle_count {
        let handle = reader.string(MAX_LABEL_LEN, "handle")?;
        if previous_handle.as_ref().is_some_and(|old| old >= &handle) {
            return Err("duckdns: naming snapshot handles are not strictly increasing".into());
        }
        let owner = reader.bytes(MAX_ACCOUNT_ID_LEN, "account id")?;
        previous_handle = Some(handle.clone());
        handles.insert(handle, owner);
    }
    reader.finish()?;
    let state = State { handles };
    validate_state(&state)?;
    Ok(state)
}

fn root_of(state: &State) -> [u8; 32] {
    if state == &State::default() {
        [0; 32]
    } else {
        Sha256::digest(encode_state(state)).into()
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut out, "{byte:02x}").expect("writing to a String cannot fail");
    }
    out
}
