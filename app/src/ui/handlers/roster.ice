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
  run replace lane=bell_load load_bell(connected_rpc, account_number) -> bell_loaded connect_generation next.number _ | bell_failed connect_generation next.number _

on account_failed(cause)
  return if cause.generation != account_generation

// THE SETTINGS VIEW SENT THE OP (`settings_view_event`, handlers/node.ice);
// what lands here is its answer. The op frees the card and the account is
// re-read; which drafts it spent is the view's own reading of the facts that
// moved — a name that came back is a rename that landed.
on account_renamed(_result)
  account_busy = false
  account_generation = account_generation + 1
  run replace lane=account_load load_account(connected_rpc, account_generation) -> account_loaded _ | account_failed _

on account_rename_failed(cause)
  account_busy = false
  error = cause.message

// Minting commits nothing: the ticket is shown to copy, and the account is
// re-read only when the OTHER device joins.
on account_ticket_minted(ticket)
  account_busy = false
  account_ticket = ticket

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
// is re-read under a fresh generation, and the ticket goes with it — one
// left on screen after its device joined is a stale blob that looks like a
// secret.
on account_changed(_result)
  account_ceremony_phase = ""
  account_ceremony_qr = ""
  account_ceremony_detail = ""
  account_ceremony_left = ""
  account_busy = false
  account_ticket = ""
  account_generation = account_generation + 1
  run replace lane=account_load load_account(connected_rpc, account_generation) -> account_loaded _ | account_failed _

on account_op_failed(cause)
  account_ceremony_phase = ""
  account_ceremony_qr = ""
  account_ceremony_detail = ""
  account_ceremony_left = ""
  account_busy = false
  error = cause.message

// THE RUN PANEL, ON ONE RUN. Every door that opens a run — the runs list, a
// chat hint's "View run", the run chip on a message a run posted, a bell, a
// duck://run link — comes through here: the agents tab and the run named,
// which the session push carries to the view. The journal itself is the
// view's own read. An empty id closes the panel.
on open_run_panel(dispatch_id)
  invalidate lane=account_ceremony
  invalidate lane=account_desktop_ceremony
  account_busy = account_busy && empty(account_ceremony_phase)
  account_ceremony_phase = ""
  account_ceremony_qr = ""
  account_ceremony_detail = ""
  account_ceremony_left = ""
  // Same tab-move rule as `select_shell_tab`; uniform on purpose, so no door
  // has to prove which tab it was pressed on before trusting the retire.
  shell_tab = ShellTab.agents
  agents_open_run = dispatch_id
  agents_opened = agents_opened + 1

// The Approvals view speaks the kernel contract: its reads and writes go
// through the kernel, and the one event it hands the app is the tab badge.
on governance_view_event(event)
  return if event.kind != "badge"
  gov_open = event_int(event, "count")

// What the Members view asks of the app. It speaks the kernel contract for
// everything it reads and signs; the clipboard is the one OS door left, and
// it is the same act as `copy_to_clipboard`.
on members_view_event(event)
  return if !connected || event.kind != "copy"
  toast = event_text(event, "label")
  toast_age = 0
  task clipboard write event_text(event, "text")

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

// What the Agents view still asks of the app. Its reads and its own writes
// go through the kernel; what is left is whether any of its agents is
// working (the rail's pulse), the ONE write it cannot sign — a registration
// first provisions the agent's program account and binds the runs module's
// own composed program — and two navigations.
on agents_view_event(event)
  return if !connected
  match agents_intent(event)
    AgentsIntent.badge
      agents_live = event_int(event, "count") > 0
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

// A committed agent write lands here: an agent's pause from the Members
// record, or a registration from the Agents view. The pause payload is the
// DESIRED state and it is named for the backend parameter it becomes:
// `true` PAUSES, `false` resumes. The registry is the authority on whether
// the signing owner may apply either. The register itself is the Agents
// view's to re-read: the block the write lands in reaches it through
// `rpc.live`.
on agent_status_set(_result)
  error = ""
