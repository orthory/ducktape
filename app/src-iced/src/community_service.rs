//! Real node/backend adapters for the native Members, Governance, and Explorer
//! surfaces. Membership and governance mutations always use the account-signed
//! control frame lane; there is deliberately no unsigned fallback.

use std::collections::{BTreeMap, BTreeSet};

use futures_util::{StreamExt as _, stream};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::backend::{Backend, Workspace};
use crate::operator_service;
use crate::profile_service;
use crate::screens::{explorer, governance, members};
use crate::transport::{self, NodeClient, NodeStatus};

const DEFAULT_VOTING_PERIOD: u64 = 1_000_000;
const EXPLORER_BLOCK_LIMIT: usize = 256;
const MAX_MEMBER_AVATARS: usize = 64;
const MAX_GOVERNANCE_PROPOSALS: usize = 4_096;
const MAX_GOVERNANCE_BALLOTS: usize = 4_096;
const MAX_GOVERNANCE_REPLY_BYTES: usize = 4 * 1024 * 1024;
const MAX_GOVERNANCE_SHARES_BYTES: usize = 512 * 1024;
const MAX_GOVERNANCE_ID_BYTES: usize = 512;
const MAX_SHARE_ACCOUNTS: usize = 256;
const MAX_SAFE_SHARES: u64 = 9_007_199_254_740_991;

#[derive(Debug, Deserialize)]
struct AccountWire {
    account_id: Vec<u8>,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    avatar: Option<String>,
    #[serde(default)]
    nodes: Vec<NodeWire>,
}

#[derive(Debug, Deserialize)]
struct NodeWire {
    node_key: Vec<u8>,
    #[serde(default)]
    label: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ProposalWire {
    proposal_id: String,
    action: Value,
    proposer: Vec<u8>,
    created_at: u64,
    deadline: u64,
    status: String,
    #[serde(default)]
    votes: Vec<(Vec<u8>, bool)>,
    #[serde(default = "validator_voter_kind")]
    voter_kind: String,
    #[serde(default)]
    electorate: Vec<(Vec<u8>, u64)>,
    #[serde(default = "dynamic_voting_rule")]
    voting_rule: Value,
}

#[derive(Debug, Deserialize)]
struct SharesWire {
    active: bool,
    #[serde(default)]
    allocations: Vec<ShareAllocationWire>,
    total: u64,
}

#[derive(Debug, Deserialize)]
struct ShareAllocationWire {
    account_id: Vec<u8>,
    shares: u64,
}

#[derive(Debug, Deserialize)]
struct UpgradeWire {
    current_version: u32,
    #[serde(default)]
    pending: Option<ScheduledUpgradeWire>,
    #[serde(default)]
    members: Vec<Vec<u8>>,
    #[serde(default)]
    ready: Vec<Vec<u8>>,
    armed: bool,
}

#[derive(Debug, Deserialize)]
struct ScheduledUpgradeWire {
    name: String,
    activation_height: u64,
    to_version: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RootOpWire {
    proposer: String,
    disposition: String,
    target: String,
    #[serde(default)]
    operations: Vec<DispatchWire>,
    #[serde(default)]
    payload: String,
    #[serde(default)]
    op_hash: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DispatchWire {
    module: String,
    origin: String,
    emitted_msgs: u64,
    emitted_events: u64,
}

pub async fn execute_members(
    backend: Option<Backend>,
    node: Option<NodeClient>,
    workspace: Option<Workspace>,
    command: members::Command,
) -> members::ServiceEvent {
    use members::{Command, ServiceEvent};

    match command {
        Command::Load => ServiceEvent::Loaded(load_members(backend, node, workspace).await),
        Command::RevealInvite(target) => {
            let result = async {
                let backend = backend.ok_or("desktop backend is unavailable")?;
                let workspace = admin_workspace(workspace.as_ref())?;
                let forms = backend
                    .workspace_invite_blob(workspace.id.clone(), target)
                    .await?;
                Ok((forms.blob, forms.short))
            }
            .await;
            ServiceEvent::InviteRevealed(result)
        }
        Command::AdmitMember(key) => ServiceEvent::ActionFinished(
            membership_action(
                backend,
                node,
                workspace,
                governance::Action::AddResident(key.clone()),
                "resident:",
                key,
            )
            .await,
        ),
        Command::PromoteMember(key) => ServiceEvent::ActionFinished(
            membership_action(
                backend,
                node,
                workspace,
                governance::Action::AddValidator(key.clone()),
                "validator:",
                key,
            )
            .await,
        ),
        Command::DemoteMember(key) => ServiceEvent::ActionFinished(
            membership_action(
                backend,
                node,
                workspace,
                governance::Action::RemoveValidator(key.clone()),
                "member-remove:",
                key,
            )
            .await,
        ),
        Command::RemoveResident(key) => ServiceEvent::ActionFinished(
            membership_action(
                backend,
                node,
                workspace,
                governance::Action::RemoveResident(key.clone()),
                "resident-remove:",
                key,
            )
            .await,
        ),
        Command::SetDisplayName(display_name) => {
            let result = async {
                let workspace = workspace.ok_or("a managed workspace is required")?;
                let (client, _) = connected_client(node, Some(&workspace)).await?;
                client
                    .submit(
                        "identity",
                        json!({ "set_account_name": { "display_name": display_name } }),
                        Some(&workspace.pubkey),
                    )
                    .await
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            }
            .await;
            ServiceEvent::ActionFinished(result)
        }
        Command::CopyText(_) | Command::ClearFocus => ServiceEvent::ActionFinished(Ok(())),
    }
}

pub async fn execute_governance(
    backend: Option<Backend>,
    node: Option<NodeClient>,
    workspace: Option<Workspace>,
    command: governance::Command,
) -> governance::ServiceEvent {
    use governance::{Command, ServiceEvent};

    if command == Command::Load {
        return ServiceEvent::Loaded(load_governance(node, workspace).await);
    }
    let proposal_id = match &command {
        Command::Propose { proposal_id, .. } | Command::Vote { proposal_id, .. } => {
            proposal_id.clone()
        }
        Command::Execute(proposal_id) => proposal_id.clone(),
        Command::Load => unreachable!("load returned above"),
    };
    let result = async {
        let backend = backend.ok_or("desktop backend is unavailable")?;
        let (client, _) = connected_client(node, workspace.as_ref()).await?;
        let payload = match command {
            Command::Propose {
                proposal_id,
                action,
            } => proposal_payload(proposal_id, action)?,
            Command::Vote {
                proposal_id,
                approve,
            } => json!({ "vote": { "proposal_id": proposal_id, "approve": approve } }),
            Command::Execute(proposal_id) => {
                json!({ "execute": { "proposal_id": proposal_id } })
            }
            Command::Load => unreachable!("load returned above"),
        };
        operator_service::submit_governance(&backend, &client, payload).await
    }
    .await;
    ServiceEvent::ActionFinished {
        proposal_id,
        result,
    }
}

pub async fn execute_explorer(
    node: Option<NodeClient>,
    workspace: Option<Workspace>,
    command: explorer::Command,
) -> explorer::ServiceEvent {
    match command {
        explorer::Command::Load => {
            explorer::ServiceEvent::Loaded(load_explorer(node, workspace).await)
        }
        explorer::Command::ClearFocus => explorer::ServiceEvent::Loaded(Ok(None)),
    }
}

async fn load_members(
    backend: Option<Backend>,
    node: Option<NodeClient>,
    workspace: Option<Workspace>,
) -> Result<Option<members::MembersData>, String> {
    let Some(client) = client_for(node, workspace.as_ref())? else {
        return Ok(None);
    };
    let status = client.status().await.map_err(|error| error.to_string())?;
    if let Some(workspace) = workspace.as_ref() {
        validate_node_identity(&status, workspace)?;
    }

    let validators = query_keys(&client, "validators").await?;
    let residents = query_keys(&client, "residents").await?;
    let accounts = query_accounts(&client).await.unwrap_or_default();
    let capabilities = query_capabilities(&client).await.unwrap_or_default();
    let can_admin = workspace
        .as_ref()
        .is_some_and(|workspace| workspace.founder || workspace.member);
    let pending_joins = if can_admin {
        match (backend.as_ref(), workspace.as_ref()) {
            (Some(backend), Some(workspace)) => backend
                .workspace_join_requests(workspace.id.clone())
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|request| members::JoinRequest {
                    joiner: request.joiner,
                    issuer: request.issuer,
                })
                .collect(),
            _ => Vec::new(),
        }
    } else {
        Vec::new()
    };

    let mut bindings = BTreeMap::new();
    let mut names = BTreeMap::new();
    let mut avatar_paths = BTreeMap::new();
    for account in accounts {
        let account_id = bytes_hex(&account.account_id);
        if let Some(name) = account.display_name.as_ref() {
            names.insert(account_id.clone(), name.clone());
        }
        if let Some(avatar) = account.avatar.as_ref().filter(|path| path.len() <= 512) {
            avatar_paths.insert(account_id.clone(), avatar.clone());
        }
        for node in account.nodes {
            let node_key = bytes_hex(&node.node_key);
            if let Some(name) = account.display_name.as_ref() {
                names.insert(node_key.clone(), name.clone());
            }
            if let Some(avatar) = account.avatar.as_ref().filter(|path| path.len() <= 512) {
                avatar_paths.insert(node_key.clone(), avatar.clone());
            }
            bindings.insert(
                node_key,
                members::BoundAccount {
                    id: account_id.clone(),
                    name: account.display_name.clone(),
                    device_label: node.label,
                },
            );
        }
    }

    // Load only avatars that can appear in this roster. Account queries may
    // contain more identities than the validator/resident directory; letting
    // those consume the bounded load budget would let unrelated profiles hide
    // the first visible avatars.
    let avatar_bytes = stream::iter(
        validators
            .iter()
            .chain(&residents)
            .filter_map(|key| avatar_paths.get(&normalize(key)).cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .take(MAX_MEMBER_AVATARS),
    )
    .map(|path| {
        let client = client.clone();
        async move {
            profile_service::load_avatar_bytes(&client, &path)
                .await
                .ok()
                .map(|bytes| (path, bytes))
        }
    })
    .buffer_unordered(8)
    .filter_map(async move |avatar| avatar)
    .collect::<BTreeMap<_, _>>()
    .await;

    let local_key = workspace
        .as_ref()
        .map(|workspace| normalize(&workspace.pubkey));
    let mut roster = Vec::with_capacity(validators.len() + residents.len());
    for (key, tier) in validators
        .into_iter()
        .map(|key| (key, members::Tier::Validator))
        .chain(
            residents
                .into_iter()
                .map(|key| (key, members::Tier::Resident)),
        )
    {
        let normalized = normalize(&key);
        let is_local = local_key.as_deref() == Some(normalized.as_str());
        let is_founder = workspace
            .as_ref()
            .is_some_and(|workspace| workspace.founder && is_local);
        let profile_name = names.get(&normalized).cloned();
        let display_name = profile_name.clone().unwrap_or_else(|| short_key(&key));
        let role = match tier {
            members::Tier::Resident => "resident",
            members::Tier::Validator if is_founder => "genesis validator",
            members::Tier::Validator
                if is_local && workspace.as_ref().is_some_and(|workspace| workspace.member) =>
            {
                "member validator"
            }
            members::Tier::Validator => "validator",
        };
        roster.push(members::Member {
            key,
            display_name: display_name.clone(),
            profile_name,
            initials: initials(&display_name),
            avatar_bytes: avatar_paths
                .get(&normalized)
                .and_then(|path| avatar_bytes.get(path))
                .cloned(),
            tier,
            role: role.into(),
            is_founder,
            is_local,
            bound_account: bindings.get(&normalized).cloned(),
            providers: providers_of(capabilities.get(&normalized).map_or(&[], Vec::as_slice)),
        });
    }

    Ok(Some(members::MembersData {
        members: roster,
        can_admin,
        workspace_role: match workspace.as_ref() {
            Some(workspace) if workspace.founder => "Genesis",
            Some(workspace) if workspace.member => "Admitted",
            _ => "Read Only",
        }
        .into(),
        invite_blob: None,
        invite_short: None,
        pending_joins,
    }))
}

async fn load_governance(
    node: Option<NodeClient>,
    workspace: Option<Workspace>,
) -> Result<Option<governance::GovernanceData>, String> {
    let Some(client) = client_for(node, workspace.as_ref())? else {
        return Ok(None);
    };
    let status = client.status().await.map_err(|error| error.to_string())?;
    if let Some(workspace) = workspace.as_ref() {
        validate_node_identity(&status, workspace)?;
    }
    let proposals = query_proposals(&client).await?;
    let shares = query_shares(&client).await?;
    let validators = query_keys(&client, "validators").await?;
    let accounts = query_accounts(&client).await.unwrap_or_default();

    let local_node = workspace
        .as_ref()
        .map(|workspace| normalize(&workspace.pubkey));
    let local_identity = local_node.as_deref().and_then(|local| {
        accounts.iter().find(|account| {
            account
                .nodes
                .iter()
                .any(|node| bytes_hex(&node.node_key) == local)
        })
    });
    let local_account = local_identity.map(|account| bytes_hex(&account.account_id));
    let local_nodes: Vec<String> = local_identity.map_or_else(
        || local_node.iter().cloned().collect(),
        |account| {
            account
                .nodes
                .iter()
                .map(|node| bytes_hex(&node.node_key))
                .collect()
        },
    );
    let mut known_accounts = BTreeSet::new();
    let mut display_names = BTreeMap::new();
    for account in &accounts {
        let account_id = bytes_hex(&account.account_id);
        if !account.nodes.is_empty() {
            known_accounts.insert(account_id.clone());
        }
        for node in &account.nodes {
            if let Some(name) = account.display_name.as_ref() {
                display_names.insert(bytes_hex(&node.node_key), name.clone());
            }
        }
    }
    let upgrade = match query_upgrade(&client).await {
        Ok(wire) => {
            let ready: BTreeSet<String> = wire.ready.iter().map(|key| bytes_hex(key)).collect();
            governance::Resource::Ready(governance::UpgradeStatus {
                current_version: wire.current_version,
                pending: wire.pending.map(|pending| governance::ScheduledUpgrade {
                    name: pending.name,
                    to_version: pending.to_version,
                    activation_height: pending.activation_height,
                }),
                armed: wire.armed,
                members: wire
                    .members
                    .into_iter()
                    .map(|key| {
                        let key = bytes_hex(&key);
                        governance::UpgradeMember {
                            display_name: display_names
                                .get(&key)
                                .cloned()
                                .unwrap_or_else(|| short_key(&key)),
                            ready: ready.contains(&key),
                            key,
                        }
                    })
                    .collect(),
            })
        }
        Err(error) => governance::Resource::Error(error),
    };

    Ok(Some(governance::GovernanceData {
        proposals,
        shares,
        local_nodes: local_nodes.clone(),
        local_account,
        member_count: validators.len(),
        legacy_can_vote: local_nodes.iter().any(|local| {
            validators
                .iter()
                .any(|validator| validator.eq_ignore_ascii_case(local))
        }),
        known_accounts: known_accounts.into_iter().collect(),
        current_height: status.height,
        upgrade,
    }))
}

async fn load_explorer(
    node: Option<NodeClient>,
    workspace: Option<Workspace>,
) -> Result<Option<Vec<explorer::BlockRecord>>, String> {
    let Some(client) = client_for(node, workspace.as_ref())? else {
        return Ok(None);
    };
    let status = client.status().await.map_err(|error| error.to_string())?;
    if let Some(workspace) = workspace.as_ref() {
        validate_node_identity(&status, workspace)?;
    }
    let accounts = query_accounts(&client).await.unwrap_or_default();
    let mut names = BTreeMap::new();
    for account in accounts {
        if let Some(name) = account.display_name {
            names.insert(bytes_hex(&account.account_id), name.clone());
            for node in account.nodes {
                names.insert(bytes_hex(&node.node_key), name.clone());
            }
        }
    }
    let mut blocks = client
        .blocks(Some(EXPLORER_BLOCK_LIMIT))
        .await
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|block| explorer_block(block, &names))
        .collect::<Result<Vec<_>, _>>()?;
    blocks.sort_by_key(|block| block.height);
    Ok(Some(blocks))
}

async fn membership_action(
    backend: Option<Backend>,
    node: Option<NodeClient>,
    workspace: Option<Workspace>,
    action: governance::Action,
    id_prefix: &str,
    subject: String,
) -> Result<(), String> {
    if hex_bytes(&subject)?.len() != 32 {
        return Err("membership keys must be exactly 32 bytes".into());
    }
    let workspace_ref = admin_workspace(workspace.as_ref())?;
    if matches!(&action, governance::Action::RemoveValidator(key) if key.eq_ignore_ascii_case(&workspace_ref.pubkey))
    {
        return Err("Members cannot remove the local node from its own validator set".into());
    }
    let backend = backend.ok_or("desktop backend is unavailable")?;
    let (client, _) = connected_client(node, Some(workspace_ref)).await?;
    let validators = query_keys(&client, "validators").await?;
    let proposals = query_proposals(&client).await?;
    let existing = proposals
        .iter()
        .find(|proposal| {
            proposal.status == governance::ProposalStatus::Open && proposal.action == action
        })
        .map(|proposal| proposal.id.clone());
    let proposal_id = existing.clone().unwrap_or_else(|| {
        mint_proposal_id(
            id_prefix,
            &subject,
            proposals.iter().map(|proposal| &proposal.id),
        )
    });
    if existing.is_none() {
        let action_wire = action_to_wire(&action)?;
        operator_service::submit_governance(
            &backend,
            &client,
            json!({
                "propose": {
                    "proposal_id": proposal_id,
                    "action": action_wire,
                    "voting_period": DEFAULT_VOTING_PERIOD,
                }
            }),
        )
        .await?;
    }
    operator_service::submit_governance(
        &backend,
        &client,
        json!({ "vote": { "proposal_id": proposal_id, "approve": true } }),
    )
    .await?;

    let voted = query_proposals(&client)
        .await?
        .into_iter()
        .find(|proposal| proposal.id == proposal_id)
        .ok_or_else(|| format!("proposal {proposal_id} disappeared"))?;
    let outcome = if voted.status == governance::ProposalStatus::Open
        && governance::can_settle_early(&voted, validators.len())
    {
        operator_service::submit_governance(
            &backend,
            &client,
            json!({ "execute": { "proposal_id": proposal_id } }),
        )
        .await?;
        query_proposals(&client)
            .await?
            .into_iter()
            .find(|proposal| proposal.id == proposal_id)
            .unwrap_or(voted)
    } else {
        voted
    };
    match outcome.status {
        governance::ProposalStatus::Passed => Ok(()),
        governance::ProposalStatus::Rejected => Err(format!(
            "the membership proposal was rejected ({proposal_id})"
        )),
        governance::ProposalStatus::Open => {
            let (yes, _) = governance::tally(&outcome);
            let required = governance::decision_threshold(&outcome, validators.len());
            Err(format!(
                "ballot cast — {yes} of {required} required approvals; waiting on the other validators ({proposal_id})"
            ))
        }
    }
}

fn proposal_payload(proposal_id: String, action: governance::Action) -> Result<Value, String> {
    Ok(json!({
        "propose": {
            "proposal_id": proposal_id,
            "action": action_to_wire(&action)?,
            "voting_period": DEFAULT_VOTING_PERIOD,
        }
    }))
}

fn action_to_wire(action: &governance::Action) -> Result<Value, String> {
    Ok(match action {
        governance::Action::AddValidator(key) => {
            json!({ "add_validator": { "key": hex_bytes(key)? } })
        }
        governance::Action::RemoveValidator(key) => {
            json!({ "remove_validator": { "key": hex_bytes(key)? } })
        }
        governance::Action::Signal(text) => json!({ "signal": { "text": text } }),
        governance::Action::AddResident(key) => {
            json!({ "add_resident": { "key": hex_bytes(key)? } })
        }
        governance::Action::RemoveResident(key) => {
            json!({ "remove_resident": { "key": hex_bytes(key)? } })
        }
        governance::Action::ScheduleUpgrade {
            name,
            activation_height,
            to_version,
        } => json!({
            "schedule_upgrade": {
                "name": name,
                "activation_height": activation_height,
                "to_version": to_version,
            }
        }),
        governance::Action::CancelUpgrade(name) => {
            json!({ "cancel_upgrade": { "name": name } })
        }
        governance::Action::UpdateModule {
            name,
            module_id,
            activation_height,
            code_hash,
        } => json!({
            "update_module": {
                "name": name,
                "module_id": module_id,
                "activation_height": activation_height,
                "code_hash": hex_bytes(code_hash)?,
            }
        }),
        governance::Action::CancelModuleUpdate { name, module_id } => json!({
            "cancel_module_update": { "name": name, "module_id": module_id }
        }),
        governance::Action::AdoptShares(allocations) => json!({
            "adopt_shares": {
                "allocations": allocations
                    .iter()
                    .map(|allocation| Ok(json!({
                        "account_id": hex_bytes(&allocation.account_id)?,
                        "shares": allocation.shares,
                    })))
                    .collect::<Result<Vec<Value>, String>>()?,
            }
        }),
        governance::Action::SetShares { account_id, shares } => json!({
            "set_shares": { "account_id": hex_bytes(account_id)?, "shares": shares }
        }),
        governance::Action::SetShareMode(enabled) => {
            json!({ "set_share_mode": { "enabled": enabled } })
        }
    })
}

async fn query_proposals(client: &NodeClient) -> Result<Vec<governance::Proposal>, String> {
    let rows: Vec<ProposalWire> = query_variant_bounded(
        client,
        "governance",
        "proposals",
        "proposals",
        MAX_GOVERNANCE_REPLY_BYTES,
    )
    .await?;
    if rows.len() > MAX_GOVERNANCE_PROPOSALS {
        return Err("governance proposal reply exceeds the desktop safety limit".into());
    }
    rows.into_iter().map(proposal_from_wire).collect()
}

async fn query_shares(client: &NodeClient) -> Result<governance::Shares, String> {
    let wire: SharesWire = query_variant_bounded(
        client,
        "governance",
        "shares",
        "shares",
        MAX_GOVERNANCE_SHARES_BYTES,
    )
    .await?;
    shares_from_wire(wire)
}

fn shares_from_wire(wire: SharesWire) -> Result<governance::Shares, String> {
    if wire.allocations.len() > MAX_SHARE_ACCOUNTS
        || wire.total > MAX_SAFE_SHARES
        || wire.allocations.iter().any(|allocation| {
            allocation.account_id.len() != 32 || allocation.shares > MAX_SAFE_SHARES
        })
        || wire
            .allocations
            .iter()
            .try_fold(0_u64, |total, row| total.checked_add(row.shares))
            != Some(wire.total)
    {
        return Err("governance shares reply exceeds the module bounds".into());
    }
    Ok(governance::Shares {
        active: wire.active,
        allocations: wire
            .allocations
            .into_iter()
            .map(|allocation| governance::ShareAllocation {
                account_id: bytes_hex(&allocation.account_id),
                shares: allocation.shares,
            })
            .collect(),
        total: wire.total,
    })
}

async fn query_upgrade(client: &NodeClient) -> Result<UpgradeWire, String> {
    query_variant(client, "upgrade", "status", "status").await
}

async fn query_accounts(client: &NodeClient) -> Result<Vec<AccountWire>, String> {
    let reply = client
        .query("identity", json!({ "all": { "from": 0, "limit": 256 } }))
        .await
        .map_err(|error| error.to_string())?;
    decode_variant(reply, "accounts")
}

async fn query_keys(client: &NodeClient, variant: &str) -> Result<Vec<String>, String> {
    let reply = client
        .query("valset", Value::String(variant.into()))
        .await
        .map_err(|error| error.to_string())?;
    let rows: Vec<Vec<u8>> = decode_variant(reply, variant)?;
    Ok(rows.into_iter().map(|key| bytes_hex(&key)).collect())
}

async fn query_capabilities(client: &NodeClient) -> Result<BTreeMap<String, Vec<String>>, String> {
    let reply = client
        .query("capability", Value::String("all".into()))
        .await
        .map_err(|error| error.to_string())?;
    let rows: Vec<(Vec<u8>, Vec<String>)> = decode_variant(reply, "all")?;
    Ok(rows
        .into_iter()
        .map(|(key, tags)| (bytes_hex(&key), tags))
        .collect())
}

async fn query_variant<T: DeserializeOwned>(
    client: &NodeClient,
    target: &str,
    query: &str,
    variant: &str,
) -> Result<T, String> {
    let reply = client
        .query(target, Value::String(query.into()))
        .await
        .map_err(|error| error.to_string())?;
    decode_variant(reply, variant)
}

async fn query_variant_bounded<T: DeserializeOwned>(
    client: &NodeClient,
    target: &str,
    query: &str,
    variant: &str,
    max_bytes: usize,
) -> Result<T, String> {
    let reply = client
        .query_bounded(target, Value::String(query.into()), max_bytes)
        .await
        .map_err(|error| error.to_string())?;
    decode_variant(reply, variant)
}

fn decode_variant<T: DeserializeOwned>(reply: Value, variant: &str) -> Result<T, String> {
    let value = reply
        .get(variant)
        .cloned()
        .ok_or_else(|| format!("{variant} query returned an invalid reply"))?;
    serde_json::from_value(value).map_err(|error| format!("invalid {variant} reply: {error}"))
}

fn proposal_from_wire(wire: ProposalWire) -> Result<governance::Proposal, String> {
    if wire.proposal_id.is_empty()
        || wire.proposal_id.len() > MAX_GOVERNANCE_ID_BYTES
        || wire.proposer.len() != 32
        || wire.deadline <= wire.created_at
        || wire.votes.len() > MAX_GOVERNANCE_BALLOTS
        || wire.electorate.len() > MAX_GOVERNANCE_BALLOTS
        || wire
            .votes
            .iter()
            .any(|(principal, _)| principal.len() != 32)
        || wire
            .electorate
            .iter()
            .any(|(principal, power)| principal.len() != 32 || *power > MAX_SAFE_SHARES)
    {
        return Err("governance proposal exceeds the module or desktop bounds".into());
    }
    Ok(governance::Proposal {
        id: wire.proposal_id,
        action: action_from_wire(&wire.action)?,
        proposer: bytes_hex(&wire.proposer),
        created_at: wire.created_at,
        deadline: wire.deadline,
        status: match wire.status.as_str() {
            "open" => governance::ProposalStatus::Open,
            "passed" => governance::ProposalStatus::Passed,
            "rejected" => governance::ProposalStatus::Rejected,
            _ => return Err("governance proposal status is invalid".into()),
        },
        votes: wire
            .votes
            .into_iter()
            .map(|(principal, approve)| governance::Ballot {
                principal: bytes_hex(&principal),
                approve,
            })
            .collect(),
        voter_kind: match wire.voter_kind.as_str() {
            "validator_node" => governance::VoterKind::ValidatorNode,
            "account" => governance::VoterKind::Account,
            _ => return Err("governance proposal voter kind is invalid".into()),
        },
        electorate: wire
            .electorate
            .into_iter()
            .map(|(principal, power)| governance::VotingPower {
                principal: bytes_hex(&principal),
                power,
            })
            .collect(),
        voting_rule: voting_rule(&wire.voting_rule)?,
    })
}

fn action_from_wire(value: &Value) -> Result<governance::Action, String> {
    let object = value
        .as_object()
        .filter(|object| object.len() == 1)
        .ok_or_else(|| "governance action is invalid".to_string())?;
    let (kind, body) = object.iter().next().expect("one action variant");
    let key = |field: &str| {
        body.get(field)
            .ok_or_else(|| format!("governance {kind} action is missing {field}"))
            .and_then(value_bytes)
            .and_then(|bytes| {
                (bytes.len() == 32)
                    .then(|| bytes_hex(&bytes))
                    .ok_or_else(|| format!("governance {kind} action has an invalid {field}"))
            })
    };
    let string = |field: &str| {
        body.get(field)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .ok_or_else(|| format!("governance {kind} action is missing {field}"))
    };
    Ok(match kind.as_str() {
        "add_validator" => governance::Action::AddValidator(key("key")?),
        "remove_validator" => governance::Action::RemoveValidator(key("key")?),
        "signal" => governance::Action::Signal(
            body.get("text")
                .and_then(Value::as_str)
                .ok_or("governance signal action is invalid")?
                .into(),
        ),
        "add_resident" => governance::Action::AddResident(key("key")?),
        "remove_resident" => governance::Action::RemoveResident(key("key")?),
        "schedule_upgrade" => governance::Action::ScheduleUpgrade {
            name: body
                .get("name")
                .and_then(Value::as_str)
                .ok_or("governance upgrade name is invalid")?
                .into(),
            activation_height: body
                .get("activation_height")
                .and_then(Value::as_u64)
                .ok_or("governance upgrade height is invalid")?,
            to_version: body
                .get("to_version")
                .and_then(Value::as_u64)
                .and_then(|version| u32::try_from(version).ok())
                .ok_or("governance upgrade version is invalid")?,
        },
        "cancel_upgrade" => governance::Action::CancelUpgrade(
            body.get("name")
                .and_then(Value::as_str)
                .ok_or("governance cancel-upgrade name is invalid")?
                .into(),
        ),
        "update_module" => {
            let code_hash = key("code_hash")?;
            governance::Action::UpdateModule {
                name: string("name")?,
                module_id: string("module_id")?,
                activation_height: body
                    .get("activation_height")
                    .and_then(Value::as_u64)
                    .ok_or("governance module-update height is invalid")?,
                code_hash,
            }
        }
        "cancel_module_update" => governance::Action::CancelModuleUpdate {
            name: string("name")?,
            module_id: string("module_id")?,
        },
        "adopt_shares" => {
            let allocations = serde_json::from_value::<Vec<ShareAllocationWire>>(
                body.get("allocations")
                    .cloned()
                    .ok_or("governance share allocations are missing")?,
            )
            .map_err(|error| format!("governance share allocations are invalid: {error}"))?;
            if allocations.is_empty()
                || allocations.len() > MAX_SHARE_ACCOUNTS
                || allocations.iter().any(|allocation| {
                    allocation.account_id.len() != 32
                        || allocation.shares == 0
                        || allocation.shares > MAX_SAFE_SHARES
                })
            {
                return Err("governance share allocations exceed the module bounds".into());
            }
            governance::Action::AdoptShares(
                allocations
                    .into_iter()
                    .map(|allocation| governance::ShareAllocation {
                        account_id: bytes_hex(&allocation.account_id),
                        shares: allocation.shares,
                    })
                    .collect(),
            )
        }
        "set_shares" => governance::Action::SetShares {
            account_id: key("account_id")?,
            shares: body
                .get("shares")
                .and_then(Value::as_u64)
                .filter(|shares| *shares <= MAX_SAFE_SHARES)
                .ok_or("governance share value is invalid")?,
        },
        "set_share_mode" => governance::Action::SetShareMode(
            body.get("enabled")
                .and_then(Value::as_bool)
                .ok_or("governance share mode is invalid")?,
        ),
        _ => return Err(format!("unknown governance action {kind:?}")),
    })
}

fn voting_rule(value: &Value) -> Result<governance::VotingRule, String> {
    if value.as_str() == Some("dynamic_validator_majority") {
        return Ok(governance::VotingRule::DynamicValidatorMajority);
    }
    if let Some(required_yes) = value
        .get("threshold")
        .and_then(|value| value.get("required_yes"))
        .and_then(Value::as_u64)
    {
        return Ok(governance::VotingRule::Threshold { required_yes });
    }
    if let Some(quorum) = value
        .get("participating_majority")
        .and_then(|value| value.get("quorum"))
        .and_then(Value::as_u64)
    {
        return Ok(governance::VotingRule::ParticipatingMajority { quorum });
    }
    Err("governance voting rule is invalid".into())
}

fn explorer_block(
    block: transport::BlockRecord,
    names: &BTreeMap<String, String>,
) -> Result<explorer::BlockRecord, String> {
    let ops = block
        .ops
        .into_iter()
        .map(|value| {
            let wire: RootOpWire = serde_json::from_value(value)
                .map_err(|error| format!("invalid explorer op: {error}"))?;
            let disposition = match wire.disposition.as_str() {
                "applied" => explorer::Disposition::Applied,
                "rejected" => explorer::Disposition::Rejected,
                _ => return Err("explorer op disposition is invalid".into()),
            };
            Ok(explorer::RootOp {
                proposer_name: names.get(&normalize(&wire.proposer)).cloned(),
                proposer: wire.proposer,
                disposition,
                target: wire.target,
                operations: wire
                    .operations
                    .into_iter()
                    .map(|dispatch| explorer::DispatchInfo {
                        module: dispatch.module,
                        origin: dispatch.origin,
                        emitted_messages: dispatch.emitted_msgs,
                        emitted_events: dispatch.emitted_events,
                    })
                    .collect(),
                payload: wire.payload,
                op_hash: wire.op_hash,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(explorer::BlockRecord {
        height: block.height,
        hash: block.hash,
        commit_hash: block.commit_hash,
        ops,
    })
}

fn providers_of(tags: &[String]) -> Vec<members::Provider> {
    let mut order = Vec::new();
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for tag in tags {
        let provider = tag
            .split_once('_')
            .map_or(tag.as_str(), |(provider, _)| provider);
        if !groups.contains_key(provider) {
            order.push(provider.to_string());
            groups.insert(provider.into(), Vec::new());
        }
        let rest = tag.strip_prefix(&format!("{provider}_"));
        let model = rest.map(|rest| {
            rest.rsplit_once('_')
                .filter(|(model, _)| !model.is_empty())
                .map_or(rest, |(model, _)| model)
        });
        if let Some(model) = model {
            let models = groups.get_mut(provider).expect("provider inserted above");
            if !models.iter().any(|known| known == model) {
                models.push(model.into());
            }
        }
    }
    order
        .into_iter()
        .map(|provider| members::Provider {
            label: title_case(&provider),
            models: groups.remove(&provider).unwrap_or_default(),
        })
        .collect()
}

fn client_for(
    node: Option<NodeClient>,
    workspace: Option<&Workspace>,
) -> Result<Option<NodeClient>, String> {
    node.map(Ok)
        .or_else(|| {
            workspace.map(|workspace| {
                NodeClient::local(workspace.ports.http).map_err(|error| error.to_string())
            })
        })
        .transpose()
}

async fn connected_client(
    node: Option<NodeClient>,
    workspace: Option<&Workspace>,
) -> Result<(NodeClient, NodeStatus), String> {
    let client = client_for(node, workspace)?.ok_or("node connection is unavailable")?;
    let status = client.status().await.map_err(|error| error.to_string())?;
    if let Some(workspace) = workspace {
        validate_node_identity(&status, workspace)?;
    }
    Ok((client, status))
}

fn validate_node_identity(status: &NodeStatus, workspace: &Workspace) -> Result<(), String> {
    if status.public_key.as_deref().is_some_and(|key| {
        !workspace.pubkey.is_empty() && !key.eq_ignore_ascii_case(&workspace.pubkey)
    }) {
        return Err(
            "the process answering this workspace port reports a different node identity".into(),
        );
    }
    Ok(())
}

fn admin_workspace(workspace: Option<&Workspace>) -> Result<&Workspace, String> {
    workspace
        .filter(|workspace| workspace.founder || workspace.member)
        .ok_or_else(|| "current validator standing is required for this operation".into())
}

fn mint_proposal_id<'a>(
    prefix: &str,
    subject: &str,
    taken: impl Iterator<Item = &'a String>,
) -> String {
    let taken: BTreeSet<&str> = taken.map(String::as_str).collect();
    let subject: String = subject.chars().take(16).collect();
    let head = format!("{prefix}{subject}:");
    (0..)
        .map(|index| format!("{head}{index}"))
        .find(|candidate| !taken.contains(candidate.as_str()))
        .expect("the natural numbers contain an unused proposal id")
}

fn validator_voter_kind() -> String {
    "validator_node".into()
}

fn dynamic_voting_rule() -> Value {
    Value::String("dynamic_validator_majority".into())
}

fn hex_bytes(value: &str) -> Result<Vec<u8>, String> {
    let value = value.trim();
    if value.is_empty()
        || !value.len().is_multiple_of(2)
        || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("expected an even-length hexadecimal key".into());
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair).expect("ASCII hex is UTF-8");
            u8::from_str_radix(pair, 16).map_err(|_| "invalid hexadecimal key".into())
        })
        .collect()
}

fn value_bytes(value: &Value) -> Result<Vec<u8>, String> {
    serde_json::from_value(value.clone()).map_err(|_| "expected a byte-array key".into())
}

fn bytes_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}

fn normalize(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn short_key(value: &str) -> String {
    let value = value.trim();
    if value.len() <= 16 {
        value.into()
    } else {
        format!("{}…{}", &value[..8], &value[value.len() - 8..])
    }
}

fn initials(name: &str) -> String {
    let words: Vec<&str> = name
        .split_whitespace()
        .filter(|word| word.chars().any(|character| character.is_alphanumeric()))
        .take(2)
        .collect();
    if words.is_empty() {
        "?".into()
    } else if words.len() == 1 {
        words[0]
            .chars()
            .filter(|character| character.is_alphanumeric())
            .take(2)
            .flat_map(char::to_uppercase)
            .collect()
    } else {
        words
            .into_iter()
            .filter_map(|word| word.chars().find(|character| character.is_alphanumeric()))
            .flat_map(char::to_uppercase)
            .collect()
    }
}

fn title_case(value: &str) -> String {
    let mut chars = value.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::WorkspacePorts;

    #[test]
    fn governance_wire_round_trips_every_action() {
        let actions = [
            governance::Action::AddValidator("11".repeat(32)),
            governance::Action::RemoveValidator("22".repeat(32)),
            governance::Action::Signal("ship it".into()),
            governance::Action::AddResident("33".repeat(32)),
            governance::Action::RemoveResident("44".repeat(32)),
            governance::Action::ScheduleUpgrade {
                name: "duck-2".into(),
                activation_height: 42,
                to_version: 2,
            },
            governance::Action::CancelUpgrade("duck-2".into()),
            governance::Action::UpdateModule {
                name: "pages-v2".into(),
                module_id: "pages".into(),
                activation_height: 55,
                code_hash: "77".repeat(32),
            },
            governance::Action::CancelModuleUpdate {
                name: "pages-v2".into(),
                module_id: "pages".into(),
            },
            governance::Action::AdoptShares(vec![governance::ShareAllocation {
                account_id: "55".repeat(32),
                shares: 7,
            }]),
            governance::Action::SetShares {
                account_id: "66".repeat(32),
                shares: 9,
            },
            governance::Action::SetShareMode(true),
        ];
        for action in actions {
            let wire = action_to_wire(&action).unwrap();
            assert_eq!(action_from_wire(&wire).unwrap(), action);
        }
    }

    #[test]
    fn legacy_proposal_defaults_to_validator_majority() {
        let principal = vec![1_u8; 32];
        let wire: ProposalWire = serde_json::from_value(json!({
            "proposal_id": "p-1",
            "action": { "signal": { "text": "hello" } },
            "proposer": principal,
            "created_at": 10,
            "deadline": 20,
            "status": "open",
            "votes": [[vec![1_u8; 32], true]],
        }))
        .unwrap();
        let proposal = proposal_from_wire(wire).unwrap();
        assert_eq!(proposal.voter_kind, governance::VoterKind::ValidatorNode);
        assert!(proposal.electorate.is_empty());
        assert_eq!((proposal.created_at, proposal.deadline), (10, 20));
        assert_eq!(
            proposal.voting_rule,
            governance::VotingRule::DynamicValidatorMajority
        );
    }

    #[test]
    fn governance_parser_rejects_oversized_ballot_and_share_sets() {
        let proposal = ProposalWire {
            proposal_id: "p-1".into(),
            action: json!({ "signal": { "text": "hello" } }),
            proposer: vec![1; 32],
            created_at: 10,
            deadline: 20,
            status: "open".into(),
            votes: vec![(vec![2; 32], true); MAX_GOVERNANCE_BALLOTS + 1],
            voter_kind: validator_voter_kind(),
            electorate: Vec::new(),
            voting_rule: dynamic_voting_rule(),
        };
        assert!(proposal_from_wire(proposal).is_err());

        let shares = SharesWire {
            active: true,
            allocations: (0..=MAX_SHARE_ACCOUNTS)
                .map(|index| ShareAllocationWire {
                    account_id: vec![u8::try_from(index % 255).unwrap(); 32],
                    shares: 1,
                })
                .collect(),
            total: (MAX_SHARE_ACCOUNTS + 1) as u64,
        };
        assert!(shares_from_wire(shares).is_err());

        assert!(
            action_from_wire(&json!({
                "add_validator": { "key": [1, 2] }
            }))
            .is_err()
        );
    }

    #[test]
    fn explorer_parser_keeps_rejected_trace_and_author_name() {
        let block = transport::BlockRecord {
            height: 7,
            hash: "aa".into(),
            commit_hash: "bb".into(),
            ops: vec![json!({
                "proposer": "CC",
                "disposition": "rejected",
                "target": "chat",
                "operations": [{
                    "module": "chat",
                    "origin": "external",
                    "emittedMsgs": 1,
                    "emittedEvents": 2,
                }],
                "payload": "{}",
                "opHash": "dd",
            })],
        };
        let names = BTreeMap::from([("cc".into(), "Eddy".into())]);
        let parsed = explorer_block(block, &names).unwrap();
        assert_eq!(parsed.ops[0].proposer_name.as_deref(), Some("Eddy"));
        assert_eq!(parsed.ops[0].disposition, explorer::Disposition::Rejected);
        assert_eq!(parsed.ops[0].operations[0].emitted_events, 2);
    }

    #[test]
    fn providers_collapse_effort_variants_in_first_seen_order() {
        let tags = vec![
            "claude_opus_high".into(),
            "codex_gpt-5.5_low".into(),
            "claude_opus_low".into(),
            "claude_sonnet_high".into(),
        ];
        assert_eq!(
            providers_of(&tags),
            vec![
                members::Provider {
                    label: "Claude".into(),
                    models: vec!["opus".into(), "sonnet".into()],
                },
                members::Provider {
                    label: "Codex".into(),
                    models: vec!["gpt-5.5".into()],
                },
            ]
        );
    }

    #[tokio::test]
    async fn members_service_refuses_local_self_removal_before_signing() {
        let key = "aa".repeat(32);
        let workspace = Workspace {
            id: "team".into(),
            name: "Team".into(),
            chain_id: "chain".into(),
            pubkey: key.clone(),
            founder: true,
            member: true,
            ports: WorkspacePorts {
                listen: 1,
                http: 2,
                rpc: 3,
                wireguard: Some(4),
                invite: Some(5),
            },
        };
        let error = membership_action(
            None,
            None,
            Some(workspace),
            governance::Action::RemoveValidator(key.clone()),
            "member-remove:",
            key,
        )
        .await
        .expect_err("members must not self-remove the local node");
        assert!(error.contains("local node"));
    }
}
