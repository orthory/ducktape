// SHELL, as a module-owned view: "run an agent on this network", answered by
// a durable task transcript and a terminal session, drawn from the facts the
// desktop app pushes. The screen body is the app's own (screens/shell.ice
// before the port). Three things only the host can draw stay the host's, as
// surfaces the view leaves slots for: the composer (the words never cross
// the wire — a send is the host's own intent), the terminal, and the answer
// Markdown. Every other act leaves as an intent the app runs.
app ShellView
  title "Shell"
  palette active_palette
  id "dev.ducktape.view.shell"
  text-size 13.5

use "../../../../../app/src/ui/theme.ice"
use "../../../../../app/src/ui/ducktape-ui/recipes.ice"
use "../../../../../app/src/ui/components/icon.ice"
use "shell.ice"
use "kit.ice"

extern crate::host
  HostError(message:str)
  AgentActivity(id:i64, title:str, detail:str, status:str)
  AgentChatEntry(id:i64, role:str, body:str, provider_label:str, provider_initial:str, status:str, run_label:str, steps:[AgentActivity], steps_label:str)
  ShellProps(dark:bool, connected:bool, surface:str, setup_open:bool, identity_options:[str], identity:str, provider_initial:str, credential:str, host_node_options:[str], host_node:str, credentials_loading:bool, terminal_running:bool, terminal_busy:bool, terminal_title:str, terminal_error:str, entries:[AgentChatEntry], activity:[AgentActivity], chat_busy:bool, chat_status:str, chat_detail:str, live:str, saga_id:str, detached_saga:str, run_line:str, grant_note:str, terminal_note:str, composer_hint:str, task_blurb:str, register_hint:str)
  stream props() -> ShellProps ! HostError
  pure show_surface(surface:&str) -> bool
  pure toggle_setup() -> bool
  pure pick_identity(value:&str) -> bool
  pure pick_host_node(value:&str) -> bool
  pure refresh_credentials() -> bool
  pure start_terminal() -> bool
  pure stop_terminal() -> bool
  pure reset_chat() -> bool
  pure detach_run() -> bool
  pure reopen_run() -> bool
  pure discard_run() -> bool
  pure open_link(url:&str) -> bool
  pure icon(name:&str) -> bytes
  pure toggle_fold(open:i64, id:i64) -> i64
  // the host's surfaces: the app's terminal for the session it holds, the
  // answer Markdown (a link it opens comes back here), and the composer
  component agent_terminal_surface() -> unit
  component agent_markdown(source:str, dark:bool) -> str
  component shell_composer(hint:str, disabled:bool) -> unit

state
  active_palette:palette[AppTheme] = AppTheme.app
  connected = false
  surface = "tasks"
  setup_open = false
  identity_options:[str] = []
  identity = ""
  provider_initial = ""
  credential = ""
  host_node_options:[str] = []
  host_node = ""
  credentials_loading = false
  terminal_running = false
  terminal_busy = false
  terminal_title = ""
  terminal_error = ""
  entries:[AgentChatEntry] = []
  activity:[AgentActivity] = []
  chat_busy = false
  chat_status = ""
  chat_detail = ""
  live = ""
  saga_id = ""
  detached_saga = ""
  run_line = ""
  grant_note = ""
  terminal_note = ""
  composer_hint = ""
  task_blurb = ""
  register_hint = ""
  dark = false
  // the reader's own: which settled turn has its work open (0 is none)
  steps_open:i64 = 0
  host_error = ""
  // a write's acknowledgement — `host::notify` returns nothing to bind
  sent = false

on mount
  stream every props() -> props_changed _ | props_failed _

on props_changed(next)
  connected = next.connected
  surface = next.surface
  setup_open = next.setup_open
  identity_options = next.identity_options
  identity = next.identity
  provider_initial = next.provider_initial
  credential = next.credential
  host_node_options = next.host_node_options
  host_node = next.host_node
  credentials_loading = next.credentials_loading
  terminal_running = next.terminal_running
  terminal_busy = next.terminal_busy
  terminal_title = next.terminal_title
  terminal_error = next.terminal_error
  entries = next.entries
  activity = next.activity
  chat_busy = next.chat_busy
  chat_status = next.chat_status
  chat_detail = next.chat_detail
  live = next.live
  saga_id = next.saga_id
  detached_saga = next.detached_saga
  run_line = next.run_line
  grant_note = next.grant_note
  terminal_note = next.terminal_note
  composer_hint = next.composer_hint
  task_blurb = next.task_blurb
  register_hint = next.register_hint
  dark = next.dark
  active_palette = AppTheme.app
  return if !next.dark
  active_palette = AppTheme.app_dark

on props_failed(error)
  host_error = error.message

on surface_changed(next)
  sent = show_surface(next)

on setup_toggled
  sent = toggle_setup()

on identity_changed(next)
  sent = pick_identity(next)

on host_node_changed(next)
  sent = pick_host_node(next)

on credentials_refresh
  sent = refresh_credentials()

on terminal_start
  sent = start_terminal()

on terminal_stop
  sent = stop_terminal()

on chat_reset
  sent = reset_chat()

on chat_detach
  sent = detach_run()

on chat_reopen
  sent = reopen_run()

on chat_discard
  sent = discard_run()

on steps_toggled(id)
  steps_open = toggle_fold(steps_open, id)

on link_opened(url)
  sent = open_link(url)

view
  box #root
    with
      w=fill
      h=fill
      bg=bg
    ShellScreen #shell
      with
        surface
        setup_open
        identity_options
        identity
        provider_initial
        credential
        host_node_options
        host_node
        credentials_loading
        terminal_running
        terminal_busy
        terminal_title
        terminal_error
        entries
        activity
        chat_busy
        chat_status
        chat_detail
        live
        saga_id
        steps_open
        detached_saga
        run_line
        grant_note
        terminal_note
        composer_hint
        task_blurb
        register_hint
        connected
        dark
      events
        shell_surface_changed -> surface_changed _
        shell_setup_toggled -> setup_toggled
        shell_identity_changed -> identity_changed _
        shell_host_node_changed -> host_node_changed _
        shell_credentials_refresh -> credentials_refresh
        shell_terminal_start -> terminal_start
        shell_terminal_stop -> terminal_stop
        shell_chat_reset -> chat_reset
        shell_chat_detach -> chat_detach
        shell_chat_reopen -> chat_reopen
        shell_chat_discard -> chat_discard
        shell_chat_steps_toggled -> steps_toggled _
        shell_open_link -> link_opened _
