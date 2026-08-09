// THIS NODE — the facts /v1/status publishes, its peers, its log stream, the
// settings screen behind them, and leaving the workspace.

on node_log_line(line)
  node_log_lines = push_log_line(node_log_lines, line)

on node_log_filter_changed(next)
  node_log_filter = next

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
  return if next.generation != node_facts_generation
  node_version = next.version
  node_root_hash = next.root_hash
  node_last_finalized = next.last_finalized_at
  node_checkpoint = next.checkpoint_height
  node_height = next.height
  node_view_label = optional_number(next.view)
  node_quorum_label = optional_number(next.quorum)
  node_reachable_label = optional_number(next.reachable_validators)

on node_facts_failed(cause)
  return if cause.generation != node_facts_generation

// A PUSHED overview sample (lifecycle.ice's gated subscription). Each frame
// answers ONE of the two snapshot topics, so every field takes its `answered`
// flag — a peers frame must not blank the consensus facts, and a status frame
// must not empty the table.
//
// NO GENERATION GUARD, and that is not an omission: a generation retires a
// stale REPLY to a request this app made, and a push answers no request. The
// freshest sample simply wins, which is what the node's own ordering already
// gives.
on node_overview_sample(next)
  node_peers = keep_peers(next.peers_answered, next.peers, node_peers)
  node_version = keep_str(next.facts_answered, next.facts.version, node_version)
  node_root_hash = keep_str(next.facts_answered, next.facts.root_hash, node_root_hash)
  node_last_finalized = keep_i64(next.facts_answered, next.facts.last_finalized_at, node_last_finalized)
  node_checkpoint = keep_i64(next.facts_answered, next.facts.checkpoint_height, node_checkpoint)
  node_height = keep_i64(next.facts_answered, next.facts.height, node_height)
  node_view_label = keep_str(next.facts_answered, optional_number(next.facts.view), node_view_label)
  node_quorum_label = keep_str(next.facts_answered, optional_number(next.facts.quorum), node_quorum_label)
  node_reachable_label = keep_str(next.facts_answered, optional_number(next.facts.reachable_validators), node_reachable_label)

// Overview | Permissions | Activity, inside Settings now that the Node rail
// seat is gone. The log stream below subscribes on this tab.
on select_node_tab(tab)
  node_tab = tab

on settings_loaded(next)
  return if next.generation != settings_generation
  settings_endpoint = next.endpoint
  settings_node_key = next.node_key
  settings_data_dir = next.data_dir
  settings_key_path = next.key_path
  settings_key_state = next.key_state
  settings_user_key = next.user_key
  settings_open_tabs = next.open_tabs

on settings_failed(cause)
  return if cause.generation != settings_generation

on settings_clear_tabs
  doc_tabs = []
  run clear_doc_tabs(connected_rpc) -> doc_tabs_saved _

// IDENTITY KEY — the session's signing seat. Unlock VERIFIES the password
// against user.key before keeping it; the old CONNECTION field stored blind.
// Optimistically stored, cleared by the failure arm — the launch window's
// unlock uses the same shape.
on settings_unlock_submit(pw)
  return if mutation_phase != "idle" || empty(pw)
  error = ""
  password = pw
  run unlock_user_key(password) -> settings_unlocked _ | settings_unlock_failed _

on settings_unlocked(_pubkey)
  error = ""

on settings_unlock_failed(cause)
  password = ""
  error = cause.message

on lock_session
  password = ""

// PREFERENCES — device-local, one endpoint at a time.
// DANGER ZONE — forget this workspace on THIS DEVICE and go back to onboarding.
on forget_workspace_submit
  return if !connected || mutation_phase != "idle"
  mutation_phase = "forget-workspace"
  error = ""
  run forget_workspace(connected_rpc) -> workspace_forgotten _ | mutation_failed _

// `forget_workspace` answers false when the prefs file could not be written.
// Throwing her out to onboarding on that answer meant the workspace was back in
// the picker at the next launch, looking like the app had ignored her.
// On success the launch window reopens; `onboarding_reopened`
// (handlers/onboarding.ice) closes the console once it is registered.
on workspace_forgotten(forgotten)
  mutation_phase = "idle"
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
  toast_tone = "info"
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

// Held here beside the loaders that fill them.
state
  module_rows:[ModuleRow] = []
  module_generation:i64 = 0

// The Modules tab picks its own seat AND fetches its own reading — a tab whose
// list is only filled by a refresh somewhere else opens empty on first click.
on open_node_modules
  node_tab = "modules"
  return if !connected
  module_generation = module_generation + 1
  run load_modules(connected_rpc, module_generation) -> modules_loaded _ | modules_failed _

on modules_loaded(next)
  return if next.generation != module_generation
  module_rows = next.rows
  error = ""

on modules_failed(cause)
  return if cause.generation != module_generation
  error = cause.message
