// WHO — members, registered agents, the DM directory, the account this device
// signs as, and the proposals the network is voting on.

on account_loaded(next)
  return if next.generation != account_generation
  account_exists = next.exists
  account_number = next.number
  account_name = next.name
  account_bio = next.bio
  account_keys = next.keys
  account_key_rows = next.key_rows

on account_failed(cause)
  return if cause.generation != account_generation

on account_name_draft_changed(next)
  account_name_draft = next

on account_rename_submit
  return if !connected || !account_exists || account_renaming || empty(trim(account_name_draft))
  account_renaming = true
  error = ""
  run every set_account_name(connected_rpc, password, trim(account_name_draft)) -> account_renamed _ | account_rename_failed _

on account_renamed(_result)
  account_renaming = false
  account_name_draft = ""
  account_generation = account_generation + 1
  run replace lane=account_load load_account(connected_rpc, account_generation) -> account_loaded _ | account_failed _

on account_rename_failed(cause)
  account_renaming = false
  error = cause.message

// THE FOUR IDENTITY OPS — found, mint a ticket, join with one, remove a key.
// Each is one user-signed frame (the CLI's `ducktape account` verbs, in the
// app), and every committed one lands in `account_changed`: the account
// picture moved, so it is re-read under a fresh generation.
on account_create_draft_changed(next)
  account_create_draft = next

// FOUNDING FROM THE CONSOLE — the door for a device that passed the welcome
// step's passkey enrolment by. It runs no recovery ceremony of its own
// because the key it signs with cannot exist without one: the launch window
// seals a minted key only after its 24 words are read back
// (`handlers/onboarding.ice`), and the only other ways to hold one are a
// restore, which IS 24 words typed in, and `ducktape wallet new`, which
// prints them.
on account_create_submit
  return if !connected || account_exists || account_busy || empty(password) || empty(trim(account_create_draft))
  account_busy = true
  error = ""
  run every create_account(connected_rpc, password, trim(account_create_draft)) -> account_changed _ | account_op_failed _

on account_key_draft_changed(next)
  account_key_draft = next

on account_key_label_draft_changed(next)
  account_key_label_draft = next

// A ticket is chain-scoped, so it carries the chain id the status stream
// named (`network_chain_id`); the backend refuses to mint before one landed.
on account_key_add_submit
  return if !connected || !account_exists || account_busy || empty(password) || empty(trim(account_key_draft))
  account_busy = true
  error = ""
  account_ticket = ""
  run every mint_key_ticket(connected_rpc, password, network_chain_id, trim(account_key_draft), trim(account_key_label_draft)) -> account_ticket_minted _ | account_op_failed _

// Minting commits nothing: the ticket is shown to copy, the drafts it
// consumed clear, and the account is re-read only when the OTHER device joins.
on account_ticket_minted(ticket)
  account_busy = false
  account_ticket = ticket
  account_key_draft = ""
  account_key_label_draft = ""

on account_join_draft_changed(next)
  account_join_draft = next

// Joining is the one op a key OUTSIDE every account performs, so it is not
// gated on `account_exists`; a key already on an account is refused by the
// module ("key already belongs to an account").
on account_key_join_submit
  return if !connected || account_busy || empty(password) || empty(trim(account_join_draft))
  account_busy = true
  error = ""
  run every join_with_ticket(connected_rpc, password, trim(account_join_draft)) -> account_changed _ | account_op_failed _

// BROWSER CEREMONIES. Each opens the auth page and blocks on its answer;
// `account_busy` holds the card until the page answers or the backend gives
// up. The label draft names the new key, exactly as it names a pasted one.
//
// A passkey is registered FROM THE PHONE by default: the stream hands back
// the QR the card shows, and `done`/`failed` close it. The desktop browser
// path is the button beside it.
on account_passkey_submit
  return if !connected || !account_exists || account_busy || empty(password)
  account_busy = true
  error = ""
  stream replace lane=account_ceremony add_passkey_by_qr(connected_rpc, password, network_chain_id, trim(account_key_label_draft)) -> account_ceremony_stepped _

on account_passkey_desktop
  return if !connected || !account_exists || account_busy || empty(password)
  account_busy = true
  error = ""
  run every register_passkey(connected_rpc, password, network_chain_id, trim(account_key_label_draft)) -> account_changed _ | account_op_failed _

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
      account_key_label_draft = ""
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

on account_ceremony_cancel
  invalidate lane=account_ceremony
  account_busy = false
  account_ceremony_phase = ""
  account_ceremony_qr = ""
  account_ceremony_detail = ""
  account_ceremony_left = ""

on account_wallet_submit
  return if !connected || !account_exists || account_busy || empty(password)
  account_busy = true
  error = ""
  run every link_wallet(connected_rpc, password, network_chain_id, trim(account_key_label_draft)) -> account_changed _ | account_op_failed _

// Logging in is the other op a key OUTSIDE every account performs: a passkey
// registered on a member device consents, in the browser, to admitting this one.
on account_login_submit
  return if !connected || account_exists || account_busy || empty(password)
  account_busy = true
  error = ""
  run every login_with_passkey(connected_rpc, password, network_chain_id, "") -> account_changed _ | account_op_failed _

on account_key_remove(pubkey)
  return if !connected || !account_exists || account_busy || empty(password) || account_keys <= 1
  account_busy = true
  error = ""
  run every remove_account_key(connected_rpc, password, pubkey) -> account_changed _ | account_op_failed _

on account_changed(_result)
  account_busy = false
  account_create_draft = ""
  account_join_draft = ""
  account_key_draft = ""
  account_key_label_draft = ""
  account_ticket = ""
  account_generation = account_generation + 1
  run replace lane=account_load load_account(connected_rpc, account_generation) -> account_loaded _ | account_failed _

on account_op_failed(cause)
  account_busy = false
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
  run every governance_vote(connected_rpc, password, gov_voting, approve) -> gov_acted _ | gov_act_failed _

on gov_execute(proposal_id)
  return if !connected || !empty(gov_voting)
  gov_voting = proposal_id
  run every governance_execute(connected_rpc, password, gov_voting) -> gov_acted _ | gov_act_failed _

// The quorum-gated membership actions the roster detail panel offers. They
// share `gov_voting` with vote/execute: one governance write is in flight.
on gov_propose(action, target_key)
  return if !connected || !empty(gov_voting)
  gov_voting = target_key
  run every governance_propose(connected_rpc, password, action, gov_voting) -> gov_acted _ | gov_act_failed _

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

// The invite modal is pure view state — minting is a separate, explicit act.
// Pause or resume an agent. The payload is the DESIRED state and it is named
// for the backend parameter it becomes: `true` PAUSES, `false` resumes. The
// roster's Pause control passes `true` and its Resume control passes `false`;
// a row wired from `agent.status` would have to invert. The registry is the
// authority on whether the signing owner may apply the requested state.
on agent_set_status(agent_id, paused)
  return if !connected
  run every set_agent_status(connected_rpc, password, agent_id, paused) -> agent_status_set _ | mutation_failed _

on agent_status_set(_result)
  agents_generation = agents_generation + 1
  error = ""
  run replace lane=agents_load load_agents(connected_rpc, agents_generation) -> agents_loaded _ | agents_failed _
