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

component ChatScreen(network_name:str, status:str, block_height:i64, bind search_draft:str, search_phase:str, search_hits:[ChatSearchHit], rooms:[ChatSidebarRow], dm_rows:[DmSidebarRow], channel_create_open:bool, connected:bool, loading:bool, mutation_phase:str, active_channel:str, active_dm_peer:str, active_dm:DmPeer, active_channel_name:str, active_channel_archived:bool, active_channel_members_only:bool, channel_members:[ChatMember], post_refusal:str, huddle_joined:bool, huddle_channel:str, huddle_channel_name:str, huddle_joined_at:i64, huddle_now:i64, call_muted:bool, huddle_popped:bool, messages:[ChatMessage], has_older_history:bool, history_view:bool, history_loading:bool, unread_boundary:i64, unread_marker_seq:i64, selected_message_seq:i64, selected_message_rev:i64, send_flash_id:str, send_flash_value:f64, message_action:str, message_menu_y:f64, bind message_action_focus:str, bind message_edit_draft:str, failed_message_draft:str, bind message_editor:editor, channel_settings_open:bool, bind channel_name_draft:str, bind member_key_draft:str, active_thread_seq:i64, thread_target_seq:i64, thread_messages:[ChatMessage], thread_selected_seq:i64, thread_selected_rev:i64, thread_message_action:str, thread_menu_y:f64, thread_send_flash_id:str, bind thread_edit_draft:str, thread_has_more:bool, thread_next_reply_offset:i64, thread_loading:bool, failed_reply_draft:str, bind reply_editor:editor, shift_held:bool)
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
    chat_scrolled(f64, f64, f64, f64)
    open_message_link(str)
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
    reply_composer_mark(str)
    chat_resized(f64, f64)
    thread_resized(f64, f64)
  row w=fill h=fill
    box
      with
        w=236.0
        h=fill
        bg=sidebar
        clip=true
      col w=fill h=fill
        box
          with
            w=fill
            h=50.0
            pl=16.0
            pr=16.0
          row
            with
              w=fill
              h=fill
              gap=8.0
              align=center
            text network_name
              with
                size=13.5
                wrap=none
                font=display
                @text-fg
            if connection_degraded(status)
              box
                with
                  w=7.0
                  h=7.0
                  bg=danger_dot
                  r=3.5
                space w=1.0 h=1.0
            if !connection_degraded(status)
              box
                with
                  w=7.0
                  h=7.0
                  bg=success_dot
                  r=3.5
                space w=1.0 h=1.0
            space w=fill
            text height_label(block_height)
              with
                size=10.5
                wrap=none
                font=code_medium
                @text-label
        box
          with
            w=fill
            h=1.0
            bg=separator
          space w=1.0 h=1.0
        box
          with
            w=fill
            pl=16.0
            pr=16.0
            pt=11.0
            pb=6.0
          // MESSAGE SEARCH LIVES HERE, not in the channel header — the
          // artifact's 31px sidebar box. The command palette keeps its
          // global shortcut and gives up this seat.
          row
            with
              w=fill
              h=31.0
              gap=6.0
              align=center
            input "" #chat-search <-> search_draft
              with
                label="Search messages"
                hint="Search…"
                // NOT `|| search_phase == "searching"`. The field went dead the instant Enter
                // was pressed and stayed dead for the whole round trip, so the
                // query could not be refined while waiting — and a disabled
                // input drops the caret besides. `chat_search_loaded` already
                // guards on `chat_search_generation`, so the LATE reply is what
                // gets dropped; killing the field bought nothing.
                disabled=!connected
                submit=emit(search_chat_submit)
                w=fill
                p=6.2
                text-size=13.0
                line-h=1.2
                @control
              // NO `border=` HERE. `active` is the base for EVERY status, not
              // just the resting one, so a border color on this line is written
              // AFTER `@control`'s `focus:border-ring` and the ring never
              // paints — the field the caret sits in looked exactly like the
              // four beside it. The recipe already owns the resting border.
              active bg=surface value=fg placeholder=hint selection=fg/18 border-w=1.0 r=8.0
              hovered bg=muted_bg border=control_line
              disabled bg=transparent value=muted
            if search_phase != "idle"
              button -> emit(clear_chat_search)
                with
                  label="Clear message search"
                  w=27.0
                  h=27.0
                  p=0.0
                  @icon_action
                box
                  with
                    w=fill
                    h=fill
                    align-x=center
                    align-y=center
                  text "×"
                    with
                      size=13.0
                      wrap=none
                active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
                hovered bg=elevated text=fg
                pressed bg=subtle text=fg
        box
          with
            w=fill
            pl=16.0
            pr=16.0
            pt=14.0
            pb=6.0
          row
            with
              w=fill
              gap=6.0
              align=center
            text "CHANNELS"
              with
                size=10.0
                wrap=none
                font=code_semibold
                @text-label
            space w=fill
            text len(rooms)
              with
                size=10.5
                wrap=none
                font=code_medium
                @text-label
            if !channel_create_open
              button -> emit(toggle_channel_create)
                with
                  label="New channel"
                  expanded=channel_create_open
                  disabled=(loading || mutation_phase != "idle" || !connected)
                  p=0.0
                  @icon_action
                // THE SAME TERM TWICE, because an svg hovers on its OWN bounds
                // and never reads the button's ink: the glyph would brighten
                // under a cursor that can press nothing. The other three mounts
                // hand down a `disabled` prop; this button spells its own out,
                // so this one repeats it rather than inventing a state field.
                IconAction
                  with
                    name="plus"
                    tone="label"
                    px=16.0
                    disabled=(loading || mutation_phase != "idle" || !connected)
                active bg=transparent text=muted border=transparent border-w=1.0 r=5.0
                hovered bg=separator text=fg
                pressed bg=subtle text=fg
            if channel_create_open
              button -> emit(toggle_channel_create)
                with
                  label="Close new channel"
                  expanded=channel_create_open
                  disabled=(loading || mutation_phase != "idle")
                  w=24.0
                  h=24.0
                  p=0.0
                  @icon_action
                box
                  with
                    w=fill
                    h=fill
                    align-x=center
                    align-y=center
                  text "×"
                    with
                      size=13.0
                      wrap=none
                active bg=separator text=muted border=transparent border-w=1.0 r=5.0
                hovered bg=subtle text=fg
                pressed bg=subtle text=fg
        scroll
          with
            dir=vertical
            w=fill
            h=fill
            bar=hidden
          col w=fill gap=2.0
            // DMs are filtered out here, not hidden by CSS: they are real
            // channels and would otherwise list twice, once under each
            // eyebrow. See `chat_sidebar_rooms`.
            for room in rooms
              ChannelButton
                with
                  channel=room.channel
                  selected=(room.channel.id == active_channel)
                  unread=room.unread
                  // EXACTLY THE TERM `choose_channel` STILL REFUSES ON. A load
                  // no longer refuses a click — the last one wins — so a row
                  // greyed while one is in flight would put the swallowing back.
                  // A mutation does still refuse, and the row now says so.
                  disabled=(mutation_phase != "idle")
                forward
                  choose_channel
            // DIRECT — the artifact's own word for it, and the honest
            // one: a two-party channel, not an encrypted one. Reads
            // carry no authorization and every node replicates the
            // state, so nothing here says "private".
            if !empty(dm_rows)
              box
                with
                  w=fill
                  pl=16.0
                  pr=16.0
                  pt=14.0
                  pb=6.0
                row
                  with
                    w=fill
                    gap=6.0
                    align=center
                  text "DIRECT"
                    with
                      size=10.0
                      wrap=none
                      font=code_semibold
                      @text-label
                  space w=fill
                  text len(dm_rows)
                    with
                      size=10.5
                      wrap=none
                      font=code_medium
                      @text-label
            for dm in dm_rows
              DmButton peer=dm.peer selected=(dm.peer.key == active_dm_peer) unread=dm.unread disabled=(mutation_phase != "idle")
                forward
                  choose_dm
        // No account footer: the rail's avatar and Settings already carry the
        // signed-in identity, and a "Not signed in" fallback under a live
        // conversation was pure noise.
    box
      with
        w=1.0
        h=fill
        bg=separator
      space w=1.0 h=1.0
    box
      with
        w=fill
        h=fill
        bg=bg
        clip=true
        px-snap=true
      row w=fill h=fill
        col w=fill h=fill
          if !empty(active_channel)
            col w=fill
              box
                with
                  w=fill
                  h=50.0
                  pl=18.0
                  pr=18.0
                row
                  with
                    w=fill
                    h=fill
                    gap=9.0
                    align=center
                  // A DM IS A PERSON, NOT A `#`. The row is resolved
                  // where `active_dm_peer` is written (`dm_peer_named`),
                  // not filtered here: Ice cannot index a list by field,
                  // so this was `for peer in dm_peers` / `if peer.key ==
                  // active_dm_peer` — every peer deep-cloned and given a
                  // scope String, per frame, so that one of them drew.
                  // A DM whose peer has left the identity roster resolves
                  // to the blank row and falls back to the channel title
                  // below, which is the derived two-party name — never a
                  // blank plate. That fall-through reads the resolved
                  // NAME, not the key: `dm_peer_named` answers a miss with
                  // the blank peer while the key stays set, so branching on
                  // the key drew an empty 24px plate with no name at all.
                  //
                  // AND IT IS BOUNDED, for the same reason the channel title
                  // below is: this row has no main-axis justification, so the
                  // ⋯ that is the only mouse route to Channel details sits at
                  // the right edge only while SOME child takes the row's
                  // slack. With the DM header shrink-sized, ⋯ packed against
                  // the peer's name and moved with its length, and a long
                  // name pushed the huddle control and ⋯ past the pane's clip.
                  if !empty(active_dm.name)
                    box
                      with
                        w=fill
                        clip=true
                      DmHeader peer=active_dm
                  if empty(active_dm.name)
                    text "#"
                      with
                        size=14.0
                        wrap=none
                        font=medium
                        @text-hint
                  // THE TITLE IS BOUNDED, because it is the one thing in this
                  // row that a USER sizes. Everything after it — the badges,
                  // the huddle control, the member count, the ⋯ that is the
                  // only mouse route to Channel details — is shrink-sized, so
                  // a `wrap=none` title claiming its full intrinsic width
                  // pushes them past the row's bounds: a channel named at
                  // length rendered the live-huddle pill as a BLANK plate you
                  // could still click, and dropped ⋯ entirely. The window's
                  // `min-size` bounds the other axis; this bounds this one.
                  if empty(active_dm.name)
                    box
                      with
                        w=fill
                        clip=true
                      text active_channel_name
                        with
                          size=14.0
                          wrap=none
                          font=display
                          @text-fg
                  if active_channel_archived
                    Badge.Outline label="Archived"
                  if active_channel_members_only
                    Badge.Outline label="Members only"
                  // The huddle control, in its three mutually exclusive
                  // states — in it here, in it elsewhere, in none.
                  if huddle_joined && huddle_channel == active_channel
                    HuddleLivePill
                      with
                        name=active_channel_name
                        elapsed=mmss(huddle_now - huddle_joined_at)
                        muted=call_muted
                        popped=huddle_popped
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
                      text "·"
                        with
                          size=12.0
                          wrap=none
                          @text-caption
                      text len(channel_members)
                        with
                          size=12.0
                          wrap=none
                          font=code
                          @text-caption
                      text "added"
                        with
                          size=12.0
                          wrap=none
                          @text-caption
                  // No `space w=fill` here any more: the TITLE is the row's
                  // flexible child now, so the slack it used to hand the
                  // spacer is the slack the title yields back when the name
                  // is long. Two fill children would split it and put the
                  // title back in the business of claiming width.
                  //
                  // No StatusPill here: the titlebar pill is on screen at the
                  // same moment, and the same word twice reads as two systems.
                  button -> emit(toggle_channel_settings)
                    with
                      label="Channel details"
                      expanded=channel_settings_open
                      w=27.0
                      h=25.0
                      p=0.0
                      @icon_action
                    box
                      with
                        w=fill
                        h=fill
                        align-x=center
                        align-y=center
                      text "⋯"
                        with
                          size=14.0
                          wrap=none
                    active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
                    hovered bg=elevated text=fg
                    pressed bg=subtle text=fg
              box
                with
                  w=fill
                  h=1.0
                  bg=separator
                space w=1.0 h=1.0
          stack w=fill h=fill
            col
              with
                w=fill
                h=fill
                gap=9.0
                pl=18.0
                pr=12.0
                pt=16.0
                pb=8.0
              if !connected
                EmptyState
                  with
                    title="Not connected"
                    description="Click the network name in the titlebar to pick or reconnect a network."
              if connected && !loading && empty(messages)
                EmptyState
                  with
                    title="No messages yet"
                    description="Nobody has posted here. Send the first message below."
              // THREE SKELETON ROWS, not a centred sentence — see
              // `SkeletonRow` for the geometry they hold to.
              if connected && loading && empty(messages)
                col w=fill gap=14.0 pt=4.0
                  SkeletonRow
                  SkeletonRow
                  SkeletonRow
              if connected && !empty(messages) && history_view
                box
                  with
                    w=fill
                    h=32.0
                    pl=10.0
                    pr=6.0
                    bg=warning_bg
                    border=warning_line
                    border-w=1.0
                    r=9.0
                  row
                    with
                      w=fill
                      h=fill
                      gap=8.0
                      align=center
                    text "Viewing history"
                      with
                        w=fill
                        size=12.5
                        wrap=none
                        @text-warning
                    button "Jump to latest" -> emit(choose_channel, active_channel)
                      with
                        h=24.0
                        p=5.0
                        @ghost_action
                      active bg=surface text=fg border=warning_line border-w=1.0 r=7.0
                      hovered bg=warning_bg text=fg
                      pressed bg=accent text=fg
              // THIS GATE IS THE SCROLL RESET. `choose_channel` clears
              // `messages` before the fetch, so the whole stack — the
              // scrollable with it — unmounts, its `scrollable::State` drops,
              // and the next room mounts a fresh one at offset 0, which under
              // `anchor-y=end` is the tail. Nothing else resets it: there is no
              // `scroll-to`, and no per-channel offset memory.
              //
              // So do NOT hoist the scrollable above this gate to share it with
              // the empty/loading arm. A shared scrollable survives the switch
              // and hands the new room the offset the old one was left at.
              // `message_stream_reset_contract` in `tests/app.ice` is the fence:
              // it asserts `#chat/message-stream` is GONE once `messages` is.
              if connected && !empty(messages)
                stack w=fill h=fill
                  sensor show=emit(chat_resized, _, _) resize=emit(chat_resized, _, _)
                    space w=fill h=fill
                  mouse press-at=emit(chat_pointer_pressed, _, _)
                    // A CONVERSATION GROWS UP FROM THE COMPOSER. `anchor-y=end`
                    // pins the scroll OFFSET, which does nothing until there is
                    // something to scroll — a channel with four messages in it
                    // left them stranded at the top of an 800px column with
                    // 350px of dead background between the last one and the
                    // composer. `h=shrink` lets the scrollable take only the
                    // height its content needs (still capped by the box's
                    // limits, so a long timeline scrolls exactly as before) and
                    // `align-y=end` drops that block onto the composer.
                    box w=fill h=fill align-y=end
                      scroll #message-stream
                        with
                          dir=vertical
                          w=fill
                          h=shrink
                          anchor-y=end
                          auto=true
                          // PREFETCH BEFORE THE HARD STOP. The offset is
                          // relative to the ANCHOR, which is the end here, so
                          // 1.0 is the top of the scrollback — `chat_scrolled`
                          // starts the older page inside the last tenth of it.
                          // iced drops a viewport identical to the last one it
                          // published, so this is one message per real scroll
                          // step, not one per wheel event.
                          scroll=emit(chat_scrolled, _, _, _, _)
                        // The page controls are the ONE part of the scrollback
                        // that is not a message, so they sit in a plain wrapper
                        // above the keyed column rather than inside it — a
                        // keyed column repeats one template over one list, and
                        // a button folded into that list is another row whose
                        // arrival shifts every index below it.
                        col
                          with
                            w=fill
                            gap=3.0
                            pr=6.0
                          // TWO ARMS, ONE BUTTON. The page is a walk of up to
                          // four sequential round trips and the control carried
                          // no busy reading at all — `disabled` alone is what a
                          // dead button looks like. The label says which it is.
                          if has_older_history && history_loading
                            box
                              with
                                w=fill
                                align-x=center
                                pt=4.0
                                pb=8.0
                              button "Loading older messages…" -> emit(load_more_history)
                                with
                                  disabled=true
                                  h=30.0
                                  p=6.0
                                  @secondary_action
                                active bg=fg/6 text=muted border=fg/10 border-w=1.0 r=8.0
                                hovered bg=fg/10 text=fg border=fg/14
                                pressed bg=fg/14 text=fg
                          if has_older_history && !history_loading
                            box
                              with
                                w=fill
                                align-x=center
                                pt=4.0
                                pb=8.0
                              button "Load older messages" -> emit(load_more_history)
                                with
                                  disabled=(mutation_phase != "idle")
                                  h=30.0
                                  p=6.0
                                  @secondary_action
                                active bg=fg/6 text=muted border=fg/10 border-w=1.0 r=8.0
                                hovered bg=fg/10 text=fg border=fg/14
                                pressed bg=fg/14 text=fg
                          // VIRTUALIZED, AND KEYED BY SEQ. Only the rows the
                          // viewport can see are laid out, so the timeline no
                          // longer shapes text for scrollback nobody is looking
                          // at — the mount stopped being linear in how far back
                          // she paged.
                          //
                          // THE KEY IS NOT DECORATION HERE. This is the app's
                          // one virtual list that PREPENDS: `chat_scrolled`
                          // fires the older page automatically inside the last
                          // tenth of the scrollback, and `prepend_history`
                          // merges up to 256 rows ahead of the timeline. Under
                          // index diffing every one of those rows hands its
                          // measured height to its neighbour, so the rows below
                          // the viewport are re-estimated at 44px, and an
                          // `anchor-y=end` offset — a fixed distance from the
                          // bottom — lands on entirely different messages. The
                          // reader gets thrown backwards mid-sentence, once per
                          // page. Keyed by seq, per-row state and per-row
                          // measurement follow the message instead of the slot,
                          // and the prepend moves nothing on screen.
                          //
                          // 44px is the middle of a real row: a run header (avatar
                          // + name line + body over 12px of card padding, plus
                          // the 14px run-boundary spacer) runs ~68, a grouped
                          // continuation ~31. Biased low on purpose — too small
                          // over-mounts for one pass and corrects itself, too
                          // large leaves a gap at the bottom until the next.
                          //
                          // The scroll above is anchor-y=end, and this needs it:
                          // measuring a never-seen row ABOVE the viewport moves
                          // everything below it, and only a bottom-anchored offset
                          // carries the visible rows along with it.
                          keyed message in messages by=message.seq
                            with
                              w=fill
                              gap=3.0
                              virtual-row=44.0
                            col w=fill gap=0.0
                              if unread_boundary > 0 && message.seq == unread_marker_seq
                                row
                                  with
                                    w=fill
                                    gap=8.0
                                    align=center
                                    pt=8.0
                                    pb=2.0
                                  box
                                    with
                                      w=fill
                                      h=1.0
                                      bg=brand/40
                                    text ""
                                  text "NEW"
                                    with
                                      size=10.0
                                      wrap=none
                                      font=code_semibold
                                      @text-brand
                                  box
                                    with
                                      w=fill
                                      h=1.0
                                      bg=brand/40
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
                                  MessageCard
                                    with
                                      message
                                      selected=true
                                      menu_open=true
                                      disabled=loading
                                      flash=0.0
                                    forward
                                      add_reaction_at
                                      remove_reaction_at
                                      open_thread_for
                                      open_message_reactions
                                      open_message_actions
                                      open_message_link
                              if message.seq != selected_message_seq && message.id == send_flash_id
                                stack #message(message.id) w=fill
                                  MessageCard
                                    with
                                      message
                                      selected=false
                                      menu_open=false
                                      disabled=false
                                      flash=send_flash_value
                                    forward
                                      add_reaction_at
                                      remove_reaction_at
                                      open_thread_for
                                      open_message_reactions
                                      open_message_actions
                                      open_message_link
                              if message.seq != selected_message_seq && message.id != send_flash_id
                                lazy message as cached_message
                                  stack #message(cached_message.id) w=fill
                                    MessageCard
                                      with
                                        message=cached_message
                                        selected=false
                                        menu_open=false
                                        disabled=false
                                        flash=0.0
                                      forward
                                        add_reaction_at
                                        remove_reaction_at
                                        open_thread_for
                                        open_message_reactions
                                        open_message_actions
                                        open_message_link
                  overlay
                    with
                      when=(selected_message_seq > 0 && message_action != "toolbar")
                      dismiss=emit(clear_message_selection)
                      backdrop=transparent
                      p=8.0
                      align-x=end
                      align-y=start
                    content
                      space w=fill h=fill
                    layer
                      float x=0.0 y=message_menu_y
                        col
                          if message_action == "more"
                            stack
                              input "" #message-action-focus <-> message_action_focus
                                with
                                  label="Message action focus"
                                  w=1.0
                                  p=0.0
                                  text-size=1.0
                                  line-h=1.0
                                active bg=transparent border=transparent value=transparent placeholder=transparent border-w=0.0 r=0.0
                                focused bg=transparent border=transparent value=transparent border-w=0.0
                              // The menu is icon + sentence rows on one raised
                              // plate — no Close row: Esc (`escape_target`) and
                              // the backdrop both dismiss, and a menu that
                              // lists its own exit reads as a dialog.
                              box
                                with
                                  w=200.0
                                  p=5.0
                                  style=raised_style()
                                col w=fill gap=1.0
                                  button -> emit(open_message_reactions, selected_message_seq, message_edit_draft, selected_message_rev)
                                    with
                                      label="Manage reactions"
                                      disabled=active_channel_archived
                                      w=fill
                                      h=30.0
                                      p=0.0
                                      @ghost_action
                                    box
                                      with
                                        w=fill
                                        h=fill
                                        pl=9.0
                                        pr=9.0
                                        align-y=center
                                      row
                                        with
                                          w=fill
                                          gap=9.0
                                          align=center
                                        Icon
                                          with
                                            name="emoji"
                                            tone="muted"
                                            px=14.0
                                        text "Add reaction"
                                          with
                                            size=12.5
                                            wrap=none
                                            @text-accent_fg
                                    active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
                                    hovered bg=fg/8 text=fg
                                    pressed bg=fg/12 text=fg
                                  button -> emit(open_thread_for, selected_message_seq)
                                    with
                                      label="Reply in thread"
                                      w=fill
                                      h=30.0
                                      p=0.0
                                      @ghost_action
                                    box
                                      with
                                        w=fill
                                        h=fill
                                        pl=9.0
                                        pr=9.0
                                        align-y=center
                                      row
                                        with
                                          w=fill
                                          gap=9.0
                                          align=center
                                        Icon
                                          with
                                            name="nav-chat"
                                            tone="muted"
                                            px=14.0
                                        text "Reply in thread"
                                          with
                                            size=12.5
                                            wrap=none
                                            @text-accent_fg
                                    active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
                                    hovered bg=fg/8 text=fg
                                    pressed bg=fg/12 text=fg
                                  button -> emit(begin_message_edit, selected_message_seq, message_edit_draft, selected_message_rev)
                                    with
                                      label="Edit message"
                                      w=fill
                                      h=30.0
                                      p=0.0
                                      @ghost_action
                                    box
                                      with
                                        w=fill
                                        h=fill
                                        pl=9.0
                                        pr=9.0
                                        align-y=center
                                      row
                                        with
                                          w=fill
                                          gap=9.0
                                          align=center
                                        Icon
                                          with
                                            name="pencil"
                                            tone="muted"
                                            px=14.0
                                        text "Edit message"
                                          with
                                            size=12.5
                                            wrap=none
                                            @text-accent_fg
                                    active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
                                    hovered bg=fg/8 text=fg
                                    pressed bg=fg/12 text=fg
                                  box
                                    with
                                      w=fill
                                      h=1.0
                                      bg=separator
                                    space w=1.0 h=1.0
                                  button -> emit(arm_message_delete, selected_message_seq, message_edit_draft, selected_message_rev)
                                    with
                                      label="Delete message"
                                      w=fill
                                      h=30.0
                                      p=0.0
                                      @ghost_action
                                    box
                                      with
                                        w=fill
                                        h=fill
                                        pl=9.0
                                        pr=9.0
                                        align-y=center
                                      row
                                        with
                                          w=fill
                                          gap=9.0
                                          align=center
                                        Icon
                                          with
                                            name="trash"
                                            tone="danger"
                                            px=14.0
                                        text "Delete message…"
                                          with
                                            size=12.5
                                            wrap=none
                                            @text-danger
                                    active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
                                    hovered bg=danger_bg text=fg
                                    pressed bg=danger_line text=fg
                          if message_action == "reactions"
                            stack
                              input "" #message-reaction-focus <-> message_action_focus
                                with
                                  label="Message reaction focus"
                                  w=1.0
                                  p=0.0
                                  text-size=1.0
                                  line-h=1.0
                                active bg=transparent border=transparent value=transparent placeholder=transparent border-w=0.0 r=0.0
                                focused bg=transparent border=transparent value=transparent border-w=0.0
                              // The picker is an ADD grid only: removing rides
                              // the message's own reaction chips, which
                              // already toggle off for `reacted_by_me`. Esc
                              // and the backdrop dismiss — no × row.
                              box p=8.0 style=raised_style()
                                flex
                                  with
                                    w=234.0
                                    wrap=wrap
                                    gap-x=2.0
                                    gap-y=2.0
                                    items=start
                                  for emoji in reaction_palette()
                                    button -> emit(add_reaction_submit, emoji)
                                      with
                                        label="Add reaction"
                                        description=emoji
                                        disabled=active_channel_archived
                                        w=27.0
                                        h=27.0
                                        p=0.0
                                        @ghost_action
                                      box
                                        with
                                          w=fill
                                          h=fill
                                          align-x=center
                                          align-y=center
                                        text emoji
                                          with
                                            size=14.0
                                            wrap=none
                                            @text-fg
                                      active bg=transparent text=fg border=transparent border-w=1.0 r=6.0
                                      hovered bg=fg/10
                                      pressed bg=fg/15
                          if message_action == "editing"
                            // NO max-w: this float is painted OVER the row it
                            // edits, so anything narrower than the column
                            // leaves the tail of the old message showing
                            // beside the field.
                            box
                              with
                                w=fill
                                p=3.0
                                style=raised_style()
                              row
                                with
                                  w=fill
                                  gap=4.0
                                  align=center
                                input "" #message-edit <-> message_edit_draft
                                  with
                                    label="Edit message"
                                    hint="Edit message"
                                    disabled=(mutation_phase != "idle")
                                    submit=emit(edit_message_submit)
                                    w=fill
                                    p=6.2
                                    text-size=13.0
                                    line-h=1.2
                                    @control
                                  active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                                  hovered bg=fg/4 border=fg/8
                                  // THE EDITOR IS BORDERLESS AT REST ON PURPOSE, so it
                                  // cannot inherit `@control`'s ring: `active` runs as the
                                  // base of every status and its `border=transparent`
                                  // lands after the recipe's focus color. `begin_message_edit`
                                  // drops the caret in here by hand, so without this line the
                                  // user was dropped into an invisible field whose HOVER read
                                  // stronger than its focus.
                                  focused bg=fg/4 border=ring
                                  disabled value=muted
                                button "Save" -> emit(edit_message_submit)
                                  with
                                    label="Save message changes"
                                    disabled=(mutation_phase != "idle" || empty(trim(message_edit_draft)))
                                    h=28.0
                                    p=6.0
                                    @primary_action
                                button -> emit(clear_message_selection)
                                  with
                                    label="Cancel message edit"
                                    disabled=(mutation_phase != "idle")
                                    w=28.0
                                    h=28.0
                                    p=0.0
                                    @icon_action
                                  box
                                    with
                                      w=fill
                                      h=fill
                                      align-x=center
                                      align-y=center
                                    text "×" size=14.0
                                  active bg=transparent text=muted r=7.0
                                  hovered bg=fg/10 text=fg
                                  pressed bg=fg/15
                          if message_action == "delete"
                            stack
                              input "" #message-delete-focus <-> message_action_focus
                                with
                                  label="Message delete focus"
                                  w=1.0
                                  p=0.0
                                  text-size=1.0
                                  line-h=1.0
                                active bg=transparent border=transparent value=transparent placeholder=transparent border-w=0.0 r=0.0
                                focused bg=transparent border=transparent value=transparent border-w=0.0
                              box p=3.0 style=raised_style()
                                row gap=5.0 align=center
                                  text "Delete this message?" size=12.5 @text-muted
                                  button "Delete" -> emit(delete_message_submit)
                                    with
                                      disabled=(mutation_phase != "idle")
                                      h=26.0
                                      p=5.0
                                      @danger_action
                                  button "Cancel" -> emit(clear_message_selection)
                                    with
                                      disabled=(mutation_phase != "idle")
                                      h=26.0
                                      p=5.0
                                      @secondary_action
                                    active bg=transparent text=muted r=6.0
                                    hovered bg=fg/10 text=fg
                                    pressed bg=fg/15
              // DATA LOSS READS AT LEAST AS LOUD AS AN EXPECTED REFUSAL. This
              // used to be a bare muted sentence, quieter than the archived-
              // channel gate a few lines up — GateNote's own danger-family
              // plate, mirrored (`danger_zone_bg`/`danger_zone_line` are the
              // REVERSIBLE-danger pair the archive button already wears: the
              // draft is recoverable, so this is not `@danger_action` loud).
              if !empty(failed_message_draft)
                box
                  with
                    w=fill
                    px=13.0
                    py=11.0
                    bg=danger_zone_bg
                    border=danger_zone_line
                    border-w=1.0
                    r=9.0
                  row
                    with
                      w=fill
                      gap=8.0
                      align=center
                    box
                      with
                        w=6.0
                        h=6.0
                        bg=danger_dot
                        r=3.0
                      space w=1.0 h=1.0
                    text "An earlier message wasn’t sent"
                      with
                        w=fill
                        size=12.5
                        line-h=1.45
                        @text-danger
                    button "Restore" -> emit(restore_failed_message)
                      with
                        disabled=(!empty(trim(editor_text(message_editor))) || mutation_phase != "idle")
                        h=28.0
                        p=5.0
                        @secondary_action
                      active bg=fg/9 text=fg border=fg/11 border-w=1.0 r=7.0
                      hovered bg=fg/14
                      pressed bg=fg/18
                    button -> emit(dismiss_failed_message)
                      with
                        label="Dismiss unsent message"
                        w=28.0
                        h=28.0
                        p=0.0
                        @icon_action
                      box
                        with
                          w=fill
                          h=fill
                          align-x=center
                          align-y=center
                        text "×" size=14.0
                      active bg=transparent text=muted r=7.0
                      hovered bg=fg/10 text=fg
                      pressed bg=fg/15
            // THE RESULTS FLOAT. This card used to be the column's first
            // child, so a search reflowed the whole conversation down by
            // 148px; as a stack layer it drops over the stream instead and
            // everything beneath keeps its place.
            if search_phase != "idle"
              box
                with
                  w=fill
                  h=fill
                  pl=18.0
                  pr=18.0
                  pt=16.0
                  align-y=start
                box
                  with
                    w=fill
                    max-h=260.0
                    p=6.0
                    bg=elevated
                    border=fg/10
                    border-w=1.0
                    r=10.0
                    shadow=shadow_popover
                    shadow-y=8.0
                    shadow-blur=24.0
                  col w=fill
                    if search_phase == "searching"
                      col w=fill gap=14.0 p=8.0
                        SkeletonRow
                    if search_phase == "done" && empty(search_hits)
                      box w=fill p=14.0 align-x=center
                        text "No messages match"
                          with
                            size=12.5
                            @text-muted
                    if search_phase == "done" && !empty(search_hits)
                      scroll
                        with
                          dir=vertical
                          w=fill
                          h=shrink
                        col w=fill gap=1.0
                          for hit in search_hits
                            ChatSearchResult hit=hit
                              forward
                                open_chat_search_hit
          // The composer is separated from the stream by a hairline and
          // carries the artifact's own 12/16/14 region padding.
          box
            with
              w=fill
              h=1.0
              bg=separator
            space w=1.0 h=1.0
          // THE GATE ABOVE THE PLATE. `post_refusal` IS `post_gate`'s
          // verdict, mirrored into state — the call used to sit at each
          // of its EIGHT mounts, and the extern ABI is by-value, so the
          // member roll was deep-cloned eight times a frame for a
          // two-branch `any()`. The seven writers it was worth avoiding
          // are now seven one-line assignments, and a lint fails the
          // build on a writer that forgets one. An empty reason renders
          // nothing and gates nothing.
          if !empty(post_refusal)
            box
              with
                w=fill
                pl=18.0
                pr=18.0
                pt=12.0
              ComposerGate
                with
                  reason=post_refusal
          box
            with
              w=fill
              pl=18.0
              pr=18.0
              pt=12.0
              pb=14.0
            box
              with
                w=fill
                bg=surface
                border=control_line
                border-w=1.0
                r=12.0
                clip=true
                shadow=shadow_popover
                shadow-y=1.0
                shadow-blur=2.0
              col w=fill
                extern rich_composer(message_editor, "Message the channel…", (loading || !connected || empty(active_channel) || !empty(post_refusal)), shift_held, 44.0, 150.0, 10.0) #message -> emit(composer_event, _)
                // The Slack seat: format controls on the left, send on the
                // right, one row under the input. `ComposerMarks` is the SAME
                // row the rail's composer mounts — it moved into a component
                // the day the rail stopped going without one.
                box
                  with
                    w=fill
                    pl=8.0
                    pr=8.0
                    pb=8.0
                  row
                    with
                      w=fill
                      gap=2.0
                      align=center
                    ComposerMarks
                      with
                        disabled=(loading || !connected || empty(active_channel) || !empty(post_refusal))
                      events
                        mark -> emit(composer_mark, _)
                    space w=fill
                    text "↵ send · ⇧↵ newline"
                      with
                        size=10.5
                        wrap=none
                        font=code_medium
                        @text-muted
                    space w=8.0
                    button "Send" -> emit(composer_event, composer_submit_event())
                      with
                        disabled=(loading || !connected || empty(active_channel) || !empty(post_refusal) || empty(trim(editor_text(message_editor))))
                        h=29.0
                        @primary_action
                        @px-12px
                        @py-7px
        // THE DETAILS DRAWER — a sidebar-toned rail with one header bar, the
        // channel's identity up top, eyebrowed NAME and MEMBERS sections, and
        // the archive act alone at the bottom where a destructive control
        // belongs. It stopped being an unlabeled pile of input rows.
        if channel_settings_open && !empty(active_channel)
          box
            with
              w=1.0
              h=fill
              bg=separator
            space w=1.0 h=1.0
          box
            with
              w=320.0
              h=fill
              bg=sidebar
            col w=fill h=fill
              box
                with
                  w=fill
                  h=50.0
                  pl=16.0
                  pr=10.0
                row
                  with
                    w=fill
                    h=fill
                    gap=6.0
                    align=center
                  text "Channel details"
                    with
                      w=fill
                      size=13.5
                      wrap=none
                      font=display
                      @text-fg
                  button -> emit(toggle_channel_settings)
                    with
                      label="Close channel details"
                      expanded=channel_settings_open
                      w=28.0
                      h=28.0
                      p=0.0
                      @icon_action
                    box
                      with
                        w=fill
                        h=fill
                        align-x=center
                        align-y=center
                      text "×" size=14.0
                    active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
                    hovered bg=fg/10 text=fg
                    pressed bg=fg/15
              box
                with
                  w=fill
                  h=1.0
                  bg=separator
                space w=1.0 h=1.0
              scroll
                with
                  dir=vertical
                  w=fill
                  h=fill
                col
                  with
                    w=fill
                    gap=16.0
                    pl=16.0
                    pr=16.0
                    pt=14.0
                    pb=14.0
                  col w=fill gap=7.0
                    row
                      with
                        w=fill
                        gap=7.0
                        align=center
                      if !active_channel_members_only
                        text "#"
                          with
                            size=14.0
                            wrap=none
                            font=medium
                            @text-hint
                      if active_channel_members_only
                        text "◆"
                          with
                            size=13.0
                            wrap=none
                            @text-label
                      text active_channel_name
                        with
                          w=fill
                          size=14.0
                          wrap=none
                          font=display
                          @text-fg
                    if active_channel_archived || active_channel_members_only
                      row
                        with
                          w=fill
                          gap=5.0
                          align=center
                        if active_channel_archived
                          Badge.Outline label="Archived"
                        if active_channel_members_only
                          Badge.Outline label="Members only"
                  col w=fill gap=6.0
                    Eyebrow label="NAME" note=""
                    row
                      with
                        w=fill
                        gap=6.0
                        align=center
                      input "" #channel-name <-> channel_name_draft
                        with
                          label="Channel name"
                          hint="Channel name"
                          disabled=(mutation_phase != "idle")
                          submit=emit(rename_channel_submit)
                          w=fill
                          p=6.6
                          text-size=13.0
                          line-h=1.2
                          @control
                        active bg=surface value=fg placeholder=muted selection=fg/18 border-w=1.0 r=8.0
                        hovered bg=surface border=control_line
                        disabled value=muted
                      button "Rename" -> emit(rename_channel_submit)
                        with
                          disabled=(mutation_phase != "idle" || empty(trim(channel_name_draft)))
                          h=29.0
                          p=6.0
                          @secondary_action
                  col w=fill gap=6.0
                    row
                      with
                        w=fill
                        gap=6.0
                        align=center
                      Eyebrow label="MEMBERS" note=""
                      space w=fill
                      // Blank at zero, like every other count in this app —
                      // the plate under this eyebrow already says "No members
                      // added", so a `0` beside it repeats it in digits.
                      text count_label(len(channel_members))
                        with
                          size=10.5
                          wrap=none
                          font=code_medium
                          @text-label
                    row
                      with
                        w=fill
                        gap=6.0
                        align=center
                      input "" #member-key <-> member_key_draft
                        with
                          label="Member public key"
                          hint="Member key (64 hex)"
                          disabled=(mutation_phase != "idle")
                          submit=emit(add_channel_member_submit)
                          w=fill
                          p=7.4
                          text-size=11.5
                          line-h=1.2
                          font=code
                          @control
                        active bg=surface value=fg placeholder=muted selection=fg/18 border-w=1.0 r=8.0
                        hovered bg=surface border=control_line
                        disabled value=muted
                      button "Add" -> emit(add_channel_member_submit)
                        with
                          disabled=(mutation_phase != "idle" || empty(trim(member_key_draft)))
                          h=29.0
                          p=6.0
                          @secondary_action
                    if empty(channel_members)
                      text "No members added. An Open channel needs none — membership only gates posting in a members-only channel."
                        with
                          w=fill
                          size=11.5
                          line-h=1.5
                          @text-caption
                    if !empty(channel_members)
                      col w=fill gap=1.0
                        for member in channel_members
                          ChatMemberRow member=member disabled=(mutation_phase != "idle")
                            forward
                              remove_channel_member_submit
              box
                with
                  w=fill
                  h=1.0
                  bg=separator
                space w=1.0 h=1.0
              box
                with
                  w=fill
                  pl=16.0
                  pr=16.0
                  pt=10.0
                  pb=12.0
                col w=fill
                  // A SOFT danger plate, not `@danger_action`. Every other
                  // `@danger_action` in the app confirms a destroy — Delete
                  // message, Discard draft. Archiving is the same toggle as the
                  // `Unarchive channel` button that replaces it, and wearing the
                  // loudest control in the app made a reversible state change
                  // read as the point of no return.
                  if !active_channel_archived
                    button "Archive channel" -> emit(archive_channel_submit)
                      with
                        disabled=(mutation_phase != "idle")
                        w=fill
                        h=30.0
                        p=6.0
                        @secondary_action
                      active bg=danger_zone_bg text=danger border=danger_zone_line border-w=1.0 r=9.0
                      hovered bg=danger_bg text=danger border=danger_line
                      pressed bg=danger_line text=fg
                  if active_channel_archived
                    button "Unarchive channel" -> emit(unarchive_channel_submit)
                      with
                        disabled=(mutation_phase != "idle")
                        w=fill
                        h=30.0
                        p=6.0
                        @secondary_action
        if active_thread_seq > 0 && !channel_settings_open
          box
            with
              w=1.0
              h=fill
              bg=separator
            space w=1.0 h=1.0
          // THE RAIL IS A PANE, NOT A CARD: the artifact's 330px sidebar-toned
          // plate with a 50px header bar and 16px body insets, mirroring the
          // details drawer one `if` up — the old 300px muted_bg card with its
          // own 12px air read as a third surface family.
          box
            with
              w=330.0
              h=fill
              bg=sidebar
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
                  box
                    with
                      w=fill
                      h=50.0
                      pl=16.0
                      pr=16.0
                    row
                      with
                        w=fill
                        h=fill
                        gap=7.0
                        align=center
                      if thread_target_seq <= 0
                        text "Thread"
                          with
                            size=13.0
                            wrap=none
                            font=display
                            @text-fg
                      if thread_target_seq > 0
                        text "Thread result"
                          with
                            size=13.0
                            wrap=none
                            font=display
                            @text-fg
                      row gap=2.0 align=center
                        // The same discriminant the main header reads: the
                        // RESOLVED name, so a roster miss shows `# <channel>`
                        // in both places instead of two readings of one room.
                        if empty(active_dm.name)
                          text "#"
                            with
                              size=11.0
                              wrap=none
                              @text-caption
                        text active_channel_name
                          with
                            size=11.0
                            wrap=none
                            @text-caption
                      space w=fill
                      button -> emit(close_thread)
                        with
                          label="Close thread"
                          disabled=(mutation_phase != "idle")
                          w=24.0
                          h=24.0
                          p=0.0
                          @icon_action
                        box
                          with
                            w=fill
                            h=fill
                            align-x=center
                            align-y=center
                          text "×" size=14.0
                        active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
                        hovered bg=fg/10 text=fg
                        pressed bg=fg/15
                  box
                    with
                      w=fill
                      h=1.0
                      bg=separator
                    space w=1.0 h=1.0
                  // STAYS TOP-ANCHORED, unlike the channel timeline. A thread
                  // is read from its ROOT down; pushing a short thread to the
                  // bottom of the rail would strand the message it is about in
                  // the middle of the pane with dead space above it. A channel
                  // is a running feed anchored at now, which is why that one
                  // grows up from its composer and this one does not.
                  scroll
                    with
                      dir=vertical
                      w=fill
                      h=fill
                      anchor-y=end
                      auto=true
                    // The 16px right inset doubles as the scrollbar
                    // clearance the code/quote slabs needed (#927).
                    //
                    // VIRTUALIZED, on the stream's terms and for the stream's
                    // reason: a thread pages in at the same 256 replies a
                    // channel does, and a plain column culls only `draw` —
                    // `update`, `mouse_interaction`, `overlay` and `layout`
                    // walk every reply ever loaded, on every event and every
                    // frame. 44px is the stream's estimate and a reply is the
                    // stream's card. The scroll above is anchor-y=end, which
                    // virtualization needs.
                    col
                      with
                        w=fill
                        gap=3.0
                        pl=16.0
                        pr=16.0
                        pt=12.0
                        pb=8.0
                        virtual-row=44.0
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
                            forward
                              open_message_link
                        // THE REST SPLIT THE WAY THE STREAM'S DO, and for the
                        // same reason: a `lazy` subtree may read nothing but
                        // its dependency, so every row whose card reads SCREEN
                        // state — the search target, the open action menu, the
                        // settling ✓ — has to be its own live arm, and what is
                        // left is a pure function of the reply. Selection wins
                        // over the flash here exactly as it does in the stream.
                        if thread_message.seq != active_thread_seq && (thread_message.seq == thread_target_seq || thread_message.seq == thread_selected_seq)
                          ThreadMessageCard
                            with
                              message=thread_message
                              selected=(thread_message.seq == thread_target_seq)
                              menu_open=(thread_message.seq == thread_selected_seq)
                              disabled=loading
                              flash=0.0
                            forward
                              add_reaction_at
                              remove_reaction_at
                              open_thread_for
                              open_thread_message_actions
                              open_thread_message_reactions
                              open_message_link
                        if thread_message.seq != active_thread_seq && thread_message.seq != thread_target_seq && thread_message.seq != thread_selected_seq && thread_message.id == thread_send_flash_id
                          ThreadMessageCard
                            with
                              message=thread_message
                              selected=false
                              menu_open=false
                              disabled=loading
                              flash=send_flash_value
                            forward
                              add_reaction_at
                              remove_reaction_at
                              open_thread_for
                              open_thread_message_actions
                              open_thread_message_reactions
                              open_message_link
                        // THE QUIET ARM. Virtualization stops an offscreen
                        // reply from being laid out; this stops a VISIBLE one
                        // from being rebuilt — ~60 nodes of a11y keys and
                        // scope strings per reply, on every frame the rail is
                        // open. `disabled=false` is the stream's bargain too: a
                        // cached row cannot see `loading`, and a row nobody is
                        // hovering has no button to disable.
                        if thread_message.seq != active_thread_seq && thread_message.seq != thread_target_seq && thread_message.seq != thread_selected_seq && thread_message.id != thread_send_flash_id
                          lazy thread_message as cached_reply
                            ThreadMessageCard
                              with
                                message=cached_reply
                                selected=false
                                menu_open=false
                                disabled=false
                                flash=0.0
                              forward
                                add_reaction_at
                                remove_reaction_at
                                open_thread_for
                                open_thread_message_actions
                                open_thread_message_reactions
                                open_message_link
                      if thread_has_more && thread_next_reply_offset >= 0 && thread_loading
                        button "Loading replies…" -> emit(load_more_thread)
                          with
                            disabled=true
                            w=fill
                            h=28.0
                            p=5.0
                            @secondary_action
                          active bg=transparent text=muted r=7.0
                          hovered bg=fg/9 text=fg
                          pressed bg=brand_bg
                      if thread_has_more && thread_next_reply_offset >= 0 && !thread_loading
                        button "Load more replies" -> emit(load_more_thread)
                          with
                            disabled=(mutation_phase != "idle")
                            w=fill
                            h=28.0
                            p=5.0
                            @secondary_action
                          active bg=transparent text=muted r=7.0
                          hovered bg=fg/9 text=fg
                          pressed bg=brand_bg
                  // Same danger-family plate as the stream's — see the note
                  // there.
                  if !empty(failed_reply_draft)
                    box
                      with
                        w=fill
                        pl=16.0
                        pr=16.0
                        pt=8.0
                      box
                        with
                          w=fill
                          px=13.0
                          py=11.0
                          bg=danger_zone_bg
                          border=danger_zone_line
                          border-w=1.0
                          r=9.0
                        row
                          with
                            w=fill
                            gap=6.0
                            align=center
                          box
                            with
                              w=6.0
                              h=6.0
                              bg=danger_dot
                              r=3.0
                            space w=1.0 h=1.0
                          text "Unsent reply"
                            with
                              w=fill
                              size=12.5
                              line-h=1.45
                              @text-danger
                          button "Restore" -> emit(restore_failed_reply)
                            with
                              disabled=(!empty(trim(editor_text(reply_editor))))
                              h=26.0
                              p=5.0
                              @secondary_action
                            active bg=fg/9 text=fg border=fg/11 border-w=1.0 r=7.0
                            hovered bg=fg/14
                            pressed bg=fg/18
                          button "×" -> emit(dismiss_failed_reply)
                            with
                              label="Dismiss unsent reply"
                              w=26.0
                              h=26.0
                              p=4.0
                              @ghost_action
                            active bg=transparent text=muted r=7.0
                            hovered bg=fg/10 text=fg
                            pressed bg=fg/15
                  // The stream's composer plate, in the rail's width: same
                  // surface/control_line/r12 chrome, the same `ComposerMarks`
                  // row, the same Send.
                  //
                  // WHAT THE 330px BUYS THE MARKS WITH is the `↵ send · ⇧↵
                  // newline` hint, not the buttons: 282px of seat cannot hold
                  // marks (~115) + hint (~120) + Send (~54) at once. The hint
                  // is a LABEL for behaviour both composers already share, and
                  // the stream's plate is on screen beside the rail saying it;
                  // the marks are the only visible door to formatting a reply.
                  //
                  // And the same REFUSAL, which it used to skip: editor and Send
                  // carry the stream's `!connected || !empty(post_gate(…))` terms
                  // verbatim, or a channel the module will refuse still buys an
                  // optimistic append and a rollback under a raw 400.
                  // `active_channel_archived` drops out because post_gate's
                  // `channel_archived` arm IS that case (see ComposerGate) — one
                  // discriminant, not two. The reason SENTENCE stays mounted once,
                  // over the stream's plate: 330px has no room to say it twice.
                  box
                    with
                      w=fill
                      pl=16.0
                      pr=16.0
                      pt=10.0
                      pb=14.0
                    box
                      with
                        w=fill
                        bg=surface
                        border=control_line
                        border-w=1.0
                        r=12.0
                        clip=true
                        shadow=shadow_popover
                        shadow-y=1.0
                        shadow-blur=2.0
                      col w=fill
                        extern rich_composer(reply_editor, "Reply…", (thread_loading || !connected || !empty(post_refusal)), shift_held, 44.0, 150.0, 10.0) #reply -> emit(reply_composer_event, _)
                        box
                          with
                            w=fill
                            pl=8.0
                            pr=8.0
                            pb=8.0
                          row
                            with
                              w=fill
                              gap=2.0
                              align=center
                            ComposerMarks
                              with
                                disabled=(thread_loading || !connected || !empty(post_refusal))
                              events
                                mark -> emit(reply_composer_mark, _)
                            space w=fill
                            button "Send" -> emit(reply_composer_event, composer_submit_event())
                              with
                                label="Send reply"
                                disabled=(thread_loading || !connected || !empty(post_refusal) || empty(trim(editor_text(reply_editor))))
                                h=28.0
                                @primary_action
                                @px-11px
                                @py-6px
              overlay
                with
                  when=(thread_selected_seq > 0 && thread_message_action != "toolbar")
                  dismiss=emit(clear_thread_message_selection)
                  backdrop=transparent
                  p=8.0
                  align-x=end
                  align-y=start
                content
                  space w=fill h=fill
                layer
                  float x=0.0 y=thread_menu_y
                    col
                      if thread_message_action == "more"
                        stack
                          input "" #thread-action-focus <-> message_action_focus
                            with
                              label="Thread action focus"
                              w=1.0
                              p=0.0
                              text-size=1.0
                              line-h=1.0
                            active bg=transparent border=transparent value=transparent placeholder=transparent border-w=0.0 r=0.0
                            focused bg=transparent border=transparent value=transparent border-w=0.0
                          // Mirrors the stream menu above: icon rows, no Close.
                          box
                            with
                              w=200.0
                              p=5.0
                              style=raised_style()
                            col w=fill gap=1.0
                              button -> emit(open_thread_message_reactions, thread_selected_seq, thread_edit_draft, thread_selected_rev)
                                with
                                  label="Manage reactions"
                                  disabled=active_channel_archived
                                  w=fill
                                  h=30.0
                                  p=0.0
                                  @ghost_action
                                box
                                  with
                                    w=fill
                                    h=fill
                                    pl=9.0
                                    pr=9.0
                                    align-y=center
                                  row
                                    with
                                      w=fill
                                      gap=9.0
                                      align=center
                                    Icon
                                      with
                                        name="emoji"
                                        tone="muted"
                                        px=14.0
                                    text "Add reaction"
                                      with
                                        size=12.5
                                        wrap=none
                                        @text-accent_fg
                                active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
                                hovered bg=fg/8 text=fg
                                pressed bg=fg/12 text=fg
                              button -> emit(begin_thread_message_edit, thread_selected_seq, thread_edit_draft, thread_selected_rev)
                                with
                                  label="Edit message"
                                  w=fill
                                  h=30.0
                                  p=0.0
                                  @ghost_action
                                box
                                  with
                                    w=fill
                                    h=fill
                                    pl=9.0
                                    pr=9.0
                                    align-y=center
                                  row
                                    with
                                      w=fill
                                      gap=9.0
                                      align=center
                                    Icon
                                      with
                                        name="pencil"
                                        tone="muted"
                                        px=14.0
                                    text "Edit message"
                                      with
                                        size=12.5
                                        wrap=none
                                        @text-accent_fg
                                active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
                                hovered bg=fg/8 text=fg
                                pressed bg=fg/12 text=fg
                              box
                                with
                                  w=fill
                                  h=1.0
                                  bg=separator
                                space w=1.0 h=1.0
                              button -> emit(arm_thread_message_delete, thread_selected_seq, thread_edit_draft, thread_selected_rev)
                                with
                                  label="Delete message"
                                  w=fill
                                  h=30.0
                                  p=0.0
                                  @ghost_action
                                box
                                  with
                                    w=fill
                                    h=fill
                                    pl=9.0
                                    pr=9.0
                                    align-y=center
                                  row
                                    with
                                      w=fill
                                      gap=9.0
                                      align=center
                                    Icon
                                      with
                                        name="trash"
                                        tone="danger"
                                        px=14.0
                                    text "Delete message…"
                                      with
                                        size=12.5
                                        wrap=none
                                        @text-danger
                                active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
                                hovered bg=danger_bg text=fg
                                pressed bg=danger_line text=fg
                      if thread_message_action == "reactions"
                        stack
                          input "" #thread-reaction-focus <-> message_action_focus
                            with
                              label="Thread reaction focus"
                              w=1.0
                              p=0.0
                              text-size=1.0
                              line-h=1.0
                            active bg=transparent border=transparent value=transparent placeholder=transparent border-w=0.0 r=0.0
                            focused bg=transparent border=transparent value=transparent border-w=0.0
                          // Same ADD grid as the stream picker — removal is
                          // the reply's own reaction chips.
                          box p=8.0 style=raised_style()
                            flex
                              with
                                w=234.0
                                wrap=wrap
                                gap-x=2.0
                                gap-y=2.0
                                items=start
                              for emoji in reaction_palette()
                                button -> emit(add_reaction_at, thread_selected_seq, emoji)
                                  with
                                    label="Add reaction"
                                    description=emoji
                                    disabled=active_channel_archived
                                    w=27.0
                                    h=27.0
                                    p=0.0
                                    @ghost_action
                                  box
                                    with
                                      w=fill
                                      h=fill
                                      align-x=center
                                      align-y=center
                                    text emoji
                                      with
                                        size=14.0
                                        wrap=none
                                        @text-fg
                                  active bg=transparent text=fg border=transparent border-w=1.0 r=6.0
                                  hovered bg=fg/10
                                  pressed bg=fg/15
                      if thread_message_action == "editing"
                        box
                          with
                            w=fill
                            p=3.0
                            style=raised_style()
                          row
                            with
                              w=fill
                              gap=4.0
                              align=center
                            input "" #thread-edit <-> thread_edit_draft
                              with
                                label="Edit message"
                                hint="Edit message"
                                disabled=(mutation_phase != "idle")
                                submit=emit(edit_thread_message_submit)
                                w=fill
                                p=6.2
                                text-size=13.0
                                line-h=1.2
                                @control
                              active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                              hovered bg=fg/4 border=fg/8
                              // Same borderless-at-rest editor as the main stream's, so
                              // the same authored ring — see `#message-edit` above.
                              focused bg=fg/4 border=ring
                              disabled value=muted
                            button "Save" -> emit(edit_thread_message_submit)
                              with
                                label="Save message changes"
                                disabled=(mutation_phase != "idle" || empty(trim(thread_edit_draft)))
                                h=28.0
                                p=6.0
                                @primary_action
                            button -> emit(clear_thread_message_selection)
                              with
                                label="Cancel message edit"
                                disabled=(mutation_phase != "idle")
                                w=28.0
                                h=28.0
                                p=0.0
                                @icon_action
                              box
                                with
                                  w=fill
                                  h=fill
                                  align-x=center
                                  align-y=center
                                text "×" size=14.0
                              active bg=transparent text=muted r=7.0
                              hovered bg=fg/10 text=fg
                              pressed bg=fg/15
                      if thread_message_action == "delete"
                        stack
                          input "" #thread-delete-focus <-> message_action_focus
                            with
                              label="Thread delete focus"
                              w=1.0
                              p=0.0
                              text-size=1.0
                              line-h=1.0
                            active bg=transparent border=transparent value=transparent placeholder=transparent border-w=0.0 r=0.0
                            focused bg=transparent border=transparent value=transparent border-w=0.0
                          box p=3.0 style=raised_style()
                            row gap=5.0 align=center
                              text "Delete this message?" size=12.5 @text-muted
                              button "Delete" -> emit(delete_thread_message_submit)
                                with
                                  disabled=(mutation_phase != "idle")
                                  h=26.0
                                  p=5.0
                                  @danger_action
                              button "Cancel" -> emit(clear_thread_message_selection)
                                with
                                  disabled=(mutation_phase != "idle")
                                  h=26.0
                                  p=5.0
                                  @secondary_action
                                active bg=transparent text=muted r=6.0
                                hovered bg=fg/10 text=fg
                                pressed bg=fg/15
