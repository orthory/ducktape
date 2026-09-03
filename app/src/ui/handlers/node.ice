// THIS NODE — the facts /v1/status publishes, its peers, its log stream, and
// the dedicated operator screen that draws them.

on node_log_line(line)
  node_log_timeline = node_log_timeline_push(node_log_timeline, line)

on node_log_filter_changed(next)
  node_log_filter = next
  node_log_timeline = node_log_timeline_filter(node_log_timeline, next)

on node_log_timeline_changed(event)
  node_log_timeline = node_log_timeline_apply(node_log_timeline, event)

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
on select_node_tab(tab)
  node_tab = tab

on settings_loaded(next)
  return if next.generation != settings_generation
  node_data_dir = next.data_dir
  settings_key_path = next.key_path
  settings_key_state = next.key_state
  settings_user_key = next.user_key
  // THIS DEVICE'S KEY DECIDES BOTH: which channels are its own DMs, and
  // whether it is seated in a members-only room. The facts load lands after the
  // first chat load, so without these the sidebar listed every DM under
  // CHANNELS and the composer stayed refused until the next delta.
  rooms = chat_sidebar_rooms(channels, dm_peers, channel_reads)
  dm_rows = chat_sidebar_dms(channels, dm_peers, channel_reads)
  post_refusal = post_gate(active_channel_archived, active_channel_members_only, channel_members, settings_user_key)
  settings_open_tabs = next.open_tabs

on settings_failed(cause)
  return if cause.generation != settings_generation

on settings_clear_tabs
  doc_tabs = []
  run every clear_doc_tabs(connected_rpc) -> doc_tabs_saved _

// IDENTITY KEY — the session's signing seat. Unlock VERIFIES the password
// against user.key before keeping it; the old CONNECTION field stored blind.
// Optimistically stored, cleared by the failure arm — the launch window's
// unlock uses the same shape.
on settings_unlock_submit(pw)
  return if mutation_phase != MutationPhase.idle || empty(pw)
  error = ""
  password = pw
  run every unlock_user_key(password) -> settings_unlocked _ | settings_unlock_failed _

on settings_unlocked(_pubkey)
  error = ""

on settings_unlock_failed(cause)
  password = ""
  error = cause.message

// Locking clears the password AND retires the session signer: the child that
// holds the opened user key must not outlive the seat it was opened for.
on lock_session
  password = ""
  flow
    from run lock_signer()
    discard

// PREFERENCES — device-local, one endpoint at a time.
// DANGER ZONE — forget this workspace on THIS DEVICE and go back to onboarding.
on forget_workspace_submit
  return if !connected || mutation_phase != MutationPhase.idle
  mutation_phase = MutationPhase.forget_workspace
  error = ""
  run every forget_workspace(connected_rpc) -> workspace_forgotten _ | mutation_failed _

// `forget_workspace` answers false when the prefs file could not be written.
// Throwing her out to onboarding on that answer meant the workspace was back in
// the picker at the next launch, looking like the app had ignored her.
// On success the launch window reopens; `onboarding_reopened`
// (handlers/onboarding.ice) closes the console once it is registered.
on workspace_forgotten(forgotten)
  mutation_phase = MutationPhase.idle
  error = "This device could not forget the workspace."
  return if !forgotten
  connected = false
  status = "Not connected"
  error = ""
  task window open onboarding -> onboarding_reopened _

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
on open_node_modules
  node_tab = NodeTab.modules
  return if !connected
  run replace lane=modules_load load_modules(connected_rpc) -> modules_loaded _ | modules_failed _

on modules_loaded(next)
  module_rows = next.rows
  error = ""

on modules_failed(cause)
  error = cause.message
