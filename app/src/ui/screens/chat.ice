// THE CHAT SCREEN — the channel sidebar (rooms and DIRECT), the message
// column with its composer, the thread rail and the channel-details drawer.
// One component, because the four panes share the reading: the same
// `active_channel`, the same `channel_members`, the same archived flag decide
// what each of them may draw and offer.
//
// A screen cannot reach app state, so every reading arrives as a prop and every
// act leaves as a named event `view.ice` routes back to the handler of the same
// name. See `screens/roster.ice` for the contract.
//
// A MOUNTED COMPONENT ADDS A PATH SEGMENT, and an id-less one adds no widget
// targets AT ALL — the checker's id walk returns early on a component call with
// no `#id` and never descends. So the mount is `ChatScreen #chat`, the focus
// calls in `handlers/chat.ice` address `#workspace-tabs/content/chat/<id>`, and
// this root deliberately carries NO `#root`: that would push every id down one
// more segment for nothing.
//
// Both `sensor`s live here and report through the screen's own events. They
// were briefly caller-filled slots: a sensor's show/resize route used to accept
// only bare `_` payloads and could not carry a component event (ui-lang#239).

component ChatScreen(network_name:str, status:str, block_height:i64, bind search_draft:str, searching:bool, search_hits:[ChatSearchHit], channels:[ChatChannel], dm_peers:[DmPeer], channel_reads:[ChannelRead], user_key:str, channel_create_open:bool, connected:bool, loading:bool, mutation_phase:str, active_channel:str, active_dm_peer:str, active_channel_name:str, active_channel_archived:bool, active_channel_members_only:bool, channel_members:[ChatMember], huddle_joined:bool, huddle_channel:str, huddle_channel_name:str, huddle_joined_at:i64, huddle_now:i64, call_muted:bool, huddle_popped:bool, messages:[ChatMessage], has_older_history:bool, history_view:bool, history_loading:bool, unread_boundary:i64, unread_marker_seq:i64, selected_message_seq:i64, selected_message_rev:i64, send_flash_id:str, send_flash_value:f64, message_action:str, message_menu_y:f64, bind message_action_focus:str, bind message_edit_draft:str, failed_message_draft:str, bind message_editor:editor, channel_settings_open:bool, bind channel_name_draft:str, bind member_key_draft:str, active_thread_seq:i64, thread_target_seq:i64, thread_messages:[ChatMessage], thread_selected_seq:i64, thread_selected_rev:i64, thread_message_action:str, thread_menu_y:f64, thread_send_flash_id:str, bind thread_edit_draft:str, thread_has_more:bool, thread_next_reply_offset:i64, thread_loading:bool, failed_reply_draft:str, bind reply_editor:editor, shift_held:bool)
  emits
    search_chat_submit()
    clear_chat_search()
    open_chat_search_hit(str, i64, i64)
    toggle_channel_create()
    choose_channel(str)
    choose_dm(str)
    toggle_channel_settings()
    pop_huddle()
    focus_huddle()
    leave_huddle_here()
    huddle_go_channel()
    join_huddle_submit()
    chat_pointer_pressed(f64, f64)
    load_more_history()
    add_reaction_at(i64, str)
    remove_reaction_at(i64, str)
    open_thread_for(i64)
    open_message_actions(i64, str, i64)
    open_message_reactions(i64, str, i64)
    begin_message_edit(i64, str, i64)
    arm_message_delete(i64, str, i64)
    clear_message_selection()
    add_reaction_submit(str)
    edit_message_submit()
    delete_message_submit()
    restore_failed_message()
    dismiss_failed_message()
    composer_event(ComposerEvent)
    composer_mark(str)
    rename_channel_submit()
    archive_channel_submit()
    unarchive_channel_submit()
    add_channel_member_submit()
    remove_channel_member_submit(str)
    thread_pointer_pressed(f64, f64)
    close_thread()
    open_thread_message_actions(i64, str, i64)
    open_thread_message_reactions(i64, str, i64)
    begin_thread_message_edit(i64, str, i64)
    arm_thread_message_delete(i64, str, i64)
    clear_thread_message_selection()
    edit_thread_message_submit()
    delete_thread_message_submit()
    load_more_thread()
    restore_failed_reply()
    dismiss_failed_reply()
    reply_composer_event(ComposerEvent)
    chat_resized(f64, f64)
    thread_resized(f64, f64)
  row w=fill h=fill
    box w=236.0 h=fill bg=sidebar clip=true
      col w=fill h=fill
        box w=fill pl=14.0 pr=14.0 pt=14.0 pb=11.0
          row w=fill gap=8.0 align=center
            text network_name size=13.5 wrap=none font=display @text-fg
            if connection_degraded(status)
              box w=7.0 h=7.0 bg=danger_dot r=3.5
                space w=1.0 h=1.0
            if !connection_degraded(status)
              box w=7.0 h=7.0 bg=success_dot r=3.5
                space w=1.0 h=1.0
            space w=fill
            text height_label(block_height) size=10.5 wrap=none font=code_medium @text-label
        box w=fill h=1.0 bg=separator
          space w=1.0 h=1.0
        box w=fill pl=12.0 pr=12.0 pt=11.0 pb=6.0
          // MESSAGE SEARCH LIVES HERE, not in the channel header — the
          // artifact's 31px sidebar box. The command palette keeps its
          // global shortcut and gives up this seat.
          row w=fill h=31.0 gap=6.0 align=center
            input "" #chat-search label="Search messages" <-> search_draft hint="Search…" disabled=(!connected || searching) submit=emit(search_chat_submit) w=fill p=6.2 text-size=13.0 line-h=1.2 @control
              active bg=surface border=border value=fg placeholder=hint selection=fg/18 border-w=1.0 r=8.0
              hovered bg=muted_bg border=control_line
              disabled bg=transparent value=muted
            if !empty(search_hits)
              button label="Clear message search" w=27.0 h=27.0 p=0.0 @icon_action -> emit(clear_chat_search)
                box w=fill h=fill align-x=center align-y=center
                  text "×" size=13.0 wrap=none @text-muted
                active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
                hovered bg=elevated text=fg
                pressed bg=subtle text=fg
        box w=fill pl=16.0 pr=16.0 pt=10.0 pb=5.0
          row w=fill gap=6.0 align=center
            text "CHANNELS" size=10.0 wrap=none font=code_semibold @text-label
            space w=fill
            text len(rooms_only(channels, dm_peers, user_key)) size=10.5 wrap=none font=code_medium @text-label
            if !channel_create_open
              button label="New channel" disabled=(loading || mutation_phase != "idle" || !connected) p=0.0 @icon_action -> emit(toggle_channel_create)
                Icon name="plus" tone="label" px=16.0
                active bg=transparent text=muted border=transparent border-w=1.0 r=5.0
                hovered bg=separator text=fg
                pressed bg=subtle text=fg
            if channel_create_open
              button label="Close new channel" disabled=(loading || mutation_phase != "idle") w=24.0 h=24.0 p=0.0 @icon_action -> emit(toggle_channel_create)
                box w=fill h=fill align-x=center align-y=center
                  text "×" size=13.0 wrap=none @text-muted
                active bg=separator text=muted border=transparent border-w=1.0 r=5.0
                hovered bg=subtle text=fg
                pressed bg=subtle text=fg
        scroll dir=vertical w=fill h=fill bar=hidden
          col w=fill gap=2.0
            // DMs are filtered out here, not hidden by CSS: they are real
            // channels and would otherwise list twice, once under each
            // eyebrow. See `rooms_only`.
            for channel in rooms_only(channels, dm_peers, user_key)
              ChannelButton channel=channel selected=(channel.id == active_channel) unread=channel_is_unread(channel_reads, channel.id, channel.head_seq)
                forward
                  choose_channel
            // DIRECT — the artifact's own word for it, and the honest
            // one: a two-party channel, not an encrypted one. Reads
            // carry no authorization and every node replicates the
            // state, so nothing here says "private".
            if !empty(dm_peers)
              box w=fill pl=8.0 pr=8.0 pt=14.0 pb=6.0
                row w=fill gap=6.0 align=center
                  text "DIRECT" size=10.0 wrap=none font=code_semibold @text-label
                  space w=fill
                  text len(dm_peers) size=10.5 wrap=none font=code_medium @text-label
            for peer in dm_peers
              DmButton peer=peer selected=(peer.key == active_dm_peer)
                forward
                  choose_dm
        // No account footer: the rail's avatar and Settings already carry the
        // signed-in identity, and a "Not signed in" fallback under a live
        // conversation was pure noise.
    box w=1.0 h=fill bg=separator
      space w=1.0 h=1.0
    box w=fill h=fill bg=bg clip=true px-snap=true
      row w=fill h=fill
        col w=fill h=fill
          if !empty(active_channel)
            col w=fill
              box w=fill h=50.0 pl=18.0 pr=18.0
                row w=fill h=fill gap=9.0 align=center
                  // A DM IS A PERSON, NOT A `#`. The peer is a filter
                  // over `dm_peers` — Ice cannot index a list by field,
                  // so the row that matches `active_dm_peer` draws and
                  // the rest do not. A DM whose peer has left the
                  // identity roster matches nothing and falls back to
                  // the channel title below, which is the derived
                  // two-party name — never a blank plate.
                  if !empty(active_dm_peer)
                    for peer in dm_peers
                      if peer.key == active_dm_peer
                        DmHeader peer=peer
                  if empty(active_dm_peer)
                    text "#" size=14.0 wrap=none font=medium @text-hint
                  if empty(active_dm_peer)
                    text active_channel_name size=14.0 wrap=none font=display @text-fg
                  if active_channel_archived
                    Badge.Outline label="Archived"
                  if active_channel_members_only
                    Badge.Outline label="Members only"
                  // The huddle control, in its three mutually exclusive
                  // states — in it here, in it elsewhere, in none.
                  if huddle_joined && huddle_channel == active_channel
                    HuddleLivePill name=active_channel_name elapsed=mmss(huddle_now - huddle_joined_at) muted=call_muted popped=huddle_popped
                      forward
                        pop_huddle
                        focus_huddle
                        leave_huddle_here
                  if huddle_joined && huddle_channel != active_channel
                    HuddleElsewhere name=huddle_channel_name
                      forward
                        huddle_go_channel
                  if !huddle_joined && !active_channel_archived
                    HuddleStart
                      forward
                        join_huddle_submit
                  // `· N added`, NOT `· N members`. `channel_members`
                  // holds the chat module's explicit `SetMembership`
                  // rows and `stage_channel` seeds none, so an ordinary
                  // Open channel reads 0 however many people post in
                  // it. The count is real — it is the added-member set,
                  // and it says so. Hidden when nobody was added, since
                  // `· 0 added` on every normal channel is noise. The
                  // artifact's `M agents` half stays omitted: ChatMember
                  // carries key + label only.
                  if !empty(channel_members)
                    row gap=4.0 align=center
                      text "·" size=12.0 wrap=none @text-caption
                      text len(channel_members) size=12.0 wrap=none font=code @text-caption
                      text "added" size=12.0 wrap=none @text-caption
                  space w=fill
                  // No StatusPill here: the titlebar pill is on screen at the
                  // same moment, and the same word twice reads as two systems.
                  button label="Channel details" w=27.0 h=25.0 p=0.0 @icon_action -> emit(toggle_channel_settings)
                    box w=fill h=fill align-x=center align-y=center
                      text "⋯" size=14.0 wrap=none @text-muted
                    active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
                    hovered bg=elevated text=fg
                    pressed bg=subtle text=fg
              box w=fill h=1.0 bg=separator
                space w=1.0 h=1.0
          stack w=fill h=fill
            col w=fill h=fill gap=9.0 pl=18.0 pr=18.0 pt=16.0 pb=8.0
              if !connected
                EmptyState title="Not connected" description="Click the network name in the titlebar to pick or reconnect a network."
              if connected && !loading && empty(messages)
                EmptyState title="No messages yet" description="Create a channel or start the conversation."
              if connected && loading && empty(messages)
                box w=fill h=fill align-x=center align-y=center
                  text "Loading messages…" size=12.5 @text-meta
              if connected && !empty(messages) && history_view
                box w=fill h=32.0 pl=10.0 pr=6.0 bg=warning_bg border=warning_line border-w=1.0 r=9.0
                  row w=fill h=fill gap=8.0 align=center
                    text "Viewing history" w=fill size=12.5 wrap=none @text-warning
                    button "Jump to latest" h=24.0 p=5.0 @ghost_action -> emit(choose_channel, active_channel)
                      active bg=surface text=fg border=warning_line border-w=1.0 r=7.0
                      hovered bg=warning_bg text=fg
                      pressed bg=accent text=fg
              if connected && !empty(messages)
                stack w=fill h=fill
                  sensor show=emit(chat_resized, _, _) resize=emit(chat_resized, _, _)
                    space w=fill h=fill
                  mouse press-at=emit(chat_pointer_pressed, _, _)
                    scroll dir=vertical w=fill h=fill anchor-y=end auto=true
                      col w=fill gap=3.0 pr=6.0
                        if has_older_history
                          box w=fill align-x=center pt=4.0 pb=8.0
                            button "Load older messages" disabled=(history_loading || mutation_phase != "idle") h=30.0 p=6.0 @secondary_action -> emit(load_more_history)
                              active bg=fg/6 text=muted border=fg/10 border-w=1.0 r=8.0
                              hovered bg=fg/10 text=fg border=fg/14
                              pressed bg=fg/14 text=fg
                        for message in messages
                          col w=fill gap=0.0
                            if unread_boundary > 0 && message.seq == unread_marker_seq
                              row w=fill gap=8.0 align=center pt=8.0 pb=2.0
                                box w=fill h=1.0 bg=brand/40
                                  text ""
                                text "New messages" size=12.5 wrap=none @text-brand
                                box w=fill h=1.0 bg=brand/40
                                  text ""
                            // LAZY OFF THE HOT PATH. A quiet row rebuilds
                            // only when its MESSAGE changes; ONLY the selected
                            // row is built live, because its card reads the
                            // selection. Hover costs no arm at all now — the
                            // toolbar reveal is the `hover` widget's draw-time
                            // check inside MessageCard, so a cached row keeps
                            // it at native latency.
                            //
                            // A message in flight LOOKS like a message: no
                            // dashed frame, no restyle — send-state lives in
                            // MessageContents' right-edge lane (pending dot,
                            // then the settle ✓ fading out). The flash arm is
                            // the one live mount that carries the animated
                            // opacity; every other unselected row stays lazy.
                            if message.seq == selected_message_seq
                              stack #message(message.id) w=fill
                                MessageCard message=message selected=true disabled=loading flash=0.0
                                  forward
                                    add_reaction_at
                                    remove_reaction_at
                                    open_thread_for
                                    open_message_reactions
                                    open_message_actions
                            if message.seq != selected_message_seq && message.id == send_flash_id
                              stack #message(message.id) w=fill
                                MessageCard message=message selected=false disabled=false flash=send_flash_value
                                  forward
                                    add_reaction_at
                                    remove_reaction_at
                                    open_thread_for
                                    open_message_reactions
                                    open_message_actions
                            if message.seq != selected_message_seq && message.id != send_flash_id
                              lazy message as cached_message
                                stack #message(cached_message.id) w=fill
                                  MessageCard message=cached_message selected=false disabled=false flash=0.0
                                    forward
                                      add_reaction_at
                                      remove_reaction_at
                                      open_thread_for
                                      open_message_reactions
                                      open_message_actions
                  overlay when=(selected_message_seq > 0 && message_action != "toolbar") dismiss=emit(clear_message_selection) backdrop=transparent p=8.0 align-x=end align-y=start
                    content
                      space w=fill h=fill
                    layer
                      float x=0.0 y=message_menu_y
                        col
                          if message_action == "more"
                            stack
                              input "" #message-action-focus label="Message action focus" <-> message_action_focus w=1.0 p=0.0 text-size=1.0 line-h=1.0
                                active bg=transparent border=transparent value=transparent placeholder=transparent border-w=0.0 r=0.0
                                focused bg=transparent border=transparent value=transparent border-w=0.0
                              // The menu is icon + sentence rows on one raised
                              // plate — no Close row: Esc (`escape_target`) and
                              // the backdrop both dismiss, and a menu that
                              // lists its own exit reads as a dialog.
                              box w=200.0 p=5.0 style=raised_style()
                                col w=fill gap=1.0
                                  button label="Manage reactions" disabled=active_channel_archived w=fill h=30.0 p=0.0 @ghost_action -> emit(open_message_reactions, selected_message_seq, message_edit_draft, selected_message_rev)
                                    box w=fill h=fill pl=9.0 pr=9.0 align-y=center
                                      row w=fill gap=9.0 align=center
                                        Icon name="emoji" tone="muted" px=14.0
                                        text "Add reaction" size=12.5 wrap=none @text-accent_fg
                                    active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
                                    hovered bg=fg/8 text=fg
                                    pressed bg=fg/12 text=fg
                                  button label="Reply in thread" w=fill h=30.0 p=0.0 @ghost_action -> emit(open_thread_for, selected_message_seq)
                                    box w=fill h=fill pl=9.0 pr=9.0 align-y=center
                                      row w=fill gap=9.0 align=center
                                        Icon name="nav-chat" tone="muted" px=14.0
                                        text "Reply in thread" size=12.5 wrap=none @text-accent_fg
                                    active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
                                    hovered bg=fg/8 text=fg
                                    pressed bg=fg/12 text=fg
                                  button label="Edit message" w=fill h=30.0 p=0.0 @ghost_action -> emit(begin_message_edit, selected_message_seq, message_edit_draft, selected_message_rev)
                                    box w=fill h=fill pl=9.0 pr=9.0 align-y=center
                                      row w=fill gap=9.0 align=center
                                        Icon name="pencil" tone="muted" px=14.0
                                        text "Edit message" size=12.5 wrap=none @text-accent_fg
                                    active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
                                    hovered bg=fg/8 text=fg
                                    pressed bg=fg/12 text=fg
                                  box w=fill h=1.0 bg=separator
                                    space w=1.0 h=1.0
                                  button label="Delete message" w=fill h=30.0 p=0.0 @ghost_action -> emit(arm_message_delete, selected_message_seq, message_edit_draft, selected_message_rev)
                                    box w=fill h=fill pl=9.0 pr=9.0 align-y=center
                                      row w=fill gap=9.0 align=center
                                        Icon name="trash" tone="danger" px=14.0
                                        text "Delete message…" size=12.5 wrap=none @text-danger
                                    active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
                                    hovered bg=danger_bg text=fg
                                    pressed bg=danger_line text=fg
                          if message_action == "reactions"
                            stack
                              input "" #message-reaction-focus label="Message reaction focus" <-> message_action_focus w=1.0 p=0.0 text-size=1.0 line-h=1.0
                                active bg=transparent border=transparent value=transparent placeholder=transparent border-w=0.0 r=0.0
                                focused bg=transparent border=transparent value=transparent border-w=0.0
                              // The picker is an ADD grid only: removing rides
                              // the message's own reaction chips, which
                              // already toggle off for `reacted_by_me`. Esc
                              // and the backdrop dismiss — no × row.
                              box p=8.0 style=raised_style()
                                flex w=234.0 wrap=wrap gap-x=2.0 gap-y=2.0 items=start
                                  for emoji in reaction_palette()
                                    button label="Add reaction" description=emoji disabled=active_channel_archived w=27.0 h=27.0 p=0.0 @ghost_action -> emit(add_reaction_submit, emoji)
                                      box w=fill h=fill align-x=center align-y=center
                                        text emoji size=14.0 wrap=none @text-fg
                                      active bg=transparent text=fg border=transparent border-w=1.0 r=6.0
                                      hovered bg=fg/10
                                      pressed bg=fg/15
                          if message_action == "editing"
                            box w=fill max-w=520.0 p=3.0 style=raised_style()
                              row w=fill gap=4.0 align=center
                                input "" #message-edit label="Edit message" <-> message_edit_draft hint="Edit message" disabled=(mutation_phase != "idle") submit=emit(edit_message_submit) w=fill p=6.2 text-size=13.0 line-h=1.2 @control
                                  active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                                  hovered bg=fg/4 border=fg/8
                                  disabled value=muted
                                button "Save" label="Save message changes" disabled=(mutation_phase != "idle" || empty(trim(message_edit_draft))) h=28.0 p=6.0 @primary_action -> emit(edit_message_submit)
                                button label="Cancel message edit" disabled=(mutation_phase != "idle") w=28.0 h=28.0 p=0.0 @icon_action -> emit(clear_message_selection)
                                  box w=fill h=fill align-x=center align-y=center
                                    text "×" size=14.0
                                  active bg=transparent text=muted r=7.0
                                  hovered bg=fg/10 text=fg
                                  pressed bg=fg/15
                          if message_action == "delete"
                            stack
                              input "" #message-delete-focus label="Message delete focus" <-> message_action_focus w=1.0 p=0.0 text-size=1.0 line-h=1.0
                                active bg=transparent border=transparent value=transparent placeholder=transparent border-w=0.0 r=0.0
                                focused bg=transparent border=transparent value=transparent border-w=0.0
                              box p=3.0 style=raised_style()
                                row gap=5.0 align=center
                                  text "Delete this message?" size=12.5 @text-muted
                                  button "Delete" disabled=(mutation_phase != "idle") h=26.0 p=5.0 @danger_action -> emit(delete_message_submit)
                                  button "Cancel" disabled=(mutation_phase != "idle") h=26.0 p=5.0 @secondary_action -> emit(clear_message_selection)
                                    active bg=transparent text=muted r=6.0
                                    hovered bg=fg/10 text=fg
                                    pressed bg=fg/15
              if !empty(failed_message_draft)
                row w=fill gap=6.0 align=center
                  text "An earlier message wasn’t sent" w=fill size=12.5 @text-muted
                  button "Restore" disabled=(!empty(trim(editor_text(message_editor))) || mutation_phase != "idle") h=28.0 p=5.0 @secondary_action -> emit(restore_failed_message)
                    active bg=fg/9 text=fg border=fg/11 border-w=1.0 r=7.0
                    hovered bg=fg/14
                    pressed bg=fg/18
                  button label="Dismiss unsent message" w=28.0 h=28.0 p=0.0 @icon_action -> emit(dismiss_failed_message)
                    box w=fill h=fill align-x=center align-y=center
                      text "×" size=14.0
                    active bg=transparent text=muted r=7.0
                    hovered bg=fg/10 text=fg
                    pressed bg=fg/15
            // THE RESULTS FLOAT. This card used to be the column's first
            // child, so typing a search reflowed the whole conversation down
            // by 148px; as a stack layer it drops over the stream instead,
            // and everything below it keeps its place.
            // THE RESULTS FLOAT. This card used to be the column's first
            // child, so a search reflowed the whole conversation down by
            // 148px; as a stack layer it drops over the stream instead and
            // everything beneath keeps its place.
            if !empty(search_hits)
              box w=fill h=fill pl=18.0 pr=18.0 pt=16.0 align-y=start
                box w=fill h=148.0 p=6.0 bg=elevated border=fg/10 border-w=1.0 r=10.0 shadow=shadow_popover shadow-y=8.0 shadow-blur=24.0
                  scroll dir=vertical w=fill h=fill
                    col w=fill gap=1.0
                      for hit in search_hits
                        ChatSearchResult hit=hit
                          forward
                            open_chat_search_hit
          // The composer is separated from the stream by a hairline and
          // carries the artifact's own 12/16/14 region padding.
          box w=fill h=1.0 bg=separator
            space w=1.0 h=1.0
          // THE GATE ABOVE THE PLATE. `post_gate` is called here rather
          // than mirrored into a state field: it is pure over three
          // facts the view already holds, `channel_members` lands in
          // SEVEN handlers, and a seventh copy is six chances to drift.
          // An empty reason renders nothing and gates nothing.
          if !empty(post_gate(active_channel_archived, active_channel_members_only, channel_members, user_key))
            box w=fill pl=16.0 pr=16.0 pt=12.0
              ComposerGate reason=post_gate(active_channel_archived, active_channel_members_only, channel_members, user_key)
          box w=fill pl=16.0 pr=16.0 pt=12.0 pb=14.0
            box w=fill bg=surface border=control_line border-w=1.0 r=12.0 clip=true shadow=shadow_popover shadow-y=1.0 shadow-blur=2.0
              col w=fill
                extern rich_composer(message_editor, "Message the channel…", (loading || !connected || empty(active_channel) || !empty(post_gate(active_channel_archived, active_channel_members_only, channel_members, user_key))), shift_held, 44.0, 150.0, 10.0) #message -> emit(composer_event, _)
                // The Slack seat: format controls on the left, send on the
                // right, one row under the input. The marks are the renderer's
                // own grammar — the buttons insert what typing the fence would.
                box w=fill pl=8.0 pr=8.0 pb=8.0
                  row w=fill gap=2.0 align=center
                    button label="Bold" disabled=(loading || !connected || empty(active_channel) || !empty(post_gate(active_channel_archived, active_channel_members_only, channel_members, user_key))) w=26.0 h=24.0 p=0.0 @ghost_action -> emit(composer_mark, "bold")
                      box w=fill h=fill align-x=center align-y=center
                        text "B" size=12.5 wrap=none font=strong @text-muted
                      active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
                      hovered bg=fg/8 text=fg
                      pressed bg=fg/12 text=fg
                    button label="Italic" disabled=(loading || !connected || empty(active_channel) || !empty(post_gate(active_channel_archived, active_channel_members_only, channel_members, user_key))) w=26.0 h=24.0 p=0.0 @ghost_action -> emit(composer_mark, "italic")
                      box w=fill h=fill align-x=center align-y=center
                        text "I" size=12.5 wrap=none font=italic @text-muted
                      active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
                      hovered bg=fg/8 text=fg
                      pressed bg=fg/12 text=fg
                    box w=1.0 h=14.0 bg=separator
                      space w=1.0 h=1.0
                    button label="Code block" disabled=(loading || !connected || empty(active_channel) || !empty(post_gate(active_channel_archived, active_channel_members_only, channel_members, user_key))) w=26.0 h=24.0 p=0.0 @ghost_action -> emit(composer_mark, "code")
                      box w=fill h=fill align-x=center align-y=center
                        Icon name="code-brackets" tone="muted" px=13.0
                      active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
                      hovered bg=fg/8 text=fg
                      pressed bg=fg/12 text=fg
                    button label="Quote" disabled=(loading || !connected || empty(active_channel) || !empty(post_gate(active_channel_archived, active_channel_members_only, channel_members, user_key))) w=26.0 h=24.0 p=0.0 @ghost_action -> emit(composer_mark, "quote")
                      box w=fill h=fill align-x=center align-y=center
                        Icon name="quote" tone="muted" px=13.0
                      active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
                      hovered bg=fg/8 text=fg
                      pressed bg=fg/12 text=fg
                    space w=fill
                    text "↵ send · ⇧↵ newline" size=10.5 wrap=none font=code_medium @text-label
                    button "Send" disabled=(loading || !connected || empty(active_channel) || !empty(post_gate(active_channel_archived, active_channel_members_only, channel_members, user_key)) || empty(trim(editor_text(message_editor)))) h=29.0 p=7.0 @primary_action -> emit(composer_event, composer_submit_event())
        // THE DETAILS DRAWER — a sidebar-toned rail with one header bar, the
        // channel's identity up top, eyebrowed NAME and MEMBERS sections, and
        // the archive act alone at the bottom where a destructive control
        // belongs. It stopped being an unlabeled pile of input rows.
        if channel_settings_open && !empty(active_channel)
          box w=1.0 h=fill bg=separator
            space w=1.0 h=1.0
          box w=320.0 h=fill bg=sidebar
            col w=fill h=fill
              box w=fill pl=16.0 pr=10.0 pt=9.0 pb=9.0
                row w=fill gap=6.0 align=center
                  text "Channel details" w=fill size=13.5 wrap=none font=display @text-fg
                  button label="Close channel details" w=28.0 h=28.0 p=0.0 @icon_action -> emit(toggle_channel_settings)
                    box w=fill h=fill align-x=center align-y=center
                      text "×" size=14.0
                    active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
                    hovered bg=fg/10 text=fg
                    pressed bg=fg/15
              box w=fill h=1.0 bg=separator
                space w=1.0 h=1.0
              scroll dir=vertical w=fill h=fill
                col w=fill gap=16.0 pl=16.0 pr=16.0 pt=14.0 pb=14.0
                  col w=fill gap=7.0
                    row w=fill gap=7.0 align=center
                      if !active_channel_members_only
                        text "#" size=14.0 wrap=none font=medium @text-hint
                      if active_channel_members_only
                        text "◆" size=13.0 wrap=none @text-label
                      text active_channel_name w=fill size=14.0 wrap=none font=display @text-fg
                    if active_channel_archived || active_channel_members_only
                      row w=fill gap=5.0 align=center
                        if active_channel_archived
                          Badge.Outline label="Archived"
                        if active_channel_members_only
                          Badge.Outline label="Members only"
                  col w=fill gap=6.0
                    Eyebrow label="NAME" note=""
                    row w=fill gap=6.0 align=center
                      input "" #channel-name label="Channel name" <-> channel_name_draft hint="Channel name" disabled=(mutation_phase != "idle") submit=emit(rename_channel_submit) w=fill p=6.6 text-size=13.0 line-h=1.2 @control
                        active bg=surface border=border value=fg placeholder=muted selection=fg/18 border-w=1.0 r=8.0
                        hovered bg=surface border=control_line
                        disabled value=muted
                      button "Rename" disabled=(mutation_phase != "idle" || empty(trim(channel_name_draft))) h=29.0 p=6.0 @secondary_action -> emit(rename_channel_submit)
                  col w=fill gap=6.0
                    row w=fill gap=6.0 align=center
                      Eyebrow label="MEMBERS" note=""
                      space w=fill
                      text len(channel_members) size=10.5 wrap=none font=code_medium @text-label
                    row w=fill gap=6.0 align=center
                      input "" #member-key label="Member public key" <-> member_key_draft hint="Member key (64 hex)" disabled=(mutation_phase != "idle") submit=emit(add_channel_member_submit) w=fill p=7.4 text-size=11.5 line-h=1.2 font=code @control
                        active bg=surface border=border value=fg placeholder=muted selection=fg/18 border-w=1.0 r=8.0
                        hovered bg=surface border=control_line
                        disabled value=muted
                      button "Add" disabled=(mutation_phase != "idle" || empty(trim(member_key_draft))) h=29.0 p=6.0 @secondary_action -> emit(add_channel_member_submit)
                    if empty(channel_members)
                      text "No members added. An Open channel needs none — membership only gates posting in a members-only channel." w=fill size=11.5 line-h=1.5 @text-caption
                    if !empty(channel_members)
                      col w=fill gap=1.0
                        for member in channel_members
                          ChatMemberRow member=member disabled=(mutation_phase != "idle")
                            forward
                              remove_channel_member_submit
              box w=fill h=1.0 bg=separator
                space w=1.0 h=1.0
              box w=fill pl=16.0 pr=16.0 pt=10.0 pb=12.0
                col w=fill
                  if !active_channel_archived
                    button "Archive channel" disabled=(mutation_phase != "idle") w=fill h=30.0 p=6.0 @danger_action -> emit(archive_channel_submit)
                  if active_channel_archived
                    button "Unarchive channel" disabled=(mutation_phase != "idle") w=fill h=30.0 p=6.0 @secondary_action -> emit(unarchive_channel_submit)
        if active_thread_seq > 0 && !channel_settings_open
          box w=1.0 h=fill bg=separator
            space w=1.0 h=1.0
          // THE RAIL IS A PANE, NOT A CARD: the artifact's 330px sidebar-toned
          // plate with a 50px header bar and 16px body insets, mirroring the
          // details drawer one `if` up — the old 300px muted_bg card with its
          // own 12px air read as a third surface family.
          box w=330.0 h=fill bg=sidebar
            stack w=fill h=fill
              sensor show=emit(thread_resized, _, _) resize=emit(thread_resized, _, _)
                space w=fill h=fill
              mouse press-at=emit(thread_pointer_pressed, _, _)
                col w=fill h=fill
                  // The header carries the CHANNEL as its caption, not a reply
                  // count — `len(thread_messages)` counts the root too, and the
                  // honest count now lives in ThreadParentBlock's replies rule.
                  // "Thread result" stays: it is the only signpost a
                  // chat-search hit gets.
                  box w=fill h=50.0 pl=16.0 pr=16.0
                    row w=fill h=fill gap=7.0 align=center
                      if thread_target_seq <= 0
                        text "Thread" size=13.0 wrap=none font=display @text-fg
                      if thread_target_seq > 0
                        text "Thread result" size=13.0 wrap=none font=display @text-fg
                      row gap=2.0 align=center
                        if empty(active_dm_peer)
                          text "#" size=11.0 wrap=none @text-caption
                        text active_channel_name size=11.0 wrap=none @text-caption
                      space w=fill
                      button label="Close thread" disabled=(mutation_phase != "idle") w=24.0 h=24.0 p=0.0 @icon_action -> emit(close_thread)
                        box w=fill h=fill align-x=center align-y=center
                          text "×" size=14.0
                        active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
                        hovered bg=fg/10 text=fg
                        pressed bg=fg/15
                  box w=fill h=1.0 bg=separator
                    space w=1.0 h=1.0
                  scroll dir=vertical w=fill h=fill anchor-y=end auto=true
                    // The 16px right inset doubles as the scrollbar
                    // clearance the code/quote slabs needed (#927).
                    col w=fill gap=3.0 pl=16.0 pr=16.0 pt=12.0 pb=8.0
                      for thread_message in thread_messages
                        // THE ROOT GETS ITS OWN DIVIDED BLOCK. One
                        // loop, one discriminant: `active_thread_seq`
                        // IS the root's seq and `thread_messages`
                        // carries it, so the split needs no state and
                        // no fn. The root's read-only block is the
                        // artifact's; its hover bar, reactions and
                        // edit/delete are not lost, they stay on the
                        // same message in the stream one pane over,
                        // which is on screen the whole time this rail
                        // is.
                        if thread_message.seq == active_thread_seq
                          ThreadParentBlock message=thread_message
                        // The settle ✓ mirrors the stream's arms: the one
                        // reply `thread_send_flash_id` anchors rides the
                        // shared fade, every other row passes 0.0.
                        if thread_message.seq != active_thread_seq && thread_message.id == thread_send_flash_id
                          ThreadMessageCard message=thread_message selected=(thread_message.seq == thread_target_seq) disabled=loading flash=send_flash_value
                            forward
                              add_reaction_at
                              remove_reaction_at
                              open_thread_for
                              open_thread_message_actions
                              open_thread_message_reactions
                        if thread_message.seq != active_thread_seq && thread_message.id != thread_send_flash_id
                          ThreadMessageCard message=thread_message selected=(thread_message.seq == thread_target_seq) disabled=loading flash=0.0
                            forward
                              add_reaction_at
                              remove_reaction_at
                              open_thread_for
                              open_thread_message_actions
                              open_thread_message_reactions
                      if thread_has_more && thread_next_reply_offset >= 0
                        button "Load more replies" disabled=(thread_loading || mutation_phase != "idle") w=fill h=28.0 p=5.0 @secondary_action -> emit(load_more_thread)
                          active bg=transparent text=muted r=7.0
                          hovered bg=fg/9 text=fg
                          pressed bg=brand_bg
                  if !empty(failed_reply_draft)
                    box w=fill pl=16.0 pr=16.0 pt=8.0
                      row w=fill gap=6.0 align=center
                        text "Unsent reply" w=fill size=12.5 @text-muted
                        button "Restore" disabled=(!empty(trim(editor_text(reply_editor)))) h=26.0 p=5.0 @secondary_action -> emit(restore_failed_reply)
                          active bg=fg/9 text=fg border=fg/11 border-w=1.0 r=7.0
                          hovered bg=fg/14
                          pressed bg=fg/18
                        button "×" label="Dismiss unsent reply" w=26.0 h=26.0 p=4.0 @ghost_action -> emit(dismiss_failed_reply)
                          active bg=transparent text=muted r=7.0
                          hovered bg=fg/10 text=fg
                          pressed bg=fg/15
                  // The stream's composer plate, in the rail's width: same
                  // surface/control_line/r12 chrome and the same seat row —
                  // hint left of Send — minus the format buttons, which the
                  // 330px plate has no room to teach twice.
                  box w=fill pl=16.0 pr=16.0 pt=10.0 pb=14.0
                    box w=fill bg=surface border=control_line border-w=1.0 r=12.0 clip=true shadow=shadow_popover shadow-y=1.0 shadow-blur=2.0
                      col w=fill
                        extern rich_composer(reply_editor, "Reply…", (thread_loading || active_channel_archived), shift_held, 44.0, 150.0, 10.0) #reply -> emit(reply_composer_event, _)
                        box w=fill pl=8.0 pr=8.0 pb=8.0
                          row w=fill gap=2.0 align=center
                            space w=fill
                            text "↵ send · ⇧↵ newline" size=10.5 wrap=none font=code_medium @text-label
                            button "Send" label="Send reply" disabled=(thread_loading || active_channel_archived || empty(trim(editor_text(reply_editor)))) h=28.0 p=6.0 @primary_action -> emit(reply_composer_event, composer_submit_event())
              overlay when=(thread_selected_seq > 0 && thread_message_action != "toolbar") dismiss=emit(clear_thread_message_selection) backdrop=transparent p=8.0 align-x=end align-y=start
                content
                  space w=fill h=fill
                layer
                  float x=0.0 y=thread_menu_y
                    col
                      if thread_message_action == "more"
                        stack
                          input "" #thread-action-focus label="Thread action focus" <-> message_action_focus w=1.0 p=0.0 text-size=1.0 line-h=1.0
                            active bg=transparent border=transparent value=transparent placeholder=transparent border-w=0.0 r=0.0
                            focused bg=transparent border=transparent value=transparent border-w=0.0
                          // Mirrors the stream menu above: icon rows, no Close.
                          box w=200.0 p=5.0 style=raised_style()
                            col w=fill gap=1.0
                              button label="Manage reactions" disabled=active_channel_archived w=fill h=30.0 p=0.0 @ghost_action -> emit(open_thread_message_reactions, thread_selected_seq, thread_edit_draft, thread_selected_rev)
                                box w=fill h=fill pl=9.0 pr=9.0 align-y=center
                                  row w=fill gap=9.0 align=center
                                    Icon name="emoji" tone="muted" px=14.0
                                    text "Add reaction" size=12.5 wrap=none @text-accent_fg
                                active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
                                hovered bg=fg/8 text=fg
                                pressed bg=fg/12 text=fg
                              button label="Edit message" w=fill h=30.0 p=0.0 @ghost_action -> emit(begin_thread_message_edit, thread_selected_seq, thread_edit_draft, thread_selected_rev)
                                box w=fill h=fill pl=9.0 pr=9.0 align-y=center
                                  row w=fill gap=9.0 align=center
                                    Icon name="pencil" tone="muted" px=14.0
                                    text "Edit message" size=12.5 wrap=none @text-accent_fg
                                active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
                                hovered bg=fg/8 text=fg
                                pressed bg=fg/12 text=fg
                              box w=fill h=1.0 bg=separator
                                space w=1.0 h=1.0
                              button label="Delete message" w=fill h=30.0 p=0.0 @ghost_action -> emit(arm_thread_message_delete, thread_selected_seq, thread_edit_draft, thread_selected_rev)
                                box w=fill h=fill pl=9.0 pr=9.0 align-y=center
                                  row w=fill gap=9.0 align=center
                                    Icon name="trash" tone="danger" px=14.0
                                    text "Delete message…" size=12.5 wrap=none @text-danger
                                active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
                                hovered bg=danger_bg text=fg
                                pressed bg=danger_line text=fg
                      if thread_message_action == "reactions"
                        stack
                          input "" #thread-reaction-focus label="Thread reaction focus" <-> message_action_focus w=1.0 p=0.0 text-size=1.0 line-h=1.0
                            active bg=transparent border=transparent value=transparent placeholder=transparent border-w=0.0 r=0.0
                            focused bg=transparent border=transparent value=transparent border-w=0.0
                          // Same ADD grid as the stream picker — removal is
                          // the reply's own reaction chips.
                          box p=8.0 style=raised_style()
                            flex w=234.0 wrap=wrap gap-x=2.0 gap-y=2.0 items=start
                              for emoji in reaction_palette()
                                button label="Add reaction" description=emoji disabled=active_channel_archived w=27.0 h=27.0 p=0.0 @ghost_action -> emit(add_reaction_at, thread_selected_seq, emoji)
                                  box w=fill h=fill align-x=center align-y=center
                                    text emoji size=14.0 wrap=none @text-fg
                                  active bg=transparent text=fg border=transparent border-w=1.0 r=6.0
                                  hovered bg=fg/10
                                  pressed bg=fg/15
                      if thread_message_action == "editing"
                        box w=fill max-w=520.0 p=3.0 style=raised_style()
                          row w=fill gap=4.0 align=center
                            input "" #thread-edit label="Edit message" <-> thread_edit_draft hint="Edit message" disabled=(mutation_phase != "idle") submit=emit(edit_thread_message_submit) w=fill p=6.2 text-size=13.0 line-h=1.2 @control
                              active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                              hovered bg=fg/4 border=fg/8
                              disabled value=muted
                            button "Save" label="Save message changes" disabled=(mutation_phase != "idle" || empty(trim(thread_edit_draft))) h=28.0 p=6.0 @primary_action -> emit(edit_thread_message_submit)
                            button label="Cancel message edit" disabled=(mutation_phase != "idle") w=28.0 h=28.0 p=0.0 @icon_action -> emit(clear_thread_message_selection)
                              box w=fill h=fill align-x=center align-y=center
                                text "×" size=14.0
                              active bg=transparent text=muted r=7.0
                              hovered bg=fg/10 text=fg
                              pressed bg=fg/15
                      if thread_message_action == "delete"
                        stack
                          input "" #thread-delete-focus label="Thread delete focus" <-> message_action_focus w=1.0 p=0.0 text-size=1.0 line-h=1.0
                            active bg=transparent border=transparent value=transparent placeholder=transparent border-w=0.0 r=0.0
                            focused bg=transparent border=transparent value=transparent border-w=0.0
                          box p=3.0 style=raised_style()
                            row gap=5.0 align=center
                              text "Delete this message?" size=12.5 @text-muted
                              button "Delete" disabled=(mutation_phase != "idle") h=26.0 p=5.0 @danger_action -> emit(delete_thread_message_submit)
                              button "Cancel" disabled=(mutation_phase != "idle") h=26.0 p=5.0 @secondary_action -> emit(clear_thread_message_selection)
                                active bg=transparent text=muted r=6.0
                                hovered bg=fg/10 text=fg
                                pressed bg=fg/15
