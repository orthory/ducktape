state
  // Window identifiers are the routing state; a separate popped/open flag
  // would only duplicate whether an identifier exists.
  onboarding_win:window-id? = none
  console_win:window-id? = none
  huddle_win:window-id? = none
  network_name = ""
  hub_step:HubStep = HubStep.loading
  hub_key_state = ""
  hub_networks:[HubNetwork] = []
  hub_selected = ""
  hub_hidden:i64 = 0
  reveal_words = ""
  onboarding_name = ""
  onboarding_error = ""
  invite_link = ""
  provision_steps:[ProvisionStep] = []
  provision_index:i64 = 0

// Capability-bearing text is consumed once by Rust and cannot enter presets,
// snapshots, captures, routes, or accessibility output.
secret restore_words
secret join_invite
