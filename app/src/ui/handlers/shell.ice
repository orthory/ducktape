// The Shell screen owns no parallel execution model. Both surfaces enter the
// already-shipped CLI contracts: PTY for raw input, sched + saga for chat.

on shell_mode_changed(next)
  return if shell_terminal_busy || shell_terminal_running || shell_chat_busy
  shell_mode = next
  shell_terminal_error = ""
  shell_chat_error = ""

on shell_provider_changed(next)
  return if shell_terminal_busy || shell_terminal_running || shell_chat_busy
  shell_provider = next
  shell_credential_options = agent_credential_names(shell_credentials, shell_provider)
  shell_credential = agent_credential_choice(shell_credentials, shell_provider, shell_credential)
  shell_terminal_error = ""
  shell_chat_error = ""

on shell_credential_changed(next)
  return if shell_terminal_busy || shell_terminal_running || shell_chat_busy
  shell_credential = next
  shell_terminal_error = ""
  shell_chat_error = ""

on shell_credentials_refresh
  return if !connected || shell_credentials_loading || shell_terminal_busy || shell_chat_busy
  shell_credentials_generation = shell_credentials_generation + 1
  shell_credentials_loading = true
  error = ""
  run replace lane=shell_credentials load_agent_credentials(connected_rpc, shell_credentials_generation) -> shell_credentials_loaded _ | shell_credentials_failed _

on shell_credentials_loaded(next)
  return if next.generation != shell_credentials_generation
  shell_credentials_loading = false
  shell_credentials = next.rows
  shell_credential_options = agent_credential_names(shell_credentials, shell_provider)
  shell_credential = agent_credential_choice(shell_credentials, shell_provider, shell_credential)

on shell_credentials_failed(cause)
  return if cause.generation != shell_credentials_generation
  shell_credentials_loading = false
  error = cause.message

on shell_terminal_start
  return if !connected || shell_terminal_busy || shell_terminal_running
  shell_terminal_busy = true
  shell_terminal_error = ""
  run replace lane=shell_terminal start_agent_terminal(connected_rpc, shell_provider, shell_credential) -> shell_terminal_started _ | shell_terminal_failed _

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
  return if !connected || shell_chat_busy || empty(shell_credential) || empty(trim(editor_text(shell_chat_draft)))
  let prompt = trim(editor_text(shell_chat_draft))
  shell_chat_entries = agent_chat_push_user(shell_chat_entries, prompt, shell_provider)
  shell_chat_draft = editor("")
  shell_chat_activity = []
  shell_chat_live = ""
  shell_chat_error = ""
  shell_chat_saga = ""
  shell_chat_status = "Thinking"
  shell_chat_detail = "Preparing the durable run"
  shell_chat_busy = true
  parallel
    stream replace lane=shell_chat agent_chat_turn(connected_rpc, shell_provider, shell_credential, shell_chat_entries) -> shell_chat_event _
    // `snap … 0.0`, not `snap-end`: the transcript is `anchor-y=end`, where
    // relative 0.0 IS the tail — `snap-end` (relative 1.0) lands at the TOP.
    task widget snap #workspace-tabs/content/shell/root/transcript 0.0 0.0 window=window_target(console_win)

// One pure reducer per field keeps this event handler flat. A progress event
// cannot accidentally settle the answer, and a terminal event clears every
// live-only surface in the same pass.
on shell_chat_event(next)
  shell_chat_activity = agent_activity_apply(shell_chat_activity, next)
  shell_chat_status = agent_event_status(shell_chat_status, next)
  shell_chat_detail = agent_event_detail(shell_chat_detail, next)
  shell_chat_saga = agent_event_saga(shell_chat_saga, next)
  shell_chat_live = agent_event_live(shell_chat_live, next)
  shell_chat_error = agent_event_error(shell_chat_error, next)
  shell_chat_entries = agent_event_entries(shell_chat_entries, next, shell_provider)
  shell_chat_busy = agent_event_busy(next)
  task widget snap #workspace-tabs/content/shell/root/transcript 0.0 0.0 window=window_target(console_win)

on shell_chat_reset
  return if shell_chat_busy
  invalidate lane=shell_chat
  shell_chat_entries = []
  shell_chat_activity = []
  shell_chat_draft = editor("")
  shell_chat_status = ""
  shell_chat_detail = ""
  shell_chat_live = ""
  shell_chat_error = ""
  shell_chat_saga = ""

on shell_chat_suggest(text)
  return if shell_chat_busy
  shell_chat_draft = editor(text)
