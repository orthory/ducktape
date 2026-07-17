//! Home wire and platform adapter.

use super::*;

pub(super) async fn load_home(
    backend: Option<Backend>,
    active: Option<Workspace>,
    client: Option<NodeClient>,
) -> Result<Option<HomeData>, String> {
    let backend = backend.ok_or_else(|| "desktop backend is unavailable".to_string())?;
    let (snapshot, identity, touch_id_available, touch_id_enrolled) = tokio::try_join!(
        backend.workspace_snapshot(),
        backend.identity_state(),
        backend.touch_id_available(),
        backend.touch_id_enrolled(),
    )?;
    let connected = match &client {
        Some(client) => client.status().await.is_ok(),
        None => false,
    };
    let mut account = match (connected, &client) {
        (true, Some(client)) => {
            load_identity_account(
                client,
                active.as_ref().map(|workspace| workspace.pubkey.as_str()),
                identity.pubkey.as_deref(),
            )
            .await?
        }
        _ => None,
    };
    if let (Some(current), Some(workspace), Some(client), Some(member_key)) = (
        account.as_ref(),
        active.as_ref(),
        client.as_ref(),
        identity.pubkey.as_deref(),
    ) {
        match account_service::complete_pending_bind(
            &backend, workspace, client, member_key, current,
        )
        .await
        {
            Ok(true) => {
                let node_key = decode_ed25519_key(&workspace.pubkey, "active node key")?;
                if let Some(bound) =
                    query_identity_account(client, json!({ "of_node": { "node_key": node_key } }))
                        .await?
                {
                    account = Some(bound);
                    profile_service::reconcile_best_effort(&backend, workspace, client).await;
                }
            }
            Ok(false) => {}
            Err(error) => tracing::debug!(
                target: "ducktape::account",
                event = "pending_link_bind_failed",
                reason = "post_link_bind_failed",
                detail = %error,
                "pending device link will retry while Home remains open"
            ),
        }
    }
    let account_name = account
        .as_ref()
        .map(|account| optional_account_text(account.get("display_name"), "display name", 64))
        .transpose()?
        .flatten()
        .unwrap_or_default();
    let account_id = account
        .as_ref()
        .map(|account| wire_key_hex(account.get("account_id"), 32, "account id"))
        .transpose()?;
    let account_avatar = account
        .as_ref()
        .map(|account| optional_account_text(account.get("avatar"), "account avatar", 512))
        .transpose()?
        .flatten();
    let account_bio = account
        .as_ref()
        .map(|account| optional_account_profile_text(account.get("bio"), "account bio", 280))
        .transpose()?
        .flatten();
    let duck_name = match (account_id.as_deref(), client.as_ref()) {
        (Some(account_id), Some(client)) if connected => {
            profile_service::duck_name(client, account_id).await?
        }
        _ => None,
    };
    let avatar_bytes = match (account_avatar.as_deref(), client.as_ref()) {
        (Some(path), Some(client)) if connected => {
            match profile_service::load_avatar_bytes(client, path).await {
                Ok(bytes) => Some(bytes),
                Err(error) => {
                    tracing::debug!(
                        target: "ducktape::account",
                        event = "profile_avatar_unavailable",
                        reason = "duckfs_avatar_read_failed",
                        detail = %error,
                        "the account avatar will use its initials fallback"
                    );
                    None
                }
            }
        }
        _ => None,
    };
    let profile = identity.pubkey.as_ref().map(|_| AccountProfile {
        display_name: account_name,
        account_id: account_id.clone().unwrap_or_default(),
        duck_name,
        avatar: account_avatar,
        avatar_bytes,
        bio: account_bio,
    });
    let custody = match (identity.pubkey.clone(), identity.state) {
        (Some(public_key), IdentityStatus::Plaintext) => Some(Custody {
            public_key,
            status: CustodyStatus::Plaintext,
        }),
        (Some(public_key), IdentityStatus::Locked) => Some(Custody {
            public_key,
            status: CustodyStatus::Locked,
        }),
        (Some(public_key), IdentityStatus::Unlocked) => Some(Custody {
            public_key,
            status: CustodyStatus::Unlocked,
        }),
        _ => None,
    };
    let (validators, residents) = match (&account, &client) {
        (Some(_), Some(client)) if connected => load_device_standings(client).await?,
        _ => (HashSet::new(), HashSet::new()),
    };
    let mut devices = match account.as_ref() {
        Some(account) => parse_devices(account, active.as_ref())?,
        None => active
            .as_ref()
            .map(|workspace| {
                vec![DeviceRow {
                    key: workspace.pubkey.clone(),
                    label: workspace.name.clone(),
                    standing: workspace_standing(workspace),
                    this_device: true,
                }]
            })
            .unwrap_or_default(),
    };
    for device in &mut devices {
        device.standing = if validators.contains(&device.key) {
            Standing::Validator
        } else if residents.contains(&device.key) {
            Standing::Resident
        } else {
            Standing::NoSeat
        };
    }
    let device_networks = match (account_id.as_deref(), active.as_ref()) {
        (Some(account_id), Some(active)) => {
            let known = snapshot
                .workspaces
                .iter()
                .map(|workspace| workspace.chain_id.clone())
                .collect();
            let live = CachedNetworkDevices {
                chain_id: active.chain_id.clone(),
                name: active.name.clone(),
                at_ms: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()
                    .try_into()
                    .unwrap_or(u64::MAX),
                rows: devices.iter().map(cached_device).collect(),
            };
            let cached = match backend
                .device_cache_update(account_id.to_string(), known, live.clone())
                .await
            {
                Ok(cached) => cached,
                Err(error) => {
                    tracing::debug!(
                        target: "ducktape::account",
                        event = "device_cache_unavailable",
                        reason = "local_state_write_failed",
                        detail = %error,
                        "inactive-network devices will not be shown"
                    );
                    vec![live]
                }
            };
            cached
                .into_iter()
                .map(|network| device_network(network, &active.chain_id))
                .collect()
        }
        _ => Vec::new(),
    };
    let member_keys = parse_member_keys(account.as_ref(), identity.pubkey.as_deref())?;
    let workspaces = snapshot
        .workspaces
        .into_iter()
        .map(|workspace| WorkspaceRow {
            active: active
                .as_ref()
                .is_some_and(|current| current.id == workspace.id),
            standing: if workspace.founder {
                Standing::Validator
            } else if workspace.member {
                Standing::Resident
            } else {
                Standing::NoSeat
            },
            id: workspace.id,
            name: workspace.name,
            network_id: workspace.chain_id,
        })
        .collect();
    Ok(Some(HomeData {
        profile,
        workspaces,
        devices,
        device_networks,
        member_keys,
        custody,
        touch_id_available,
        touch_id_enrolled,
        disconnected: !connected,
    }))
}

fn parse_member_keys(
    account: Option<&Value>,
    local_key: Option<&str>,
) -> Result<Vec<MemberKeyRow>, String> {
    let Some(account) = account else {
        return Ok(Vec::new());
    };
    let keys = account
        .get("member_keys")
        .and_then(Value::as_array)
        .ok_or_else(|| "node returned an invalid member-key list".to_string())?;
    if keys.is_empty() || keys.len() > 256 {
        return Err("node returned an invalid member-key list".into());
    }
    keys.iter()
        .map(|value| {
            let object = value
                .as_object()
                .ok_or_else(|| "node returned a malformed member key".to_string())?;
            let kind = match object.get("kind").and_then(Value::as_str) {
                Some("ed25519") => AccountKeyKind::Ed25519,
                Some("p256") => AccountKeyKind::P256,
                Some("webauthn_p256") => AccountKeyKind::WebauthnP256,
                _ => return Err("node returned an unsupported member-key kind".into()),
            };
            let expected = match kind {
                AccountKeyKind::Ed25519 => &[32][..],
                AccountKeyKind::P256 | AccountKeyKind::WebauthnP256 => &[33, 65][..],
            };
            let bytes = object
                .get("pubkey")
                .and_then(Value::as_array)
                .ok_or_else(|| "node returned a malformed member key".to_string())?;
            if !expected.contains(&bytes.len()) {
                return Err("node returned a malformed member key".into());
            }
            let key = wire_bytes_hex(bytes)
                .ok_or_else(|| "node returned a malformed member key".to_string())?;
            let label = match object.get("label") {
                Some(Value::String(label))
                    if label.len() <= 64 && !label.chars().any(char::is_control) =>
                {
                    Some(label.clone())
                }
                Some(Value::Null) | None => None,
                _ => return Err("node returned an invalid member-key label".into()),
            };
            Ok(MemberKeyRow {
                this_device: local_key.is_some_and(|local| local.eq_ignore_ascii_case(&key)),
                key,
                kind,
                label,
            })
        })
        .collect()
}

async fn load_identity_account(
    client: &NodeClient,
    node_key: Option<&str>,
    member_key: Option<&str>,
) -> Result<Option<Value>, String> {
    if let Some(node_key) = node_key {
        let node_key = decode_ed25519_key(node_key, "active node key")?;
        if let Some(account) =
            query_identity_account(client, json!({ "of_node": { "node_key": node_key } })).await?
        {
            return Ok(Some(account));
        }
    }
    let Some(member_key) = member_key else {
        return Ok(None);
    };
    let member_key = decode_ed25519_key(member_key, "local member key")?;
    query_identity_account(client, json!({ "of_member": { "member_key": member_key } })).await
}

async fn query_identity_account(
    client: &NodeClient,
    query: Value,
) -> Result<Option<Value>, String> {
    let reply = client
        .query("identity", query)
        .await
        .map_err(|error| error.to_string())?;
    match reply.get("account") {
        Some(Value::Null) | None => Ok(None),
        Some(account @ Value::Object(_)) => Ok(Some(account.clone())),
        Some(_) => Err("node returned an invalid identity account".into()),
    }
}

fn parse_devices(account: &Value, active: Option<&Workspace>) -> Result<Vec<DeviceRow>, String> {
    let nodes = account
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| "node returned an invalid bound-node list".to_string())?;
    if nodes.len() > 256 {
        return Err("node returned too many bound nodes".into());
    }
    nodes
        .iter()
        .map(|node| {
            let node = node
                .as_object()
                .ok_or_else(|| "node returned a malformed bound node".to_string())?;
            let key = wire_key_hex(node.get("node_key"), 32, "bound node key")?;
            let label = optional_account_text(node.get("label"), "node label", 64)?
                .unwrap_or_else(|| "Device".into());
            Ok(DeviceRow {
                this_device: active
                    .is_some_and(|workspace| workspace.pubkey.eq_ignore_ascii_case(&key)),
                label,
                standing: Standing::NoSeat,
                key,
            })
        })
        .collect()
}

async fn load_device_standings(
    client: &NodeClient,
) -> Result<(HashSet<String>, HashSet<String>), String> {
    let (validators, residents) = tokio::try_join!(
        load_standing_keys(client, "validators"),
        load_standing_keys(client, "residents"),
    )?;
    Ok((validators, residents))
}

async fn load_standing_keys(client: &NodeClient, variant: &str) -> Result<HashSet<String>, String> {
    let reply = client
        .query("valset", Value::String(variant.into()))
        .await
        .map_err(|error| error.to_string())?;
    let rows = reply
        .get(variant)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("node returned an invalid {variant} list"))?;
    if rows.len() > 512 {
        return Err(format!("node returned too many {variant}"));
    }
    rows.iter()
        .map(|row| wire_key_hex(Some(row), 32, variant))
        .collect()
}

fn workspace_standing(workspace: &Workspace) -> Standing {
    if workspace.founder {
        Standing::Validator
    } else if workspace.member {
        Standing::Resident
    } else {
        Standing::NoSeat
    }
}

fn cached_device(device: &DeviceRow) -> CachedDeviceRow {
    CachedDeviceRow {
        node_key: device.key.clone(),
        label: (device.label != "Device").then(|| device.label.clone()),
        standing: match device.standing {
            Standing::Validator => DeviceStanding::Validator,
            Standing::Resident => DeviceStanding::Resident,
            Standing::NoSeat => DeviceStanding::NoSeat,
        },
        this_device: device.this_device,
    }
}

fn device_network(network: CachedNetworkDevices, active_chain: &str) -> DeviceNetworkGroup {
    DeviceNetworkGroup {
        active: network.chain_id == active_chain,
        network_id: network.chain_id,
        name: network.name,
        at_ms: network.at_ms,
        devices: network
            .rows
            .into_iter()
            .map(|device| DeviceRow {
                key: device.node_key,
                label: device.label.unwrap_or_else(|| "Device".into()),
                standing: match device.standing {
                    DeviceStanding::Validator => Standing::Validator,
                    DeviceStanding::Resident => Standing::Resident,
                    DeviceStanding::NoSeat => Standing::NoSeat,
                },
                this_device: device.this_device,
            })
            .collect(),
    }
}

fn optional_account_text(
    value: Option<&Value>,
    field: &str,
    max_bytes: usize,
) -> Result<Option<String>, String> {
    match value {
        Some(Value::String(value))
            if value.len() <= max_bytes && !value.chars().any(char::is_control) =>
        {
            Ok(Some(value.clone()))
        }
        Some(Value::Null) | None => Ok(None),
        _ => Err(format!("node returned an invalid {field}")),
    }
}

fn optional_account_profile_text(
    value: Option<&Value>,
    field: &str,
    max_bytes: usize,
) -> Result<Option<String>, String> {
    match value {
        Some(Value::String(value)) if !value.is_empty() && value.len() <= max_bytes => {
            Ok(Some(value.clone()))
        }
        Some(Value::Null) | None => Ok(None),
        _ => Err(format!("node returned an invalid {field}")),
    }
}

fn wire_key_hex(value: Option<&Value>, len: usize, field: &str) -> Result<String, String> {
    let bytes = value
        .and_then(Value::as_array)
        .ok_or_else(|| format!("node returned an invalid {field}"))?;
    if bytes.len() != len {
        return Err(format!("node returned an invalid {field}"));
    }
    wire_bytes_hex(bytes).ok_or_else(|| format!("node returned an invalid {field}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_wire_keys_reject_malformed_bytes() {
        assert_eq!(
            wire_bytes_hex(&[json!(0), json!(15), json!(255)]).unwrap(),
            "000fff"
        );
        assert!(wire_bytes_hex(&[]).is_none());
        assert!(wire_bytes_hex(&[json!(256)]).is_none());
        assert!(wire_bytes_hex(&[json!("1")]).is_none());
    }
}
