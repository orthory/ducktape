use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

use crate::{
    MAX_ACCOUNT_ID_BYTES, MAX_ROUTE_LABEL_BYTES, MAX_ROUTE_STATEMENT_JSON_BYTES,
    MAX_ROUTES_PER_ACCOUNT, RouteName, RouteRecord, RouteSummary, RouteTarget, RouteTargetKind,
    validate_account_id, validate_authorization, validate_route_statement,
};

const STATE_FORMAT_VERSION: u8 = 1;
const MAX_RECORD_BYTES: usize = MAX_ROUTE_STATEMENT_JSON_BYTES + 1024;

type RouteKey = (Vec<u8>, RouteName);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct State {
    routes: BTreeMap<RouteKey, RouteRecord>,
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
                    target: match &route.target {
                        RouteTarget::DuckFs { .. } => RouteTargetKind::DuckFs,
                        RouteTarget::LoopbackHttp => RouteTargetKind::LoopbackHttp,
                    },
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
        if encode_state(&decoded) != bytes {
            return Err("gateway: snapshot is not canonical".into());
        }
        if root_of(&decoded) != expected {
            return Err("gateway: snapshot root mismatch".into());
        }
        self.committed = decoded;
        self.pending = None;
        Ok(())
    }
}

fn validate_record(record: &RouteRecord) -> Result<(), String> {
    validate_route_statement(&record.statement)?;
    validate_authorization(&record.authorization)
}

fn encode_state(state: &State) -> Vec<u8> {
    let mut out = vec![STATE_FORMAT_VERSION];
    push_count(&mut out, state.routes.len());
    for ((account_id, name), record) in &state.routes {
        push_bytes(&mut out, account_id);
        match &name.label {
            None => out.push(0),
            Some(label) => {
                out.push(1);
                push_bytes(&mut out, label.as_bytes());
            }
        }
        push_bytes(
            &mut out,
            &serde_json::to_vec(record).expect("route record serializes"),
        );
    }
    out
}

fn decode_state(bytes: &[u8]) -> Result<State, String> {
    let mut reader = Reader::new(bytes);
    if reader.u8()? != STATE_FORMAT_VERSION {
        return Err("gateway: unsupported snapshot version".into());
    }
    let count = reader.count("route count")?;
    let mut routes = BTreeMap::new();
    let mut previous: Option<RouteKey> = None;
    let mut per_account: BTreeMap<Vec<u8>, usize> = BTreeMap::new();
    for _ in 0..count {
        let account_id = reader.bytes(MAX_ACCOUNT_ID_BYTES, "account id")?;
        validate_account_id(&account_id)?;
        let name = match reader.u8()? {
            0 => RouteName::apex(),
            1 => {
                let label = String::from_utf8(reader.bytes(MAX_ROUTE_LABEL_BYTES, "route label")?)
                    .map_err(|error| format!("gateway: route label is not utf-8: {error}"))?;
                RouteName::named(label)
            }
            marker => {
                return Err(format!("gateway: unsupported route name marker {marker}"));
            }
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
        let record_bytes = reader.bytes(MAX_RECORD_BYTES, "route record")?;
        let record: RouteRecord = serde_json::from_slice(&record_bytes)
            .map_err(|error| format!("gateway: route record: {error}"))?;
        validate_record(&record)?;
        if record.statement.account_id != key.0 || record.statement.name != key.1 {
            return Err("gateway: snapshot key does not match its record".into());
        }
        previous = Some(key.clone());
        routes.insert(key, record);
    }
    reader.finish()?;
    Ok(State { routes })
}

fn push_count(out: &mut Vec<u8>, count: usize) {
    out.extend_from_slice(&(count as u64).to_le_bytes());
}

fn push_bytes(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    out.extend_from_slice(bytes);
}

fn root_of(state: &State) -> [u8; 32] {
    if state == &State::default() {
        [0; 32]
    } else {
        Sha256::digest(encode_state(state)).into()
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn u8(&mut self) -> Result<u8, String> {
        let byte = *self
            .bytes
            .get(self.offset)
            .ok_or_else(|| "gateway: snapshot truncated".to_string())?;
        self.offset += 1;
        Ok(byte)
    }

    fn count(&mut self, what: &str) -> Result<usize, String> {
        let raw = self.take(8)?;
        let count = u64::from_le_bytes(raw.try_into().expect("eight bytes"));
        let count = usize::try_from(count).map_err(|_| "gateway: count overflows usize")?;
        if count > self.remaining() / 18 {
            return Err(format!("gateway: {what} exceeds remaining snapshot bytes"));
        }
        Ok(count)
    }

    fn bytes(&mut self, maximum: usize, what: &str) -> Result<Vec<u8>, String> {
        let raw = self.take(8)?;
        let length = u64::from_le_bytes(raw.try_into().expect("eight bytes"));
        if length > maximum as u64 {
            return Err(format!("gateway: snapshot {what} exceeds {maximum} bytes"));
        }
        let length = usize::try_from(length).map_err(|_| "gateway: length overflows usize")?;
        Ok(self.take(length)?.to_vec())
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], String> {
        let end = self
            .offset
            .checked_add(length)
            .filter(|end| *end <= self.bytes.len())
            .ok_or_else(|| "gateway: snapshot truncated".to_string())?;
        let value = &self.bytes[self.offset..end];
        self.offset = end;
        Ok(value)
    }

    fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }

    fn finish(self) -> Result<(), String> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err("gateway: snapshot has trailing bytes".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        MemberAuthorization, ROUTE_FORMAT_VERSION, RouteAudience, RouteDefinition, RouteMethod,
        RoutePolicy, RouteStatement, RouteTarget,
    };

    fn record(name: RouteName, revision: u64) -> RouteRecord {
        RouteRecord {
            statement: RouteStatement {
                version: ROUTE_FORMAT_VERSION,
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
        assert_eq!(live[0].target, RouteTargetKind::LoopbackHttp);
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
