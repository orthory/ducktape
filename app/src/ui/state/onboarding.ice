state
  // Window identifiers are the routing state; a separate popped/open flag
  // would only duplicate whether an identifier exists.
  onboarding_win:window-id? = none
  console_win:window-id? = none
  huddle_win:window-id? = none
  network_name = ""
  hub_step:HubStep = HubStep.loading
  hub_networks:[HubNetwork] = []
  hub_selected = ""
  hub_hidden:i64 = 0
  hub_wallets:[WalletInfo] = []
  hub_wallet_selected = ""
  onboarding_name = ""
  onboarding_error = ""
  invite_link = ""
  provision_steps:[ProvisionStep] = []
  provision_index:i64 = 0
  // THE WELCOME STEP — a picked network whose chain this device's key has no
  // account on. `hub_chain_id` is that chain (every key consent is scoped to
  // it); the ceremony trio is the launch window's reading of the QR stream:
  // `ceremony_phase` is `working | show_qr | done | failed` or "" for none.
  hub_chain_id = ""
  welcome_name_draft = ""
  ceremony_phase = ""
  ceremony_qr = ""
  ceremony_detail = ""

// Capability-bearing text is consumed once by Rust and cannot enter presets,
// snapshots, captures, routes, or accessibility output.
secret restore_words
secret join_invite
