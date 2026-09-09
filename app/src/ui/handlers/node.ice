// THIS NODE — the facts /v1/status publishes, its peers, its log stream, and
// the dedicated operator screen that draws them.

on node_log_line(line)
  node_log_timeline = node_log_timeline_push(node_log_timeline, line)

on node_view_event(event)
  match node_intent(event)
    NodeIntent.copy
      toast = event_text(event, "label")
      toast_age = 0
      task clipboard write event_text(event, "text")
    NodeIntent.tab
      node_tab = node_event_tab(event)
      return if node_tab != NodeTab.modules || !connected
      run replace lane=modules_load load_modules(connected_rpc) -> modules_loaded _ | modules_failed _
    NodeIntent.log_filter
      node_log_filter = event_text(event, "filter")
      node_log_timeline = node_log_timeline_filter(node_log_timeline, node_log_filter)
    NodeIntent.log_timeline
      node_log_timeline = node_log_timeline_drain(node_log_timeline)

on peers_loaded(next)
  return if next.generation != node_peers_generation
  node_peers = next.peers

on peers_failed(cause)
  return if cause.generation != node_peers_generation

// The consensus facts /v1/status already publishes and the console dropped:
// app-hash, view, quorum, reachable validators, finality and the gc watermark.
//
// `view`, `quorum` and `reachable_validators` arrive as `i64?` — a resident
// publishes no consensus block at all — and Ice cannot read an optional into an
// `i64`. `optional_number` is the seam: it renders the number, or `—` when the
// node genuinely has no reading, so the console prints an absence as an absence
// instead of as a measured zero.
on node_facts_loaded(next)
  node_key = next.public_key
  node_version = next.version
  node_root_hash = next.root_hash
  network_chain_id = next.chain_id
  // The title follows the chain the node serves — the network's own name,
  // the same on every member's screen.
  network_name = network_label(network_chain_id, connected_rpc)
  node_last_finalized = next.last_finalized_at
  node_checkpoint = next.checkpoint_height
  node_height = next.height
  node_view_label = optional_number(next.view)
  node_quorum_label = optional_number(next.quorum)
  node_reachable_label = optional_number(next.reachable_validators)
  node_phase = next.phase
  node_phase_since = next.phase_since
  node_sync_target = next.sync_target
  node_sync_applied = next.sync_applied
  node_sync_retries = next.sync_retries
  node_sync_failures = next.sync_failures
  node_sync_last_error = next.sync_last_error
  // THE OS HANDED THIS PROCESS A LINK, and this status is the first moment it
  // can be judged: the open plane refuses a link whose `?net=` names another
  // network, and `network_chain_id` — set from this same document above — is
  // what it compares against. Blanked before the run, so it is spent once and
  // no later reconnect re-opens it.
  // A launch link WAITS for that first status rather than being refused on
  // the way there: a poll that fails while the node is still coming up would
  // otherwise eat a link that opens fine a second later.
  // ponytail: so an app that never connects at all opens nothing and says
  // nothing — give the link its own visible pending/refused plate if that is
  // ever felt.
  let launch_link = startup_duck_link
  startup_duck_link = ""
  return if empty(launch_link)
  run every duck_echo_str(launch_link) -> open_message_link _ | external_url_failed _

on node_facts_failed(_cause)

// A PUSHED status document (lifecycle.ice's ungated subscription). It answers
// no request, so the freshest sample simply wins in the node's stream order.
on node_status_pushed(next)
  node_key = next.public_key
  node_version = next.version
  node_root_hash = next.root_hash
  network_chain_id = next.chain_id
  // The title follows the chain the node serves — the network's own name,
  // the same on every member's screen.
  network_name = network_label(network_chain_id, connected_rpc)
  node_last_finalized = next.last_finalized_at
  node_checkpoint = next.checkpoint_height
  node_height = next.height
  node_view_label = optional_number(next.view)
  node_quorum_label = optional_number(next.quorum)
  node_reachable_label = optional_number(next.reachable_validators)
  node_phase = next.phase
  node_phase_since = next.phase_since
  node_sync_target = next.sync_target
  node_sync_applied = next.sync_applied
  node_sync_retries = next.sync_retries
  node_sync_failures = next.sync_failures
  node_sync_last_error = next.sync_last_error

// The peers table's own push, from the tab-gated subscription beside it.
on node_peers_pushed(next)
  node_peers = next.peers

// Overview | Permissions | Activity | Modules on the Node rail surface. The
// log stream subscribes only while its tab is visible.
on settings_loaded(next)
  return if next.generation != settings_generation
  node_data_dir = next.data_dir
  settings_key_path = next.key_path
  settings_key_state = next.key_state
  settings_user_key = next.user_key
  // THIS DEVICE'S KEY — and the account it is bound to — DECIDES whether it
  // is seated in a members-only room. The facts load lands after the first
  // chat load, so without this the composer stayed refused until the next
  // delta.
  post_refusal = post_gate(active_channel_archived, active_channel_members_only, channel_members, settings_user_key)
  settings_open_tabs = next.open_tabs

on settings_failed(cause)
  return if cause.generation != settings_generation

// SETTINGS is a MODULE-OWNED VIEW (module_view.rs): the facts go in as props
// and every act comes back as ONE intent this handler signs. The four rail
// handlers Settings shares with the rest of the console (the tab, reconnect,
// the network switch, the theme) are reached by a `flow` so their bodies
// stay in one place; everything Settings alone does is an arm here. The
// drafts are the view's: an intent carries what the reader typed, and an op
// that consumed a draft says so through `settings_drafts_cleared`.
on settings_view_event(event)
  match settings_intent(event)
    SettingsIntent.tab
      flow
        from done settings_event_tab(event)
        done -> select_shell_tab _
    SettingsIntent.reconnect
      flow
        from done true
        done -> reconnect()
    SettingsIntent.switch_network
      flow
        from done true
        done -> switch_network()
    // IDENTITY KEY — the session's signing seat. Unlock VERIFIES the password
    // against user.key before keeping it; the old CONNECTION field stored
    // blind. Optimistically stored, cleared by the failure arm — the launch
    // window's unlock uses the same shape.
    SettingsIntent.unlock
      return if mutation_phase != MutationPhase.idle || empty(event_text(event, "password"))
      error = ""
      password = event_text(event, "password")
      run every unlock_user_key(connected_rpc, password) -> settings_unlocked _ | settings_unlock_failed _
    // Locking clears the password AND retires the session signer: the child
    // that holds the opened user key must not outlive the seat it was
    // opened for.
    SettingsIntent.lock
      password = ""
      signer_key = ""
      flow
        from run lock_signer()
        discard
    SettingsIntent.rename
      return if !connected || !account_exists || account_renaming || empty(event_text(event, "name"))
      account_renaming = true
      error = ""
      run every set_account_name(connected_rpc, password, event_text(event, "name")) -> account_renamed _ | account_rename_failed _
    // THE FOUR IDENTITY OPS — found, mint a ticket, join with one, remove a
    // key. Each is one user-signed frame (the CLI's `ducktape account` verbs,
    // in the app), and every committed one lands in `account_changed`
    // (handlers/roster.ice): the account picture moved, so it is re-read
    // under a fresh generation.
    //
    // FOUNDING FROM THE CONSOLE — the door for a device that passed the
    // welcome step's passkey enrolment by. It runs no recovery ceremony of
    // its own because the key it signs with cannot exist without one: the
    // launch window seals a minted key only after its 24 words are read back
    // (`handlers/onboarding.ice`), and the only other ways to hold one are a
    // restore, which IS 24 words typed in, and `ducktape wallet new`, which
    // prints them.
    SettingsIntent.create
      return if !connected || account_exists || account_busy || empty(password) || empty(event_text(event, "name"))
      account_busy = true
      error = ""
      run every create_account(connected_rpc, password, event_text(event, "name")) -> account_changed _ | account_op_failed _
    // A ticket is chain-scoped, so it carries the chain id the status stream
    // named (`network_chain_id`); the backend refuses to mint before one
    // landed.
    SettingsIntent.key_add
      return if !connected || !account_exists || account_busy || empty(password) || empty(event_text(event, "pubkey"))
      account_busy = true
      error = ""
      account_ticket = ""
      run every mint_key_ticket(connected_rpc, password, network_chain_id, event_text(event, "pubkey"), event_text(event, "label")) -> account_ticket_minted _ | account_op_failed _
    // Joining is the one op a key OUTSIDE every account performs, so it is
    // not gated on `account_exists`; a key already on an account is refused
    // by the module ("key already belongs to an account").
    SettingsIntent.join
      return if !connected || account_busy || empty(password) || empty(event_text(event, "ticket"))
      account_busy = true
      error = ""
      run every join_with_ticket(connected_rpc, password, event_text(event, "ticket")) -> account_changed _ | account_op_failed _
    SettingsIntent.key_remove
      return if !connected || !account_exists || account_busy || empty(password) || account_keys <= 1
      account_busy = true
      error = ""
      run every remove_account_key(connected_rpc, password, event_text(event, "pubkey")) -> account_changed _ | account_op_failed _
    // BROWSER CEREMONIES. Each opens the auth page and blocks on its answer;
    // `account_busy` holds the card until the page answers or the backend
    // gives up. The label names the new key, exactly as it names a pasted one.
    //
    // A passkey is registered FROM THE PHONE by default: the stream hands
    // back the QR the card shows, and `done`/`failed` close it. The desktop
    // browser path is the button beside it.
    SettingsIntent.passkey
      return if !connected || !account_exists || account_busy || empty(password)
      account_busy = true
      error = ""
      account_ceremony_phase = "working"
      account_ceremony_detail = "Preparing the passkey…"
      stream replace lane=account_ceremony add_passkey_by_qr(connected_rpc, password, network_chain_id, event_text(event, "label")) -> account_ceremony_stepped _
    SettingsIntent.passkey_desktop
      return if !connected || !account_exists || account_busy || empty(password)
      account_busy = true
      error = ""
      account_ceremony_phase = "working"
      account_ceremony_detail = "Continue in the browser…"
      run replace lane=account_desktop_ceremony register_passkey(connected_rpc, password, network_chain_id, event_text(event, "label")) -> account_changed _ | account_op_failed _
    SettingsIntent.ceremony_cancel
      invalidate lane=account_ceremony
      invalidate lane=account_desktop_ceremony
      account_busy = false
      account_ceremony_phase = ""
      account_ceremony_qr = ""
      account_ceremony_detail = ""
      account_ceremony_left = ""
    SettingsIntent.wallet
      return if !connected || !account_exists || account_busy || empty(password)
      account_busy = true
      error = ""
      account_ceremony_phase = "working"
      account_ceremony_detail = "Continue in the browser…"
      run replace lane=account_desktop_ceremony link_wallet(connected_rpc, password, network_chain_id, event_text(event, "label")) -> account_changed _ | account_op_failed _
    // Logging in is the other op a key OUTSIDE every account performs: a
    // passkey registered on a member device consents, in the browser, to
    // admitting this one.
    SettingsIntent.login
      return if !connected || account_exists || account_busy || empty(password)
      account_busy = true
      error = ""
      account_ceremony_phase = "working"
      account_ceremony_detail = "Continue in the browser…"
      run replace lane=account_desktop_ceremony login_with_passkey(connected_rpc, password, network_chain_id, "") -> account_changed _ | account_op_failed _
    SettingsIntent.copy
      toast = event_text(event, "label")
      toast_age = 0
      task clipboard write event_text(event, "text")
    SettingsIntent.clear_tabs
      doc_tabs = []
      run every clear_doc_tabs(connected_rpc) -> doc_tabs_saved _
    SettingsIntent.light
      flow
        from done true
        done -> set_appearance_light()
    SettingsIntent.dark
      flow
        from done true
        done -> set_appearance_dark()
    SettingsIntent.notifications
      desktop_notifications = event_flag(event, "enabled")
      run replace lane=notify_save save_desktop_notifications(desktop_notifications) -> desktop_notifications_saved _

on settings_unlocked(pubkey)
  error = ""
  // THE SEAT MOVED WITHOUT THE CONNECTION MOVING. Nothing draws this; the live
  // agent lane keys on it, because its entitlement to a run's output is this
  // key's and an unlock in place bumps no `connect_generation`.
  signer_key = pubkey

on settings_unlock_failed(cause)
  password = ""
  error = cause.message

// The app's one clipboard action: every Copy button routes here so the toast
// copy lives at the call site and the write itself stays native.
on copy_to_clipboard(text, label)
  toast = label
  toast_age = 0
  task clipboard write text

// One tick per 300ms while a toast shows: the age belongs to THIS toast
// (setters zero it), so every toast lives its full ~2.8s. The old shape was
// one shared 2800ms interval — a toast raised late in the window flashed
// and vanished.
on dismiss_toast
  toast = ""
  toast_age = 0

on toast_tick
  toast_age = toast_age + 1
  return if toast_age < 9
  toast = ""
  toast_age = 0

// The Modules tab picks its own seat AND fetches its own reading — a tab whose
// list is only filled by a refresh somewhere else opens empty on first click.
on modules_loaded(next)
  module_rows = next.rows
  error = ""

on modules_failed(cause)
  error = cause.message
