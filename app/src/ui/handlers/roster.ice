// WHO — members, registered agents, the DM directory, the account this device
// signs as, and the proposals the network is voting on.

on account_loaded(next)
  return if next.generation != account_generation
  account_exists = next.exists
  // SETTINGS' READING ONLY. The DM derivation does NOT hang off this field any
  // more: `load_dm_peers` resolves the account number it needs for itself and
  // stamps each row's `channel_id`, and all three DM decisions read that one id
  // — so a late or failed account load can no longer scatter DMs into the room
  // list.
  account_number = next.number
  account_name = next.name
  account_bio = next.bio
  account_keys = next.keys
  account_key_rows = next.key_rows

on account_failed(cause)
  return if cause.generation != account_generation

// THE SETTINGS VIEW SENT THE OP (`settings_view_event`, handlers/node.ice);
// what lands here is its answer. A committed op tells the view which drafts
// it consumed — the rename its name, a mint the key pair, a re-read every
// draft the card offers — so the view clears those and only those.
on account_renamed(_result)
  account_renaming = false
  settings_drafts_cleared = settings_drafts_cleared + 1
  settings_drafts_scope = "name"
  account_generation = account_generation + 1
  run replace lane=account_load load_account(connected_rpc, account_generation) -> account_loaded _ | account_failed _

on account_rename_failed(cause)
  account_renaming = false
  error = cause.message

// Minting commits nothing: the ticket is shown to copy, the drafts it
// consumed clear, and the account is re-read only when the OTHER device joins.
on account_ticket_minted(ticket)
  account_busy = false
  account_ticket = ticket
  settings_drafts_cleared = settings_drafts_cleared + 1
  settings_drafts_scope = "keys"

// `done` is `account_changed`'s body inlined (a handler cannot call a
// handler): the account picture moved, so it is re-read under a fresh
// generation.
on account_ceremony_stepped(next)
  let phase = ceremony_phase(next)
  account_ceremony_phase = next.phase
  account_ceremony_qr = next.qr
  account_ceremony_detail = next.detail
  account_ceremony_left = next.left
  match phase
    CeremonyPhase.done
      account_ceremony_phase = ""
      account_ceremony_qr = ""
      account_busy = false
      settings_drafts_cleared = settings_drafts_cleared + 1
      settings_drafts_scope = "label"
      account_generation = account_generation + 1
      run replace lane=account_load load_account(connected_rpc, account_generation) -> account_loaded _ | account_failed _
    CeremonyPhase.failed
      account_ceremony_phase = ""
      account_ceremony_qr = ""
      account_busy = false
      error = next.detail
    CeremonyPhase.show_qr
      error = ""
    CeremonyPhase.working
      error = ""

// Every committed identity op lands here: the account picture moved, so it
// is re-read under a fresh generation, and every draft the card offers goes
// with it — a ticket left on screen after its device joined is a stale blob
// that looks like a secret.
on account_changed(_result)
  account_ceremony_phase = ""
  account_ceremony_qr = ""
  account_ceremony_detail = ""
  account_ceremony_left = ""
  account_busy = false
  account_ticket = ""
  settings_drafts_cleared = settings_drafts_cleared + 1
  settings_drafts_scope = "account"
  account_generation = account_generation + 1
  run replace lane=account_load load_account(connected_rpc, account_generation) -> account_loaded _ | account_failed _

on account_op_failed(cause)
  account_ceremony_phase = ""
  account_ceremony_qr = ""
  account_ceremony_detail = ""
  account_ceremony_left = ""
  account_busy = false
  error = cause.message

on agents_loaded(next)
  return if next.generation != agents_generation
  agents_answered = true
  agents_rows = next.agents
  agents_runs = next.runs
  agents_capabilities = next.capabilities
  agents_actions = next.actions
  // the open journal follows the register: the op that moved the register
  // may have moved the open run too
  return if empty(agents_open_run)
  run replace lane=agent_journal load_run_journal(connected_rpc, agents_open_run) -> agent_journal_loaded _ | agent_journal_failed _

// The journal read answers for the run it was asked about; a read that
// lands after the reader opened another run, or closed it, is dropped.
on agent_journal_loaded(next)
  return if next.run_id != agents_open_run
  agents_journal = next

on agent_journal_failed(cause)
  error = cause.message

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

// The Approvals view's intents. The guest names a proposal and an answer;
// the endpoint, the key and the write are this handler's, exactly as they
// were when the screen was native. An intent that names no proposal — a
// malformed one — is refused by the empty-id guard like an empty vote.
on governance_view_event(event)
  return if !connected || !empty(gov_voting) || empty(gov_event_proposal(event))
  gov_voting = gov_event_proposal(event)
  match gov_intent(event)
    GovIntent.vote
      run every governance_vote(connected_rpc, password, gov_voting, gov_event_approves(event)) -> gov_acted _ | gov_act_failed _
    GovIntent.execute
      run every governance_execute(connected_rpc, password, gov_voting) -> gov_acted _ | gov_act_failed _

// What the Members view asks of the app. `copy` is the same act as
// `copy_to_clipboard`; the ballot shares `gov_voting` with vote/execute — one
// governance write is in flight — and an agent's pause is an owner-gated
// write, immediate, no ballot.
on members_view_event(event)
  return if !connected
  match roster_intent(event)
    RosterIntent.copy
      toast = event_text(event, "label")
      toast_age = 0
      task clipboard write event_text(event, "text")
    RosterIntent.agent_status
      run every set_agent_status(connected_rpc, password, event_text(event, "agent_id"), event_flag(event, "paused")) -> agent_status_set _ | mutation_failed _
    RosterIntent.propose
      return if !empty(gov_voting) || empty(event_text(event, "key"))
      gov_voting = event_text(event, "key")
      run every governance_propose(connected_rpc, password, event_text(event, "action"), gov_voting) -> gov_acted _ | gov_act_failed _

on gov_acted(_result)
  gov_voting = ""
  gov_generation = gov_generation + 1
  run replace lane=governance_load load_governance(connected_rpc, gov_generation) -> governance_loaded _ | governance_failed _

on gov_act_failed(cause)
  gov_voting = ""
  error = cause.message

on members_loaded(next)
  return if next.generation != members_generation
  members_answered = true
  members_rows = next.members

on members_failed(cause)
  return if cause.generation != members_generation

// The DIRECT peer directory. Loaded with the workspace, because the sidebar
// section that reads it is on screen from the first frame.
on dm_peers_loaded(next)
  return if next.generation != dm_peers_generation
  dm_peers = next.peers
  // The directory decides which channels are DMs and who the header names, so
  // both mirrors move with it — see state/chat.ice's `rooms` note.
  rooms = chat_sidebar_rooms(channels, dm_peers, channel_reads)
  dm_rows = chat_sidebar_dms(channels, dm_peers, channel_reads)
  active_dm = dm_peer_named(dm_peers, active_dm_peer)

on dm_peers_failed(cause)
  return if cause.generation != dm_peers_generation

// What the Agents view asks of the app. A pause is the same owner-gated
// write the Members record offers; a save rewrites one record from the
// editor's whole draft; a register provisions the program account under the
// signing account and registers the draft against it. The endpoint, the key
// and the writes are this handler's; the view only ever hands over what the
// reader typed.
on agents_view_event(event)
  return if !connected
  match agents_intent(event)
    AgentsIntent.status
      run every set_agent_status(connected_rpc, password, event_text(event, "agent_id"), event_flag(event, "paused")) -> agent_status_set _ | mutation_failed _
    AgentsIntent.save
      run every save_agent(connected_rpc, password, event.detail) -> agent_status_set _ | mutation_failed _
    AgentsIntent.register
      run every register_agent(connected_rpc, password, account_number, event.detail) -> agent_status_set _ | mutation_failed _
    AgentsIntent.open_run
      agents_open_run = event_text(event, "run_id")
      agents_journal = empty_run_journal()
      run replace lane=agent_journal load_run_journal(connected_rpc, agents_open_run) -> agent_journal_loaded _ | agent_journal_failed _

// Every committed agent write lands here: pause, resume, save, register. The
// pause payload is the DESIRED state and it is named for the backend
// parameter it becomes: `true` PAUSES, `false` resumes. The registry is the
// authority on whether the signing owner may apply any of them. The register
// is re-read under a fresh generation, and `agents_committed` tells the view
// the drafts it held were consumed.
on agent_status_set(_result)
  agents_committed = agents_committed + 1
  agents_generation = agents_generation + 1
  error = ""
  run replace lane=agents_load load_agents(connected_rpc, agents_generation) -> agents_loaded _ | agents_failed _
