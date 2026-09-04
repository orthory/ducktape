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
// A TIMELINE IS ITS OWN EVENT ISLAND. A keyed lazy nested directly in
// ChatScreen inherits every event the screen declares, even though a message
// row can only fire these six. With 4,096 rows that used to manufacture
// 48 callback routes per row on every unrelated rebuild. This component keeps
// the row loop's routing surface equal to what the row can actually do.
component MessageTimeline(messages:[ChatMessage], unread_boundary:i64, unread_marker_seq:i64, selected_message_seq:i64)
  emits
    add_reaction_at(i64, str)
    remove_reaction_at(i64, str)
    open_thread_for(i64)
    open_message_reactions(i64, str, i64)
    open_message_actions(i64, str, i64)
    open_message_link(str)
  // KEYED BY STABLE VIEW IDENTITY. This is the app's one virtual list that
  // prepends, so row state and measured height must follow the message rather
  // than the slot it occupied before the page arrived.
  keyed message in messages by=message.view_key
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
      // The selected row stays live because selection is screen state. Quiet
      // rows keep their own element/layout memo within the whole-timeline
      // boundary. The plain dependency is intentional: the
      // outer keyed lazy lends `messages` as a closure-local value, while the
      // compiler permits nested cheap-key capture only from app state. A live
      // batch rebuilds this bounded window once; an unchanged frame never
      // enters it at all.
      if message.seq == selected_message_seq
        stack #message(message.id) w=fill
          MessageCard
            with
              message
              selected=true
              menu_open=true
            forward
              add_reaction_at
              remove_reaction_at
              open_thread_for
              open_message_reactions
              open_message_actions
              open_message_link
      if message.seq != selected_message_seq
        lazy message as cached_message
          stack #message(cached_message.id) w=fill
            MessageCard
              with
                message=cached_message
                selected=false
                menu_open=false
              forward
                add_reaction_at
                remove_reaction_at
                open_thread_for
                open_message_reactions
                open_message_actions
                open_message_link

// Same boundary for the rail: the root, target and menu rows stay live; quiet
// replies keep their per-row memo. Paging controls stay outside this component
// because they are constant-size chrome, not part of the chain-fed list.
component ThreadTimeline(messages:[ChatMessage], active_thread_seq:i64, thread_target_seq:i64, thread_selected_seq:i64)
  emits
    add_reaction_at(i64, str)
    remove_reaction_at(i64, str)
    open_thread_for(i64)
    open_thread_message_actions(i64, str, i64)
    open_thread_message_reactions(i64, str, i64)
    open_message_link(str)
  keyed thread_message in messages by=thread_message.view_key
    with
      w=fill
      gap=3.0
      virtual-row=44.0
    col w=fill gap=0.0
      if thread_message.seq == active_thread_seq
        ThreadParentBlock message=thread_message
          forward
            open_message_link
      if thread_message.seq != active_thread_seq && (thread_message.seq == thread_target_seq || thread_message.seq == thread_selected_seq)
        ThreadMessageCard
          with
            message=thread_message
            selected=(thread_message.seq == thread_target_seq)
            menu_open=(thread_message.seq == thread_selected_seq)
          forward
            add_reaction_at
            remove_reaction_at
            open_thread_for
            open_thread_message_actions
            open_thread_message_reactions
            open_message_link
      if thread_message.seq != active_thread_seq && thread_message.seq != thread_target_seq && thread_message.seq != thread_selected_seq
        lazy thread_message as cached_reply
          ThreadMessageCard
            with
              message=cached_reply
              selected=false
              menu_open=false
            forward
              add_reaction_at
              remove_reaction_at
              open_thread_for
              open_thread_message_actions
              open_thread_message_reactions
              open_message_link

component ChatScreen(endpoint:str, network_name:str, network_chain_id:str, status:str, block_height:i64, bind search_draft:str, search_phase:SearchPhase, search_query:str, search_hits:[ChatSearchHit], rooms:[ChatSidebarRow], dm_rows:[DmSidebarRow], channel_create_open:bool, connected:bool, loading:bool, mutation_phase:MutationPhase, active_channel:str, active_dm_peer:str, active_dm:DmPeer, active_channel_name:str, active_channel_archived:bool, active_channel_members_only:bool, channel_members:[ChatMember], post_refusal:str, huddle_joined:bool, huddle_channel:str, huddle_channel_name:str, huddle_joined_at:i64, huddle_now:i64, call_muted:bool, huddle_popped:bool, messages:[ChatMessage], has_older_history:bool, history_view:bool, at_live_tail:bool, history_loading:bool, unread_boundary:i64, unread_marker_seq:i64, selected_message_seq:i64, selected_message_rev:i64, message_action:MessageAction, bind message_edit_draft:str, channel_settings_open:bool, bind channel_name_draft:str, bind member_key_draft:str, active_thread_seq:i64, thread_target_seq:i64, thread_messages:[ChatMessage], thread_selected_seq:i64, thread_selected_rev:i64, thread_message_action:MessageAction, bind thread_edit_draft:str, thread_has_more:bool, thread_next_reply_seq:i64, thread_loading:bool)
  lifetime retained
  emits
    search_chat_submit()
    clear_chat_search()
    open_chat_search_hit(str, i64, i64)
    toggle_channel_create()
    choose_channel(str)
    choose_dm(str)
    toggle_channel_settings()
    focus_huddle()
    leave_huddle_here()
    huddle_go_channel()
    join_huddle_submit()
    load_more_history()
    chat_scrolled(f64, f64, f64, f64)
    open_message_link(str)
    copy_to_clipboard(str, str)
    copy_message_link(str)
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
    composer_submitted(ComposerKind, str, str)
    rename_channel_submit()
    archive_channel_submit()
    unarchive_channel_submit()
    add_channel_member_submit()
    remove_channel_member_submit(str)
    close_thread()
    open_thread_message_actions(i64, str, i64)
    open_thread_message_reactions(i64, str, i64)
    begin_thread_message_edit(i64, str, i64)
    arm_thread_message_delete(i64, str, i64)
    clear_thread_message_selection()
    edit_thread_message_submit()
    delete_thread_message_submit()
    load_more_thread()
  state
    message_action_focus = ""
    chat_pointer_y = 0.0
    chat_height = 720.0
    thread_pointer_y = 0.0
    thread_height = 720.0
  // PRESSES, NOT MOVES. Geometry belongs to this screen instance: the pointer
  // y is read exactly once when an action menu opens, and its computed anchor
  // stays in the view; only the selected message crosses into app state.
  on chat_pointer_pressed(_x, y)
    chat_pointer_y = y
  on chat_resized(_width, height)
    chat_height = height
  on thread_pointer_pressed(_x, y)
    thread_pointer_y = y
  on thread_resized(_width, height)
    thread_height = height
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
                // NOT `|| search_phase == SearchPhase.searching`. The field went dead the instant Enter
                // was pressed and stayed dead for the whole round trip, so the
                // query could not be refined while waiting — and a disabled
                // input drops the caret besides. The `chat_search` replace lane
                // drops the superseded reply; killing the field bought nothing.
                disabled=!connected
                submit=emit(search_chat_submit)
                w=fill
                p=6.2
                text-size=13.0
                line-h=1.2
                @control
              // NO `border=` HERE — the recipe already owns the resting border.
              active bg=surface value=fg placeholder=hint selection=fg/18 border-w=1.0 r=8.0
              hovered bg=muted_bg border=control_line
              disabled bg=transparent value=muted
            // THE FLOAT'S GATE, OR THE FIELD'S: phase alone left un-clearable
            // text on every path that parks idle with a draft standing —
            // `chat_search_failed`, the picker dismissals, `channel_created`.
            // The button clears both the float and the field, so either
            // earns it.
            if search_phase != SearchPhase.idle || !empty(trim(search_draft))
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
                  text "×" size=13.0 wrap=none
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
                  disabled=(loading || mutation_phase != MutationPhase.idle || !connected)
                  p=0.0
                  @icon_action
                // `color=inherit` (ducktape-ui#606): the glyph draws the
                // button's status-resolved text color — hover ink keys on the
                // BUTTON's bounds, disabled ink on the status ladder, no
                // second spelling of the disabled term. `active text=label`
                // keeps the resting `label` tone the old tinted mount named;
                // token and tone are the same hex in both palettes.
                svg icon("plus") memory
                  with
                    color=inherit
                    w=16.0
                    h=16.0
                active bg=transparent text=label border=transparent border-w=1.0 r=5.0
                hovered bg=separator text=fg
                pressed bg=subtle text=fg
            if channel_create_open
              button -> emit(toggle_channel_create)
                with
                  label="Close new channel"
                  expanded=channel_create_open
                  disabled=(loading || mutation_phase != MutationPhase.idle)
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
                  text "×" size=13.0 wrap=none
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
                  disabled=(mutation_phase != MutationPhase.idle)
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
              DmButton
                with
                  peer=dm.peer
                  selected=(dm.peer.key == active_dm_peer)
                  unread=dm.unread
                  disabled=(mutation_phase != MutationPhase.idle)
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
                    box w=fill clip=true
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
                    box w=fill clip=true
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
                  // THE HUDDLE CONTROL, and it is now TWO states, not three:
                  // start one, or raise the window a running one was popped
                  // into. The two that are gone — the live pill in the
                  // huddle's own room, and the "call in progress elsewhere"
                  // chip in every other room — were both saying what the dock
                  // beside this header says on every screen, with faces, a
                  // clock and a way in. One huddle surface at a time.
                  if huddle_joined && huddle_channel == active_channel && huddle_popped
                    HuddleLivePill
                      with
                        elapsed=mmss(huddle_now - huddle_joined_at)
                        muted=call_muted
                      forward
                        focus_huddle
                        leave_huddle_here
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
                      text "⋯" size=14.0 wrap=none
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
                col
                  with
                    w=fill
                    gap=14.0
                    pt=4.0
                  SkeletonRow
                  SkeletonRow
                  SkeletonRow
              // NO "VIEWING HISTORY" BAND. `history_view` is still the read
              // cursor's gate (`lifecycle.ice` refuses to mark a room read off a
              // window around an old message) — it just has no banner any more.
              // An amber band pushing the conversation down 32px told the reader
              // something she could already see, and the one control on it, the
              // way back to now, is a float over the timeline instead.
              // THE EMPTY LOADING STATE RESETS THE STREAM. A room switch clears
              // `messages`, this gate unmounts the old scrollable and its offset,
              // and the arriving root window mounts at the tail.
              if connected && !empty(messages)
                stack w=fill h=fill
                  sensor show=chat_resized resize=chat_resized
                    space w=fill h=fill
                  mouse press-at=chat_pointer_pressed
                    // A CONVERSATION GROWS UP FROM THE COMPOSER. `anchor-y=end`
                    // pins the scroll OFFSET, which does nothing until there is
                    // something to scroll — a channel with four messages in it
                    // left them stranded at the top of an 800px column with
                    // 350px of dead background between the last one and the
                    // composer. `h=shrink` lets the scrollable take only the
                    // height its content needs (still capped by the box's
                    // limits, so a long timeline scrolls exactly as before) and
                    // `align-y=end` drops that block onto the composer.
                    box
                      with
                        w=fill
                        h=fill
                        align-y=end
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
                                  disabled=(mutation_phase != MutationPhase.idle)
                                  h=30.0
                                  p=6.0
                                  @secondary_action
                                active bg=fg/6 text=muted border=fg/10 border-w=1.0 r=8.0
                                hovered bg=fg/10 text=fg border=fg/14
                                pressed bg=fg/14 text=fg
                          // VIRTUALIZED, AND KEYED BY STABLE VIEW IDENTITY. Only the rows the
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
                          // page. Every row gets a numeric client identity which
                          // same-ID merges carry forward, so per-row state and
                          // measurement follow the message through both prepend
                          // and confirmation instead of following a slot.
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
                          // WHOLE-TIMELINE MEMO. The value stays borrowed from
                          // root state and only enters the cached element when
                          // this cheap revision (or one of the four visible
                          // screen inputs) moves. Composer edits, clocks and
                          // unrelated live planes therefore build one memo
                          // widget instead of enumerating every message.
                          //
                          // `loading` STAYS OUT OF THE KEY. It is the
                          // workspace hydration flag — a page load moves it
                          // while a full chat timeline is on screen — and
                          // the only thing in here that read it was the
                          // live row's `disabled=`. A room switch and a
                          // reconnect both empty the stream before they
                          // raise the flag, so no row ever drew under the
                          // chat's own load; keying on it cloned every
                          // message for a dim that never showed. No row
                          // reads the flag now, so the live row routes like
                          // the quiet rows always did: the reaction handlers
                          // keep refusing while loading; the openers never
                          // did.
                          lazy messages by active_channel, unread_boundary, unread_marker_seq, selected_message_seq as cached_messages
                            MessageTimeline
                              with
                                messages=cached_messages
                                unread_boundary
                                unread_marker_seq
                                selected_message_seq
                              forward
                                add_reaction_at
                                remove_reaction_at
                                open_thread_for
                                open_message_reactions
                                open_message_actions
                                open_message_link
                  overlay
                    with
                      when=(selected_message_seq > 0 && message_action != MessageAction.toolbar)
                      dismiss=emit(clear_message_selection)
                      backdrop=transparent
                      p=8.0
                      align-x=end
                      align-y=start
                    content
                      space w=fill h=fill
                    layer
                      // THE LAYER IS THE MENU, NOT THE PANE. Codegen wraps an
                      // overlay's layer in a press swallower (a press on a menu
                      // row's padding must not fall through to the backdrop), so
                      // a fill-sized layer carrying the pointer-y offset as
                      // padding covered the backdrop end to end: every press on
                      // the pane died in the swallower and Esc was the menu's
                      // only exit. The offset is a pressable gap instead, routed
                      // to the same dismiss the backdrop carries.
                        col
                          mouse press=emit(clear_message_selection)
                            space w=fill h=block_action_menu_y(chat_pointer_y, chat_height)
                          if message_action == MessageAction.more
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
                                  // LIVE, SO THE PRESS REACHES THE REFUSAL. A
                                  // disabled row is pixel-identical to a live
                                  // one here (`@ghost_action` idles at the same
                                  // ink), so it read as a working row that ate
                                  // the click; now the press lands in
                                  // `open_message_reactions`, which answers with
                                  // the archived banner.
                                  button -> emit(open_message_reactions, selected_message_seq, message_edit_draft, selected_message_rev)
                                    with
                                      label="Manage reactions"
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
                                  // COPY LINK. A message has no address a
                                  // member could type: the channel id is a
                                  // uuid and the seq is nowhere on screen, so
                                  // this row is the only way one leaves the
                                  // app. The built link names its network, so
                                  // pasted into a workspace on another chain
                                  // it refuses instead of opening that
                                  // chain's message 42.
                                  button -> emit(copy_message_link, duck_channel_message_link(active_channel, selected_message_seq, network_chain_id))
                                    with
                                      label="Copy message link"
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
                                            name="link"
                                            tone="muted"
                                            px=14.0
                                        text "Copy link"
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
                          if message_action == MessageAction.reactions
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
                          if message_action == MessageAction.editing
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
                                    disabled=(mutation_phase != MutationPhase.idle)
                                    submit=emit(edit_message_submit)
                                    w=fill
                                    p=6.2
                                    text-size=13.0
                                    line-h=1.2
                                    @control
                                  active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                                  hovered bg=fg/4 border=fg/8
                                  // THE EDITOR IS BORDERLESS AT REST ON PURPOSE, and the
                                  // recipe's ring alone on a transparent field is a thin
                                  // outline around nothing. `begin_message_edit` drops the
                                  // caret in here by hand, so focus also lifts the plate —
                                  // without the `bg=` the field's HOVER read stronger than
                                  // its focus.
                                  focused bg=fg/4 border=ring
                                  disabled value=muted
                                button "Save" -> emit(edit_message_submit)
                                  with
                                    label="Save message changes"
                                    disabled=(mutation_phase != MutationPhase.idle || empty(trim(message_edit_draft)))
                                    h=28.0
                                    p=6.0
                                    @primary_action
                                button -> emit(clear_message_selection)
                                  with
                                    label="Cancel message edit"
                                    disabled=(mutation_phase != MutationPhase.idle)
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
                          if message_action == MessageAction.delete
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
                                      disabled=(mutation_phase != MutationPhase.idle)
                                      h=26.0
                                      p=5.0
                                      @danger_action
                                  button "Cancel" -> emit(clear_message_selection)
                                    with
                                      disabled=(mutation_phase != MutationPhase.idle)
                                      h=26.0
                                      p=5.0
                                      @secondary_action
                                    active bg=transparent text=muted r=6.0
                                    hovered bg=fg/10 text=fg
                                    pressed bg=fg/15
            // JUMP TO LATEST — A FLOAT AT THE TIMELINE'S BOTTOM EDGE, where
            // every chat app puts it, and where the reader's eyes already are.
            // It used to be a button inside an amber "Viewing history" band at
            // the TOP of the column: the one control that means "take me back to
            // now" sat as far from now as the pane allows, and only ever
            // appeared for a search-hit window — a reader who had simply
            // scrolled up had no way back but the wheel.
            //
            // TWO WAYS TO BE AWAY FROM NOW, ONE PILL. `history_view` is a window
            // around an old message, which is not the tail whatever the scroll
            // offset says; `at_live_tail` is the offset itself, published by the
            // stream's own `chat_scrolled` (`near_scroll_tail`). At the tail with
            // no history window, the pill is not mounted at all.
            if !empty(messages) && (history_view || !at_live_tail)
              box
                with
                  w=fill
                  h=fill
                  pl=18.0
                  pr=18.0
                  pb=10.0
                  align-x=center
                  align-y=end
                button "↓  Jump to latest" -> emit(choose_channel, active_channel)
                  with
                    h=28.0
                    p=10.0
                    @ghost_action
                  active bg=surface text=muted border=border border-w=1.0 r=14.0
                  hovered bg=fg/6 text=fg border=fg/14
                  pressed bg=accent text=fg
            // THE RESULTS FLOAT. This card used to be the column's first
            // child, so a search reflowed the whole conversation down by
            // 148px; as a stack layer it drops over the stream instead and
            // everything beneath keeps its place.
            //
            // THE GATE CARRIES THE RETIREMENT, so the three arms below stay
            // the phase reads they already were and no edit to one of them can
            // leave an empty card floating. `done` alone stood while the
            // reader typed on: this field is enter-to-submit and two-way
            // bound, so a keystroke runs no handler and only
            // `trim(draft) == query` can retire an answer as the box moves
            // away from it.
            //
            // ONLY THE ZERO-HIT ARM RETIRES THAT WAY. Hit rows are still
            // useful under a draft nobody has sent — they are the thing being
            // typed toward — so `!empty(search_hits)` holds the float up on
            // its own until a new query is sent or the box is cleared, which
            // is what the pages hits float does. A sentence claiming NOTHING
            // matched has no such life: it is a claim about one string.
            if search_phase == SearchPhase.searching || !empty(search_hits) || search_answer_stands(search_query, search_draft, search_phase == SearchPhase.searching)
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
                    if search_phase == SearchPhase.searching
                      col
                        with
                          w=fill
                          gap=14.0
                          p=8.0
                        SkeletonRow
                    if search_phase == SearchPhase.done && empty(search_hits)
                      box
                        with
                          w=fill
                          p=14.0
                          align-x=center
                        text "No messages match" size=12.5 @text-muted
                    if search_phase == SearchPhase.done && !empty(search_hits)
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
              ComposerGate reason=post_refusal
          box
            with
              w=fill
              pl=18.0
              pr=18.0
              pt=12.0
              pb=14.0
            // ducktape-ui#697: the room's own composer instance. The key is
            // the room, so the draft IS the room's draft; the plate, marks
            // row, Send and the failed-send banner all moved inside.
            ChatComposer #composer(composer_scope(endpoint, active_channel))
              with
                kind=ComposerKind.message
                compact=false
                hint="Message the channel…"
                blocked=(loading || !connected || empty(active_channel) || !empty(post_refusal))
                restore_blocked=(mutation_phase != MutationPhase.idle)
                failed_note="An earlier message wasn’t sent"
              events
                submitted -> emit(composer_submitted, _, _, _)
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
                          disabled=(mutation_phase != MutationPhase.idle)
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
                          disabled=(mutation_phase != MutationPhase.idle || empty(trim(channel_name_draft)))
                          h=29.0
                          p=6.0
                          @secondary_action
                  // A channel id is a uuid: this is the only place a member
                  // can get one out of the app. The link names its network.
                  button "Copy channel link" -> emit(copy_to_clipboard, duck_channel_link(active_channel, network_chain_id), "Channel link copied")
                    with
                      label="Copy channel link"
                      w=fill
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
                          disabled=(mutation_phase != MutationPhase.idle)
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
                          disabled=(mutation_phase != MutationPhase.idle || empty(trim(member_key_draft)))
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
                          ChatMemberRow
                            with
                              member=member
                              disabled=(mutation_phase != MutationPhase.idle)
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
                        disabled=(mutation_phase != MutationPhase.idle)
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
                        disabled=(mutation_phase != MutationPhase.idle)
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
              sensor show=thread_resized resize=thread_resized
                space w=fill h=fill
              mouse press-at=thread_pointer_pressed
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
                          disabled=(mutation_phase != MutationPhase.idle)
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
                      // The same whole-list boundary as the channel stream.
                      // Reply-composer edits and unrelated app state stop at
                      // the cheap revision instead of walking every loaded
                      // reply. Paging chrome stays outside the memo, so its
                      // busy phase does not invalidate the timeline, and the
                      // workspace `loading` flag stays out of the key for the
                      // same reason the stream's does.
                      lazy thread_messages by active_channel, active_thread_seq, thread_target_seq, thread_selected_seq as cached_thread_messages
                        ThreadTimeline
                          with
                            messages=cached_thread_messages
                            active_thread_seq
                            thread_target_seq
                            thread_selected_seq
                          forward
                            add_reaction_at
                            remove_reaction_at
                            open_thread_for
                            open_thread_message_actions
                            open_thread_message_reactions
                            open_message_link
                      if thread_has_more && thread_next_reply_seq > 0 && thread_loading
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
                      if thread_has_more && thread_next_reply_seq > 0 && !thread_loading
                        button "Load more replies" -> emit(load_more_thread)
                          with
                            disabled=(mutation_phase != MutationPhase.idle)
                            w=fill
                            h=28.0
                            p=5.0
                            @secondary_action
                          active bg=transparent text=muted r=7.0
                          hovered bg=fg/9 text=fg
                          pressed bg=brand_bg
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
                  // discriminant, not two. The stream's `loading` drops out for a
                  // different reason: it is the STREAM's readiness, the rail's is
                  // `thread_loading`, and `reply_composer_event` no longer refuses
                  // on it either — a term in the guard that the button does not
                  // wear is a dead control. The reason SENTENCE stays mounted once,
                  // over the stream's plate: 330px has no room to say it twice.
                  box
                    with
                      w=fill
                      pl=16.0
                      pr=16.0
                      pt=10.0
                      pb=14.0
                    // The rail's composer instance, keyed by the THREAD: a
                    // reply draft belongs to the thread it replies to, and
                    // closing the rail no longer discards it — the instance
                    // (and its words) are simply waiting for the rail to
                    // reopen on the same thread.
                    ChatComposer #reply_composer(thread_scope(endpoint, active_channel, active_thread_seq))
                      with
                        kind=ComposerKind.reply
                        compact=true
                        hint="Reply…"
                        blocked=(thread_loading || !connected || !empty(post_refusal))
                        restore_blocked=false
                        failed_note="Unsent reply"
                      events
                        submitted -> emit(composer_submitted, _, _, _)
              overlay
                with
                  when=(thread_selected_seq > 0 && thread_message_action != MessageAction.toolbar)
                  dismiss=emit(clear_thread_message_selection)
                  backdrop=transparent
                  p=8.0
                  align-x=end
                  align-y=start
                content
                  space w=fill h=fill
                layer
                  // Same shape as the stream menu: the layer is the menu, the
                  // gap above it dismisses.
                    col
                      mouse press=emit(clear_thread_message_selection)
                        space w=fill h=block_action_menu_y(thread_pointer_y, thread_height)
                      if thread_message_action == MessageAction.more
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
                              // Live for the same reason as the stream's twin.
                              button -> emit(open_thread_message_reactions, thread_selected_seq, thread_edit_draft, thread_selected_rev)
                                with
                                  label="Manage reactions"
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
                              // A reply is addressable exactly like a stream
                              // message — same channel, its own seq — and the
                              // rail is where a reader stands when they want
                              // to hand one to somebody.
                              button -> emit(copy_message_link, duck_channel_message_link(active_channel, thread_selected_seq, network_chain_id))
                                with
                                  label="Copy message link"
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
                                        name="link"
                                        tone="muted"
                                        px=14.0
                                    text "Copy link"
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
                      if thread_message_action == MessageAction.reactions
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
                      if thread_message_action == MessageAction.editing
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
                                disabled=(mutation_phase != MutationPhase.idle)
                                submit=emit(edit_thread_message_submit)
                                w=fill
                                p=6.2
                                text-size=13.0
                                line-h=1.2
                                @control
                              active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                              hovered bg=fg/4 border=fg/8
                              // Same borderless-at-rest editor as the main stream's, so
                              // the same focused plate — see `#message-edit` above.
                              focused bg=fg/4 border=ring
                              disabled value=muted
                            button "Save" -> emit(edit_thread_message_submit)
                              with
                                label="Save message changes"
                                disabled=(mutation_phase != MutationPhase.idle || empty(trim(thread_edit_draft)))
                                h=28.0
                                p=6.0
                                @primary_action
                            button -> emit(clear_thread_message_selection)
                              with
                                label="Cancel message edit"
                                disabled=(mutation_phase != MutationPhase.idle)
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
                      if thread_message_action == MessageAction.delete
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
                                  disabled=(mutation_phase != MutationPhase.idle)
                                  h=26.0
                                  p=5.0
                                  @danger_action
                              button "Cancel" -> emit(clear_thread_message_selection)
                                with
                                  disabled=(mutation_phase != MutationPhase.idle)
                                  h=26.0
                                  p=5.0
                                  @secondary_action
                                active bg=transparent text=muted r=6.0
                                hovered bg=fg/10 text=fg
                                pressed bg=fg/15
