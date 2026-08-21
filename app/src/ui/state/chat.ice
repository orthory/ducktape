state
  channels:[ChatChannel] = []
  rooms:[ChatSidebarRow] = []
  messages:[ChatMessage] = []
  // Renderer invalidation for the whole-timeline lazy island. Domain handlers
  // advance it only when the corresponding owned window changes.
  messages_revision:i64 = 0
  chat_generation:i64 = 0
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
  thread_messages_revision:i64 = 0
  thread_next_reply_seq:i64 = 0
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
  // THE COMPOSERS ARE NOT HERE (ducktape-ui#697): each is a retained
  // `ChatComposer` instance keyed by `(endpoint, room)` / `(endpoint, thread)`,
  // so no app state can be handed to the wrong room — and neither is the
  // failed-send stash, which followed the reader out of the room its words
  // were written in until a slice keyed it to that room (ducktape-ui#698).
  chat_search_draft = ""
  chat_search_hits:[ChatSearchHit] = []
  chat_search_phase:SearchPhase = SearchPhase.idle
  // THE STRING THE FLOAT'S ZERO-HIT ARM IS SPEAKING FOR — the query a search
  // was actually SENT for, `""` while no answer stands. The phase alone could
  // not carry it: this box is enter-to-submit with no `change=` route, so a
  // keystroke writes the draft and runs no handler, and `done` went on
  // standing over a string the node never saw. Only the WITH-HITS arm may
  // outlive the box — rows stay until a new query is sent or the box is
  // cleared, exactly as the pages hits float does; see `screens/chat.ice`.
  chat_search_query = ""
  history_view = false
  has_older_history = false

  // Direct-message roster and the resolved peer for the active channel.
  dm_peers:[DmPeer] = []
  dm_rows:[DmSidebarRow] = []
  dm_peers_generation:i64 = 0
  active_dm_peer = ""
  active_dm:DmPeer = no_dm_peer()
