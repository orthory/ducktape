use std::collections::BTreeMap;

use sdk::codec::{Cursor, push_bytes, push_opt_str};
use sha2::{Digest, Sha256};

use crate::{
    CredentialRecord, MAX_CREDENTIAL_GRANTS, MAX_ROUTE_LABEL_BYTES, MAX_ROUTE_STATEMENT_JSON_BYTES,
    MAX_ROUTES_PER_ACCOUNT, RouteName, RouteRecord, RouteSummary, validate_account_id,
    validate_authorization, validate_credential_name, validate_route_statement,
};

const MAX_RECORD_BYTES: usize = MAX_ROUTE_STATEMENT_JSON_BYTES + 1024;
/// A credential record is scalar metadata plus a bounded grant set; each grant
/// is at most [`crate::MAX_ACCOUNT_ID_BYTES`]. This caps its JSON on decode.
const MAX_CREDENTIAL_RECORD_BYTES: usize = 8 * 1024;

type RouteKey = (Vec<u8>, RouteName);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct State {
    routes: BTreeMap<RouteKey, RouteRecord>,
    credentials: BTreeMap<String, CredentialRecord>,
}

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

    /// Clone committed state once per block, then stage later writes in place.
    fn pending_mut(&mut self) -> &mut State {
        if self.pending.is_none() {
            self.pending = Some(self.committed.clone());
        }
        self.pending
            .as_mut()
            .expect("pending state was initialized")
    }

    pub fn route(
        &self,
        account_id: &[u8],
        name: &RouteName,
    ) -> Result<Option<RouteRecord>, String> {
        validate_account_id(account_id)?;
        name.validate()?;
        Ok(self
            .effective()
            .routes
            .get(&(account_id.to_vec(), name.clone()))
            .cloned())
    }

    pub fn routes(&self, account_id: &[u8]) -> Result<Vec<RouteSummary>, String> {
        validate_account_id(account_id)?;
        Ok(self
            .effective()
            .routes
            .range((account_id.to_vec(), RouteName::apex())..)
            .take_while(|((owner, _), _)| owner.as_slice() == account_id)
            .filter_map(|(_, record)| {
                let route = record.statement.route.as_ref()?;
                Some(RouteSummary {
                    name: record.statement.name.clone(),
                    publisher_node: record.statement.publisher_node.clone(),
                    revision: record.statement.revision,
                    target: route.target.kind_name().to_string(),
                })
            })
            .collect())
    }

    pub fn set_route(&mut self, record: RouteRecord) -> Result<(), String> {
        validate_record(&record)?;
        let key = (
            record.statement.account_id.clone(),
            record.statement.name.clone(),
        );
        let existing = self.effective().routes.get(&key);
        if existing.is_none()
            && self
                .effective()
                .routes
                .keys()
                .filter(|(account_id, _)| account_id == &key.0)
                .count()
                >= MAX_ROUTES_PER_ACCOUNT
        {
            return Err(format!(
                "gateway: account route count exceeds {MAX_ROUTES_PER_ACCOUNT}"
            ));
        }
        let expected = match existing {
            None => 1,
            Some(current) => current
                .statement
                .revision
                .checked_add(1)
                .ok_or_else(|| "gateway: route revision is exhausted".to_string())?,
        };
        if record.statement.revision != expected {
            return Err(format!(
                "gateway: route revision must be {expected}, got {}",
                record.statement.revision
            ));
        }
        self.pending_mut().routes.insert(key, record);
        Ok(())
    }

    pub fn credential(&self, name: &str) -> Result<Option<CredentialRecord>, String> {
        validate_credential_name(name)?;
        Ok(self.effective().credentials.get(name).cloned())
    }

    pub fn credentials(&self) -> Vec<CredentialRecord> {
        self.effective().credentials.values().cloned().collect()
    }

    /// Register or replace a credential record. First registration in consensus
    /// order wins the name: a record whose owner differs from the committed one
    /// is refused. Grants MUST arrive empty here (managed by grant/revoke).
    pub fn set_credential(&mut self, record: CredentialRecord) -> Result<(), String> {
        validate_credential_name(&record.name)?;
        validate_account_id(&record.owner_account)?;
        if !record.grants.is_empty() {
            return Err("gateway: credential registration carries no grants".into());
        }
        let taken_by_other = self
            .effective()
            .credentials
            .get(&record.name)
            .is_some_and(|existing| existing.owner_account != record.owner_account);
        if taken_by_other {
            return Err("gateway: credential name already registered".into());
        }
        self.pending_mut()
            .credentials
            .insert(record.name.clone(), record);
        Ok(())
    }

    /// Load an owner's record for a grant/revoke/remove mutation, refusing when
    /// the name is unknown or owned by a different account.
    fn owned_credential(
        &self,
        name: &str,
        owner_account: &[u8],
    ) -> Result<CredentialRecord, String> {
        let record = self
            .credential(name)?
            .ok_or_else(|| "gateway: credential is not registered".to_string())?;
        if record.owner_account != owner_account {
            return Err("gateway: credential is owned by another account".into());
        }
        Ok(record)
    }

    pub fn remove_credential(&mut self, name: &str, owner_account: &[u8]) -> Result<(), String> {
        self.owned_credential(name, owner_account)?;
        self.pending_mut().credentials.remove(name);
        Ok(())
    }

    pub fn grant_credential(
        &mut self,
        name: &str,
        owner_account: &[u8],
        account: Vec<u8>,
    ) -> Result<(), String> {
        validate_account_id(&account)?;
        let mut record = self.owned_credential(name, owner_account)?;
        let is_new_grant = record.grants.insert(account);
        let over_cap = is_new_grant && record.grants.len() > MAX_CREDENTIAL_GRANTS;
        if over_cap {
            return Err(format!(
                "gateway: credential grant count exceeds {MAX_CREDENTIAL_GRANTS}"
            ));
        }
        self.pending_mut()
            .credentials
            .insert(name.to_string(), record);
        Ok(())
    }

    pub fn revoke_credential(
        &mut self,
        name: &str,
        owner_account: &[u8],
        account: &[u8],
    ) -> Result<(), String> {
        let mut record = self.owned_credential(name, owner_account)?;
        record.grants.remove(account);
        self.pending_mut()
            .credentials
            .insert(name.to_string(), record);
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

    /// Decode a canonical snapshot and adopt it as committed WITHOUT a root
    /// check. The merged gateway module verifies the COMBINED root once, over
    /// both planes, so the per-plane check is the merged module's job.
    pub fn adopt(&mut self, bytes: &[u8]) -> Result<(), String> {
        let decoded = decode_state(bytes)?;
        if encode_state(&decoded) != bytes {
            return Err("gateway: snapshot is not canonical".into());
        }
        self.committed = decoded;
        self.pending = None;
        Ok(())
    }

    pub fn install(&mut self, bytes: &[u8], expected: [u8; 32]) -> Result<(), String> {
        self.adopt(bytes)?;
        if root_of(&self.committed) != expected {
            return Err("gateway: snapshot root mismatch".into());
        }
        Ok(())
    }
}

fn validate_record(record: &RouteRecord) -> Result<(), String> {
    validate_route_statement(&record.statement)?;
    validate_authorization(&record.authorization)
}

fn encode_state(state: &State) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(state.routes.len() as u64).to_le_bytes());
    for ((account_id, name), record) in &state.routes {
        push_bytes(&mut out, account_id);
        push_opt_str(&mut out, name.label.as_deref());
        push_bytes(
            &mut out,
            &serde_json::to_vec(record).expect("route record serializes"),
        );
    }
    out.extend_from_slice(&(state.credentials.len() as u64).to_le_bytes());
    for (name, record) in &state.credentials {
        push_bytes(&mut out, name.as_bytes());
        push_bytes(
            &mut out,
            &serde_json::to_vec(record).expect("credential record serializes"),
        );
    }
    out
}

fn decode_state(bytes: &[u8]) -> Result<State, String> {
    let mut reader = Cursor::new(bytes);
    let count = reader.u64("snapshot route count").map_err(codec_err)?;
    // Each entry carries at least an account length + one account byte, a name
    // flag, and a record length: 18 bytes.
    reader
        .bound(count, 18, "snapshot route count")
        .map_err(codec_err)?;
    let mut routes = BTreeMap::new();
    let mut previous: Option<RouteKey> = None;
    let mut per_account: BTreeMap<Vec<u8>, usize> = BTreeMap::new();
    for _ in 0..count {
        let account_id = reader
            .bytes("snapshot account id")
            .map_err(codec_err)?
            .to_vec();
        validate_account_id(&account_id)?;
        let name = match reader
            .opt_str(MAX_ROUTE_LABEL_BYTES, "snapshot route label")
            .map_err(codec_err)?
        {
            None => RouteName::apex(),
            Some(label) => RouteName::named(label),
        };
        name.validate()?;
        let key = (account_id.clone(), name);
        if previous.as_ref().is_some_and(|old| old >= &key) {
            return Err("gateway: snapshot keys are not strictly increasing".into());
        }
        let account_count = per_account.entry(account_id).or_default();
        *account_count += 1;
        if *account_count > MAX_ROUTES_PER_ACCOUNT {
            return Err(format!(
                "gateway: snapshot account exceeds {MAX_ROUTES_PER_ACCOUNT} routes"
            ));
        }
        let record_bytes = reader.bytes("snapshot route record").map_err(codec_err)?;
        if record_bytes.len() > MAX_RECORD_BYTES {
            return Err(format!(
                "gateway: snapshot route record exceeds {MAX_RECORD_BYTES} bytes"
            ));
        }
        let record: RouteRecord = serde_json::from_slice(record_bytes)
            .map_err(|error| format!("gateway: route record: {error}"))?;
        validate_record(&record)?;
        if record.statement.account_id != key.0 || record.statement.name != key.1 {
            return Err("gateway: snapshot key does not match its record".into());
        }
        previous = Some(key.clone());
        routes.insert(key, record);
    }
    let credentials = decode_credentials(&mut reader)?;
    reader.finish("snapshot").map_err(codec_err)?;
    Ok(State {
        routes,
        credentials,
    })
}

fn decode_credentials(reader: &mut Cursor) -> Result<BTreeMap<String, CredentialRecord>, String> {
    let count = reader.u64("snapshot credential count").map_err(codec_err)?;
    // Each entry carries at least a name length + one name byte and a record
    // length: 10 bytes.
    reader
        .bound(count, 10, "snapshot credential count")
        .map_err(codec_err)?;
    let mut credentials = BTreeMap::new();
    let mut previous: Option<String> = None;
    for _ in 0..count {
        let name = reader
            .string("snapshot credential name")
            .map_err(codec_err)?;
        validate_credential_name(&name)?;
        if previous.as_ref().is_some_and(|old| old >= &name) {
            return Err("gateway: snapshot credential names are not strictly increasing".into());
        }
        let record_bytes = reader
            .bytes("snapshot credential record")
            .map_err(codec_err)?;
        if record_bytes.len() > MAX_CREDENTIAL_RECORD_BYTES {
            return Err(format!(
                "gateway: snapshot credential record exceeds {MAX_CREDENTIAL_RECORD_BYTES} bytes"
            ));
        }
        let record: CredentialRecord = serde_json::from_slice(record_bytes)
            .map_err(|error| format!("gateway: credential record: {error}"))?;
        if record.name != name {
            return Err("gateway: snapshot credential key does not match its record".into());
        }
        validate_credential_name(&record.name)?;
        validate_account_id(&record.owner_account)?;
        if record.grants.len() > MAX_CREDENTIAL_GRANTS {
            return Err(format!(
                "gateway: snapshot credential exceeds {MAX_CREDENTIAL_GRANTS} grants"
            ));
        }
        previous = Some(name.clone());
        credentials.insert(name, record);
    }
    Ok(credentials)
}

/// [`Cursor`] reports through [`sdk::Error`]; this crate speaks `String`.
fn codec_err(error: sdk::Error) -> String {
    match error {
        sdk::Error::Module(message) => format!("gateway: {message}"),
        other => format!("gateway: {other}"),
    }
}

fn root_of(state: &State) -> [u8; 32] {
    if state == &State::default() {
        [0; 32]
    } else {
        Sha256::digest(encode_state(state)).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        MemberAuthorization, RouteAudience, RouteDefinition, RouteMethod, RoutePolicy,
        RouteStatement, RouteTarget,
    };

    fn record(name: RouteName, revision: u64) -> RouteRecord {
        RouteRecord {
            statement: RouteStatement {
                version: 1,
                chain_id: "test".into(),
                account_id: vec![1; 32],
                name,
                publisher_node: vec![2; 32],
                revision,
                route: Some(RouteDefinition {
                    target: RouteTarget::LoopbackHttp,
                    policy: RoutePolicy {
                        audience: RouteAudience::Owner,
                        methods: vec![RouteMethod::Get],
                        max_request_bytes: 0,
                        max_response_bytes: 1024,
                        allow_authorization: false,
                        allow_upgrade: false,
                    },
                }),
            },
            authorization: MemberAuthorization {
                signer: vec![3; 32],
                signature: vec![4; 64],
            },
        }
    }

    #[test]
    fn one_revision_stream_covers_apex_and_named_providers() {
        let mut registry = Registry::new();
        registry.set_route(record(RouteName::apex(), 1)).unwrap();
        registry
            .set_route(record(RouteName::named("api"), 1))
            .unwrap();
        registry.commit();
        assert!(
            registry
                .route(&[1; 32], &RouteName::apex())
                .unwrap()
                .is_some()
        );
        assert!(
            registry
                .route(&[1; 32], &RouteName::named("api"))
                .unwrap()
                .is_some()
        );

        let mut replacement = record(RouteName::apex(), 2);
        replacement.statement.route = None;
        registry.set_route(replacement).unwrap();
        registry.commit();
        assert!(
            registry
                .route(&[1; 32], &RouteName::apex())
                .unwrap()
                .unwrap()
                .statement
                .route
                .is_none()
        );
        let live = registry.routes(&[1; 32]).unwrap();
        assert_eq!(live.len(), 1);
        assert_eq!(live[0].name, RouteName::named("api"));
        assert_eq!(live[0].target, "loopback_http");
    }

    #[test]
    fn canonical_snapshot_round_trips_and_rejects_trailing_bytes() {
        let mut registry = Registry::new();
        registry
            .set_route(record(RouteName::named("api"), 1))
            .unwrap();
        registry.commit();
        let snapshot = registry.snapshot();
        let root = registry.root_bytes();
        let mut restored = Registry::new();
        restored.install(&snapshot, root).unwrap();
        assert_eq!(restored.root_bytes(), root);
        let mut trailing = snapshot;
        trailing.push(0);
        assert!(restored.install(&trailing, root).is_err());
    }

    #[test]
    fn exhausted_revision_fails_closed() {
        let key = (vec![1; 32], RouteName::apex());
        let mut registry = Registry::new();
        registry
            .committed
            .routes
            .insert(key.clone(), record(RouteName::apex(), u64::MAX));
        let replacement = registry.committed.routes[&key].clone();
        assert!(
            registry
                .set_route(replacement)
                .unwrap_err()
                .contains("exhausted")
        );
    }
}
