// WHO — members, registered agents, the DM directory, the account this device
// signs as, and the proposals the network is voting on.

on account_loaded(next)
  return if next.generation != account_generation
  account_bound = next.bound
  account_id = next.account_id
  account_name = next.display_name
  account_bio = next.bio
  account_members = next.members
  account_nodes = next.nodes

on account_failed(cause)
  return if cause.generation != account_generation

on account_name_draft_changed(next)
  account_name_draft = next

on account_rename_submit
  return if !connected || !account_bound || account_renaming || empty(trim(account_name_draft))
  account_renaming = true
  error = ""
  run set_account_name(connected_rpc, password, trim(account_name_draft)) -> account_renamed _ | account_rename_failed _

on account_renamed(_result)
  account_renaming = false
  account_name_draft = ""
  account_generation = account_generation + 1
  run load_account(connected_rpc, account_generation) -> account_loaded _ | account_failed _

on account_rename_failed(cause)
  account_renaming = false
  error = cause.message

on agents_loaded(next)
  return if next.generation != agents_generation
  agents_answered = true
  agents_rows = next.agents

on agents_failed(cause)
  return if cause.generation != agents_generation
  agents_answered = true

on governance_loaded(next)
  return if next.generation != gov_generation
  gov_answered = true
  gov_rows = next.proposals

on governance_failed(cause)
  return if cause.generation != gov_generation
  gov_answered = true

on gov_vote(proposal_id, approve)
  return if !connected || !empty(gov_voting)
  gov_voting = proposal_id
  run governance_vote(connected_rpc, password, gov_voting, approve) -> gov_acted _ | gov_act_failed _

on gov_execute(proposal_id)
  return if !connected || !empty(gov_voting)
  gov_voting = proposal_id
  run governance_execute(connected_rpc, password, gov_voting) -> gov_acted _ | gov_act_failed _

// The quorum-gated membership actions the roster detail panel offers. They
// share `gov_voting` with vote/execute: one governance write is in flight.
on gov_propose(action, target_key)
  return if !connected || !empty(gov_voting)
  gov_voting = target_key
  run governance_propose(connected_rpc, password, action, gov_voting) -> gov_acted _ | gov_act_failed _

on gov_acted(_result)
  gov_voting = ""
  gov_generation = gov_generation + 1
  run load_governance(connected_rpc, gov_generation) -> governance_loaded _ | governance_failed _

on gov_act_failed(cause)
  gov_voting = ""
  error = cause.message

on members_loaded(next)
  return if next.generation != members_generation
  members_answered = true
  members_rows = next.members
  members_validators = next.validators
  members_residents = next.residents

on members_failed(cause)
  return if cause.generation != members_generation

// The DIRECT peer directory. Loaded with the workspace, because the sidebar
// section that reads it is on screen from the first frame.
on dm_peers_loaded(next)
  return if next.generation != dm_peers_generation
  dm_peers = next.peers

on dm_peers_failed(cause)
  return if cause.generation != dm_peers_generation

// ROSTER — one screen, one filter, one detail panel. An empty key closes it.
on open_member(key)
  members_selected = key

on pick_members_filter(filter)
  members_filter = filter

// The invite modal is pure view state — minting is a separate, explicit act.
// Pause or resume an agent. The payload is the DESIRED state and it is named
// for the backend parameter it becomes: `true` PAUSES, `false` resumes. The
// roster's Pause control passes `true` and its Resume control passes `false`;
// a row wired from `agent.status` would have to invert. Only its owner may ask:
// the view offers this on `is_mine` rows, and the node refuses anyone else.
on agent_set_status(agent_id, paused)
  return if !connected
  run set_agent_status(connected_rpc, password, agent_id, paused) -> agent_status_set _ | mutation_failed _

on agent_status_set(_result)
  agents_generation = agents_generation + 1
  error = ""
  run load_agents(connected_rpc, agents_generation) -> agents_loaded _ | agents_failed _
