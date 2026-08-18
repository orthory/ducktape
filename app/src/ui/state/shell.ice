state
  shell_tab:ShellTab = ShellTab.chat
  shell_mode:ShellMode = ShellMode.chat
  shell_provider = "codex"
  shell_credentials:[AgentCredential] = []
  shell_credential_options:[str] = []
  shell_credential = ""
  shell_credentials_generation:i64 = 0
  shell_credentials_loading = false
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
  shell_chat_error = ""
  shell_chat_saga = ""
