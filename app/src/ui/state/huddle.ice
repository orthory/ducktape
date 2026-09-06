state
  huddle_joined = false
  huddle_channel = ""
  huddle_channel_name = ""
  huddle_joined_at:i64 = 0
  huddle_now:i64 = 0
  call_status = ""
  call_muted = false
  call_peers:[CallEvent] = []
  call_camera = false
  call_sharing = false
  call_video_live = false
  // Whose screen the panel stages, whole: a peer's node key, `you` for this
  // device's own share, empty for nobody. See `huddle_stage_peer`.
  huddle_stage = ""
  // THE DOCK'S ONE PREFERENCE, and it is a SESSION fact, not a stored one:
  // nothing writes it to disk, so every launch starts with the huddle showing.
  // `false` is expanded — a joined huddle draws itself, and folding it to the
  // pill is a choice this device then keeps until the huddle ends.
  huddle_roster:[HuddleParticipant] = []
  huddle_rows:[HuddleTileRow] = []
