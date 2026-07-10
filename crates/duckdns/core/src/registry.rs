//! Pure replicated registry. Authorization facts are supplied by the adapter:
//! account ids for account operations and the authenticated provider node key.
//! Membership/identity lookups and final eligible-provider filtering stay out
//! of this SDK-free core.

use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};

use crate::codec::{Reader, push_bytes, push_string, push_u64};
use crate::{
    DuckDnsName, HandleRegistration, MAX_ANNOUNCEMENTS_PER_NODE, MAX_LABEL_LEN, MAX_QUERY_LIMIT,
    NODE_KEY_LEN, NodeRegistration, ResolvedNode, ResolvedService, ServiceAnnouncement,
    ServiceAuthority, ServiceIdentity, ServiceScope, derive_chain_label, node_label,
    validate_handle,
};

const MAX_ACCOUNT_ID_LEN: usize = 1024;
const STATE_FORMAT_VERSION: u8 = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
struct StoredNodeRegistration {
    account_id: Option<Vec<u8>>,
    announcements: BTreeSet<ServiceAnnouncement>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct State {
    /// Human aliases are indexed by handle for deterministic lookup. State
    /// validation enforces the inverse one-handle-per-account invariant.
    handles: BTreeMap<String, Vec<u8>>,
    nodes: BTreeMap<Vec<u8>, StoredNodeRegistration>,
}

/// SDK-free DuckDNS state machine with block staging.
pub struct Registry {
    chain_label: String,
    committed: State,
    pending: Option<State>,
}

impl Registry {
    pub fn new(chain_id: &str) -> Result<Self, String> {
        Ok(Self {
            chain_label: derive_chain_label(chain_id)?,
            committed: State::default(),
            pending: None,
        })
    }

    pub fn chain_label(&self) -> &str {
        &self.chain_label
    }

    fn effective(&self) -> &State {
        self.pending.as_ref().unwrap_or(&self.committed)
    }

    pub fn handle_owner(&self, handle: &str) -> Option<&[u8]> {
        self.effective().handles.get(handle).map(Vec::as_slice)
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

    pub fn node_registration(&self, node: &[u8]) -> Option<NodeRegistration> {
        self.effective()
            .nodes
            .get(node)
            .map(|registration| NodeRegistration {
                account_id: registration.account_id.clone(),
                announcements: registration.announcements.iter().cloned().collect(),
            })
    }

    /// Declaratively replace one account's optional handle. Renames are atomic,
    /// and unregistering a handle deliberately leaves service declarations
    /// intact because those declarations are owned by AccountId, not aliases.
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

    /// Replace one authenticated node's full declaration set. `account` is
    /// required exactly when the replacement contains account-scoped services.
    pub fn replace_announcements(
        &mut self,
        node: &[u8],
        account: Option<&[u8]>,
        announcements: Vec<ServiceAnnouncement>,
    ) -> Result<(), String> {
        validate_node(node)?;
        if announcements.len() > MAX_ANNOUNCEMENTS_PER_NODE {
            return Err(format!(
                "duckdns: {} announcements exceed the {MAX_ANNOUNCEMENTS_PER_NODE} per-node cap",
                announcements.len()
            ));
        }
        for announcement in &announcements {
            announcement.validate()?;
        }
        let replacement: BTreeSet<_> = announcements.into_iter().collect();
        let needs_account = replacement
            .iter()
            .any(|announcement| announcement.scope == ServiceScope::Account);
        let account_id = if needs_account {
            let account = account.ok_or_else(|| {
                "duckdns: account announcements require a bound account".to_string()
            })?;
            validate_account(account)?;
            Some(account.to_vec())
        } else {
            None
        };

        let mut next = self.effective().clone();
        if replacement.is_empty() {
            next.nodes.remove(node);
        } else {
            next.nodes.insert(
                node.to_vec(),
                StoredNodeRegistration {
                    account_id,
                    announcements: replacement,
                },
            );
        }
        validate_state(&next)?;
        self.pending = Some(next);
        Ok(())
    }

    /// Resolve against replicated declarations only. The system adapter must
    /// remove providers lacking current standing and, for account services,
    /// nodes no longer bound to the captured account.
    pub fn resolve_service(&self, name: &DuckDnsName) -> Result<Option<ResolvedService>, String> {
        name.validate()?;
        let Some((identity, authority)) = self.binding_for_name(name) else {
            return Ok(None);
        };
        let requested_node = match name {
            DuckDnsName::NodeService { node, .. } => Some(node),
            _ => None,
        };
        let account_owner = match &authority {
            ServiceAuthority::Account { account_id } => Some(account_id.as_slice()),
            ServiceAuthority::Network => None,
        };
        let mut providers = Vec::new();
        for (node, registration) in &self.effective().nodes {
            if account_owner.is_some_and(|owner| registration.account_id.as_deref() != Some(owner))
            {
                continue;
            }
            if !registration.announcements.iter().any(|announcement| {
                announcement.scope == identity.scope && announcement.service == identity.service
            }) {
                continue;
            }
            let label = node_label(node)?;
            if requested_node.is_some_and(|requested| requested != &label) {
                continue;
            }
            providers.push(ResolvedNode {
                node: node.clone(),
                node_label: label,
            });
        }
        if providers.is_empty() {
            return Ok(None);
        }
        Ok(Some(ResolvedService {
            identity,
            authority,
            providers,
        }))
    }

    fn binding_for_name(&self, name: &DuckDnsName) -> Option<(ServiceIdentity, ServiceAuthority)> {
        match name {
            DuckDnsName::Account { .. } => None,
            DuckDnsName::AccountService { service, handle } => {
                let account_id = self.effective().handles.get(handle)?.clone();
                Some((
                    ServiceIdentity {
                        scope: ServiceScope::Account,
                        service: service.clone(),
                    },
                    ServiceAuthority::Account { account_id },
                ))
            }
            DuckDnsName::NetworkService { service, chain }
            | DuckDnsName::NodeService { service, chain, .. }
                if chain == &self.chain_label =>
            {
                Some((
                    ServiceIdentity {
                        scope: ServiceScope::Network,
                        service: service.clone(),
                    },
                    ServiceAuthority::Network,
                ))
            }
            DuckDnsName::NetworkService { .. } | DuckDnsName::NodeService { .. } => None,
        }
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

fn validate_node(node: &[u8]) -> Result<(), String> {
    if node.len() != NODE_KEY_LEN {
        return Err(format!(
            "duckdns: node key must be {NODE_KEY_LEN} bytes, got {}",
            node.len()
        ));
    }
    Ok(())
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
    for (node, registration) in &state.nodes {
        validate_node(node)?;
        if registration.announcements.is_empty() {
            return Err("duckdns: empty node announcement sets must be omitted".into());
        }
        if registration.announcements.len() > MAX_ANNOUNCEMENTS_PER_NODE {
            return Err(format!(
                "duckdns: node exceeds the {MAX_ANNOUNCEMENTS_PER_NODE} announcement cap"
            ));
        }
        let needs_account = registration
            .announcements
            .iter()
            .any(|announcement| announcement.scope == ServiceScope::Account);
        match (needs_account, &registration.account_id) {
            (true, Some(account)) => validate_account(account)?,
            (true, None) => {
                return Err("duckdns: account declarations must capture an account id".into());
            }
            (false, Some(_)) => {
                return Err(
                    "duckdns: network-only declarations must not capture an account id".into(),
                );
            }
            (false, None) => {}
        }
        for announcement in &registration.announcements {
            announcement.validate()?;
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
    push_u64(&mut out, state.nodes.len());
    for (node, registration) in &state.nodes {
        push_bytes(&mut out, node);
        match &registration.account_id {
            None => out.push(0),
            Some(account_id) => {
                out.push(1);
                push_bytes(&mut out, account_id);
            }
        }
        push_u64(&mut out, registration.announcements.len());
        for announcement in &registration.announcements {
            out.push(match announcement.scope {
                ServiceScope::Account => 0,
                ServiceScope::Network => 1,
            });
            push_string(&mut out, &announcement.service);
        }
    }
    out
}

fn decode_state(bytes: &[u8]) -> Result<State, String> {
    let mut reader = Reader::new(bytes);
    let version = reader.u8()?;
    if version != STATE_FORMAT_VERSION {
        return Err(format!(
            "duckdns: unsupported registry snapshot version {version}"
        ));
    }
    let handle_count = reader.count("handle", 16)?;
    let mut handles = BTreeMap::new();
    let mut previous_handle: Option<String> = None;
    for _ in 0..handle_count {
        let handle = reader.string(MAX_LABEL_LEN, "handle")?;
        if previous_handle.as_ref().is_some_and(|old| old >= &handle) {
            return Err("duckdns: registry snapshot handles are not strictly increasing".into());
        }
        let owner = reader.bytes(MAX_ACCOUNT_ID_LEN, "account id")?;
        previous_handle = Some(handle.clone());
        handles.insert(handle, owner);
    }

    let node_count = reader.count("node", 16)?;
    let mut nodes = BTreeMap::new();
    let mut previous_node: Option<Vec<u8>> = None;
    for _ in 0..node_count {
        let node = reader.bytes(NODE_KEY_LEN, "node key")?;
        validate_node(&node)?;
        if previous_node.as_ref().is_some_and(|old| old >= &node) {
            return Err("duckdns: registry snapshot node keys are not strictly increasing".into());
        }
        let account_id = match reader.u8()? {
            0 => None,
            1 => Some(reader.bytes(MAX_ACCOUNT_ID_LEN, "account id")?),
            tag => {
                return Err(format!(
                    "duckdns: registry snapshot has unknown account binding tag {tag}"
                ));
            }
        };
        let count = reader.count("announcement", 4)?;
        if count == 0 || count > MAX_ANNOUNCEMENTS_PER_NODE {
            return Err(format!(
                "duckdns: registry snapshot announcement count is outside 1..={MAX_ANNOUNCEMENTS_PER_NODE}"
            ));
        }
        let mut announcements = BTreeSet::new();
        let mut previous: Option<ServiceAnnouncement> = None;
        for _ in 0..count {
            let scope = match reader.u8()? {
                0 => ServiceScope::Account,
                1 => ServiceScope::Network,
                tag => {
                    return Err(format!(
                        "duckdns: registry snapshot has unknown scope tag {tag}"
                    ));
                }
            };
            let announcement = ServiceAnnouncement {
                scope,
                service: reader.string(MAX_LABEL_LEN, "service")?,
            };
            if previous.as_ref().is_some_and(|old| old >= &announcement) {
                return Err(
                    "duckdns: registry snapshot announcements are not strictly increasing".into(),
                );
            }
            previous = Some(announcement.clone());
            announcements.insert(announcement);
        }
        previous_node = Some(node.clone());
        nodes.insert(
            node,
            StoredNodeRegistration {
                account_id,
                announcements,
            },
        );
    }
    reader.finish()?;
    let state = State { handles, nodes };
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
