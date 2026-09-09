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
  bell_marking = bell_marking && account_number == next.number
  bell_items = bell_account_items(bell_items, account_number, next.number)
  bell_presentations = merge_bell_presentations(bell_visible_items(bell_items, next.number, settings_user_key), bell_presentations, [])
  bell_unread = bell_unread_count(bell_items, next.number, settings_user_key)
  bell_error = ""
  bell_read_through = keep_i64(account_number == next.number, bell_read_through, 0)
  bell_clear_through = keep_i64(account_number == next.number, bell_clear_through, 0)
  invalidate lane=bell_context
  invalidate lane=bell_mark
  invalidate lane=bell_navigation
  bell_marking = false
  account_number = next.number
  account_name = next.name
  account_bio = next.bio
  account_keys = next.keys
  account_key_rows = next.key_rows
  run replace lane=bell_load load_bell(connected_rpc, account_number) -> bell_loaded connect_generation next.number _ | bell_failed connect_generation next.number _

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
  agents_journal_op = agents_journal_op + 1
  run replace lane=agent_journal load_run_journal(connected_rpc, network_chain_id, connect_generation, account_number, agents_journal_op, agents_open_run) -> agent_journal_loaded _

// THE JOURNAL READ, INSTALLED ONLY IN ITS OWN SCOPE. The run id is the
// subject, not the identity: two networks can carry the same one, and a
// reconnect to the same endpoint is a different session — so a read started on
// A, answering after the app moved to B with that run still open, would install
// A's journal under B. Success AND refusal meet the same fence, which is why
// the read is infallible and carries its scope in the answer: an error arm has
// nowhere to put one.
// THE RUN PANEL, ON ONE RUN. Every door that opens a run — the runs list, a
// chat hint's "View run", the run chip on a message a run posted, a bell, a
// duck://run link — comes through here: the agents tab, the run named, and
// its journal read under a fresh op so a slower earlier read cannot land over
// it. An empty id closes the panel.
on open_run_panel(dispatch_id)
  shell_tab = ShellTab.agents
  agents_open_run = dispatch_id
  agents_journal = empty_run_journal()
  agents_journal_op = agents_journal_op + 1
  run replace lane=agent_journal load_run_journal(connected_rpc, network_chain_id, connect_generation, account_number, agents_journal_op, agents_open_run) -> agent_journal_loaded _

on agent_journal_loaded(next)
  return if !journal_in_scope(next, connected_rpc, network_chain_id, connect_generation, account_number, agents_journal_op, agents_open_run)
  agents_journal = next
  return if empty(next.error)
  error = next.error

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
      flow
        from done event_text(event, "dispatch_id")
        done -> open_run_panel _
    // A CHIP IS A LINK. The run panel's places carry duck:// addresses, and
    // the open plane in `handlers/chat.ice` is the one place a link becomes
    // navigation, whichever tab it was pressed on.
    AgentsIntent.open_link
      flow
        from done event_text(event, "url")
        done -> open_message_link _
    // THE EXPLICIT ASSOCIATION. The reader named a participant and a
    // conversation; the app reads them under this device's key on the network
    // it is connected to. Empty names close the panel, which is the same read
    // with nothing to read. Opening always lands at the tail.
    AgentsIntent.messaging_open
      messaging_participant = event_text(event, "participant")
      messaging_conversation = event_text(event, "conversation")
      messaging_send_error = ""
      // A SEND IN FLIGHT WHEN THE SCOPE MOVES NEVER ANSWERS INTO IT: its
      // outcome is dropped by the fence below, so the flag it set would stay
      // raised and leave the composer dead. Opening (or closing) the panel is
      // the reset. The message may still land — the outbox is what makes that
      // safe, not this flag.
      messaging_sending = false
      messaging_loading = true
      messaging_load_op = messaging_load_op + 1
      run replace lane=messaging_load load_messaging(connected_rpc, network_chain_id, connect_generation, account_number, messaging_load_op, event_text(event, "participant"), event_text(event, "conversation"), 0, true) -> messaging_loaded _
    AgentsIntent.messaging_page
      messaging_loading = true
      messaging_load_op = messaging_load_op + 1
      run replace lane=messaging_load load_messaging(connected_rpc, network_chain_id, connect_generation, account_number, messaging_load_op, messaging_participant, messaging_conversation, event_int(event, "from_seq"), event_flag(event, "newest")) -> messaging_loaded _
    // The body crosses EXACTLY as it was written: this handler carries it, it
    // does not trim, shorten or normalize it. Over-length is refused by the
    // network, and the panel says so with the draft still in the box.
    //
    // ONE SEND AT A TIME FROM THIS PANEL. Two in flight allocate against the
    // same credential and race for a sequence, and their outcomes can land in
    // either order — so a repeated intent is refused at dispatch rather than
    // settled later. The button is already disabled while sending; this is the
    // half that does not depend on a view drawing itself correctly.
    AgentsIntent.messaging_send
      return if messaging_sending
      messaging_sending = true
      messaging_send_error = ""
      messaging_send_op = messaging_send_op + 1
      run every send_agent_message(connected_rpc, network_chain_id, connect_generation, account_number, messaging_send_op, messaging_participant, messaging_conversation, event_text(event, "kind"), event_text(event, "recipient"), event_text(event, "body"), event_int(event, "reply_to"), password) -> messaging_send_done _

// ONE READING, INSTALLED ONLY IN ITS OWN SCOPE. The read carries the endpoint
// and chain it ran against; a network switch, a participant change or another
// conversation opened while it was in flight makes it an answer about
// something else, and it is dropped rather than drawn under the new name.
on messaging_loaded(next)
  return if !messaging_in_scope(next, connected_rpc, network_chain_id, connect_generation, account_number, messaging_load_op, messaging_participant, messaging_conversation)
  messaging = next
  messaging_loading = false

// A SEND'S OUTCOME, IN THE SCOPE IT HAPPENED IN. A refusal keeps the composer's
// draft and touches nothing else — nothing is appended to the conversation on
// the way out, so a message appears when the network says it was admitted and
// not before.
on messaging_send_done(outcome)
  return if !messaging_send_in_scope(outcome, connected_rpc, network_chain_id, connect_generation, account_number, messaging_send_op, messaging_participant, messaging_conversation)
  messaging_sending = false
  messaging_send_error = outcome.refusal
  return if !empty(outcome.refusal)
  messaging_sent = messaging_sent + 1
  messaging_loading = true
  messaging_load_op = messaging_load_op + 1
  run replace lane=messaging_load load_messaging(connected_rpc, network_chain_id, connect_generation, account_number, messaging_load_op, messaging_participant, messaging_conversation, 0, true) -> messaging_loaded _

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
