state
  node_key = ""
  node_data_dir = ""
  settings_key_path = ""
  settings_key_state = ""
  // Full local user key used by the channel membership post gate.
  settings_user_key = ""
  settings_open_tabs:i64 = 0
  settings_generation:i64 = 0

  account_exists = false
  // The decimal account number; "" when the local key belongs to no account.
  account_number = ""
  account_name = ""
  account_bio = ""
  account_keys:i64 = 0
  account_generation:i64 = 0
  account_renaming = false
  // The account's key associations, as the settings card lists them.
  account_key_rows:[AccountKeyRow] = []
  // One identity op in flight (create / mint / join / remove) — the buttons
  // wait on it the way Rename waits on `account_renaming`.
  account_busy = false
  // "Add a device": the ticket minted for the other device's pasted key
  // (shown until the next op clears it).
  account_ticket = ""
  // THE DRAFTS ARE THE SETTINGS VIEW'S. What the app owes it is which of
  // them a committed op consumed: the count moves once per op and the scope
  // names the drafts (`name`, `keys`, `label`, `account`).
  settings_drafts_cleared:i64 = 0
  settings_drafts_scope = ""
  // The console's "no account on this network" banner, dismissable for the
  // session; and the Settings card's reading of a QR ceremony (phase is
  // `working | show_qr | done | failed`, "" for none).
  account_banner_dismissed = false
  account_ceremony_phase = ""
  account_ceremony_qr = ""
  account_ceremony_detail = ""
  account_ceremony_left = ""

  node_log_timeline:NodeLogTimelineState = node_log_timeline_state()
  node_log_filter = ""
  node_peers:[PeerRow] = []
  node_peers_generation:i64 = 0
  node_version = ""
  node_root_hash = ""
  // The chain id the connected node serves — what an AddKey consent minted
  // here is scoped to; "" until the first status lands (or on a chainless node).
  network_chain_id = ""
  // A negative number means the node has published no measurement.
  node_last_finalized:i64 = -1
  node_checkpoint:i64 = -1
  node_height:i64 = -1
  node_phase = ""
  node_phase_since:i64 = -1
  node_sync_target:i64 = -1
  node_sync_applied:i64 = -1
  node_sync_retries:i64 = 0
  node_sync_failures:i64 = 0
  node_sync_last_error = ""
  node_tab:NodeTab = NodeTab.overview

  // Optional consensus readings are stored as rendered labels so absence is
  // never coerced to a measured zero.
  node_view_label = "—"
  node_quorum_label = "—"
  node_reachable_label = "—"
  module_rows:[ModuleRow] = []
