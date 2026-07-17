//! Last-known bound devices for inactive registered networks.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

use super::Backend;
use super::private_fs;
use super::workspace_service::write_atomic;

const CACHE_VERSION: u32 = 1;
const MAX_ACCOUNTS: usize = 64;
const MAX_NETWORKS: usize = 256;
const MAX_ROWS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceStanding {
    Validator,
    Resident,
    NoSeat,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CachedDeviceRow {
    pub node_key: String,
    pub label: Option<String>,
    pub standing: DeviceStanding,
    pub this_device: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CachedNetworkDevices {
    pub chain_id: String,
    pub name: String,
    pub at_ms: u64,
    pub rows: Vec<CachedDeviceRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Cache {
    version: u32,
    accounts: BTreeMap<String, BTreeMap<String, CachedNetworkDevices>>,
}

impl Default for Cache {
    fn default() -> Self {
        Self {
            version: CACHE_VERSION,
            accounts: BTreeMap::new(),
        }
    }
}

impl Backend {
    pub async fn device_cache_update(
        &self,
        account_id: String,
        known_chain_ids: Vec<String>,
        live: CachedNetworkDevices,
    ) -> Result<Vec<CachedNetworkDevices>, String> {
        let root = self.root.clone();
        self.control
            .run(move || update(&root, account_id, known_chain_ids, live))
            .await
    }
}

fn update(
    root: &std::path::Path,
    account_id: String,
    known_chain_ids: Vec<String>,
    live: CachedNetworkDevices,
) -> Result<Vec<CachedNetworkDevices>, String> {
    validate_hex_key(&account_id)?;
    if known_chain_ids.len() > MAX_NETWORKS {
        return Err("too many registered networks for the device cache".into());
    }
    validate_network(&live)?;
    let known: HashSet<String> = known_chain_ids.into_iter().collect();
    if !known.contains(&live.chain_id) {
        return Err("active network is not registered".into());
    }

    private_fs::ensure_private_dir(root)?;
    let path = root.join("device-cache.json");
    let mut cache = load(&path);
    if cache.accounts.len() >= MAX_ACCOUNTS && !cache.accounts.contains_key(&account_id) {
        cache.accounts.clear();
    }
    let networks = cache.accounts.entry(account_id).or_default();
    networks.retain(|chain_id, _| known.contains(chain_id));
    networks.insert(live.chain_id.clone(), live.clone());

    let mut result: Vec<_> = networks.values().cloned().collect();
    result.sort_by(|left, right| {
        (right.chain_id == live.chain_id)
            .cmp(&(left.chain_id == live.chain_id))
            .then_with(|| right.at_ms.cmp(&left.at_ms))
    });
    let bytes = serde_json::to_vec_pretty(&cache)
        .map_err(|error| format!("encode device cache: {error}"))?;
    write_atomic(&path, &bytes)?;
    Ok(result)
}

fn load(path: &std::path::Path) -> Cache {
    let Ok(Some(bytes)) = private_fs::read(path) else {
        return Cache::default();
    };
    let Ok(cache) = serde_json::from_slice::<Cache>(&bytes) else {
        return Cache::default();
    };
    if validate_cache(&cache).is_err() {
        return Cache::default();
    }
    cache
}

fn validate_cache(cache: &Cache) -> Result<(), String> {
    if cache.version != CACHE_VERSION || cache.accounts.len() > MAX_ACCOUNTS {
        return Err("unsupported device cache".into());
    }
    for (account, networks) in &cache.accounts {
        validate_hex_key(account)?;
        if networks.len() > MAX_NETWORKS {
            return Err("device cache has too many networks".into());
        }
        for (chain_id, network) in networks {
            if chain_id != &network.chain_id {
                return Err("device cache network key mismatch".into());
            }
            validate_network(network)?;
        }
    }
    Ok(())
}

fn validate_network(network: &CachedNetworkDevices) -> Result<(), String> {
    validate_text(&network.chain_id, 256, "network id")?;
    validate_text(&network.name, 128, "network name")?;
    if network.rows.len() > MAX_ROWS {
        return Err("device cache has too many device rows".into());
    }
    for row in &network.rows {
        validate_hex_key(&row.node_key)?;
        if let Some(label) = row.label.as_deref() {
            validate_text(label, 64, "node label")?;
        }
    }
    Ok(())
}

fn validate_text(value: &str, max: usize, field: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > max || value.chars().any(char::is_control) {
        return Err(format!("invalid {field} in device cache"));
    }
    Ok(())
}

fn validate_hex_key(value: &str) -> Result<(), String> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("invalid key in device cache".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_rejects_unbounded_or_cross_keyed_rows() {
        let row = CachedDeviceRow {
            node_key: "11".repeat(32),
            label: Some("Laptop".into()),
            standing: DeviceStanding::Resident,
            this_device: true,
        };
        let network = CachedNetworkDevices {
            chain_id: "chain-a".into(),
            name: "A".into(),
            at_ms: 1,
            rows: vec![row],
        };
        let cache = Cache {
            version: CACHE_VERSION,
            accounts: BTreeMap::from([(
                "22".repeat(32),
                BTreeMap::from([("wrong-chain".into(), network)]),
            )]),
        };
        assert!(validate_cache(&cache).is_err());
    }

    #[test]
    fn update_keeps_registered_networks_active_first_and_prunes_forgotten() {
        let root = std::env::temp_dir().join(format!(
            "ducktape-device-cache-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let network = |chain: &str, at_ms| CachedNetworkDevices {
            chain_id: chain.into(),
            name: chain.into(),
            at_ms,
            rows: Vec::new(),
        };
        let account = "22".repeat(32);
        update(
            &root,
            account.clone(),
            vec!["a".into(), "b".into()],
            network("a", 1),
        )
        .unwrap();
        let rows = update(
            &root,
            account.clone(),
            vec!["a".into(), "b".into()],
            network("b", 2),
        )
        .unwrap();
        assert_eq!(
            rows.iter()
                .map(|row| row.chain_id.as_str())
                .collect::<Vec<_>>(),
            ["b", "a"]
        );
        let rows = update(&root, account, vec!["b".into()], network("b", 3)).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].chain_id, "b");
        std::fs::remove_dir_all(root).unwrap();
    }
}
