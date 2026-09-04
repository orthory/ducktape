// The Shell screen owns no parallel execution model. Both surfaces enter the
// already-shipped CLI contracts: PTY for an interactive session, sched + saga
// for a durable task.
//
// NOTHING HERE REFUSES A SWITCH. The old guards (`return if … busy`) locked the
// surface toggle, the pickers and the reset behind whatever was running, which
// meant a stalled run wedged the whole tab — the one state with no way out. A
// run in flight is a reason to keep the CURRENT run's inputs stable, not a
// reason to trap the operator in it, so the switch is always live and every
// exit (`shell_chat_detach`, `shell_terminal_stop`) is reachable at any time.

on shell_surface_changed(next)
  shell_surface = next

on shell_setup_toggled
  shell_setup_open = !shell_setup_open

// The identity is ONE pick that answers two questions, so it settles both
// fields — and the host list narrows to the peers that can serve the provider
// it names.
on shell_identity_changed(next)
  shell_identity = next
  shell_provider = agent_identity_provider(shell_identities, next)
  shell_credential = agent_identity_credential(shell_identities, next)
  shell_host_node_options = agent_host_node_options(shell_host_nodes, shell_provider, shell_credential)
  shell_host_node = agent_host_node_choice(shell_host_node_options, shell_host_node)
  shell_host_node_key = agent_host_node_key(shell_host_nodes, shell_host_node)
  shell_terminal_error = ""

on shell_host_node_changed(next)
  shell_host_node = next
  shell_host_node_key = agent_host_node_key(shell_host_nodes, next)
  shell_terminal_error = ""

// The COMPUTE reads ask two questions of the same visit — which credentials
// this operator holds, and which peers announced a provider — so both reads are
// issued together and share the visit's generation.
on shell_credentials_refresh
  return if !connected || shell_credentials_loading
  shell_credentials_generation = shell_credentials_generation + 1
  shell_credentials_loading = true
  error = ""
  parallel
    run replace lane=shell_credentials load_agent_credentials(connected_rpc, shell_credentials_generation) -> shell_credentials_loaded _ | shell_credentials_failed _
    run replace lane=shell_host_nodes load_agent_host_nodes(connected_rpc, shell_credentials_generation) -> shell_host_nodes_loaded _ | shell_host_nodes_failed _

on shell_credentials_loaded(next)
  return if next.generation != shell_credentials_generation
  shell_credentials_loading = false
  shell_credentials = next.rows
  shell_identities = agent_identities(shell_credentials)
  shell_identity_options = agent_identity_options(shell_identities)
  shell_identity = agent_identity_choice(shell_identities, shell_identity)
  shell_provider = agent_identity_provider(shell_identities, shell_identity)
  shell_credential = agent_identity_credential(shell_identities, shell_identity)
  shell_host_node_options = agent_host_node_options(shell_host_nodes, shell_provider, shell_credential)
  shell_host_node = agent_host_node_choice(shell_host_node_options, shell_host_node)
  shell_host_node_key = agent_host_node_key(shell_host_nodes, shell_host_node)

on shell_credentials_failed(cause)
  return if cause.generation != shell_credentials_generation
  shell_credentials_loading = false
  error = cause.message

// A registry that dropped the picked peer must not keep sending work to it:
// the key is re-derived from the fresh rows, and an absent row resolves to ""
// — the connected node.
on shell_host_nodes_loaded(next)
  return if next.generation != shell_credentials_generation
  shell_host_nodes = next.rows
  shell_host_node_options = agent_host_node_options(shell_host_nodes, shell_provider, shell_credential)
  shell_host_node = agent_host_node_choice(shell_host_node_options, shell_host_node)
  shell_host_node_key = agent_host_node_key(shell_host_nodes, shell_host_node)

on shell_host_nodes_failed(cause)
  return if cause.generation != shell_credentials_generation
  error = cause.message

on shell_terminal_start
  return if !connected || shell_terminal_busy || shell_terminal_running
  shell_terminal_busy = true
  shell_terminal_error = ""
  run replace lane=shell_terminal start_agent_terminal(connected_rpc, shell_provider, shell_credential, shell_host_node_key) -> shell_terminal_started _ | shell_terminal_failed _

on shell_terminal_started(next)
  shell_terminal = next.session
  shell_terminal_title = next.title
  shell_terminal_running = true
  shell_terminal_busy = false
  task focus_agent_terminal(shell_terminal) -> shell_terminal_focused

on shell_terminal_focused

on shell_terminal_stop
  shell_terminal = idle_agent_terminal()
  shell_terminal_running = false
  shell_terminal_busy = false
  shell_terminal_title = ""

on shell_terminal_notice(next)
  shell_terminal_running = next.running
  shell_terminal_title = keep_str(!empty(next.title), next.title, shell_terminal_title)
  return if shell_terminal_running
  shell_terminal = idle_agent_terminal()

on shell_terminal_failed(cause)
  shell_terminal_busy = false
  shell_terminal_running = false
  shell_terminal_error = cause.message

on shell_composer_event(event)
  shell_chat_draft = apply_composer_event(shell_chat_draft, event)
  return if !composer_submits(event)
  return if !connected || shell_chat_busy || empty(shell_credential) || !empty(shell_detached_saga)
  return if empty(trim(editor_text(shell_chat_draft)))
  let prompt = trim(editor_text(shell_chat_draft))
  shell_chat_entries = agent_chat_push_user(shell_chat_entries, prompt, shell_provider)
  shell_chat_draft = editor("")
  shell_chat_activity = []
  shell_chat_live = ""
  shell_chat_saga = ""
  shell_chat_status = "Thinking"
  shell_chat_detail = "Preparing the durable run"
  shell_chat_busy = true
  shell_setup_open = false
  parallel
    stream replace lane=shell_chat agent_chat_turn(connected_rpc, shell_provider, shell_credential, shell_host_node_key, shell_chat_entries) -> shell_chat_event _
    // The transcript is `anchor-y=end`, where relative 0.0 is the tail.
    task widget snap #workspace-tabs/content/shell/root/transcript 0.0 0.0 window=window_target(console_win)

// One pure reducer per field keeps this event handler flat. A progress event
// cannot accidentally settle the answer, and a terminal event folds the live
// surfaces INTO the turn they belong to — the work it did and the run id behind
// it settle with the answer instead of being wiped by the next prompt.
on shell_chat_event(next)
  shell_chat_activity = agent_activity_apply(shell_chat_activity, next)
  shell_chat_status = agent_event_status(shell_chat_status, next)
  shell_chat_detail = agent_event_detail(shell_chat_detail, next)
  shell_chat_saga = agent_event_saga(shell_chat_saga, next)
  shell_chat_live = agent_event_live(shell_chat_live, next)
  shell_chat_entries = agent_event_entries(shell_chat_entries, next, shell_provider, shell_chat_saga, shell_chat_activity)
  shell_chat_busy = agent_event_busy(next)
  task widget snap #workspace-tabs/content/shell/root/transcript 0.0 0.0 window=window_target(console_win)

// STOP WATCHING, NOT STOP RUNNING — and the difference is the whole point of a
// durable run. The saga keeps executing, retries and commits whether or not
// this app holds a socket on it, so the turn closes on the run id that reaches
// it again rather than on a lie about having cancelled anything.
on shell_chat_detach
  return if !shell_chat_busy || empty(shell_chat_saga)
  invalidate lane=shell_chat
  shell_chat_entries = agent_chat_detach(shell_chat_entries, shell_provider, shell_chat_saga, shell_chat_activity)
  shell_detached_saga = shell_chat_saga
  shell_chat_busy = false
  shell_chat_activity = []
  shell_chat_status = ""
  shell_chat_detail = ""
  shell_chat_live = ""

on shell_chat_reopen
  return if shell_chat_busy || !connected || empty(shell_detached_saga)
  let saga = shell_detached_saga
  shell_chat_entries = agent_chat_drop_detached(shell_chat_entries)
  shell_detached_saga = ""
  shell_chat_saga = saga
  shell_chat_activity = []
  shell_chat_live = ""
  shell_chat_status = "Reattaching"
  shell_chat_detail = "Reading the run this node already committed to"
  shell_chat_busy = true
  stream replace lane=shell_chat agent_chat_watch(connected_rpc, shell_provider, saga) -> shell_chat_event _

on shell_chat_discard
  return if shell_chat_busy
  shell_chat_entries = agent_chat_drop_detached(shell_chat_entries)
  shell_detached_saga = ""
  shell_chat_saga = ""

on shell_chat_steps_toggled(id)
  shell_steps_open = keep_i64(shell_steps_open == id, 0, id)

on shell_chat_reset
  return if shell_chat_busy
  invalidate lane=shell_chat
  shell_chat_entries = []
  shell_chat_activity = []
  shell_chat_draft = editor("")
  shell_chat_status = ""
  shell_chat_detail = ""
  shell_chat_live = ""
  shell_chat_saga = ""
  shell_detached_saga = ""
  shell_steps_open = 0
