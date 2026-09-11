state
  // THE TIMELINE IS NOT HERE. The Chat tab is a module-owned view on the
  // kernel contract (`crates/views/chat`): it reads its own room, its threads
  // and its search off the index, and it signs its own writes. What stays is
  // what the rest of the app reads — the sidebar the bell and the tray share,
  // the room the whole app navigates to, the huddle, and the composers, which
  // are host surfaces whose words never cross the wire.
  channels:[ChatChannel] = []
  rooms:[ChatSidebarRow] = []
  chat_generation:i64 = 0
  channel_reads:[ChannelRead] = []
  unread_boundary:i64 = 0
  active_channel = ""
  active_channel_name = ""
  active_channel_archived = false
  active_channel_members_only = false
  // THE ROOM'S ROSTER IS THE COMPOSERS' — the mention menu's candidates, and
  // the post gate re-read at delivery (`submit_verdict`). Cached at the same
  // writes as its inputs; evaluating the gate in the view would clone the
  // member list once per composer per frame.
  channel_members:[ChatMember] = []
  post_refusal = ""
  // THE SEQ A LANDING OPENS THE WINDOW AROUND — a search hit, or a
  // `duck://channel/<id>#<seq>`. It is an input to the view's own read key,
  // so moving it re-reads the room around that message; 0 is the live tail.
  chat_land_seq:i64 = 0
  // THE ROW THE EDIT COMPOSER WAS OPENED ON. The view decides which rows may
  // be edited and hands over the markdown; the app remembers the revision so
  // the save is the compare-and-set the menu was armed for.
  chat_edit_seq:i64 = 0
  chat_edit_rev:i64 = 0
  // A refused or failed send hands its words back to the composer it came
  // from; the composer is a host surface, so the hand-off is a call.
  composer_stashed = false
  // the roster hand-off's acknowledgement (`chat_composer_roster`)
  composer_roster_set = false
  // the edit seed's acknowledgement (`chat_composer_seed`)
  composer_seeded = false
  // EVERY AGENT RUN THIS NODE HOLDS, for the whole node. Which of them reach
  // the screen is decided in the chat seat, against the room on screen when
  // the frame is built.
  live_agents:[LiveAgentRow] = []
  // Moves once per admitted send: the view snaps its stream to the tail on it.
  chat_sent_serial:i64 = 0
  // EVERY SEND STILL IN FLIGHT. The view's timeline is its own reading of the
  // index, which cannot know about an operation no block carries yet — so the
  // admitted sends live here and the view paints them at the tail of the
  // surface each was written in.
  chat_pending_sends:[PendingSend] = []
  // Moves on every ⌘C over the chat tab. The chord is a keyboard
  // subscription, which is the app's door; the rows it lifts are the view's.
  chat_copy_chord_serial:i64 = 0
  channel_draft = ""
  channel_create_open = false
  channel_create_members_only = false
  pending_channel = ""
  // THE COMPOSERS ARE NOT HERE (ducktape-ui#697): each is a retained
  // `ChatComposer` instance keyed by `(endpoint, room)` / `(endpoint, thread)`,
  // so no app state can be handed to the wrong room — and neither is the
  // failed-send stash, which followed the reader out of the room its words
  // were written in until a slice keyed it to that room (ducktape-ui#698).
  //
  // IS THE READER LOOKING AT NOW? The stream is bottom-anchored, and the view
  // publishes the offset on every real scroll step. The app takes one thing
  // off it: `history_view`, which is what keeps the live fold from marking a
  // room read while she is above the tail or parked on a landing.
  chat_at_tail = true
  history_view = false
  // THE CHAIN THE ROOMS ON SCREEN WERE LEARNED FROM. A workspace switch does
  // not change the endpoint, so a console can live right through one with no
  // reconnect at all — and every resync fold only ever ADDS rows. Compared
  // against `network_chain_id` (the node's own pushed status), this is what
  // tells the resync that the list it is folding into belongs to a network
  // this node has left. Empty until the first landing names one.
  chat_chain_id = ""

  // Direct-message roster and the resolved peer for the active channel.
  dm_peers:[DmPeer] = []
  dm_rows:[DmSidebarRow] = []
  dm_peers_generation:i64 = 0
  active_dm_peer = ""
  active_dm:DmPeer = no_dm_peer()
