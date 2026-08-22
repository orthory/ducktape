state
  shell_tab:ShellTab = ShellTab.chat
  shell_surface:ShellSurface = ShellSurface.tasks
  // WHO runs the work, asked ONCE. `--cred` decides the provider (the CLI
  // refuses one that contradicts it), so the screen picks a credential and
  // reads the provider off it — it never asks for both.
  shell_identities:[AgentIdentity] = []
  shell_identity_options:[str] = []
  shell_identity = ""
  shell_provider = "codex"
  shell_credential = ""
  shell_credentials:[AgentCredential] = []
  shell_credentials_generation:i64 = 0
  shell_credentials_loading = false
  // The pickers are a SETUP, not chrome: they open when the operator asks for
  // them and when nothing is picked yet, and otherwise the header carries the
  // one line they settled on.
  shell_setup_open = false
  // WHICH peer runs the work. The label is what the picker shows; the key is
  // the `--host-node` value behind it, and "" means the connected node.
  shell_host_nodes:[AgentHostNode] = []
  shell_host_node_options:[str] = ["This node"]
  shell_host_node = "This node"
  shell_host_node_key = ""
  shell_terminal:AgentTerminalSession = idle_agent_terminal()
  shell_terminal_running = false
  shell_terminal_busy = false
  shell_terminal_title = ""
  shell_terminal_error = ""
  shell_chat_entries:[AgentChatEntry] = []
  shell_chat_activity:[AgentActivity] = []
  shell_chat_draft:editor = ""
  shell_chat_busy = false
  shell_chat_status = ""
  shell_chat_detail = ""
  shell_chat_live = ""
  shell_chat_saga = ""
  // The run the operator stopped watching, "" when there is none. A MIRROR of
  // the trailing entry's id, held here because the composer and the view read
  // it every frame and scanning `shell_chat_entries` for it would hand the
  // whole transcript across the extern ABI on each one.
  shell_detached_saga = ""
  // Which settled turn has its work open. 0 is none — an entry id is never 0.
  shell_steps_open:i64 = 0
