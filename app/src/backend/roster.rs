use super::*;

/// One member of the network: a validator (quorum seat), a resident
/// (mesh + statesync standing), or a registered agent.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct MemberRow {
    pub key: String,
    pub label: String,
    pub role: String,
    pub is_this_node: bool,
    pub is_agent: bool,
    /// an agent's capability tag; empty for a human member.
    pub model: String,
    /// a HUMAN row: the mesh reports this key as a live peer (this node is
    /// live by definition). An AGENT row: the registry says active rather than
    /// paused — `MemberPresence` renders the two vocabularies apart on
    /// `is_agent`. Neither is "working right now"; that is `AgentRow.live`.
    pub live: bool,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct MembersData {
    pub generation: i64,
    pub members: Vec<MemberRow>,
}

/// Load the roster: validators, then residents, then the registered agents —
/// one list, this node marked, liveness folded in from the mesh sample.
pub async fn load_members(rpc: String, generation: i64) -> Result<MembersData, HydrationError> {
    offscreen_guard(generation)?;
    async {
        let client = rpc_client(&rpc)?;
        let node_key = client.status().await?.public_key;
        let live_keys = live_peer_keys(&client).await;
        let mut members = Vec::new();
        for (query, role) in [("validators", "validator"), ("residents", "resident")] {
            let reply: serde_json::Value =
                client.query("valset", &serde_json::json!(query)).await?;
            let keys = reply[query].as_array().cloned().unwrap_or_default();
            for key in keys {
                let hex = hex_encode(&json_bytes(&key));
                let is_this_node = hex == node_key;
                members.push(MemberRow {
                    label: short_label(&hex),
                    live: is_this_node || live_keys.contains(&hex),
                    is_this_node,
                    is_agent: false,
                    model: String::new(),
                    role: role.into(),
                    key: hex,
                });
            }
        }
        // registered agents are members of the workspace too — the roster shows
        // people AND machines, keyed on the agent id (agents hold no node key;
        // the roster labels that cell "agent id", not "public key").
        let agents = load_agents(rpc, generation).await.map(|data| data.agents);
        for agent in agents.unwrap_or_default() {
            members.push(MemberRow {
                key: agent.id,
                label: agent.name,
                role: "agent".into(),
                is_this_node: false,
                is_agent: true,
                model: agent.capability,
                // for an agent row this is REGISTRATION state (active vs
                // paused), which is what `MemberPresence` renders for a
                // machine — not "working now". The run-in-flight fact is
                // `AgentRow.live`, and only that one may pulse the rail.
                live: agent.status == "active",
            });
        }
        Ok(MembersData {
            generation,
            members,
        })
    }
    .await
    .map_err(|message: String| HydrationError {
        generation,
        message: user_error(message),
    })
}

/// The peer sample's live keys, full hex — the join key for member liveness.
/// A node that cannot answer `/v1/peers` simply reports nobody live.
async fn live_peer_keys(rpc: &RpcClient) -> BTreeSet<String> {
    let Ok(reply) = rpc.peers().await else {
        return BTreeSet::new();
    };
    reply["peers"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        // `connected` and `peer`, NOT `live`/`key`: those are the names
        // `PeerView` serializes (bin/noded/src/peers.rs). Reading the wrong
        // ones made every lookup return null, so this set came back empty on
        // every call and every member rendered offline.
        .filter(|peer| peer["connected"].as_bool().unwrap_or(false))
        .filter_map(|peer| peer["peer"].as_str().map(str::to_string))
        .collect()
}

/// This node holds a quorum seat — the ONE authority predicate behind the
/// approvals gate, the members Invite button and the forge write gate.
pub fn members_is_admin(rows: Vec<MemberRow>) -> bool {
    rows.iter()
        .any(|row| row.is_this_node && row.role == "validator")
}

/// This node's standing: `validator` | `resident` | `guest`, or `""` when the
/// roster has not answered.
///
/// The empty answer is load-bearing. `load_members` is one of thirteen parallel
/// loads, so it can be the only one that fails, and folding its silence into
/// `guest` told a validator's operator — with no error anywhere on screen —
/// that this device may not post. `""` lights the STANDING UNKNOWN arm in
/// node.ice instead.
///
/// An empty vec is the only unanswered signal a pure row function has, and it
/// is a sound one: an answered roster always carries the chain's own
/// validators. So a roster that DID answer and holds no row for this node is a
/// real guest and still reads `guest` — the guest card is not collateral here.
pub fn member_tier(rows: Vec<MemberRow>) -> String {
    if rows.is_empty() {
        return String::new();
    }
    rows.iter()
        .find(|row| row.is_this_node)
        .map_or_else(|| "guest".into(), |row| row.role.clone())
}

/// The All / Humans / Agents / Validators strip.
pub fn filter_members(rows: Vec<MemberRow>, filter: String) -> Vec<MemberRow> {
    rows.into_iter()
        .filter(|row| match filter.as_str() {
            "humans" => !row.is_agent,
            "agents" => row.is_agent,
            "validators" => row.role == "validator",
            _ => true,
        })
        .collect()
}

/// One governance proposal, rendered.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct ProposalRow {
    pub id: String,
    pub action: String,
    /// what the action actually does — the `GovAction` payload, rendered.
    pub detail: String,
    pub proposer: String,
    pub status: String,
    pub deadline: i64,
    pub approvals: i64,
    pub rejections: i64,
    /// the frozen rule's discriminant: `threshold` | `participating_majority`.
    /// The two bars are NOT interchangeable — a threshold counts YES power, a
    /// participating majority counts TURNOUT and then compares yes against no.
    pub rule: String,
    /// how many YES votes would pass this proposal AT ITS CURRENT TALLY, in
    /// `approvals`' own unit — the one number the dots, the `3 / 4` reading and
    /// the note may compare `approvals` against. Under
    /// `ParticipatingMajority{quorum}` that is `max(quorum − no, no + 1)`, which
    /// is exactly `turnout >= quorum && yes > no` restated as a yes count.
    pub required_yes: i64,
    pub electorate: i64,
    pub open: bool,
    /// The block a settled proposal was EXECUTED at, derived from the op feed
    /// (see [`settle_heights`]). 0 when the proposal is still open, or when it
    /// settled further back than the op window reaches.
    pub settled_height: i64,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct GovernanceData {
    pub generation: i64,
    pub proposals: Vec<ProposalRow>,
}

/// Load the proposal register, open proposals first, newest first within.
pub async fn load_governance(
    rpc: String,
    generation: i64,
) -> Result<GovernanceData, HydrationError> {
    offscreen_guard(generation)?;
    async {
        let rpc = rpc_client(&rpc)?;
        let reply: serde_json::Value = rpc
            .query("governance", &serde_json::json!("proposals"))
            .await?;
        let mut proposals: Vec<ProposalRow> = reply["proposals"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|view| {
                let votes = view["votes"].as_array().cloned().unwrap_or_default();
                let approvals = votes
                    .iter()
                    .filter(|vote| vote[1].as_bool().unwrap_or(false))
                    .count();
                let status = tagged_name(&view["status"]);
                let action = tagged_name(&view["action"]);
                let rejections = count_i64(votes.len() - approvals);
                ProposalRow {
                    id: view["proposal_id"].as_str().unwrap_or_default().to_string(),
                    open: status == "open",
                    detail: gov_action_detail(&view["action"]),
                    proposer: short_label(&hex_encode(&json_bytes(&view["proposer"]))),
                    deadline: view["deadline"].as_i64().unwrap_or(0),
                    approvals: count_i64(approvals),
                    rule: tagged_name(&view["voting_rule"]),
                    required_yes: yes_needed(&view["voting_rule"], rejections),
                    rejections,
                    electorate: count_i64(
                        view["electorate"]
                            .as_array()
                            .map_or(0, |members| members.len()),
                    ),
                    settled_height: 0,
                    action,
                    status,
                }
            })
            .collect();
        let any_settled = proposals.iter().any(|proposal| !proposal.open);
        if any_settled {
            let settled = settle_heights(&rpc).await;
            for proposal in &mut proposals {
                proposal.settled_height = settled.get(&proposal.id).copied().unwrap_or(0);
            }
        }
        proposals.sort_by(|left, right| {
            right
                .open
                .cmp(&left.open)
                .then(right.deadline.cmp(&left.deadline))
        });
        Ok(GovernanceData {
            generation,
            proposals,
        })
    }
    .await
    .map_err(|message: String| HydrationError {
        generation,
        message: user_error(message),
    })
}

/// How far back the settle-height derivation reads the op feed.
const SETTLE_SCAN_BLOCKS: usize = 400;

/// Proposal id -> the height it SETTLED at.
///
/// `ProposalView` omits the settle height, but settling is an ordinary op:
/// `GovMsg::Execute { proposal_id }` applied against the governance module. So
/// the height is recoverable from the block feed every explorer row already
/// reads — no module change, and no invented number: a proposal that settled
/// before the window simply has no entry, and its row prints no height.
async fn settle_heights(client: &RpcClient) -> BTreeMap<String, i64> {
    let Ok(blocks) = client.blocks(SETTLE_SCAN_BLOCKS).await else {
        return BTreeMap::new();
    };
    let mut heights = BTreeMap::new();
    for block in &blocks {
        let height = block["height"].as_i64().unwrap_or(0);
        for op in block["ops"].as_array().cloned().unwrap_or_default() {
            let governance_op = op["target"].as_str() == Some("governance");
            let applied = op["disposition"].as_str() == Some("applied");
            if !governance_op || !applied {
                continue;
            }
            // The feed carries the payload as its json TEXT preview, so the
            // execute variant is read back out of that text.
            let Some(payload) = op["payload"].as_str() else {
                continue;
            };
            let Ok(message) = serde_json::from_str::<serde_json::Value>(payload) else {
                continue;
            };
            let Some(id) = message["execute"]["proposal_id"].as_str() else {
                continue;
            };
            heights.insert(id.to_string(), height);
        }
    }
    heights
}

/// The `GovAction` payload as one readable clause — what the op DOES, which
/// the bare variant tag never says.
pub(crate) fn gov_action_detail(action: &serde_json::Value) -> String {
    let Some(tagged) = action.as_object() else {
        return String::new();
    };
    let Some((variant, payload)) = tagged.iter().next() else {
        return String::new();
    };
    let key = payload.get("key").map(json_bytes).unwrap_or_default();
    if !key.is_empty() {
        return format!("key {}", short_label(&hex_encode(&key)));
    }
    if let Some(text) = payload.get("text").and_then(|text| text.as_str()) {
        return text.to_string();
    }
    match variant.as_str() {
        "update_module" => format!(
            "{} → h {}",
            payload["name"].as_str().unwrap_or_default(),
            payload["activation_height"].as_i64().unwrap_or(0)
        ),
        "set_share_mode" => match payload["enabled"].as_bool().unwrap_or(false) {
            true => "account shares".into(),
            false => "one ballot per validator".into(),
        },
        _ => String::new(),
    }
}

/// How many YES votes pass this proposal at its current tally.
///
/// `Threshold{required_yes}` is already that number. `ParticipatingMajority`
/// is NOT: its `quorum` is a TURNOUT bar, and passing also needs `yes > no`
/// (crates/modules/system/governance/src/lib.rs, `settle`). Reading `quorum`
/// into a yes counter renders "quorum met" on a Signal vote that will not
/// settle, so restate the whole rule as the yes count it implies —
/// `yes >= quorum − no` IS `yes + no >= quorum`, and `yes >= no + 1` IS
/// `yes > no`.
pub(crate) fn yes_needed(rule: &serde_json::Value, rejections: i64) -> i64 {
    let Some(tagged) = rule.as_object() else {
        return 0;
    };
    let Some((variant, payload)) = tagged.iter().next() else {
        return 0;
    };
    match variant.as_str() {
        "participating_majority" => {
            let quorum = payload["quorum"].as_i64().unwrap_or(0);
            quorum.saturating_sub(rejections).max(rejections + 1)
        }
        _ => payload["required_yes"].as_i64().unwrap_or(0),
    }
}

/// Open a membership proposal. The app could vote and settle but never OPEN
/// one; `action` is `add_validator` | `add_resident` | `remove_validator`.
pub async fn governance_propose(
    rpc: String,
    password: String,
    action: String,
    target_key: String,
) -> Result<bool, AppError> {
    async {
        let key = public_key(&target_key, "member public key")?;
        let action = match action.as_str() {
            "add_validator" => governance::GovAction::AddValidator { key },
            "add_resident" => governance::GovAction::AddResident { key },
            "remove_validator" => governance::GovAction::RemoveValidator { key },
            other => return Err(format!("unknown membership action `{other}`")),
        };
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "governance",
            governance::encode_msg(&governance::GovMsg::Propose {
                proposal_id: fresh_id("proposal"),
                action,
                voting_period: GOVERNANCE_VOTING_PERIOD,
            }),
            password,
        )
        .await
    }
    .await
    .map_err(app_error)?;
    Ok(true)
}

/// Cast (or change) this node's ballot.
pub async fn governance_vote(
    rpc: String,
    password: String,
    proposal_id: String,
    approve: bool,
) -> Result<bool, AppError> {
    async {
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "governance",
            governance::encode_msg(&governance::GovMsg::Vote {
                proposal_id,
                approve,
            }),
            password,
        )
        .await
    }
    .await
    .map_err(app_error)?;
    Ok(true)
}

/// Tally and settle a proposal past its deadline (anyone may trigger).
pub async fn governance_execute(
    rpc: String,
    password: String,
    proposal_id: String,
) -> Result<bool, AppError> {
    async {
        let rpc = rpc_client(&rpc)?;
        signed_write(
            &rpc,
            "governance",
            governance::encode_msg(&governance::GovMsg::Execute { proposal_id }),
            password,
        )
        .await
    }
    .await
    .map_err(app_error)?;
    Ok(true)
}
