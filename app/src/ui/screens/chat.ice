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
// The two `sensor` seats are `slot`s, filled by the mount. A sensor's
// show/resize route accepts only bare `_` payloads, so it cannot carry a
// component event — the measurement has to stay in the caller's scope, and the
// screen keeps only the seat so the sensor still measures the same box.

component ChatScreen(account_name:str, account_id:str, connected_rpc:str, status:str, block_height:i64, bind search_draft:str, searching:bool, search_hits:[ChatSearchHit], channels:[ChatChannel], dm_peers:[DmPeer], channel_reads:[ChannelRead], user_key:str, channel_create_open:bool, connected:bool, loading:bool, mutation_phase:str, active_channel:str, active_dm_peer:str, active_channel_name:str, active_channel_archived:bool, active_channel_members_only:bool, channel_members:[ChatMember], huddle_joined:bool, huddle_channel:str, huddle_channel_name:str, huddle_joined_at:i64, huddle_now:i64, messages:[ChatMessage], history_view:bool, history_loading:bool, unread_boundary:i64, selected_message_seq:i64, hovered_message_seq:i64, selected_message_rev:i64, message_action:str, message_menu_y:f64, bind message_action_focus:str, bind message_edit_draft:str, failed_message_draft:str, bind message_editor:editor, channel_settings_open:bool, bind channel_name_draft:str, bind member_key_draft:str, active_thread_seq:i64, thread_target_seq:i64, thread_messages:[ChatMessage], thread_hovered_seq:i64, thread_selected_seq:i64, thread_selected_rev:i64, thread_message_action:str, thread_menu_y:f64, bind thread_edit_draft:str, thread_has_more:bool, thread_next_reply_offset:i64, thread_loading:bool, failed_reply_draft:str, bind reply_editor:editor)
  emits
    search_chat_submit()
    clear_chat_search()
    open_chat_search_hit(str, i64, i64)
    toggle_channel_create()
    choose_channel(str)
    choose_dm(str)
    toggle_channel_settings()
    pop_huddle()
    leave_huddle_here()
    huddle_go_channel()
    join_huddle_submit()
    chat_pointer_moved(f64, f64)
    load_more_history()
    message_entered(i64)
    message_exited(i64)
    add_reaction_at(i64, str)
    remove_reaction_at(i64, str)
    open_thread_for(i64)
    open_message_actions(i64, str, i64)
    open_message_actions_accessibly(i64, str, i64)
    open_message_reactions(i64, str, i64)
    begin_message_edit(i64, str, i64)
    arm_message_delete(i64, str, i64)
    clear_message_selection()
    add_reaction_submit(str)
    remove_reaction_submit(str)
    edit_message_submit()
    delete_message_submit()
    restore_failed_message()
    dismiss_failed_message()
    send_message_submit()
    rename_channel_submit()
    archive_channel_submit()
    unarchive_channel_submit()
    add_channel_member_submit()
    remove_channel_member_submit(str)
    thread_pointer_moved(f64, f64)
    close_thread()
    thread_message_entered(i64)
    thread_message_exited(i64)
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
    send_reply_submit()
  row w=fill h=fill
    box w=236.0 h=fill bg=sidebar clip=true
      col w=fill h=fill
        box w=fill pl=14.0 pr=14.0 pt=14.0 pb=11.0
          row w=fill gap=8.0 align=center
            text network_label(account_name, connected_rpc) size=13.5 wrap=none font=display @text-fg
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
              button label="Close new channel" disabled=(loading || mutation_phase != "idle") w=18.0 h=18.0 p=0.0 @icon_action -> emit(toggle_channel_create)
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
        box w=fill h=1.0 bg=separator
          space w=1.0 h=1.0
        box w=fill pl=14.0 pr=14.0 pt=11.0 pb=11.0
          row w=fill gap=9.0 align=center
            PersonAvatar initials=initial_of(account_name) plate=26.0 ink=10.0
            col w=fill gap=1.0
              if !empty(account_name)
                text account_name size=12.0 wrap=none font=display @text-fg
              if empty(account_name)
                text "Not signed in" size=12.0 wrap=none font=display @text-muted
              text account_id size=9.5 wrap=none font=display @text-hint
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
                    HuddleLivePill name=active_channel_name elapsed=mmss(huddle_now - huddle_joined_at)
                      forward
                        pop_huddle
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
                  StatusPill degraded=connection_degraded(status) loading=loading
                  button label="Channel details" w=27.0 h=25.0 p=0.0 @icon_action -> emit(toggle_channel_settings)
                    box w=fill h=fill align-x=center align-y=center
                      text "⋯" size=14.0 wrap=none @text-muted
                    active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
                    hovered bg=elevated text=fg
                    pressed bg=subtle text=fg
              box w=fill h=1.0 bg=separator
                space w=1.0 h=1.0
          col w=fill h=fill gap=9.0 pl=18.0 pr=18.0 pt=16.0 pb=8.0
            if !empty(search_hits)
              box w=fill h=148.0 p=6.0 bg=elevated border=fg/10 border-w=1.0 r=10.0
                scroll dir=vertical w=fill h=fill
                  col w=fill gap=1.0
                    for hit in search_hits
                      ChatSearchResult hit=hit
                        forward
                          open_chat_search_hit
            if !connected
              EmptyState title="Connect to a node" description="Set the RPC endpoint in the sidebar."
            if connected && empty(messages)
              EmptyState title="No messages yet" description="Create a channel or start the conversation."
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
                slot chat_sensor
                mouse move=emit(chat_pointer_moved, _, _)
                  scroll dir=vertical w=fill h=fill
                    col w=fill gap=3.0
                      if history_has_older(messages)
                        box w=fill align-x=center pt=4.0 pb=8.0
                          button "Load older messages" disabled=(history_loading || mutation_phase != "idle") h=30.0 p=6.0 @secondary_action -> emit(load_more_history)
                            active bg=fg/6 text=muted border=fg/10 border-w=1.0 r=8.0
                            hovered bg=fg/10 text=fg border=fg/14
                            pressed bg=fg/14 text=fg
                      for message in messages
                        col w=fill gap=0.0
                          if unread_boundary > 0 && message.seq == first_unread_seq(messages, unread_boundary)
                            row w=fill gap=8.0 align=center pt=8.0 pb=2.0
                              box w=fill h=1.0 bg=brand/40
                                text ""
                              text "New messages" size=12.5 wrap=none @text-brand
                              box w=fill h=1.0 bg=brand/40
                                text ""
                          stack #message(message.id) w=fill
                            // THE ONE DASHED BORDER IN THE CONSOLE.
                            // `message.pending` is this app's only
                            // optimistic write — seeded by
                            // `optimistic_message`, cleared by
                            // `merge_message_send_result` — so the
                            // design system's "not yet settled is
                            // dashed" rule is drawn here and nowhere
                            // else. The ring is over the card, so the
                            // card keeps its own plate.
                            UnfinalizedFrame pending=message.pending
                              MessageCard message=message selected=(message.seq == selected_message_seq) hovered=(message.seq == hovered_message_seq) disabled=loading
                                forward
                                  message_entered
                                  message_exited
                                  add_reaction_at
                                  remove_reaction_at
                                  open_thread_for
                                  open_message_actions_accessibly
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
                            box w=190.0 p=4.0 style=raised_style()
                              col w=fill gap=1.0
                                button "React" label="Manage reactions" disabled=active_channel_archived w=fill h=28.0 p=6.0 @ghost_action -> emit(open_message_reactions, selected_message_seq, message_edit_draft, selected_message_rev)
                                  active bg=transparent text=muted r=6.0
                                  hovered bg=fg/10 text=fg
                                  pressed bg=fg/15
                                button "Open thread" w=fill h=28.0 p=6.0 @ghost_action -> emit(open_thread_for, selected_message_seq)
                                  active bg=transparent text=muted r=6.0
                                  hovered bg=fg/10 text=fg
                                  pressed bg=fg/15
                                button "Edit" w=fill h=28.0 p=6.0 @ghost_action -> emit(begin_message_edit, selected_message_seq, message_edit_draft, selected_message_rev)
                                  active bg=transparent text=muted r=6.0
                                  hovered bg=fg/10 text=fg
                                  pressed bg=fg/15
                                button "Delete" w=fill h=28.0 p=6.0 @danger_action -> emit(arm_message_delete, selected_message_seq, message_edit_draft, selected_message_rev)
                                button "Close" label="Close message actions" w=fill h=28.0 p=6.0 @secondary_action -> emit(clear_message_selection)
                                  active bg=transparent text=muted r=6.0
                                  hovered bg=fg/10 text=fg
                                  pressed bg=fg/15
                        if message_action == "reactions"
                          stack
                            input "" #message-reaction-focus label="Message reaction focus" <-> message_action_focus w=1.0 p=0.0 text-size=1.0 line-h=1.0
                              active bg=transparent border=transparent value=transparent placeholder=transparent border-w=0.0 r=0.0
                              focused bg=transparent border=transparent value=transparent border-w=0.0
                            box p=3.0 style=raised_style()
                              row gap=2.0 align=center
                                button "+ 👍" label="Add thumbs up reaction" disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> emit(add_reaction_submit, "👍")
                                  active bg=transparent text=fg r=6.0
                                  hovered bg=fg/10
                                  pressed bg=fg/15
                                button "+ ♥" label="Add heart reaction" disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> emit(add_reaction_submit, "❤️")
                                  active bg=transparent text=fg r=6.0
                                  hovered bg=fg/10
                                  pressed bg=fg/15
                                button "+ 😄" label="Add smile reaction" disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> emit(add_reaction_submit, "😄")
                                  active bg=transparent text=fg r=6.0
                                  hovered bg=fg/10
                                  pressed bg=fg/15
                                button "+ 🎉" label="Add celebration reaction" disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> emit(add_reaction_submit, "🎉")
                                  active bg=transparent text=fg r=6.0
                                  hovered bg=fg/10
                                  pressed bg=fg/15
                                button "+ 👀" label="Add eyes reaction" disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> emit(add_reaction_submit, "👀")
                                  active bg=transparent text=fg r=6.0
                                  hovered bg=fg/10
                                  pressed bg=fg/15
                                button "+ 🙌" label="Add raised hands reaction" disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> emit(add_reaction_submit, "🙌")
                                  active bg=transparent text=fg r=6.0
                                  hovered bg=fg/10
                                  pressed bg=fg/15
                                for message in messages
                                  if message.seq == selected_message_seq
                                    for reaction in message.reactions
                                      if reaction.reacted_by_me
                                        button label="Remove my reaction" description=reaction.emoji disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> emit(remove_reaction_submit, reaction.emoji)
                                          text reaction.emoji size=13.0 @text-fg
                                          active bg=fg/7 text=fg r=6.0
                                          hovered bg=fg/12
                                          pressed bg=fg/17
                                button "×" label="Close reactions" disabled=(mutation_phase != "idle") w=26.0 h=26.0 p=4.0 @secondary_action -> emit(clear_message_selection)
                                  active bg=transparent text=muted r=6.0
                                  hovered bg=fg/10 text=fg
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
                editor #message <-> message_editor hint="Message the channel…" disabled=(loading || !connected || empty(active_channel) || !empty(post_gate(active_channel_archived, active_channel_members_only, channel_members, user_key))) min-h=44.0 max-h=150.0 size=13.5 line-h=1.3 p=6.6 wrap=word key-binding=composer_keys() -> emit(send_message_submit)
                  active bg=transparent border=transparent value=fg placeholder=hint selection=fg/18 border-w=0.0 r=0.0
                  hovered bg=transparent border=transparent
                  focused bg=transparent border=ring border-w=1.0
                  disabled value=muted
                box w=fill pl=10.0 pr=8.0 pb=8.0
                  row w=fill gap=10.0 align=center
                    space w=fill
                    text "↵ send · ⇧↵ newline" size=10.5 wrap=none font=code_medium @text-label
                    button "Send" disabled=(loading || !connected || empty(active_channel) || !empty(post_gate(active_channel_archived, active_channel_members_only, channel_members, user_key)) || empty(trim(editor_text(message_editor)))) h=29.0 p=7.0 @primary_action -> emit(send_message_submit)
        if channel_settings_open && !empty(active_channel)
          box w=1.0 h=fill bg=fg/8
            text ""
          box w=300.0 h=fill p=12.0 bg=muted_bg
            col w=fill h=fill gap=8.0
              row w=fill h=28.0 gap=6.0 align=center
                text "Channel details" w=fill size=14.0 font=display @text-fg
                button label="Close channel details" w=28.0 h=28.0 p=0.0 @icon_action -> emit(toggle_channel_settings)
                  box w=fill h=fill align-x=center align-y=center
                    text "×" size=14.0
                  active bg=transparent text=muted r=7.0
                  hovered bg=fg/10 text=fg
                  pressed bg=fg/15
              Separator
              row w=fill gap=5.0 align=center
                input "" #channel-name label="Channel name" <-> channel_name_draft hint="Channel name" disabled=(mutation_phase != "idle") submit=emit(rename_channel_submit) w=fill p=6.2 text-size=13.0 line-h=1.2 @control
                  active bg=transparent border=fg/10 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                  hovered bg=fg/4 border=fg/14
                  disabled value=muted
                button "Rename" disabled=(mutation_phase != "idle" || empty(trim(channel_name_draft))) w=56.0 h=28.0 p=5.0 @secondary_action -> emit(rename_channel_submit)
              row w=fill gap=5.0 align=center
                if !active_channel_archived
                  button "Archive" disabled=(mutation_phase != "idle") h=28.0 p=5.0 @danger_action -> emit(archive_channel_submit)
                if active_channel_archived
                  button "Unarchive" disabled=(mutation_phase != "idle") h=28.0 p=5.0 @secondary_action -> emit(unarchive_channel_submit)
                    active bg=transparent text=muted r=7.0
                    hovered bg=fg/10 text=fg
                    pressed bg=fg/15
                space w=fill
                text len(channel_members) size=12.0 font=code @text-muted
              row w=fill gap=5.0 align=center
                input "" #member-key label="Member public key" <-> member_key_draft hint="64-character member key" disabled=(mutation_phase != "idle") submit=emit(add_channel_member_submit) w=fill p=7.4 text-size=12.0 line-h=1.2 font=code @control
                  active bg=transparent border=fg/10 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                  hovered bg=fg/4 border=fg/14
                  disabled value=muted
                button "Add" disabled=(mutation_phase != "idle" || empty(trim(member_key_draft))) w=40.0 h=28.0 p=5.0 @secondary_action -> emit(add_channel_member_submit)
              if !empty(channel_members)
                scroll dir=vertical w=fill h=fill
                  col w=fill gap=2.0
                    for member in channel_members
                      ChatMemberRow member=member disabled=(mutation_phase != "idle")
                        forward
                          remove_channel_member_submit
        if active_thread_seq > 0 && !channel_settings_open
          box w=1.0 h=fill bg=fg/8
            text ""
          box w=300.0 h=fill p=12.0 bg=muted_bg
            stack w=fill h=fill
              slot thread_sensor
              mouse move=emit(thread_pointer_moved, _, _)
                col w=fill h=fill gap=8.0
                  row w=fill h=28.0 gap=6.0 align=center
                    if thread_target_seq <= 0
                      text "Thread" w=fill size=14.0 font=display @text-fg
                    if thread_target_seq > 0
                      text "Thread result" w=fill size=14.0 font=display @text-fg
                    text len(thread_messages) size=12.0 font=code @text-muted
                    button label="Close thread" disabled=(mutation_phase != "idle") w=28.0 h=28.0 p=0.0 @icon_action -> emit(close_thread)
                      box w=fill h=fill align-x=center align-y=center
                        text "×" size=14.0
                      active bg=transparent text=muted r=7.0
                      hovered bg=fg/11 text=fg
                      pressed bg=brand_bg
                  Separator
                  scroll dir=vertical w=fill h=fill
                    col w=fill gap=1.0
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
                        if thread_message.seq != active_thread_seq
                          ThreadMessageCard message=thread_message selected=(thread_message.seq == thread_target_seq) hovered=(thread_message.seq == thread_hovered_seq) disabled=loading
                            forward
                              thread_message_entered
                              thread_message_exited
                              add_reaction_at
                              remove_reaction_at
                              open_thread_message_actions
                              open_thread_message_reactions
                      if thread_has_more && thread_next_reply_offset >= 0
                        button "Load more replies" disabled=(thread_loading || mutation_phase != "idle") w=fill h=28.0 p=5.0 @secondary_action -> emit(load_more_thread)
                          active bg=transparent text=muted r=7.0
                          hovered bg=fg/9 text=fg
                          pressed bg=brand_bg
                  if !empty(failed_reply_draft)
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
                  box w=fill p=5.0 bg=transparent border=fg/12 border-w=1.0 r=7.0
                    row w=fill gap=5.0 align=end
                      editor #reply <-> reply_editor hint="Reply…" disabled=(thread_loading || active_channel_archived) min-h=44.0 max-h=150.0 size=13.5 line-h=1.3 p=6.6 wrap=word key-binding=composer_keys() -> emit(send_reply_submit)
                        active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=0.0 r=9.0
                        hovered bg=fg/4 border=fg/8 border-w=1.0
                        focused bg=fg/6 border=ring border-w=1.0
                        disabled value=muted
                      button "Send" label="Send reply" disabled=(thread_loading || active_channel_archived || empty(trim(editor_text(reply_editor)))) h=28.0 p=6.0 @primary_action -> emit(send_reply_submit)
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
                          box w=190.0 p=4.0 style=raised_style()
                            col w=fill gap=1.0
                              button "React" label="Manage reactions" disabled=active_channel_archived w=fill h=28.0 p=6.0 @ghost_action -> emit(open_thread_message_reactions, thread_selected_seq, thread_edit_draft, thread_selected_rev)
                                active bg=transparent text=muted r=6.0
                                hovered bg=fg/10 text=fg
                                pressed bg=fg/15
                              button "Edit" w=fill h=28.0 p=6.0 @ghost_action -> emit(begin_thread_message_edit, thread_selected_seq, thread_edit_draft, thread_selected_rev)
                                active bg=transparent text=muted r=6.0
                                hovered bg=fg/10 text=fg
                                pressed bg=fg/15
                              button "Delete" w=fill h=28.0 p=6.0 @danger_action -> emit(arm_thread_message_delete, thread_selected_seq, thread_edit_draft, thread_selected_rev)
                              button "Close" label="Close message actions" w=fill h=28.0 p=6.0 @secondary_action -> emit(clear_thread_message_selection)
                                active bg=transparent text=muted r=6.0
                                hovered bg=fg/10 text=fg
                                pressed bg=fg/15
                      if thread_message_action == "reactions"
                        stack
                          input "" #thread-reaction-focus label="Thread reaction focus" <-> message_action_focus w=1.0 p=0.0 text-size=1.0 line-h=1.0
                            active bg=transparent border=transparent value=transparent placeholder=transparent border-w=0.0 r=0.0
                            focused bg=transparent border=transparent value=transparent border-w=0.0
                          box p=3.0 style=raised_style()
                            row gap=2.0 align=center
                              button "+ 👍" label="Add thumbs up reaction" disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> emit(add_reaction_at, thread_selected_seq, "👍")
                                active bg=transparent text=fg r=6.0
                                hovered bg=fg/10
                                pressed bg=fg/15
                              button "+ ♥" label="Add heart reaction" disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> emit(add_reaction_at, thread_selected_seq, "❤️")
                                active bg=transparent text=fg r=6.0
                                hovered bg=fg/10
                                pressed bg=fg/15
                              button "+ 😄" label="Add smile reaction" disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> emit(add_reaction_at, thread_selected_seq, "😄")
                                active bg=transparent text=fg r=6.0
                                hovered bg=fg/10
                                pressed bg=fg/15
                              button "+ 🎉" label="Add celebration reaction" disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> emit(add_reaction_at, thread_selected_seq, "🎉")
                                active bg=transparent text=fg r=6.0
                                hovered bg=fg/10
                                pressed bg=fg/15
                              button "+ 👀" label="Add eyes reaction" disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> emit(add_reaction_at, thread_selected_seq, "👀")
                                active bg=transparent text=fg r=6.0
                                hovered bg=fg/10
                                pressed bg=fg/15
                              button "+ 🙌" label="Add raised hands reaction" disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> emit(add_reaction_at, thread_selected_seq, "🙌")
                                active bg=transparent text=fg r=6.0
                                hovered bg=fg/10
                                pressed bg=fg/15
                              for thread_message in thread_messages
                                if thread_message.seq == thread_selected_seq
                                  for reaction in thread_message.reactions
                                    if reaction.reacted_by_me
                                      button label="Remove my reaction" description=reaction.emoji disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> emit(remove_reaction_at, thread_selected_seq, reaction.emoji)
                                        text reaction.emoji size=13.0 @text-fg
                                        active bg=fg/7 text=fg r=6.0
                                        hovered bg=fg/12
                                        pressed bg=fg/17
                              button "×" label="Close reactions" disabled=(mutation_phase != "idle") w=26.0 h=26.0 p=4.0 @secondary_action -> emit(clear_thread_message_selection)
                                active bg=transparent text=muted r=6.0
                                hovered bg=fg/10 text=fg
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
