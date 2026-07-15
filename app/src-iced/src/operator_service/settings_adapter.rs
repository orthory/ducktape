use super::*;

const PREFERENCES_FILE: &str = "iced-preferences.json";
const MAX_MUTED_CHANNELS: usize = 256;
const MAX_CHANNEL_BYTES: usize = 128;
const DEFAULT_VOTING_PERIOD: u64 = 1_000_000;

static PREFERENCES_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

/// Settings-only facts that do not belong to the daemon or workspace registry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SettingsContext {
    pub active_channel: Option<String>,
    pub forget_needs_force: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesktopPreferences {
    pub mode: theme::Mode,
    pub accent: usize,
    pub notifications: settings::NotificationPrefs,
}

impl Default for DesktopPreferences {
    fn default() -> Self {
        Self {
            mode: theme::Mode::Light,
            accent: 0,
            notifications: settings::NotificationPrefs::default(),
        }
    }
}

pub(super) async fn execute(
    backend: Option<Backend>,
    node: Option<NodeClient>,
    workspace: Option<Workspace>,
    context: SettingsContext,
    command: settings::Command,
) -> settings::ServiceEvent {
    use settings::{Command, ServiceEvent};

    match command {
        Command::Load => {
            ServiceEvent::Loaded(load_settings(node.as_ref(), workspace.as_ref(), context).await)
        }
        Command::SetTheme(mode) => {
            ServiceEvent::PreferencesSaved(update_preferences(|preferences| {
                preferences.mode = mode
            }))
        }
        Command::SetAccent(accent) => ServiceEvent::PreferencesSaved(if accent < 5 {
            update_preferences(|preferences| preferences.accent = accent)
        } else {
            Err("accent index is outside the supported palette".into())
        }),
        Command::SetNotifications(notifications) => ServiceEvent::PreferencesSaved(
            validate_notifications(&notifications)
                .and_then(|()| update_preferences(|prefs| prefs.notifications = notifications)),
        ),
        Command::RequestLeave => ServiceEvent::DangerFinished(
            request_leave(backend.as_ref(), node.as_ref(), workspace.as_ref()).await,
        ),
        Command::ForgetWorkspace { force } => ServiceEvent::DangerFinished(
            forget_workspace(backend.as_ref(), workspace.as_ref(), force).await,
        ),
        Command::OpenAccount | Command::OpenNetworks | Command::OpenMembers | Command::OpenNode => {
            ServiceEvent::PreferencesSaved(Err(
                "settings navigation must be handled by the desktop shell".into(),
            ))
        }
    }
}

pub(super) fn load_preferences() -> Result<DesktopPreferences, String> {
    load_preferences_at(&preferences_path()?)
}

async fn load_settings(
    node: Option<&NodeClient>,
    workspace: Option<&Workspace>,
    context: SettingsContext,
) -> Result<Option<settings::SettingsData>, String> {
    if node.is_none() && workspace.is_none() {
        return Ok(None);
    }
    let owned_client = local_client(node, workspace)?;
    let client = node.or(owned_client.as_ref());
    let validators = match client {
        Some(client) => match client.status().await {
            Ok(status) => {
                if let Some(workspace) = workspace {
                    validate_node_identity(&status, workspace)?;
                }
                query_keys(client, "validators").await.ok()
            }
            Err(_) => None,
        },
        None => None,
    };
    let roster_loaded = validators.is_some();
    let validator_count = validators.as_ref().map(Vec::len).unwrap_or(usize::from(
        workspace.is_some_and(|workspace| workspace.member),
    ));
    let in_validator_set = validators.as_ref().map_or_else(
        || workspace.is_some_and(|workspace| workspace.member),
        |validators| {
            workspace.is_some_and(|workspace| {
                validators
                    .iter()
                    .any(|key| key.eq_ignore_ascii_case(&workspace.pubkey))
            })
        },
    );
    Ok(Some(settings::SettingsData {
        client_mode: workspace.is_none(),
        can_control_node: workspace.is_some(),
        workspace_name: workspace.map(|workspace| workspace.name.clone()),
        network_id: workspace.map(|workspace| workspace.chain_id.clone()),
        active_channel: context.active_channel,
        in_validator_set,
        validator_count,
        roster_loaded,
        forget_needs_force: context.forget_needs_force,
    }))
}

async fn forget_workspace(
    backend: Option<&Backend>,
    workspace: Option<&Workspace>,
    force: bool,
) -> Result<(), String> {
    let backend = backend.ok_or_else(|| "desktop backend is unavailable".to_string())?;
    let workspace = workspace.ok_or_else(|| "no managed workspace is active".to_string())?;
    backend
        .forget_workspace(workspace.id.clone(), force)
        .await
        .map(|_| ())
}

async fn request_leave(
    backend: Option<&Backend>,
    node: Option<&NodeClient>,
    workspace: Option<&Workspace>,
) -> Result<(), String> {
    let backend = backend.ok_or_else(|| "desktop backend is unavailable".to_string())?;
    let workspace = workspace.ok_or_else(|| "no managed workspace is active".to_string())?;
    let owned_client = local_client(node, Some(workspace))?;
    let client = node
        .or(owned_client.as_ref())
        .ok_or_else(|| "the managed node is unavailable".to_string())?;
    let status = client.status().await.map_err(|error| error.to_string())?;
    validate_node_identity(&status, workspace)?;
    let key = decode_key(&workspace.pubkey)?;
    let validators = query_keys(client, "validators").await?;
    if !validators
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(&workspace.pubkey))
    {
        return Err("this node is not in the current validator set".into());
    }
    if validators.len() < 2 {
        return Err("a solo node cannot remove the last validator; forget it instead".into());
    }

    let action = json!({ "remove_validator": { "key": key } });
    let mut proposals = governance_proposals(client).await?;
    let existing = proposals.iter().find(|proposal| {
        proposal.get("status").and_then(Value::as_str) == Some("open")
            && proposal.get("action") == Some(&action)
    });
    let proposal_id = existing
        .and_then(|proposal| proposal.get("proposal_id").and_then(Value::as_str))
        .map(str::to_owned)
        .unwrap_or_else(|| mint_proposal_id(&proposals, &workspace.pubkey));
    if existing.is_none() {
        submit_governance(
            backend,
            client,
            json!({
                "propose": {
                    "proposal_id": proposal_id.clone(),
                    "action": action,
                    "voting_period": DEFAULT_VOTING_PERIOD
                }
            }),
        )
        .await?;
    }
    submit_governance(
        backend,
        client,
        json!({ "vote": { "proposal_id": proposal_id.clone(), "approve": true } }),
    )
    .await?;
    proposals = governance_proposals(client).await?;
    let voted = proposal(&proposals, &proposal_id)?;
    if voted.get("status").and_then(Value::as_str) == Some("open")
        && can_settle_early(voted, validators.len())?
    {
        submit_governance(
            backend,
            client,
            json!({ "execute": { "proposal_id": proposal_id.clone() } }),
        )
        .await?;
        proposals = governance_proposals(client).await?;
    }
    match proposal(&proposals, &proposal_id)?
        .get("status")
        .and_then(Value::as_str)
    {
        Some("passed") => Ok(()),
        Some("rejected") => Err(format!(
            "the membership proposal was rejected ({proposal_id})"
        )),
        _ => {
            let (yes, _) = tally(proposal(&proposals, &proposal_id)?)?;
            let required =
                decision_threshold(proposal(&proposals, &proposal_id)?, validators.len())?;
            Err(format!(
                "ballot cast — {yes} of {required} required approvals; waiting on the other validators ({proposal_id})"
            ))
        }
    }
}

async fn governance_proposals(client: &NodeClient) -> Result<Vec<Value>, String> {
    let reply = client
        .query("governance", Value::String("proposals".into()))
        .await
        .map_err(|error| error.to_string())?;
    Ok(variant_array(&reply, "proposals")?.to_vec())
}

fn proposal<'a>(proposals: &'a [Value], proposal_id: &str) -> Result<&'a Value, String> {
    proposals
        .iter()
        .find(|proposal| proposal.get("proposal_id").and_then(Value::as_str) == Some(proposal_id))
        .ok_or_else(|| format!("proposal {proposal_id} disappeared"))
}

fn mint_proposal_id(proposals: &[Value], subject: &str) -> String {
    let taken: HashSet<&str> = proposals
        .iter()
        .filter_map(|proposal| proposal.get("proposal_id").and_then(Value::as_str))
        .collect();
    let head = format!("leave:{}:", &subject[..subject.len().min(16)]);
    (0..)
        .map(|index| format!("{head}{index}"))
        .find(|candidate| !taken.contains(candidate.as_str()))
        .expect("an unbounded sequence contains an unused proposal id")
}

fn can_settle_early(proposal: &Value, member_count: usize) -> Result<bool, String> {
    let (yes, total) = tally(proposal)?;
    match proposal.get("voting_rule") {
        Some(Value::String(rule)) if rule == "dynamic_validator_majority" => {
            Ok(yes > member_count as u64 / 2)
        }
        Some(Value::Object(rule)) if rule.contains_key("threshold") => Ok(yes
            >= rule["threshold"]["required_yes"]
                .as_u64()
                .ok_or_else(|| "governance threshold is invalid".to_string())?),
        Some(Value::Object(rule)) if rule.contains_key("participating_majority") => {
            let quorum = rule["participating_majority"]["quorum"]
                .as_u64()
                .ok_or_else(|| "governance quorum is invalid".to_string())?;
            let remaining = electorate_power(proposal, member_count)?
                .checked_sub(yes)
                .ok_or_else(|| "governance vote power exceeds the electorate".to_string())?;
            Ok(total >= quorum && yes > remaining)
        }
        None => Ok(yes > member_count as u64 / 2),
        _ => Err("governance voting rule is invalid".into()),
    }
}

fn decision_threshold(proposal: &Value, member_count: usize) -> Result<u64, String> {
    match proposal.get("voting_rule") {
        Some(Value::String(rule)) if rule == "dynamic_validator_majority" => {
            Ok(member_count as u64 / 2 + 1)
        }
        Some(Value::Object(rule)) if rule.contains_key("threshold") => {
            rule["threshold"]["required_yes"]
                .as_u64()
                .ok_or_else(|| "governance threshold is invalid".into())
        }
        Some(Value::Object(rule)) if rule.contains_key("participating_majority") => {
            rule["participating_majority"]["quorum"]
                .as_u64()
                .ok_or_else(|| "governance quorum is invalid".into())
        }
        None => Ok(member_count as u64 / 2 + 1),
        _ => Err("governance voting rule is invalid".into()),
    }
}

fn tally(proposal: &Value) -> Result<(u64, u64), String> {
    let electorate = proposal
        .get("electorate")
        .and_then(Value::as_array)
        .ok_or_else(|| "governance proposal electorate is invalid".to_string())?;
    let powers: BTreeMap<String, u64> = electorate
        .iter()
        .map(|row| {
            let row = row
                .as_array()
                .filter(|row| row.len() == 2)
                .ok_or_else(|| "governance electorate row is invalid".to_string())?;
            Ok((
                bytes_hex(&value_bytes(&row[0])?),
                row[1].as_u64().unwrap_or(0),
            ))
        })
        .collect::<Result<_, String>>()?;
    let votes = proposal
        .get("votes")
        .and_then(Value::as_array)
        .ok_or_else(|| "governance proposal votes are invalid".to_string())?;
    let mut yes = 0_u64;
    let mut total = 0_u64;
    for row in votes {
        let row = row
            .as_array()
            .filter(|row| row.len() == 2)
            .ok_or_else(|| "governance vote row is invalid".to_string())?;
        let power = if powers.is_empty() {
            1
        } else {
            powers
                .get(&bytes_hex(&value_bytes(&row[0])?))
                .copied()
                .unwrap_or(0)
        };
        total = total
            .checked_add(power)
            .ok_or_else(|| "governance vote power overflowed".to_string())?;
        if row[1].as_bool() == Some(true) {
            yes = yes
                .checked_add(power)
                .ok_or_else(|| "governance yes power overflowed".to_string())?;
        }
    }
    Ok((yes, total))
}

fn electorate_power(proposal: &Value, member_count: usize) -> Result<u64, String> {
    let electorate = proposal
        .get("electorate")
        .and_then(Value::as_array)
        .ok_or_else(|| "governance proposal electorate is invalid".to_string())?;
    if electorate.is_empty() {
        return Ok(member_count as u64);
    }
    electorate.iter().try_fold(0_u64, |total, row| {
        row.as_array()
            .and_then(|row| row.get(1))
            .and_then(Value::as_u64)
            .map(|power| total.saturating_add(power))
            .ok_or_else(|| "governance electorate row is invalid".to_string())
    })
}

fn preferences_path() -> Result<PathBuf, String> {
    #[cfg(windows)]
    let home = std::env::var_os("USERPROFILE");
    #[cfg(not(windows))]
    let home = std::env::var_os("HOME");
    let home = home
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| "could not determine the current user's home directory".to_string())?;
    Ok(home.join(".ducktape").join(PREFERENCES_FILE))
}

fn update_preferences(update: impl FnOnce(&mut DesktopPreferences)) -> Result<(), String> {
    let path = preferences_path()?;
    let _guard = PREFERENCES_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| "preferences lock is poisoned".to_string())?;
    let mut preferences = load_preferences_at(&path)?;
    update(&mut preferences);
    save_preferences_at(&path, &preferences)
}

fn load_preferences_at(path: &Path) -> Result<DesktopPreferences, String> {
    let text = match private_fs::read_to_string(path)? {
        Some(text) => text,
        None => return Ok(DesktopPreferences::default()),
    };
    let value: Value = match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(_) => return Ok(DesktopPreferences::default()),
    };
    let mode = match value.get("theme").and_then(Value::as_str) {
        Some("dark") => theme::Mode::Dark,
        _ => theme::Mode::Light,
    };
    let accent = value
        .get("accent")
        .and_then(Value::as_u64)
        .filter(|accent| *accent < 5)
        .unwrap_or(0) as usize;
    let notifications = value
        .get("notifications")
        .and_then(|value| notifications_from_json(value).ok())
        .unwrap_or_default();
    Ok(DesktopPreferences {
        mode,
        accent,
        notifications,
    })
}

fn save_preferences_at(path: &Path, preferences: &DesktopPreferences) -> Result<(), String> {
    let value = json!({
        "theme": match preferences.mode { theme::Mode::Light => "light", theme::Mode::Dark => "dark" },
        "accent": preferences.accent,
        "notifications": notifications_json(&preferences.notifications),
    });
    private_fs::write_atomic(
        path,
        &serde_json::to_vec_pretty(&value).map_err(|error| error.to_string())?,
    )
}

fn validate_notifications(notifications: &settings::NotificationPrefs) -> Result<(), String> {
    if notifications.muted_channels.len() > MAX_MUTED_CHANNELS
        || notifications
            .muted_channels
            .iter()
            .any(|channel| channel.is_empty() || channel.len() > MAX_CHANNEL_BYTES)
    {
        return Err("muted channel preferences exceed the desktop safety limit".into());
    }
    Ok(())
}

fn notifications_json(preferences: &settings::NotificationPrefs) -> Value {
    json!({
        "enabled": preferences.enabled,
        "mentions": preferences.mentions,
        "replies": preferences.replies,
        "huddles": preferences.huddles,
        "runs": preferences.runs,
        "forge": preferences.forge,
        "governance": preferences.governance,
        "mutedChannels": preferences.muted_channels,
    })
}

fn notifications_from_json(value: &Value) -> Result<settings::NotificationPrefs, String> {
    let defaults = settings::NotificationPrefs::default();
    let bool_value =
        |key: &str, fallback: bool| value.get(key).and_then(Value::as_bool).unwrap_or(fallback);
    let muted_channels = value
        .get("mutedChannels")
        .and_then(Value::as_array)
        .map(|channels| {
            channels
                .iter()
                .map(|channel| {
                    channel
                        .as_str()
                        .map(str::to_owned)
                        .ok_or_else(|| "muted channel preference is not a string".to_string())
                })
                .collect()
        })
        .transpose()?
        .unwrap_or_default();
    let preferences = settings::NotificationPrefs {
        enabled: bool_value("enabled", defaults.enabled),
        mentions: bool_value("mentions", defaults.mentions),
        replies: bool_value("replies", defaults.replies),
        huddles: bool_value("huddles", defaults.huddles),
        runs: bool_value("runs", defaults.runs),
        forge: bool_value("forge", defaults.forge),
        governance: bool_value("governance", defaults.governance),
        muted_channels,
    };
    validate_notifications(&preferences)?;
    Ok(preferences)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn governance_threshold_uses_frozen_power() {
        let proposal = json!({
            "voting_rule": { "participating_majority": { "quorum": 2 } },
            "electorate": [[[1], 2], [[2], 1]],
            "votes": [[[1], true]]
        });
        assert!(can_settle_early(&proposal, 99).unwrap());
        assert_eq!(decision_threshold(&proposal, 99).unwrap(), 2);
    }

    #[test]
    fn preferences_round_trip_and_validate_muted_channels() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("iced-preferences.json");
        let preferences = DesktopPreferences {
            mode: theme::Mode::Dark,
            accent: 4,
            notifications: settings::NotificationPrefs {
                muted_channels: vec!["general".into()],
                ..settings::NotificationPrefs::default()
            },
        };
        save_preferences_at(&path, &preferences).unwrap();
        assert_eq!(load_preferences_at(&path).unwrap(), preferences);
    }
}
