//! Pure replicated registry. Authorization facts are supplied by the adapter:
//! account ids for account operations and the authenticated provider node key.
//! Membership/identity lookups and final eligible-provider filtering stay out
//! of this SDK-free core.

use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};

use crate::codec::{Reader, push_bytes, push_string, push_u64};
use crate::{
    DuckDnsName, MAX_ANNOUNCEMENTS_PER_NODE, MAX_LABEL_LEN, NODE_KEY_LEN, ResolvedNode,
    ResolvedService, ServiceAnnouncement, ServiceAuthority, ServiceIdentity, ServiceScope,
    derive_chain_label, node_label, validate_handle,
};

const MAX_ACCOUNT_ID_LEN: usize = 1024;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct State {
    handles: BTreeMap<String, Vec<u8>>,
    announcements: BTreeMap<Vec<u8>, BTreeSet<ServiceAnnouncement>>,
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

    pub fn node_announcements(&self, node: &[u8]) -> Vec<ServiceAnnouncement> {
        self.effective()
            .announcements
            .get(node)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn claim_handle(&mut self, account: &[u8], handle: String) -> Result<(), String> {
        validate_account(account)?;
        validate_handle(&handle)?;
        let mut next = self.effective().clone();
        match next.handles.get(&handle) {
            Some(owner) if owner != account => {
                return Err(format!(
                    "duckdns: handle {handle:?} is already claimed by another account"
                ));
            }
            Some(_) => return Ok(()),
            None => {
                next.handles.insert(handle, account.to_vec());
            }
        }
        validate_state(&next)?;
        self.pending = Some(next);
        Ok(())
    }

    /// Release a handle and every declaration beneath it. The cleanup spans all
    /// of the owning account's nodes so no unresolvable stale records remain.
    pub fn release_handle(&mut self, account: &[u8], handle: &str) -> Result<(), String> {
        validate_account(account)?;
        validate_handle(handle)?;
        let mut next = self.effective().clone();
        match next.handles.get(handle) {
            Some(owner) if owner == account => {}
            Some(_) => {
                return Err(format!(
                    "duckdns: handle {handle:?} belongs to another account"
                ));
            }
            None => return Err(format!("duckdns: handle {handle:?} is not claimed")),
        }
        next.handles.remove(handle);
        next.announcements.retain(|_, announcements| {
            announcements.retain(|announcement| {
                !matches!(
                    &announcement.scope,
                    ServiceScope::Account { handle: announced } if announced == handle
                )
            });
            !announcements.is_empty()
        });
        validate_state(&next)?;
        self.pending = Some(next);
        Ok(())
    }

    /// Replace one authenticated node's full declaration set. `account` is
    /// required only when the replacement contains account-scoped services.
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

        let account_handles: BTreeSet<&str> = replacement
            .iter()
            .filter_map(|announcement| match &announcement.scope {
                ServiceScope::Account { handle } => Some(handle.as_str()),
                ServiceScope::Network => None,
            })
            .collect();
        if !account_handles.is_empty() {
            let account = account.ok_or_else(|| {
                "duckdns: account announcements require a bound account".to_string()
            })?;
            validate_account(account)?;
            for handle in account_handles {
                match self.effective().handles.get(handle) {
                    Some(owner) if owner == account => {}
                    Some(_) => {
                        return Err(format!("duckdns: account does not own handle {handle:?}"));
                    }
                    None => return Err(format!("duckdns: handle {handle:?} is not claimed")),
                }
            }
        }

        let mut next = self.effective().clone();
        if replacement.is_empty() {
            next.announcements.remove(node);
        } else {
            next.announcements.insert(node.to_vec(), replacement);
        }
        validate_state(&next)?;
        self.pending = Some(next);
        Ok(())
    }

    /// Resolve against replicated declarations only. The system adapter must
    /// remove providers lacking current standing and, for account services, nodes
    /// no longer bound to the owning account.
    pub fn resolve_service(&self, name: &DuckDnsName) -> Result<Option<ResolvedService>, String> {
        name.validate()?;
        let Some((identity, authority)) = self.binding_for_name(name) else {
            return Ok(None);
        };
        let requested_node = match name {
            DuckDnsName::NodeService { node, .. } => Some(node),
            _ => None,
        };
        let mut providers = Vec::new();
        for (node, announcements) in &self.effective().announcements {
            if !announcements.iter().any(|announcement| {
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
                        scope: ServiceScope::Account {
                            handle: handle.clone(),
                        },
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
    for (handle, owner) in &state.handles {
        validate_handle(handle)?;
        validate_account(owner)?;
    }
    for (node, announcements) in &state.announcements {
        validate_node(node)?;
        if announcements.is_empty() {
            return Err("duckdns: empty node announcement sets must be omitted".into());
        }
        if announcements.len() > MAX_ANNOUNCEMENTS_PER_NODE {
            return Err(format!(
                "duckdns: node exceeds the {MAX_ANNOUNCEMENTS_PER_NODE} announcement cap"
            ));
        }
        for announcement in announcements {
            announcement.validate()?;
            if let ServiceScope::Account { handle } = &announcement.scope
                && !state.handles.contains_key(handle)
            {
                return Err(format!(
                    "duckdns: account announcement refers to unclaimed handle {handle:?}"
                ));
            }
        }
    }
    Ok(())
}

fn encode_state(state: &State) -> Vec<u8> {
    let mut out = Vec::new();
    push_u64(&mut out, state.handles.len());
    for (handle, owner) in &state.handles {
        push_string(&mut out, handle);
        push_bytes(&mut out, owner);
    }
    push_u64(&mut out, state.announcements.len());
    for (node, announcements) in &state.announcements {
        push_bytes(&mut out, node);
        push_u64(&mut out, announcements.len());
        for announcement in announcements {
            match &announcement.scope {
                ServiceScope::Account { handle } => {
                    out.push(0);
                    push_string(&mut out, handle);
                }
                ServiceScope::Network => out.push(1),
            }
            push_string(&mut out, &announcement.service);
        }
    }
    out
}

fn decode_state(bytes: &[u8]) -> Result<State, String> {
    let mut reader = Reader::new(bytes);
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
    let mut announcements = BTreeMap::new();
    let mut previous_node: Option<Vec<u8>> = None;
    for _ in 0..node_count {
        let node = reader.bytes(NODE_KEY_LEN, "node key")?;
        validate_node(&node)?;
        if previous_node.as_ref().is_some_and(|old| old >= &node) {
            return Err("duckdns: registry snapshot node keys are not strictly increasing".into());
        }
        let count = reader.count("announcement", 4)?;
        if count == 0 || count > MAX_ANNOUNCEMENTS_PER_NODE {
            return Err(format!(
                "duckdns: registry snapshot announcement count is outside 1..={MAX_ANNOUNCEMENTS_PER_NODE}"
            ));
        }
        let mut set = BTreeSet::new();
        let mut previous: Option<ServiceAnnouncement> = None;
        for _ in 0..count {
            let scope = match reader.u8()? {
                0 => ServiceScope::Account {
                    handle: reader.string(MAX_LABEL_LEN, "handle")?,
                },
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
            set.insert(announcement);
        }
        previous_node = Some(node.clone());
        announcements.insert(node, set);
    }
    reader.finish()?;
    let state = State {
        handles,
        announcements,
    };
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
