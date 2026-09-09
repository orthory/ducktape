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
  agents_capabilities = next.capabilities
  agents_actions = next.actions

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
    // THE EXPLICIT ASSOCIATION. The reader named a participant and a
    // conversation; the app reads them under this device's key on the network
    // it is connected to. Empty names close the panel, which is the same read
    // with nothing to read. Opening always lands at the tail.
    AgentsIntent.messaging_open
      messaging_participant = event_text(event, "participant")
      messaging_conversation = event_text(event, "conversation")
      messaging_send_error = ""
      messaging_loading = true
      run replace lane=messaging_load load_messaging(connected_rpc, network_chain_id, event_text(event, "participant"), event_text(event, "conversation"), 0, true) -> messaging_loaded _
    AgentsIntent.messaging_page
      messaging_loading = true
      run replace lane=messaging_load load_messaging(connected_rpc, network_chain_id, messaging_participant, messaging_conversation, event_int(event, "from_seq"), event_flag(event, "newest")) -> messaging_loaded _
    // The body crosses EXACTLY as it was written: this handler carries it, it
    // does not trim, shorten or normalize it. Over-length is refused by the
    // network, and the panel says so with the draft still in the box.
    AgentsIntent.messaging_send
      messaging_sending = true
      messaging_send_error = ""
      run every send_agent_message(connected_rpc, network_chain_id, messaging_participant, messaging_conversation, event_text(event, "kind"), event_text(event, "recipient"), event_text(event, "body"), event_int(event, "reply_to"), password) -> messaging_send_done _

// ONE READING, INSTALLED ONLY IN ITS OWN SCOPE. The read carries the endpoint
// and chain it ran against; a network switch, a participant change or another
// conversation opened while it was in flight makes it an answer about
// something else, and it is dropped rather than drawn under the new name.
on messaging_loaded(next)
  return if !messaging_in_scope(next, connected_rpc, network_chain_id, messaging_participant, messaging_conversation)
  messaging = next
  messaging_loading = false

// A SEND'S OUTCOME, IN THE SCOPE IT HAPPENED IN. A refusal keeps the composer's
// draft and touches nothing else — nothing is appended to the conversation on
// the way out, so a message appears when the network says it was admitted and
// not before.
on messaging_send_done(outcome)
  return if !messaging_send_in_scope(outcome, connected_rpc, network_chain_id, messaging_participant, messaging_conversation)
  messaging_sending = false
  messaging_send_error = outcome.refusal
  return if !empty(outcome.refusal)
  messaging_sent = messaging_sent + 1
  messaging_loading = true
  run replace lane=messaging_load load_messaging(connected_rpc, network_chain_id, messaging_participant, messaging_conversation, 0, true) -> messaging_loaded _

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
