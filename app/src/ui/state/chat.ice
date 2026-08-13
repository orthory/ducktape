state
  channels:[ChatChannel] = []
  rooms:[ChatSidebarRow] = []
  messages:[ChatMessage] = []
  // Recent room windows and both composer drafts are keyed by their owning
  // room/thread so navigation never paints or posts another room's text.
  message_cache:[ChannelWindow] = []
  message_drafts:[ChannelDraft] = []
  reply_drafts:[ChannelDraft] = []
  chat_generation:i64 = 0
  chat_window_loading = false
  channel_reads:[ChannelRead] = []
  unread_boundary:i64 = 0
  unread_marker_seq:i64 = 0
  active_channel = ""
  active_channel_name = ""
  active_channel_archived = false
  active_channel_members_only = false
  channel_members:[ChatMember] = []
  // Cached at the same writes as its inputs; evaluating the list-taking gate
  // in the view would clone the member list once per composer per frame.
  post_refusal = ""
  channel_settings_open = false
  channel_name_draft = ""
  member_key_draft = ""
  selected_message_seq:i64 = 0
  selected_message_rev:i64 = 0
  message_action:MessageAction = MessageAction.toolbar
  message_edit_draft = ""
  active_thread_seq:i64 = 0
  thread_target_seq:i64 = 0
  thread_messages:[ChatMessage] = []
  thread_next_reply_offset:i64 = 0
  thread_has_more = false
  thread_loading = false
  thread_generation:i64 = 0
  thread_selected_seq:i64 = 0
  thread_selected_rev:i64 = 0
  thread_message_action:MessageAction = MessageAction.toolbar
  thread_edit_draft = ""
  history_loading = false
  channel_draft = ""
  channel_create_open = false
  channel_create_members_only = false
  pending_channel = ""
  message_draft = ""
  message_editor:editor = ""
  reply_editor:editor = ""
  composer_focus:ComposerFocus = ComposerFocus.unfocused
  failed_message_draft = ""
  failed_reply_draft = ""
  chat_search_draft = ""
  chat_search_hits:[ChatSearchHit] = []
  chat_search_phase:SearchPhase = SearchPhase.idle
  history_view = false
  has_older_history = false

  // Direct-message roster and the resolved peer for the active channel.
  dm_peers:[DmPeer] = []
  dm_rows:[DmSidebarRow] = []
  dm_peers_generation:i64 = 0
  active_dm_peer = ""
  active_dm:DmPeer = no_dm_peer()
