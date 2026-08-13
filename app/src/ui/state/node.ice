state
  settings_endpoint = ""
  node_key = ""
  node_data_dir = ""
  settings_key_path = ""
  settings_key_state = ""
  // Full local user key used by the channel membership post gate.
  settings_user_key = ""
  settings_open_tabs:i64 = 0
  settings_generation:i64 = 0

  account_bound = false
  account_id = ""
  account_name = ""
  account_bio = ""
  account_members:i64 = 0
  account_nodes:i64 = 0
  account_generation:i64 = 0
  account_name_draft = ""
  account_renaming = false

  node_log_timeline:NodeLogTimelineState = node_log_timeline_state()
  node_log_filter = ""
  node_peers:[PeerRow] = []
  node_peers_generation:i64 = 0
  node_version = ""
  node_root_hash = ""
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
