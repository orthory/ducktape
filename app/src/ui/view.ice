component StatusBadge(label:str)
  row align=center
    match label
      "active"
        Badge.Success label=label
      "paused"
        Badge.Warning label=label
      "open"
        Badge.Success label=label
      "closed"
        Badge.Destructive label=label
      "merged"
        Badge.Success label=label
      "passed"
        Badge.Success label=label
      "rejected"
        Badge.Destructive label=label
      "applied"
        Badge.Success label=label
      "discarded"
        Badge.Warning label=label
      _
        Badge.Outline label=label

// THE EXPLORER'S WORKSPACE SEARCH — state and route.
//
// These four handlers and the two fields belong in state.ice and
// handlers/lifecycle.ice with every other explorer route; they sit here only
// because this wave owns view.ice alone and the search shipped with its whole
// stack built and no way in: `search_workspace` is implemented (chat, pages,
// the forge trackers, duckfs paths, agent runs, with kind counts), declared in
// backend.ice, and called by nothing, while ExplorerCard had no call site.
// Move them the next time lifecycle.ice is open.
state
  explorer_hits:[ExplorerHit] = []
  explorer_searching = false
  explorer_search_generation:i64 = 0

on explorer_search_submit
  return if !connected || explorer_searching || empty(trim(explorer_query))
  explorer_search_generation = explorer_search_generation + 1
  explorer_searching = true
  explorer_hits = []
  error = ""
  run search_workspace(connected_rpc, trim(explorer_query), explorer_search_generation) -> explorer_results_loaded _ | explorer_search_failed _

on explorer_results_loaded(next)
  return if next.generation != explorer_search_generation
  explorer_hits = next.hits
  explorer_searching = false
  error = ""

on explorer_search_failed(cause)
  return if cause.generation != explorer_search_generation
  explorer_searching = false
  error = cause.message

on clear_explorer_search
  explorer_search_generation = explorer_search_generation + 1
  explorer_query = ""
  explorer_hits = []
  explorer_searching = false

view
  // THE PHASE GATE — the one branch in front of the console. `phase` is the
  // single discriminant: "console" mounts the shell, every other value mounts
  // the pre-workspace column, and a device with no workspace on disk starts
  // there. Without this branch `Leave workspace` strands the user on a
  // stripped titlebar over a dead console, and a fresh device boots a shell it
  // cannot connect.
  col w=fill h=fill
    // OMITTED, not faked: `peers_live`/`peers_total` have no state field — the
    // gossip reading rides NodeFacts but nothing stores it — so the frozen
    // props are fed 0 and LiveStatusStrip drops the segment rather than
    // printing a count nobody measured.
    if phase != "console"
      OnboardingPhase phase=phase name=onboarding_name node_api=canonical_endpoint(rpc) invite=invite_link steps=provision_steps step_index=provision_index height=block_height peers_live=0 peers_total=0 tier=member_tier(members_rows) error=onboarding_error busy=(mutation_phase != "idle")
    if phase == "console"
      WorkspaceTabs network=network_label(account_name, connected_rpc) status=status height=block_height loading=(loading || mutation_phase != "idle") degraded=connection_degraded(status) tab=shell_tab bell_count=bell_unread approvals=open_proposals(gov_rows) account=account_name agent_live=any_agent_active(agents_rows) phase=phase tier=member_tier(members_rows) root_hash=node_root_hash consensus_view=node_view quorum=node_quorum reachable=node_reachable last_finalized=node_last_finalized checkpoint=node_checkpoint #workspace-tabs
        notice:
          col w=fill
            if error != ""
              box w=fill pl=12.0 pr=12.0 pb=8.0
                box w=fill p=8.0 bg=danger_bg border=danger_line border-w=1.0 r=12.0
                  row w=fill gap=8.0 align=center
                    box w=20.0 h=20.0 align-x=center align-y=center bg=danger_dot r=10.0
                      text "!" size=14.0 font=medium @text-danger_fg
                    text error w=fill size=13.5 @text-fg
                    button "Dismiss" h=26.0 p=5.0 @ghost_action -> dismiss_error
                      active bg=transparent text=muted r=7.0
                      hovered bg=fg/9 text=fg
                      pressed bg=fg/14
        chat:
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
                    input "" #chat-search label="Search messages" <-> chat_search_draft hint="Search…" disabled=(!connected || chat_searching) submit=search_chat_submit w=fill p=6.2 text-size=13.0 line-h=1.2 @control
                      active bg=surface border=border value=fg placeholder=hint selection=fg/18 border-w=1.0 r=8.0
                      hovered bg=muted_bg border=control_line
                      disabled bg=transparent value=muted
                    if !empty(chat_search_hits)
                      button label="Clear message search" w=27.0 h=27.0 p=0.0 @icon_action -> clear_chat_search
                        box w=fill h=fill align-x=center align-y=center
                          text "×" size=13.0 wrap=none @text-muted
                        active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
                        hovered bg=elevated text=fg
                        pressed bg=subtle text=fg
                box w=fill pl=16.0 pr=16.0 pt=10.0 pb=5.0
                  row w=fill gap=6.0 align=center
                    text "CHANNELS" size=10.0 wrap=none font=code_semibold @text-label
                    space w=fill
                    text len(channels) size=10.5 wrap=none font=code_medium @text-label
                    if !channel_create_open
                      button label="New channel" disabled=(loading || mutation_phase != "idle" || !connected) p=0.0 @icon_action -> toggle_channel_create
                        Icon name="plus" tone="label" px=16.0
                        active bg=transparent text=muted border=transparent border-w=1.0 r=5.0
                        hovered bg=separator text=fg
                        pressed bg=subtle text=fg
                    if channel_create_open
                      button label="Close new channel" disabled=(loading || mutation_phase != "idle") w=18.0 h=18.0 p=0.0 @icon_action -> toggle_channel_create
                        box w=fill h=fill align-x=center align-y=center
                          text "×" size=13.0 wrap=none @text-muted
                        active bg=separator text=muted border=transparent border-w=1.0 r=5.0
                        hovered bg=subtle text=fg
                        pressed bg=subtle text=fg
                scroll dir=vertical w=fill h=fill bar=hidden
                  col w=fill gap=2.0
                    for channel in channels
                      ChannelButton channel=channel selected=(channel.id == active_channel) unread=channel_is_unread(channel_reads, channel.id, channel.head_seq)
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
                          text "#" size=14.0 wrap=none font=medium @text-hint
                          text active_channel_name size=14.0 wrap=none font=display @text-fg
                          if active_channel_archived
                            Badge.Outline label="Archived"
                          if active_channel_members_only
                            Badge.Outline label="Members only"
                          // The huddle control, in its three mutually exclusive
                          // states — in it here, in it elsewhere, in none.
                          if huddle_joined && huddle_channel == active_channel
                            HuddleLivePill name=active_channel_name elapsed=mmss(huddle_now - huddle_joined_at)
                          if huddle_joined && huddle_channel != active_channel
                            HuddleElsewhere name=huddle_channel_name
                          if !huddle_joined && !active_channel_archived
                            HuddleStart
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
                          button label="Channel details" w=27.0 h=25.0 p=0.0 @icon_action -> toggle_channel_settings
                            box w=fill h=fill align-x=center align-y=center
                              text "⋯" size=14.0 wrap=none @text-muted
                            active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
                            hovered bg=elevated text=fg
                            pressed bg=subtle text=fg
                      box w=fill h=1.0 bg=separator
                        space w=1.0 h=1.0
                  col w=fill h=fill gap=9.0 pl=18.0 pr=18.0 pt=16.0 pb=8.0
                    if !empty(chat_search_hits)
                      box w=fill h=148.0 p=6.0 bg=elevated border=fg/10 border-w=1.0 r=10.0
                        scroll dir=vertical w=fill h=fill
                          col w=fill gap=1.0
                            for hit in chat_search_hits
                              ChatSearchResult hit=hit
                    if !connected
                      EmptyState title="Connect to a node" description="Set the RPC endpoint in the sidebar."
                    if connected && empty(messages)
                      EmptyState title="No messages yet" description="Create a channel or start the conversation."
                    if connected && !empty(messages) && history_view
                      box w=fill h=32.0 pl=10.0 pr=6.0 bg=warning_bg border=warning_line border-w=1.0 r=9.0
                        row w=fill h=fill gap=8.0 align=center
                          text "Viewing history" w=fill size=12.5 wrap=none @text-warning
                          button "Jump to latest" h=24.0 p=5.0 @ghost_action -> choose_channel(active_channel)
                            active bg=surface text=fg border=warning_line border-w=1.0 r=7.0
                            hovered bg=warning_bg text=fg
                            pressed bg=accent text=fg
                    if connected && !empty(messages)
                      stack w=fill h=fill
                        mouse move=chat_pointer_moved
                          sensor show=chat_resized resize=chat_resized
                            scroll dir=vertical w=fill h=fill
                              col w=fill gap=3.0
                                if history_has_older(messages)
                                  box w=fill align-x=center pt=4.0 pb=8.0
                                    button "Load older messages" disabled=(history_loading || mutation_phase != "idle") h=30.0 p=6.0 @secondary_action -> load_more_history
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
                                      MessageCard message=message selected=(message.seq == selected_message_seq) hovered=(message.seq == hovered_message_seq) disabled=loading
                        overlay when=(selected_message_seq > 0 && message_action != "toolbar") dismiss=clear_message_selection backdrop=transparent p=8.0 align-x=end align-y=start
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
                                        button "React" label="Manage reactions" disabled=active_channel_archived w=fill h=28.0 p=6.0 @ghost_action -> open_message_reactions(selected_message_seq, message_edit_draft, selected_message_rev)
                                          active bg=transparent text=muted r=6.0
                                          hovered bg=fg/10 text=fg
                                          pressed bg=fg/15
                                        button "Open thread" w=fill h=28.0 p=6.0 @ghost_action -> open_thread_for(selected_message_seq)
                                          active bg=transparent text=muted r=6.0
                                          hovered bg=fg/10 text=fg
                                          pressed bg=fg/15
                                        button "Edit" w=fill h=28.0 p=6.0 @ghost_action -> begin_message_edit(selected_message_seq, message_edit_draft, selected_message_rev)
                                          active bg=transparent text=muted r=6.0
                                          hovered bg=fg/10 text=fg
                                          pressed bg=fg/15
                                        button "Delete" w=fill h=28.0 p=6.0 @danger_action -> arm_message_delete(selected_message_seq, message_edit_draft, selected_message_rev)
                                        button "Close" label="Close message actions" w=fill h=28.0 p=6.0 @secondary_action -> clear_message_selection
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
                                        button "+ 👍" label="Add thumbs up reaction" disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> add_reaction_submit("👍")
                                          active bg=transparent text=fg r=6.0
                                          hovered bg=fg/10
                                          pressed bg=fg/15
                                        button "+ ♥" label="Add heart reaction" disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> add_reaction_submit("❤️")
                                          active bg=transparent text=fg r=6.0
                                          hovered bg=fg/10
                                          pressed bg=fg/15
                                        button "+ 😄" label="Add smile reaction" disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> add_reaction_submit("😄")
                                          active bg=transparent text=fg r=6.0
                                          hovered bg=fg/10
                                          pressed bg=fg/15
                                        button "+ 🎉" label="Add celebration reaction" disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> add_reaction_submit("🎉")
                                          active bg=transparent text=fg r=6.0
                                          hovered bg=fg/10
                                          pressed bg=fg/15
                                        button "+ 👀" label="Add eyes reaction" disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> add_reaction_submit("👀")
                                          active bg=transparent text=fg r=6.0
                                          hovered bg=fg/10
                                          pressed bg=fg/15
                                        button "+ 🙌" label="Add raised hands reaction" disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> add_reaction_submit("🙌")
                                          active bg=transparent text=fg r=6.0
                                          hovered bg=fg/10
                                          pressed bg=fg/15
                                        for message in messages
                                          if message.seq == selected_message_seq
                                            for reaction in message.reactions
                                              if reaction.reacted_by_me
                                                button label="Remove my reaction" description=reaction.emoji disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> remove_reaction_submit(reaction.emoji)
                                                  text reaction.emoji size=13.0 @text-fg
                                                  active bg=fg/7 text=fg r=6.0
                                                  hovered bg=fg/12
                                                  pressed bg=fg/17
                                        button "×" label="Close reactions" disabled=(mutation_phase != "idle") w=26.0 h=26.0 p=4.0 @secondary_action -> clear_message_selection
                                          active bg=transparent text=muted r=6.0
                                          hovered bg=fg/10 text=fg
                                          pressed bg=fg/15
                                if message_action == "editing"
                                  box w=fill max-w=520.0 p=3.0 style=raised_style()
                                    row w=fill gap=4.0 align=center
                                      input "" #message-edit label="Edit message" <-> message_edit_draft hint="Edit message" disabled=(mutation_phase != "idle") submit=edit_message_submit w=fill p=6.2 text-size=13.0 line-h=1.2 @control
                                        active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                                        hovered bg=fg/4 border=fg/8
                                        disabled value=muted
                                      button "Save" label="Save message changes" disabled=(mutation_phase != "idle" || empty(trim(message_edit_draft))) h=28.0 p=6.0 @primary_action -> edit_message_submit
                                      button label="Cancel message edit" disabled=(mutation_phase != "idle") w=28.0 h=28.0 p=0.0 @icon_action -> clear_message_selection
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
                                        button "Delete" disabled=(mutation_phase != "idle") h=26.0 p=5.0 @danger_action -> delete_message_submit
                                        button "Cancel" disabled=(mutation_phase != "idle") h=26.0 p=5.0 @secondary_action -> clear_message_selection
                                          active bg=transparent text=muted r=6.0
                                          hovered bg=fg/10 text=fg
                                          pressed bg=fg/15
                    if !empty(failed_message_draft)
                      row w=fill gap=6.0 align=center
                        text "An earlier message wasn’t sent" w=fill size=12.5 @text-muted
                        button "Restore" disabled=(!empty(trim(editor_text(message_editor))) || mutation_phase != "idle") h=28.0 p=5.0 @secondary_action -> restore_failed_message
                          active bg=fg/9 text=fg border=fg/11 border-w=1.0 r=7.0
                          hovered bg=fg/14
                          pressed bg=fg/18
                        button label="Dismiss unsent message" w=28.0 h=28.0 p=0.0 @icon_action -> dismiss_failed_message
                          box w=fill h=fill align-x=center align-y=center
                            text "×" size=14.0
                          active bg=transparent text=muted r=7.0
                          hovered bg=fg/10 text=fg
                          pressed bg=fg/15
                  // The composer is separated from the stream by a hairline and
                  // carries the artifact's own 12/16/14 region padding.
                  box w=fill h=1.0 bg=separator
                    space w=1.0 h=1.0
                  box w=fill pl=16.0 pr=16.0 pt=12.0 pb=14.0
                    box w=fill bg=surface border=control_line border-w=1.0 r=12.0 clip=true shadow=shadow_popover shadow-y=1.0 shadow-blur=2.0
                      col w=fill
                        editor #message <-> message_editor hint="Message the channel…" disabled=(loading || !connected || empty(active_channel) || active_channel_archived) min-h=44.0 max-h=150.0 size=13.5 line-h=1.3 p=6.6 wrap=word key-binding=composer_keys() -> send_message_submit
                          active bg=transparent border=transparent value=fg placeholder=hint selection=fg/18 border-w=0.0 r=0.0
                          hovered bg=transparent border=transparent
                          focused bg=transparent border=ring border-w=1.0
                          disabled value=muted
                        box w=fill pl=10.0 pr=8.0 pb=8.0
                          row w=fill gap=10.0 align=center
                            space w=fill
                            text "↵ send · ⇧↵ newline" size=10.5 wrap=none font=code_medium @text-label
                            button "Send" disabled=(loading || !connected || empty(active_channel) || active_channel_archived || empty(trim(editor_text(message_editor)))) h=29.0 p=7.0 @primary_action -> send_message_submit
                if channel_settings_open && !empty(active_channel)
                  box w=1.0 h=fill bg=fg/8
                    text ""
                  box w=300.0 h=fill p=12.0 bg=muted_bg
                    col w=fill h=fill gap=8.0
                      row w=fill h=28.0 gap=6.0 align=center
                        text "Channel details" w=fill size=14.0 font=display @text-fg
                        button label="Close channel details" w=28.0 h=28.0 p=0.0 @icon_action -> toggle_channel_settings
                          box w=fill h=fill align-x=center align-y=center
                            text "×" size=14.0
                          active bg=transparent text=muted r=7.0
                          hovered bg=fg/10 text=fg
                          pressed bg=fg/15
                      Separator
                      row w=fill gap=5.0 align=center
                        input "" #channel-name label="Channel name" <-> channel_name_draft hint="Channel name" disabled=(mutation_phase != "idle") submit=rename_channel_submit w=fill p=6.2 text-size=13.0 line-h=1.2 @control
                          active bg=transparent border=fg/10 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                          hovered bg=fg/4 border=fg/14
                          disabled value=muted
                        button "Rename" disabled=(mutation_phase != "idle" || empty(trim(channel_name_draft))) w=56.0 h=28.0 p=5.0 @secondary_action -> rename_channel_submit
                      row w=fill gap=5.0 align=center
                        if !active_channel_archived
                          button "Archive" disabled=(mutation_phase != "idle") h=28.0 p=5.0 @danger_action -> archive_channel_submit
                        if active_channel_archived
                          button "Unarchive" disabled=(mutation_phase != "idle") h=28.0 p=5.0 @secondary_action -> unarchive_channel_submit
                            active bg=transparent text=muted r=7.0
                            hovered bg=fg/10 text=fg
                            pressed bg=fg/15
                        space w=fill
                        text len(channel_members) size=12.0 font=code @text-muted
                      row w=fill gap=5.0 align=center
                        input "" #member-key label="Member public key" <-> member_key_draft hint="64-character member key" disabled=(mutation_phase != "idle") submit=add_channel_member_submit w=fill p=7.4 text-size=12.0 line-h=1.2 font=code @control
                          active bg=transparent border=fg/10 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                          hovered bg=fg/4 border=fg/14
                          disabled value=muted
                        button "Add" disabled=(mutation_phase != "idle" || empty(trim(member_key_draft))) w=40.0 h=28.0 p=5.0 @secondary_action -> add_channel_member_submit
                      if !empty(channel_members)
                        scroll dir=vertical w=fill h=fill
                          col w=fill gap=2.0
                            for member in channel_members
                              ChatMemberRow member=member disabled=(mutation_phase != "idle")
                if active_thread_seq > 0 && !channel_settings_open
                  box w=1.0 h=fill bg=fg/8
                    text ""
                  box w=300.0 h=fill p=12.0 bg=muted_bg
                    stack w=fill h=fill
                      mouse move=thread_pointer_moved
                        sensor show=thread_resized resize=thread_resized
                          col w=fill h=fill gap=8.0
                            row w=fill h=28.0 gap=6.0 align=center
                              if thread_target_seq <= 0
                                text "Thread" w=fill size=14.0 font=display @text-fg
                              if thread_target_seq > 0
                                text "Thread result" w=fill size=14.0 font=display @text-fg
                              text len(thread_messages) size=12.0 font=code @text-muted
                              button label="Close thread" disabled=(mutation_phase != "idle") w=28.0 h=28.0 p=0.0 @icon_action -> close_thread
                                box w=fill h=fill align-x=center align-y=center
                                  text "×" size=14.0
                                active bg=transparent text=muted r=7.0
                                hovered bg=fg/11 text=fg
                                pressed bg=brand_bg
                            Separator
                            scroll dir=vertical w=fill h=fill
                              col w=fill gap=1.0
                                for thread_message in thread_messages
                                  ThreadMessageCard message=thread_message selected=(thread_message.seq == thread_target_seq) hovered=(thread_message.seq == thread_hovered_seq) disabled=loading
                                if thread_has_more && thread_next_reply_offset >= 0
                                  button "Load more replies" disabled=(thread_loading || mutation_phase != "idle") w=fill h=28.0 p=5.0 @secondary_action -> load_more_thread
                                    active bg=transparent text=muted r=7.0
                                    hovered bg=fg/9 text=fg
                                    pressed bg=brand_bg
                            if !empty(failed_reply_draft)
                              row w=fill gap=6.0 align=center
                                text "Unsent reply" w=fill size=12.5 @text-muted
                                button "Restore" disabled=(!empty(trim(editor_text(reply_editor)))) h=26.0 p=5.0 @secondary_action -> restore_failed_reply
                                  active bg=fg/9 text=fg border=fg/11 border-w=1.0 r=7.0
                                  hovered bg=fg/14
                                  pressed bg=fg/18
                                button "×" label="Dismiss unsent reply" w=26.0 h=26.0 p=4.0 @ghost_action -> dismiss_failed_reply
                                  active bg=transparent text=muted r=7.0
                                  hovered bg=fg/10 text=fg
                                  pressed bg=fg/15
                            box w=fill p=5.0 bg=transparent border=fg/12 border-w=1.0 r=7.0
                              row w=fill gap=5.0 align=end
                                editor #reply <-> reply_editor hint="Reply…" disabled=(thread_loading || active_channel_archived) min-h=44.0 max-h=150.0 size=13.5 line-h=1.3 p=6.6 wrap=word key-binding=composer_keys() -> send_reply_submit
                                  active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=0.0 r=9.0
                                  hovered bg=fg/4 border=fg/8 border-w=1.0
                                  focused bg=fg/6 border=ring border-w=1.0
                                  disabled value=muted
                                button "Send" label="Send reply" disabled=(thread_loading || active_channel_archived || empty(trim(editor_text(reply_editor)))) h=28.0 p=6.0 @primary_action -> send_reply_submit
                      overlay when=(thread_selected_seq > 0 && thread_message_action != "toolbar") dismiss=clear_thread_message_selection backdrop=transparent p=8.0 align-x=end align-y=start
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
                                      button "React" label="Manage reactions" disabled=active_channel_archived w=fill h=28.0 p=6.0 @ghost_action -> open_thread_message_reactions(thread_selected_seq, thread_edit_draft, thread_selected_rev)
                                        active bg=transparent text=muted r=6.0
                                        hovered bg=fg/10 text=fg
                                        pressed bg=fg/15
                                      button "Edit" w=fill h=28.0 p=6.0 @ghost_action -> begin_thread_message_edit(thread_selected_seq, thread_edit_draft, thread_selected_rev)
                                        active bg=transparent text=muted r=6.0
                                        hovered bg=fg/10 text=fg
                                        pressed bg=fg/15
                                      button "Delete" w=fill h=28.0 p=6.0 @danger_action -> arm_thread_message_delete(thread_selected_seq, thread_edit_draft, thread_selected_rev)
                                      button "Close" label="Close message actions" w=fill h=28.0 p=6.0 @secondary_action -> clear_thread_message_selection
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
                                      button "+ 👍" label="Add thumbs up reaction" disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> add_reaction_at(thread_selected_seq, "👍")
                                        active bg=transparent text=fg r=6.0
                                        hovered bg=fg/10
                                        pressed bg=fg/15
                                      button "+ ♥" label="Add heart reaction" disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> add_reaction_at(thread_selected_seq, "❤️")
                                        active bg=transparent text=fg r=6.0
                                        hovered bg=fg/10
                                        pressed bg=fg/15
                                      button "+ 😄" label="Add smile reaction" disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> add_reaction_at(thread_selected_seq, "😄")
                                        active bg=transparent text=fg r=6.0
                                        hovered bg=fg/10
                                        pressed bg=fg/15
                                      button "+ 🎉" label="Add celebration reaction" disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> add_reaction_at(thread_selected_seq, "🎉")
                                        active bg=transparent text=fg r=6.0
                                        hovered bg=fg/10
                                        pressed bg=fg/15
                                      button "+ 👀" label="Add eyes reaction" disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> add_reaction_at(thread_selected_seq, "👀")
                                        active bg=transparent text=fg r=6.0
                                        hovered bg=fg/10
                                        pressed bg=fg/15
                                      button "+ 🙌" label="Add raised hands reaction" disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> add_reaction_at(thread_selected_seq, "🙌")
                                        active bg=transparent text=fg r=6.0
                                        hovered bg=fg/10
                                        pressed bg=fg/15
                                      for thread_message in thread_messages
                                        if thread_message.seq == thread_selected_seq
                                          for reaction in thread_message.reactions
                                            if reaction.reacted_by_me
                                              button label="Remove my reaction" description=reaction.emoji disabled=(mutation_phase != "idle" || active_channel_archived) h=26.0 p=5.0 @ghost_action -> remove_reaction_at(thread_selected_seq, reaction.emoji)
                                                text reaction.emoji size=13.0 @text-fg
                                                active bg=fg/7 text=fg r=6.0
                                                hovered bg=fg/12
                                                pressed bg=fg/17
                                      button "×" label="Close reactions" disabled=(mutation_phase != "idle") w=26.0 h=26.0 p=4.0 @secondary_action -> clear_thread_message_selection
                                        active bg=transparent text=muted r=6.0
                                        hovered bg=fg/10 text=fg
                                        pressed bg=fg/15
                              if thread_message_action == "editing"
                                box w=fill max-w=520.0 p=3.0 style=raised_style()
                                  row w=fill gap=4.0 align=center
                                    input "" #thread-edit label="Edit message" <-> thread_edit_draft hint="Edit message" disabled=(mutation_phase != "idle") submit=edit_thread_message_submit w=fill p=6.2 text-size=13.0 line-h=1.2 @control
                                      active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                                      hovered bg=fg/4 border=fg/8
                                      disabled value=muted
                                    button "Save" label="Save message changes" disabled=(mutation_phase != "idle" || empty(trim(thread_edit_draft))) h=28.0 p=6.0 @primary_action -> edit_thread_message_submit
                                    button label="Cancel message edit" disabled=(mutation_phase != "idle") w=28.0 h=28.0 p=0.0 @icon_action -> clear_thread_message_selection
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
                                      button "Delete" disabled=(mutation_phase != "idle") h=26.0 p=5.0 @danger_action -> delete_thread_message_submit
                                      button "Cancel" disabled=(mutation_phase != "idle") h=26.0 p=5.0 @secondary_action -> clear_thread_message_selection
                                        active bg=transparent text=muted r=6.0
                                        hovered bg=fg/10 text=fg
                                        pressed bg=fg/15
        pages:
          row w=fill h=fill
            box w=230.0 h=fill bg=sidebar clip=true
              col w=fill h=fill gap=0.0
                box w=fill pl=14.0 pr=14.0 pt=14.0 pb=11.0
                  row w=fill gap=8.0 align=center
                    text "Pages" size=13.5 wrap=none font=display @text-fg
                    text len(pages) size=10.5 wrap=none font=code_medium @text-hint
                    space w=fill
                    if !page_create_open
                      button label="New page" disabled=(loading || mutation_phase != "idle" || !connected) p=0.0 @icon_action -> toggle_page_create
                        Icon name="plus" tone="label" px=16.0
                        active bg=transparent text=muted border=transparent border-w=1.0 r=5.0
                        hovered bg=separator text=fg
                        pressed bg=subtle text=fg
                    if page_create_open
                      button label="Close new page" disabled=(loading || mutation_phase != "idle") w=18.0 h=18.0 p=0.0 @icon_action -> toggle_page_create
                        box w=fill h=fill align-x=center align-y=center
                          text "×" size=13.0 wrap=none @text-muted
                        active bg=separator text=muted border=transparent border-w=1.0 r=5.0
                        hovered bg=subtle text=fg
                        pressed bg=subtle text=fg
                box w=fill h=1.0 bg=separator
                  space w=1.0 h=1.0
                if page_create_open
                  row w=fill h=28.0 gap=5.0 align=center
                    input "" #new-page label="New page title" <-> page_draft hint="New page" disabled=(loading || mutation_phase != "idle" || !connected) submit=create_page_submit w=fill p=6.2 text-size=13.0 line-h=1.2 @control
                      active bg=muted_bg border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=8.0
                      hovered bg=elevated border=fg/21
                      disabled bg=muted_bg/54 value=muted
                    button label="Create page" disabled=(loading || mutation_phase != "idle" || !connected || empty(trim(page_draft))) w=28.0 h=28.0 p=0.0 @icon_action -> create_page_submit
                      box w=fill h=fill align-x=center align-y=center
                        text "+" size=14.0
                scroll dir=vertical w=fill h=fill
                  col w=fill gap=2.0
                    for page in pages
                      PageButton page=page selected=(page.id == active_page)
            box w=1.0 h=fill bg=separator
              space w=1.0 h=1.0
            mouse move=pages_pointer_moved
              row w=fill h=fill
                col w=fill h=fill
                  // The 50px document header bar: the page title and the one
                  // always-on trust signal the surface carries.
                  if connected && !empty(active_page)
                    col w=fill
                      box w=fill h=50.0 pl=22.0 pr=22.0
                        row w=fill h=fill gap=9.0 align=center
                          text active_page_title w=fill size=13.5 wrap=none font=display @text-fg
                          // THE TICK IS EARNED, NEVER ASSUMED. One discriminant,
                          // one match, and `✓ synced` is painted for "saved"
                          // alone: a write the node REFUSED says so, and an edit
                          // still sitting in the draft ("idle") carries no mark
                          // at all. The old predicate read "nothing in flight",
                          // which is true of both of those. The `offline` pill
                          // goes with it — this bar only draws inside
                          // `if connected && !empty(active_page)`, so it never
                          // could paint.
                          match block_autosave_status
                            "saving"
                              box px=9.0 py=4.0 bg=warning_bg border=warning_line border-w=1.0 r=7.0
                                text "saving…" size=10.5 wrap=none font=code_medium @text-warning
                            "error"
                              box px=9.0 py=4.0 bg=danger_bg border=danger_line border-w=1.0 r=7.0
                                text "not saved" size=10.5 wrap=none font=code_medium @text-danger
                            "saved"
                              box px=9.0 py=4.0 bg=final_bg border=final_line border-w=1.0 r=7.0
                                text "✓ synced" size=10.5 wrap=none font=code_medium @text-success_tick
                            _
                              space w=1.0 h=1.0
                      box w=fill h=1.0 bg=separator
                        space w=1.0 h=1.0
                  if connected && !empty(doc_tab_rows(doc_tabs, pages, active_page))
                    box w=fill h=34.0 pl=8.0 pr=8.0 bg=sidebar border=separator border-w=1.0
                      scroll dir=horizontal w=fill h=fill bar=hidden
                        row h=fill gap=2.0 align=center
                          for tab in doc_tab_rows(doc_tabs, pages, active_page)
                            row gap=0.0 align=center
                              button label="Open page tab" h=26.0 p=5.0 @ghost_action -> choose_page(tab.id)
                                row h=fill gap=5.0 align=center
                                  if tab.active
                                    text tab.title size=13.0 wrap=none font=medium @text-fg
                                  if !tab.active
                                    text tab.title size=13.0 wrap=none @text-muted
                                active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
                                hovered bg=fg/5 text=fg
                                pressed bg=fg/8
                              button "×" label="Close page tab" w=20.0 h=20.0 p=0.0 @icon_action -> close_doc_tab(tab.id)
                                active bg=transparent text=muted r=6.0
                                hovered bg=fg/8 text=fg
                                pressed bg=fg/12
                  stack w=fill h=fill clip=true
                    sensor show=pages_resized resize=pages_resized
                      space w=fill h=fill
                    if !connected
                      EmptyState title="Connect to a node" description="Set the RPC endpoint in the sidebar."
                    if connected && empty(active_page)
                      EmptyState title="No page selected" description="Create a page from the sidebar."
                    if connected && !empty(active_page)
                      scroll dir=vertical w=fill h=fill bar=hidden
                        box w=fill max-w=720.0 mx=auto pl=22.0 pr=22.0 pt=26.0 pb=40.0
                          col w=fill gap=8.0
                            row w=fill h=28.0 gap=5.0 align=center
                              if !empty(active_page_parent)
                                text active_page_parent w=fill size=12.0 wrap=none font=code @text-muted
                              if empty(active_page_parent)
                                space w=fill
                              input "" #page-search label="Search pages" <-> page_search_draft hint="Search pages…" disabled=(!connected || page_searching) submit=search_pages_submit w=190.0 p=6.2 text-size=13.0 line-h=1.2 @control
                                active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                                hovered bg=fg/5 border=fg/8
                                disabled value=muted
                              if !empty(page_search_hits)
                                button label="Clear page search" w=28.0 h=28.0 p=0.0 @icon_action -> clear_page_search
                                  box w=fill h=fill align-x=center align-y=center
                                    text "×" size=14.0
                                  active bg=transparent text=muted r=7.0
                                  hovered bg=fg/10 text=fg
                                  pressed bg=fg/15
                              if !page_delete_armed
                                button label="Page menu" disabled=(mutation_phase != "idle") w=28.0 h=28.0 p=0.0 @icon_action -> arm_page_delete
                                  box w=fill h=fill align-x=center align-y=center
                                    text "•••" size=13.0
                                  active bg=transparent text=muted r=7.0
                                  hovered bg=fg/10 text=fg
                                  pressed bg=fg/15
                              if page_delete_armed
                                button "Delete page" disabled=(mutation_phase != "idle") h=26.0 p=5.0 @danger_action -> delete_page_submit
                            box w=fill pl=56.0
                              PageTitleEditor rpc=connected_rpc password=password page_id=active_page title=active_page_title disabled=(loading || !connected || mutation_phase != "idle") #page-title(scope_key(connected_rpc, active_page))
                            if !empty(page_search_hits)
                              box w=fill h=148.0 p=5.0 bg=elevated border=fg/8 border-w=1.0 r=9.0
                                scroll dir=vertical w=fill h=fill
                                  col w=fill gap=1.0
                                    for hit in page_search_hits
                                      PageSearchResult hit=hit
                            if !empty(orphaned_block_drafts) || !empty(orphaned_comment_drafts)
                              box w=fill p=7.0 bg=elevated border=fg/9 border-w=1.0 r=9.0
                                col w=fill gap=5.0
                                  text "Recovered drafts" size=13.0 font=medium @text-fg
                                  for recovered_block in orphaned_block_drafts
                                    row w=fill gap=5.0 align=center
                                      text recovered_block w=fill size=13.5 @text-muted
                                      button "Use" label="Use as block" disabled=(loading || mutation_phase != "idle" || !empty(block_draft)) h=26.0 p=5.0 @ghost_action -> use_orphaned_block_draft(recovered_block)
                                        active bg=fg/9 text=fg border=fg/12 border-w=1.0 r=7.0
                                        hovered bg=fg/14
                                        pressed bg=fg/18
                                      button "Discard" disabled=(loading || mutation_phase != "idle") h=26.0 p=5.0 @danger_action -> discard_orphaned_block_draft(recovered_block)
                                  for recovered_comment in orphaned_comment_drafts
                                    row w=fill gap=5.0 align=center
                                      text recovered_comment w=fill size=13.5 @text-muted
                                      button "Use" label="Use as block" disabled=(loading || mutation_phase != "idle" || !empty(block_draft)) h=26.0 p=5.0 @ghost_action -> use_orphaned_comment_draft(recovered_comment)
                                        active bg=fg/9 text=fg border=fg/12 border-w=1.0 r=7.0
                                        hovered bg=fg/14
                                        pressed bg=fg/18
                                      button "Discard" disabled=(loading || mutation_phase != "idle") h=26.0 p=5.0 @danger_action -> discard_orphaned_comment_draft(recovered_comment)
                            if empty(blocks) && !block_insert_open
                              box w=fill pl=56.0
                                button "Write something…" label="Start writing" disabled=loading w=fill p=6.0 @ghost_action -> open_root_block_insert
                                  active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
                                  hovered bg=fg/4 text=fg border=fg/7
                                  pressed bg=fg/8
                            if block_insert_open && empty(block_insert_after_id)
                              InlineBlockInsert kind=new_block_kind kinds=block_kinds disabled=loading prefix="" #block-insert-row(block_insert_after_id)
                                stack w=fill
                                  if new_block_kind != "Divider"
                                    col w=fill gap=2.0
                                      input "" #block-insert label="New block" <-> block_draft hint="Type, or / for a block kind…" disabled=loading submit=add_block_submit w=fill p=5.0 text-size=13.5 line-h=1.3 @control
                                        active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=1.0 r=6.0
                                        hovered bg=fg/2 border=fg/5
                                        disabled value=muted
                                      if !empty(slash_kind_matches(block_draft, editable_block_kinds))
                                        box w=fill p=3.0 bg=surface border=border border-w=1.0 r=8.0 shadow=shadow_popover shadow-y=3.0 shadow-blur=12.0
                                          col w=fill gap=1.0
                                            for kind in slash_kind_matches(block_draft, editable_block_kinds)
                                              button label="Set block kind" w=fill h=24.0 p=4.0 @ghost_action -> pick_slash_kind(kind)
                                                row w=fill h=fill gap=6.0 align=center
                                                  text kind w=fill size=13.0 wrap=none @text-fg
                                                active bg=transparent text=fg border=transparent border-w=1.0 r=6.0
                                                hovered bg=brand/14 text=fg
                                                pressed bg=brand/20
                                  if new_block_kind == "Divider"
                                    button "Insert divider" disabled=loading w=fill h=28.0 p=5.0 @secondary_action -> add_block_submit
                            keyed block in blocks by=block.key
                              col w=fill gap=1.0
                                DocumentBlock block=block selected=(block.id == selected_block_id) hovered=(block.id == hovered_block_id) disabled=loading #block(block.id)
                                  col w=fill
                                    if block.pending
                                      box w=fill p=5.0 bg=fg/3 r=6.0
                                        BlockContents block=block
                                    if !block.pending && block.kind == "Page"
                                      button label=block.kind description=block.text w=fill p=5.0 @ghost_action -> choose_page(block.id)
                                        BlockContents block=block
                                        active bg=transparent text=fg border=transparent border-w=1.0 r=6.0
                                        hovered bg=fg/3 text=fg border=transparent
                                        pressed bg=fg/6 text=fg
                                    if !block.pending && block.kind != "Page" && block.id != selected_block_id
                                      button label=block.kind description=block.text w=fill p=5.0 @ghost_action -> select_block(block.key, block.id, block.kind, block.text, block.checked, false)
                                        BlockContents block=block
                                        active bg=transparent text=fg border=transparent border-w=1.0 r=6.0
                                        hovered bg=fg/3 text=fg border=transparent
                                        pressed bg=fg/6 text=fg
                                    if !block.pending && block.kind != "Page" && block.id == selected_block_id
                                      BlockLine block=block #line
                                        col w=fill
                                          if block.kind == "Divider"
                                            Separator
                                          if block.kind != "Divider"
                                            input "" #block-edit label="Edit block" <-> block_edit_draft change=block_text_changed hint="Type something…" disabled=(mutation_phase != "idle") w=fill p=4.0 text-size=13.5 line-h=1.3 @control
                                              active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=1.0 r=5.0
                                              hovered bg=fg/2 border=fg/5
                                              disabled value=muted
                                if block_insert_open && block.id == block_insert_after_id
                                  InlineBlockInsert kind=new_block_kind kinds=block_kinds disabled=loading prefix=block.prefix #block-insert-row(block_insert_after_id)
                                    stack w=fill
                                      if new_block_kind != "Divider"
                                        col w=fill gap=2.0
                                          input "" #block-insert label="New block" <-> block_draft hint="Type, or / for a block kind…" disabled=loading submit=add_block_submit w=fill p=5.0 text-size=13.5 line-h=1.3 @control
                                            active bg=transparent border=transparent value=fg placeholder=muted selection=fg/18 border-w=1.0 r=6.0
                                            hovered bg=fg/2 border=fg/5
                                            disabled value=muted
                                          if !empty(slash_kind_matches(block_draft, editable_block_kinds))
                                            box w=fill p=3.0 bg=surface border=border border-w=1.0 r=8.0 shadow=shadow_popover shadow-y=3.0 shadow-blur=12.0
                                              col w=fill gap=1.0
                                                for kind in slash_kind_matches(block_draft, editable_block_kinds)
                                                  button label="Set block kind" w=fill h=24.0 p=4.0 @ghost_action -> pick_slash_kind(kind)
                                                    row w=fill h=fill gap=6.0 align=center
                                                      text kind w=fill size=13.0 wrap=none @text-fg
                                                    active bg=transparent text=fg border=transparent border-w=1.0 r=6.0
                                                    hovered bg=brand/14 text=fg
                                                    pressed bg=brand/20
                                      if new_block_kind == "Divider"
                                        button "Insert divider" disabled=loading w=fill h=28.0 p=5.0 @secondary_action -> add_block_submit
                    overlay when=(connected && !empty(active_page) && !empty(selected_block_id) && block_actions_open) dismiss=close_block_actions backdrop=transparent p=0.0 align-x=start align-y=start
                      content
                        space w=fill h=fill
                      layer
                        float x=(block_menu_x + 10.0) y=block_menu_y
                          BlockActionsMenu block_id=selected_block_id kind=selected_block_kind disabled=(loading || mutation_phase != "idle") delete_armed=block_delete_armed editable_kinds=editable_block_kinds
                // The artifact hangs a 306px rail off the document, not a
                // floating card. The Spec tab is omitted: pages carry no kind,
                // no last-editor and no derivation pipeline (see omissions).
                if connected && !empty(active_page) && block_comments_open
                  box w=1.0 h=fill bg=separator
                    space w=1.0 h=1.0
                  box w=306.0 h=fill bg=sidebar clip=true
                    col w=fill h=fill
                      box w=fill h=50.0 pl=16.0 pr=16.0
                        row w=fill h=fill gap=18.0 align=center
                          TabLabel label="Comments" count=block_comment_thread_total active=true
                          space w=fill
                          button label="Close comments" disabled=(mutation_phase != "idle") w=24.0 h=24.0 p=4.0 @icon_action -> close_block_comments
                            box w=fill h=fill align-x=center align-y=center
                              text "×" size=13.0 wrap=none @text-muted
                            active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
                            hovered bg=elevated text=fg
                            pressed bg=subtle text=fg
                      box w=fill h=1.0 bg=separator
                        space w=1.0 h=1.0
                      col w=fill h=fill p=12.0 gap=6.0
                        if empty(active_block_comment_thread)
                          scroll dir=vertical w=fill h=fill
                            col w=fill gap=1.0
                              if empty(block_comment_threads) && !block_comment_threads_loading
                                text "No comments yet" w=fill size=12.5 align-x=center @text-muted
                              for comment_thread in block_comment_threads
                                PageCommentThreadButton thread=comment_thread
                              if block_comment_threads_has_more
                                button "More" disabled=(block_comment_threads_loading || mutation_phase != "idle") h=24.0 p=4.0 @secondary_action -> load_more_block_threads
                                  active bg=transparent text=muted r=6.0
                                  hovered bg=fg/9 text=fg
                                  pressed bg=fg/14
                        if !empty(active_block_comment_thread)
                          row w=fill gap=5.0 align=center
                            button "← Threads" disabled=(block_thread_comments_loading || mutation_phase != "idle") h=24.0 p=4.0 @secondary_action -> close_block_comment_thread
                              active bg=transparent text=muted r=6.0
                              hovered bg=fg/9 text=fg
                              pressed bg=fg/14
                          scroll dir=vertical w=fill h=fill
                            col w=fill gap=1.0
                              for page_comment in block_thread_comments
                                PageCommentCard comment=page_comment
                              if block_thread_comments_has_more
                                button "More" disabled=(block_thread_comments_loading || mutation_phase != "idle") h=24.0 p=4.0 @secondary_action -> load_more_block_comments
                                  active bg=transparent text=muted r=6.0
                                  hovered bg=fg/9 text=fg
                                  pressed bg=fg/14
                        row w=fill gap=5.0 align=center
                          input "" #block-comment(scope_key(connected_rpc, selected_block_id)) label="New block comment" <-> block_comment_draft hint="Add a comment…" disabled=(mutation_phase != "idle" || block_comment_threads_loading || block_thread_comments_loading) submit=post_block_comment_submit w=fill p=6.2 text-size=13.0 line-h=1.2 @control
                            active bg=transparent border=fg/8 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                            hovered bg=fg/4 border=fg/11
                            disabled value=muted
                          button "Post" disabled=(mutation_phase != "idle" || empty(trim(block_comment_draft)) || block_comment_threads_loading || block_thread_comments_loading) h=28.0 p=5.0 @primary_action -> post_block_comment_submit
        files:
          col w=fill h=fill
            ScreenHeader title="Files" meta=fs_path
              space w=1.0 h=1.0
            // WHERE THE WRITE CONTROLS LIVE — decided here, once. The artifact's
            // Files screen is a read-only browser, but this app ships a working
            // mkdir / new file / delete / edit and dropping them would be a
            // regression. They sit in ONE bar under the header, never as per-row
            // hover affordances, so the three panes below stay the artifact's read
            // surface and the destructive verb always names the selected object.
            box w=fill pl=20.0 pr=20.0 pt=10.0 pb=10.0
              row w=fill h=28.0 gap=8.0 align=center
                button "↑" label="Parent directory" disabled=(fs_loading || empty(fs_path)) w=26.0 h=26.0 p=0.0 @icon_action -> fs_open_parent
                  active bg=surface text=muted border=card_line border-w=1.0 r=7.0
                  hovered bg=elevated text=fg
                  pressed bg=subtle
                input "" #fs-new label="New entry name" <-> fs_new_name change=fs_new_name_changed hint="new name…" disabled=fs_loading w=160.0 p=5.0 text-size=13.0 line-h=1.2 @control
                  active bg=surface border=border value=fg placeholder=hint selection=fg/18 border-w=1.0 r=7.0
                  hovered bg=muted_bg border=control_line
                  disabled bg=muted_bg/54 value=muted
                button "+ Folder" disabled=(fs_loading || empty(trim(fs_new_name))) h=26.0 p=5.0 @secondary_action -> fs_mkdir_submit
                button "+ File" disabled=(fs_loading || empty(trim(fs_new_name))) h=26.0 p=5.0 @secondary_action -> fs_new_file_submit
                space w=fill
                if fs_loading
                  text "Loading…" size=12.5 wrap=none @text-caption
                if !empty(fs_preview_path) && fs_preview_path != fs_delete_target
                  button "Delete object" disabled=fs_loading h=26.0 p=5.0 @secondary_action -> fs_arm_delete(fs_preview_path)
                    active bg=transparent text=muted border=card_line border-w=1.0 r=7.0
                    hovered bg=danger_zone_bg text=fg border=danger_zone_line
                    pressed bg=danger_zone_bg
                if !empty(fs_preview_path) && fs_preview_path == fs_delete_target
                  button "Delete for real" disabled=fs_loading h=26.0 p=5.0 @danger_action -> fs_delete_submit
                button "History" h=26.0 p=5.0 @secondary_action -> fs_toggle_history
                  active bg=surface text=muted border=card_line border-w=1.0 r=7.0
                  hovered bg=elevated text=fg
                  pressed bg=subtle
            box w=fill h=1.0 bg=separator
              space w=1.0 h=1.0
            row w=fill h=fill
              // 206px directory pane. `files_ls` loads one level at a time, so this
              // is the current level's directories, not a recursively expanded tree
              // — depth stays 0 until a per-level expansion state exists.
              box w=206.0 h=fill bg=sidebar clip=true
                scroll dir=vertical w=fill h=fill bar=hidden
                  col w=fill pl=6.0 pr=6.0 pt=8.0 pb=8.0 gap=1.0
                    for entry in fs_entries
                      if entry.kind == "dir"
                        FsTreeRow entry=entry selected=false depth=0.0
              box w=1.0 h=fill bg=separator
                space w=1.0 h=1.0
              col w=fill h=fill
                if fs_history_open
                  scroll dir=vertical w=fill h=fill
                    col w=fill p=18.0 gap=8.0
                      if !empty(fs_diff_from)
                        col w=fill gap=6.0
                          row w=fill gap=8.0 align=center
                            GroupLabel label="CHANGES VS HEAD"
                            space w=fill
                            button "Back" h=22.0 p=4.0 @secondary_action -> fs_close_diff
                              active bg=surface text=muted border=card_line border-w=1.0 r=6.0
                              hovered bg=elevated text=fg
                              pressed bg=subtle
                          if empty(fs_diff)
                            text "No differences." size=12.5 @text-caption
                          for entry in fs_diff
                            row w=fill gap=8.0 align=center
                              text entry.kind w=64.0 size=12.0 wrap=none font=code @text-meta
                              text entry.path w=fill size=12.0 wrap=none font=code @text-fg
                      if empty(fs_diff_from)
                        col w=fill gap=8.0
                          GroupLabel label="SNAPSHOTS"
                          for snapshot in fs_history
                            box w=fill p=11.0 bg=surface border=card_line border-w=1.0 r=10.0
                              col w=fill gap=3.0
                                row w=fill gap=8.0 align=center
                                  text snapshot.short_id size=12.0 wrap=none font=code @text-fg
                                  text height_label(snapshot.height) size=12.0 wrap=none font=code @text-meta
                                  space w=fill
                                  text snapshot.author size=12.0 wrap=none font=code @text-meta
                                  button "Diff" h=20.0 p=3.0 @ghost_action -> fs_show_diff(snapshot.id)
                                    active bg=surface text=muted border=card_line border-w=1.0 r=6.0
                                    hovered bg=elevated text=fg
                                    pressed bg=subtle
                                if !empty(snapshot.message)
                                  text snapshot.message size=13.5 @text-fg
                if !fs_history_open
                  col w=fill h=fill
                    ObjectTableHeader
                    if empty(fs_entries) && !fs_loading
                      box w=fill p=22.0
                        EmptyPlate message="Empty directory — nothing is committed under this path."
                    if !empty(fs_entries)
                      scroll dir=vertical w=fill h=fill
                        col w=fill
                          for entry in fs_entries
                            ObjectRow entry=entry selected=(entry.path == fs_preview_path)
                    if !empty(fs_preview_path)
                      col w=fill h=300.0
                        box w=fill h=1.0 bg=separator
                          space w=1.0 h=1.0
                        box w=fill h=fill p=16.0
                          col w=fill h=fill gap=8.0
                            row w=fill gap=8.0 align=center
                              text fs_preview_path w=fill size=12.0 wrap=none font=code @text-meta
                              if fs_preview_truncated
                                text "first 64 KiB" size=12.5 wrap=none @text-caption
                              if !fs_preview_binary && !fs_editing && !fs_preview_truncated
                                button "Edit" h=22.0 p=4.0 @secondary_action -> fs_begin_edit
                                  active bg=surface text=muted border=card_line border-w=1.0 r=6.0
                                  hovered bg=elevated text=fg
                                  pressed bg=subtle
                              if fs_editing
                                button "Cancel" h=22.0 p=4.0 @secondary_action -> fs_cancel_edit
                                  active bg=surface text=muted border=card_line border-w=1.0 r=6.0
                                  hovered bg=elevated text=fg
                                  pressed bg=subtle
                              if fs_editing
                                button "Save" disabled=fs_loading h=22.0 p=4.0 @primary_action -> fs_save_edit
                            stack w=fill h=fill
                              if fs_editing
                                editor #fs-editor <-> fs_editor hint="File contents…" disabled=fs_loading min-h=200.0 size=12.0 line-h=1.3 p=6.6 wrap=word
                                  active bg=surface border=border value=fg placeholder=hint selection=fg/18 border-w=1.0 r=8.0
                                  hovered bg=muted_bg border=control_line
                                  focused bg=muted_bg border=ring border-w=1.0
                              if !fs_editing
                                scroll dir=vertical w=fill h=fill
                                  col w=fill gap=6.0
                                    if fs_preview_binary
                                      text fs_preview_text size=12.0 font=code @text-meta
                                    if !fs_preview_binary
                                      text fs_preview_text size=12.0 font=code @text-fg
              if !empty(fs_preview_path)
                for entry in fs_entries
                  if entry.path == fs_preview_path
                    ObjectPanel entry=entry
        members:
          row w=fill h=fill
            col w=fill h=fill
              ScreenHeader title="Members" meta=members_summary(members_validators, members_residents)
                // NO INVITE BUTTON YET. `mint_invite` exists in the backend and
                // `open_invite_modal` exists as view state, but nothing routes the
                // mint itself, so the button would open a modal with no act in it.
                space w=1.0 h=1.0
              // All / Humans / Agents / Validators. `filter_members` owns the
              // predicate so the strip and the list can never disagree.
              col w=fill
                box w=fill pl=22.0 pr=22.0 pt=12.0 pb=12.0
                  row w=fill gap=7.0 align=center
                    button label="Show every member" p=0.0 @ghost_action -> pick_members_filter("all")
                      FilterChip label="All" count=len(members_rows) selected=(members_filter == "all")
                      active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
                      hovered bg=row_hover text=fg
                      pressed bg=elevated text=fg
                    button label="Show people only" p=0.0 @ghost_action -> pick_members_filter("humans")
                      FilterChip label="Humans" count=len(filter_members(members_rows, "humans")) selected=(members_filter == "humans")
                      active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
                      hovered bg=row_hover text=fg
                      pressed bg=elevated text=fg
                    button label="Show agents only" p=0.0 @ghost_action -> pick_members_filter("agents")
                      FilterChip label="Agents" count=len(filter_members(members_rows, "agents")) selected=(members_filter == "agents")
                      active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
                      hovered bg=row_hover text=fg
                      pressed bg=elevated text=fg
                    button label="Show validators only" p=0.0 @ghost_action -> pick_members_filter("validators")
                      FilterChip label="Validators" count=len(filter_members(members_rows, "validators")) selected=(members_filter == "validators")
                      active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
                      hovered bg=row_hover text=fg
                      pressed bg=elevated text=fg
                    space w=fill
                box w=fill h=1.0 bg=separator
                  space w=1.0 h=1.0
              if empty(filter_members(members_rows, members_filter))
                box w=fill h=fill p=22.0
                  EmptyPlate message="No members here yet — validators, residents and registered agents appear as they join."
              if !empty(filter_members(members_rows, members_filter))
                scroll dir=vertical w=fill h=fill
                  col w=fill pl=12.0 pr=12.0 pt=6.0 pb=6.0 gap=1.0
                    for member in filter_members(members_rows, members_filter)
                      col w=fill
                        if member.key == members_selected
                          button label="Open member" description=member.label w=fill p=0.0 @ghost_action -> open_member(member.key)
                            MemberRowCard member=member
                            active bg=elevated text=fg border=transparent border-w=1.0 r=9.0
                            hovered bg=elevated text=fg
                            pressed bg=subtle text=fg
                        if member.key != members_selected
                          button label="Open member" description=member.label w=fill p=0.0 @ghost_action -> open_member(member.key)
                            MemberRowCard member=member
                            active bg=transparent text=fg border=transparent border-w=1.0 r=9.0
                            hovered bg=row_hover text=fg
                            pressed bg=elevated text=fg
            if !empty(members_selected)
              for member in members_rows
                if member.key == members_selected
                  MemberDetail member=member admin=members_is_admin(members_rows)
        agents:
          col w=fill h=fill
            ScreenHeader title="Agents" meta=agents_summary(agents_rows)
              space w=1.0 h=1.0
            // The registry explainer. The artifact states the whole model in this
            // one strip and the English UI never did: the registry is the record of
            // WHO may do WHAT under WHICH grant, and the doing itself is recorded
            // separately as that agent's runs.
            col w=fill
              box w=fill pl=22.0 pr=22.0 pt=12.0 pb=10.0
                text "The registry records who may act, what they may do, and under whose grant — every entry here is on chain. The acting itself is recorded separately, as each agent's runs." w=fill size=12.0 line-h=1.5 @text-caption
              box w=fill h=1.0 bg=separator
                space w=1.0 h=1.0
            if empty(agents_rows)
              box w=fill h=fill p=22.0
                EmptyPlate message="No agents registered — a registered agent appears here with its capability and grants."
            if !empty(agents_rows)
              scroll dir=vertical w=fill h=fill
                col w=fill p=18.0 gap=11.0
                  for agent in agents_rows
                    AgentCard agent=agent
        forge:
          col w=fill h=fill
            // THE REPO OVERVIEW. Reachable again: `forge_close_repo` clears the open
            // repo, which nothing did before — once a repo was opened the grid was
            // gone for the rest of the session.
            if empty(forge_repo)
              scroll dir=vertical w=fill h=fill
                col w=fill p=22.0 gap=18.0
                  ForgeOrgHeader org=network_label(account_name, connected_rpc) about="" repos=len(forge_repos) tier=member_tier(members_rows)
                  if empty(forge_repos)
                    EmptyPlate message="No repos — a consensus-backed repo appears here once it is created."
                  if !empty(forge_repos)
                    grid min-cell=380.0 gap=13.0
                      for repo in forge_repos
                        RepoCard repo=repo
            if !empty(forge_repo)
              col w=fill h=fill
                box w=fill pl=22.0 pr=22.0 pt=14.0 pb=12.0
                  stack w=fill
                    row w=fill gap=9.0 align=center
                      button label="Switch repository" w=fill p=0.0 @ghost_action -> forge_toggle_repo_menu
                        RepoCrumb org=network_label(account_name, connected_rpc) repo=forge_repo branch="" open=forge_repo_menu
                        active bg=transparent text=fg border=transparent border-w=1.0 r=9.0
                        hovered bg=row_hover text=fg
                        pressed bg=elevated text=fg
                      button "All repos" h=28.0 p=6.0 @secondary_action -> forge_close_repo
                    if forge_repo_menu
                      pin x=0.0 y=38.0
                        Popover width=290.0
                          col w=fill gap=1.0
                            for repo in forge_repos
                              RepoMenuRow repo=repo active=(repo.name == forge_repo)
                box w=fill h=1.0 bg=separator
                  space w=1.0 h=1.0
                if forge_item_number <= 0
                  col w=fill h=fill
                    if !empty(forge_branches)
                      box w=fill pl=22.0 pr=22.0 pt=10.0 pb=10.0
                        scroll dir=horizontal w=fill h=22.0 bar=hidden
                          row h=fill gap=4.0 align=center
                            for branch in forge_branches
                              box h=20.0 pl=7.0 pr=7.0 align-y=center bg=surface border=border border-w=1.0 r=10.0
                                text branch size=9.0 wrap=none font=code_semibold @text-meta
                    if empty(forge_items)
                      box w=fill p=22.0
                        EmptyPlate message="No issues or pull requests — the tracker is empty for this repo."
                    if !empty(forge_items)
                      scroll dir=vertical w=fill h=fill
                        col w=fill pl=12.0 pr=12.0 pt=6.0 pb=18.0 gap=1.0
                          for item in forge_items
                            TrackerRow item=item
                    // NO GATE NOTE HERE. `ForgeGateNote` told a resident the
                    // node refuses their merge; `ForgeMsg::MergePr` authorizes
                    // on `author_from_origin` alone, so the write succeeds and
                    // the plate described an enforcement that does not exist.
                    // The one true sentence about it lives beside the merge
                    // button, where the decision is made.
                if forge_item_number > 0
                  scroll dir=vertical w=fill h=fill
                    col w=fill p=22.0 gap=14.0
                      BackToList kind=forge_item_kind
                      row w=fill gap=9.0 align=center
                        text forge_item_title w=fill size=16.0 wrap=none font=display @text-primary
                        if forge_item_kind == "pr"
                          PrStatePill state=forge_item_state
                        if forge_item_kind != "pr"
                          StatusBadge label=forge_item_state
                      row w=fill gap=10.0 align=center
                        if !empty(forge_item_author)
                          text forge_item_author size=11.0 wrap=none font=code_medium @text-meta
                        if !empty(forge_item_branches)
                          text forge_item_branches size=12.0 wrap=none font=code @text-meta
                        if forge_item_files_changed > 0
                          DiffCount additions=forge_item_additions deletions=forge_item_deletions files=forge_item_files_changed
                        space w=fill
                      if !empty(forge_item_body)
                        IssueBodyCard author=forge_item_author body=forge_item_body
                      if !empty(forge_item_diff)
                        col w=fill gap=6.0
                          if forge_item_diff_truncated
                            text "Patch truncated — the counts above cover the whole diff." size=12.5 @text-caption
                          // The pane's header names the patch's own branch pair:
                          // `forge_item_diff` is the WHOLE unified patch, and its
                          // per-file headers ride inside it as `file` diff rows.
                          DiffPane file=forge_item_branches additions=forge_item_additions deletions=forge_item_deletions lines=diff_lines(forge_item_diff)
                      if forge_item_kind == "pr"
                        col w=fill gap=9.0
                          GroupLabel label="MERGE"
                          if forge_item_state == "merged"
                            MergedBanner note=forge_merge_note(forge_item_merge_oid, forge_item_branches)
                          if forge_item_state == "closed"
                            text "Closed without merging." size=12.5 @text-caption
                          if forge_item_state == "open"
                            col w=fill gap=9.0
                              if !empty(forge_merge_conflicts)
                                col w=fill gap=3.0
                                  text "Merge conflicts — resolve on the branch and push again:" size=12.5 @text-caption
                                  for conflict_path in forge_merge_conflicts
                                    text conflict_path size=12.0 font=code @text-fg
                              MergeAdvisory change_requests=forge_item_change_requests
                              row w=fill gap=10.0 align=center
                                MergeButton busy=forge_merge_busy disabled=(!connected || empty(forge_item_source_oid))
                                // The tally belongs where the decision is made,
                                // and it is loaded already. The sentence beside
                                // it is the whole permission model this module
                                // has: none. Approvals never block a merge.
                                text forge_item_approvals size=12.0 wrap=none font=code_medium @text-meta
                                text "approvals" size=12.5 wrap=none @text-caption
                                text "Approvals are advisory — merging is never gated." w=fill size=12.5 @text-caption
                      if forge_item_kind == "pr"
                        col w=fill gap=9.0
                          GroupLabel label="REVIEWS"
                          if empty(forge_item_reviews)
                            text "No reviews yet." size=12.5 @text-caption
                          for review in forge_item_reviews
                            ReviewCard review=review
                          row w=fill gap=6.0 align=center
                            button label="Pick comment verdict" h=24.0 p=5.0 @ghost_action -> forge_review_pick("comment")
                              text verdict_pick_label(forge_review_verdict, "comment", "Comment") size=13.0
                              active bg=surface text=fg border=card_line border-w=1.0 r=7.0
                              hovered bg=elevated text=fg
                              pressed bg=subtle text=fg
                            button label="Pick approve verdict" h=24.0 p=5.0 @ghost_action -> forge_review_pick("approve")
                              text verdict_pick_label(forge_review_verdict, "approve", "Approve") size=13.0
                              active bg=final_bg text=fg border=final_line border-w=1.0 r=7.0
                              hovered bg=success_bg text=fg
                              pressed bg=success_bg text=fg
                            button label="Pick request-changes verdict" h=24.0 p=5.0 @ghost_action -> forge_review_pick("request_changes")
                              text verdict_pick_label(forge_review_verdict, "request_changes", "Request changes") size=13.0
                              active bg=alert_bg text=fg border=alert_line border-w=1.0 r=7.0
                              hovered bg=danger_zone_bg text=fg
                              pressed bg=danger_zone_bg text=fg
                            space w=fill
                          row w=fill gap=6.0 align=center
                            input "" #forge-review-body label="Review body" <-> forge_review_draft hint="Leave a review…" disabled=(forge_review_busy || !connected) submit=forge_review_submit w=fill p=6.2 text-size=13.0 line-h=1.2 @control
                              active bg=surface border=border value=fg placeholder=hint selection=fg/18 border-w=1.0 r=8.0
                              hovered bg=muted_bg border=control_line
                              disabled bg=muted_bg/54 value=muted
                            button "Submit review" disabled=(forge_review_busy || !connected || empty(forge_item_source_oid)) h=28.0 p=6.0 @primary_action -> forge_review_submit
                      col w=fill gap=9.0
                        GroupLabel label="DISCUSSION"
                        if empty(forge_discussion)
                          text "No discussion yet." size=12.5 @text-caption
                        for message in forge_discussion
                          row w=fill gap=9.0 align=start
                            MessageAvatar initials=message.initial kind=message.avatar_kind
                            col w=fill gap=2.0
                              row w=fill gap=7.0 align=center
                                text message.author size=13.0 wrap=none font=display @text-fg
                                text message.meta size=11.0 wrap=none font=code_medium @text-meta
                                space w=fill
                              MessageBody message=message
                        flex w=fill gap=8.0 items=end
                          editor #forge-note <-> forge_discussion_editor hint="Write a note…" disabled=(loading || !connected || empty(forge_item_channel)) min-h=38.0 max-h=120.0 size=13.5 line-h=1.3 p=6.0 wrap=word key-binding=composer_keys() -> forge_note_submit
                            active bg=surface border=card_line value=fg placeholder=hint selection=fg/18 border-w=1.0 r=8.0
                            hovered bg=muted_bg border=control_line
                            focused bg=muted_bg border=ring border-w=1.0
                            disabled value=muted
                          button "Send" disabled=(loading || !connected || empty(forge_item_channel) || !empty(forge_discussion_pending) || empty(trim(editor_text(forge_discussion_editor)))) w=60.0 h=28.0 p=6.0 @primary_action -> forge_note_submit
        governance:
          scroll dir=vertical w=fill h=fill
            col w=fill p=22.0 gap=16.0
              row gap=9.0 align=center
                text "Approvals" size=16.0 wrap=none font=display @text-primary
                // The chip counts what is WAITING. Finalized rows have their own
                // section below and are never folded into this number.
                if open_proposals(gov_rows) > 0
                  CountChip label=pending_label(gov_rows)
              // The artifact bands the screen when the reader cannot vote. Its words
              // are ADMIN/MAINTAINER; ours are the tiers this chain actually grants.
              if !members_is_admin(members_rows)
                GateNote reason="Approval votes are cast by this network's validators, and this node does not hold validator standing." next="You can still read every proposal and follow its tally while it runs."
              // Empty means nothing OPEN. A workspace whose every decision settled
              // still gets the plate, not a silent screen.
              if open_proposals(gov_rows) <= 0
                EmptyPlate message="No proposals waiting — every decision on this network is finalized."
              if open_proposals(gov_rows) > 0
                col w=fill gap=12.0
                  for proposal in gov_rows
                    if proposal.open
                      ProposalCard proposal=proposal busy=(!empty(gov_voting))
              // The FINALIZED eyebrow is gated on the settled subset, never on the
              // combined register — otherwise it hangs over nothing.
              if !empty(settled_proposals(gov_rows))
                col w=fill gap=10.0
                  GroupLabel label="RECENTLY FINALIZED"
                  for proposal in settled_proposals(gov_rows)
                    SettledProposalRow proposal=proposal
        settings:
          scroll dir=vertical w=fill h=fill
            col w=fill p=22.0 gap=18.0
              text "Settings" size=16.0 wrap=none font=display @text-primary
              grid min-cell=420.0 gap=22.0
                col w=fill gap=9.0
                  GroupLabel label="NETWORK"
                  GroupCard
                    col w=fill
                      KeyValueRow label="Workspace" value=network_label(account_name, connected_rpc) last=false
                      KeyValueRow label="Endpoint" value=settings_endpoint last=false
                      KeyValueRow label="Node key" value=settings_node_key last=false
                      KeyValueRow label="Block height" value=height_label(settings_height) last=false
                      // The artifact's last NETWORK row: the roster reading with an
                      // inline accent link onto the Members screen.
                      box w=fill px=15.0 py=13.0
                        row w=fill gap=10.0 align=center
                          text "Members" size=12.5 wrap=none @text-accent_fg
                          space w=fill
                          text members_summary(members_validators, members_residents) size=12.0 wrap=none font=code_medium @text-secondary_fg
                          button "manage" h=22.0 p=0.0 @ghost_action -> select_shell_tab("members")
                            active bg=transparent text=brand border=transparent border-w=1.0 r=6.0
                            hovered bg=elevated text=brand
                            pressed bg=subtle text=brand
                col w=fill gap=9.0
                  GroupLabel label="CONNECTION"
                  box w=fill p=15.0 bg=surface border=card_line border-w=1.0 r=11.0
                    col w=fill gap=9.0
                      input "" #rpc label="RPC endpoint" <-> rpc hint="Node URL" disabled=(loading || (mutation_phase != "idle" && mutation_phase != "recovering")) submit=reconnect w=fill p=6.2 text-size=13.0 line-h=1.2 @control
                        active bg=muted_bg border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=9.0
                        hovered bg=elevated border=fg/21
                        disabled bg=muted_bg/54 value=muted
                      input "" #password label="Local key password" secure=true <-> password hint="Key password" disabled=(loading || mutation_phase != "idle") w=fill p=6.2 text-size=13.0 line-h=1.2 @control
                        active bg=muted_bg border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=9.0
                        hovered bg=elevated border=fg/21
                        disabled bg=muted_bg/54 value=muted
                      row w=fill gap=9.0 align=center
                        if connection_degraded(status)
                          Badge.Destructive label=status
                        if !connection_degraded(status)
                          Badge.Success label=status
                        space w=fill
                        button "Connect" disabled=(loading || (mutation_phase != "idle" && mutation_phase != "recovering")) h=32.0 p=7.0 @primary_action -> reconnect
                col w=fill gap=9.0
                  GroupLabel label="YOUR IDENTITY"
                  box w=fill p=15.0 bg=surface border=card_line border-w=1.0 r=11.0
                    row w=fill gap=13.0 align=center
                      PersonAvatar initials=initial_of(account_name) plate=40.0 ink=14.0
                      // clip: the key line is four `wrap=none` runs over a 64-hex
                      // key, so it cannot shrink — without this it paints over the
                      // rename controls in the next column.
                      col w=fill gap=3.0 clip=true
                        row w=fill gap=7.0 align=center
                          if !empty(account_name)
                            text account_name size=13.5 wrap=none font=display @text-fg
                          if empty(account_name)
                            text "(unnamed)" size=13.5 wrap=none @text-muted
                          // The badge carries STANDING on this network — validator,
                          // resident, guest — not whether a local key is bound. The
                          // artifact's ADMIN/MAINTAINER words name authority this
                          // chain does not grant, so the app keeps its own.
                          if members_is_admin(members_rows)
                            Badge.Secondary label=member_tier(members_rows)
                          if !members_is_admin(members_rows)
                            Badge.Outline label=member_tier(members_rows)
                        // The key line says WHICH keypair this is and that it lives
                        // on this device — the custody clause the artifact carries.
                        row w=fill gap=5.0 align=center
                          text account_id size=10.5 wrap=none font=code_medium @text-hint
                          text "·" size=10.5 wrap=none font=code_medium @text-hint
                          text member_tier(members_rows) size=10.5 wrap=none font=code_medium @text-hint
                          text "keypair on this device" size=10.5 wrap=none font=code_medium @text-hint
                      col gap=5.0
                        row w=fill h=28.0 gap=5.0 align=center
                          input "" #account-rename label="New display name" <-> account_name_draft change=account_name_draft_changed hint="rename account…" disabled=account_renaming w=150.0 p=5.0 text-size=13.0 line-h=1.2 @control
                            active bg=elevated border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                            hovered bg=elevated border=fg/21
                            disabled bg=muted_bg/54 value=muted
                          button "Rename" disabled=(account_renaming || empty(trim(account_name_draft))) h=28.0 p=5.0 @secondary_action -> account_rename_submit
                        row gap=8.0 align=center
                          text account_members size=12.0 wrap=none font=code @text-meta
                          text "keys" size=12.5 wrap=none @text-meta
                          text account_nodes size=12.0 wrap=none font=code @text-meta
                          text "nodes" size=12.5 wrap=none @text-meta
                          space w=fill
                          button "Copy key" disabled=empty(account_id) h=28.0 p=7.0 @secondary_action -> copy_to_clipboard(account_id, "Key copied")
                col w=fill gap=9.0
                  GroupLabel label="IDENTITY KEY"
                  GroupCard
                    col w=fill
                      KeyValueRow label="Key state" value=settings_key_state last=false
                      KeyValueRow label="Key path" value=settings_key_path last=true
                // NO PREFERENCES GROUP. `Change receipts` was a placebo: every
                // finality mark in the app — FinalityChip, the chat tick, the
                // merge stamp — renders unconditionally, so the switch wrote a
                // value nothing read. It also painted ON from the state default
                // and flipped itself OFF a beat later, because the loader
                // answers `false` for an absent key. The group comes back the
                // day the marks are actually gated on it.
                col w=fill gap=9.0
                  GroupLabel label="THIS DEVICE"
                  box w=fill bg=surface border=card_line border-w=1.0 r=11.0 clip=true
                    col w=fill
                      box w=fill px=15.0 py=13.0
                        row w=fill gap=10.0 align=center
                          col w=fill gap=1.0
                            text "Open page tabs" size=12.5 @text-accent_fg
                            text "Preferences persist per endpoint in app-prefs.json beside the user key." size=12.5 @text-meta
                          text settings_open_tabs size=12.0 wrap=none font=code_medium @text-secondary_fg
                          button "Forget tabs" h=28.0 p=5.0 @secondary_action -> settings_clear_tabs
                col w=fill gap=9.0
                  // The one warmed eyebrow in the console: #c79a8a, not the #bdbbb1
                  // every other group label wears.
                  text "DANGER ZONE" size=9.0 wrap=none font=code_semibold @text-danger_label
                  box w=fill p=15.0 bg=danger_zone_bg border=danger_zone_line border-w=1.0 r=11.0
                    row w=fill gap=13.0 align=center
                      col w=fill gap=2.0
                        text "Leave this workspace" size=12.5 wrap=none font=medium @text-accent_fg
                        text "Removes the workspace from THIS DEVICE and returns to onboarding. Nothing on the network changes and no key is destroyed." size=10.5 @text-meta
                      button "Leave workspace" disabled=(!connected || mutation_phase != "idle") h=32.0 p=8.0 @icon_action -> forget_workspace_submit
                        active bg=danger_solid text=brand_fg border=danger_solid border-w=1.0 r=8.0
                        hovered bg=danger_solid_hover text=brand_fg border=danger_solid_hover
                        pressed bg=danger_solid_hover text=brand_fg
              // ── THIS NODE ──────────────────────────────────────────────────────
              // The rail has eight seats and none of them is Node: the artifact puts
              // the node's own facts under Settings, reached from the rail footer.
              // This is that relocation, kept whole — Overview / Permissions /
              // Activity, with the log console under Activity.
              box w=fill h=1.0 bg=separator
                space w=1.0 h=1.0
              col w=fill gap=13.0
                row w=fill gap=10.0 align=center
                  text "This node" size=16.0 wrap=none font=display @text-primary
                  StatusPill degraded=connection_degraded(status) loading=loading
                  space w=fill
                row gap=3.0 align=center
                  button label="Node overview" p=0.0 @ghost_action -> select_node_tab("overview")
                    box px=15.0 py=0.0
                      TabLabel label="Overview" count=0 active=(node_tab == "overview")
                    active bg=transparent text=muted border=transparent border-w=1.0 r=8.0
                    hovered bg=row_hover text=fg
                    pressed bg=elevated text=fg
                  button label="Node permissions" p=0.0 @ghost_action -> select_node_tab("permissions")
                    box px=15.0 py=0.0
                      TabLabel label="Permissions" count=0 active=(node_tab == "permissions")
                    active bg=transparent text=muted border=transparent border-w=1.0 r=8.0
                    hovered bg=row_hover text=fg
                    pressed bg=elevated text=fg
                  button label="Node activity" p=0.0 @ghost_action -> select_node_tab("activity")
                    box px=15.0 py=0.0
                      TabLabel label="Activity" count=0 active=(node_tab == "activity")
                    active bg=transparent text=muted border=transparent border-w=1.0 r=8.0
                    hovered bg=row_hover text=fg
                    pressed bg=elevated text=fg
                match node_tab
                  "permissions"
                    col w=fill gap=18.0
                      NodeAccessCard tier=member_tier(members_rows) admin=members_is_admin(members_rows)
                      PermissionMatrix tier=member_tier(members_rows)
                  "activity"
                    col w=fill gap=9.0
                      row w=fill gap=10.0 align=center
                        GroupLabel label="LOG RING"
                        space w=fill
                        input "" #log-filter label="Filter logs" <-> node_log_filter change=node_log_filter_changed hint="filter logs…" w=200.0 p=6.2 text-size=13.0 line-h=1.2 @control
                          active bg=surface border=border value=fg placeholder=hint selection=fg/18 border-w=1.0 r=8.0
                          hovered bg=muted_bg border=control_line
                      box w=fill h=420.0
                        NodeLogConsole source=settings_endpoint
                          col w=fill gap=5.0
                            if empty(node_log_lines)
                              text "Waiting for the node's log ring…" size=12.0 wrap=none font=code @text-input
                            for line in filter_log_lines(node_log_lines, node_log_filter)
                              LogLine parts=split_log_line(line.line)
                  _
                    col w=fill gap=13.0
                      GroupLabel label="NETWORK"
                      // Three readings this node can actually prove. The artifact's
                      // FINALITY (ms) and ROUND cards are omitted: /v1/status
                      // publishes neither, and `view`/`quorum` are absent on a
                      // non-validator rather than filled with a misleading zero.
                      grid min-cell=170.0 gap=10.0
                        StatCard label="HEIGHT" value=height_label_short(block_height) note=""
                        StatCard label="CHECKPOINT" value=height_label_short(node_checkpoint) note=""
                        StatCard label="LAST FINALIZED" value=relative_time(node_last_finalized) note=""
                      if members_is_admin(members_rows)
                        grid min-cell=170.0 gap=10.0
                          StatCard label="VALIDATORS REACHED" value=tally_label(node_reachable, node_quorum) note="of quorum"
                      GroupCard
                        col w=fill
                          KeyValueRow label="App hash" value=node_root_hash last=true
                      if !empty(node_peers)
                        col w=fill gap=9.0
                          GroupLabel label="PEERS"
                          GroupCard
                            col w=fill
                              for peer in node_peers
                                box w=fill px=15.0 py=11.0
                                  row w=fill gap=8.0 align=center
                                    if peer.live
                                      Dot plate=7.0
                                    if !peer.live
                                      box w=7.0 h=7.0 bg=presence_off r=3.5
                                        space w=1.0 h=1.0
                                    text peer.key w=fill size=12.0 wrap=none font=code @text-fg
                                    text peer.height size=12.0 wrap=none font=code @text-muted
        explorer:
          col w=fill h=fill
            col w=fill pl=24.0 pr=24.0 pt=22.0 gap=16.0
              ScreenTitle title="Explorer" detail="Search everything this workspace has recorded, or read the blocks this node verified for itself — newest first, each one openable for the ops it carried."
              // THE QUERY BOX, on the artifact's own 1.5px ink outline. The
              // seven kind chips under it are OMITTED, not faked: filtering
              // needs a `filter_explorer_hits(hits, kind)` the backend does not
              // have, and `ExplorerResults.kinds` has nowhere to land.
              box w=fill max-w=860.0
                row w=fill gap=10.0 align=center
                  box w=fill pl=14.0 pr=14.0 pt=2.0 pb=2.0 bg=surface border=primary border-w=1.5 r=11.0
                    row w=fill gap=10.0 align=center
                      Icon name="search" tone="label" px=16.0
                      input "" #explorer-search label="Search this workspace" <-> explorer_query hint="Search messages, pages, issues, files, runs…" disabled=(!connected || explorer_searching) submit=explorer_search_submit w=fill p=6.2 text-size=13.0 line-h=1.2 @control
                        active bg=transparent border=transparent value=fg placeholder=hint selection=fg/18 border-w=0.0 r=0.0
                        hovered bg=transparent border=transparent
                        disabled value=muted
                      if !empty(trim(explorer_query))
                        button label="Clear workspace search" w=22.0 h=22.0 p=0.0 @icon_action -> clear_explorer_search
                          box w=fill h=fill align-x=center align-y=center
                            text "×" size=14.0 wrap=none @text-muted
                          active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
                          hovered bg=elevated text=fg
                          pressed bg=subtle text=fg
                  if explorer_searching
                    text "Searching…" size=12.5 wrap=none @text-caption
                  if explorer_loading
                    text "Loading…" size=12.5 wrap=none @text-caption
                  button "Refresh" disabled=explorer_loading h=30.0 p=7.0 @outline_action -> refresh_explorer
            col w=fill h=fill p=18.0 gap=11.0
              // RESULTS TAKE THE SCREEN while a query stands; the block ledger
              // is what the screen falls back to. A hit is a READING, not a
              // route: nothing here dispatches on `hit.target` yet, so the card
              // is not wrapped in a button that would go nowhere.
              if !empty(explorer_hits)
                scroll dir=vertical w=fill h=fill
                  box w=fill max-w=860.0
                    col w=fill gap=8.0
                      for hit in explorer_hits
                        ExplorerCard hit=hit
              if empty(explorer_hits) && !explorer_searching && !empty(trim(explorer_query))
                EmptyPlate message="Nothing matched that query in this workspace."
              if empty(explorer_hits) && empty(explorer_blocks) && !explorer_loading && empty(trim(explorer_query))
                EmptyState title="No blocks yet" description="Non-empty blocks appear here as they finalize."
              if empty(explorer_hits) && !empty(explorer_blocks)
                row w=fill h=fill gap=10.0
                  box w=340.0 h=fill p=6.0 bg=muted_bg border=fg/10 border-w=1.0 r=10.0
                    scroll dir=vertical w=fill h=fill
                      col w=fill gap=1.0
                        for block in explorer_blocks
                          button label="Inspect block" w=fill p=6.0 @ghost_action -> select_explorer_block(block.height)
                            row w=fill h=fill gap=8.0 align=center
                              text block.height size=12.0 wrap=none font=code @text-fg
                              text block.hash w=fill size=12.0 wrap=none font=code @text-muted
                              text block.op_count size=12.0 wrap=none font=code @text-muted
                            active bg=transparent text=fg border=transparent border-w=1.0 r=7.0
                            hovered bg=row_hover text=fg
                            pressed bg=accent
                  box w=fill h=fill p=8.0 bg=muted_bg border=fg/10 border-w=1.0 r=10.0
                    stack w=fill h=fill
                      if explorer_selected <= 0
                        EmptyState title="Select a block" description="Its operations and dispatch traces appear here."
                      if explorer_selected > 0
                        scroll dir=vertical w=fill h=fill
                          col w=fill gap=6.0
                            for op in explorer_ops_at(explorer_ops, explorer_selected)
                              box w=fill p=8.0 bg=surface border=fg/10 border-w=1.0 r=9.0
                                col w=fill gap=3.0
                                  row w=fill gap=8.0 align=center
                                    text op.target size=14.0 wrap=none font=display @text-fg
                                    StatusBadge label=op.disposition
                                    space w=fill
                                    text op.op_hash size=12.0 wrap=none font=code @text-muted
                                  row w=fill gap=8.0 align=center
                                    text "by" size=11.0 wrap=none font=code_medium @text-muted
                                    text op.proposer size=12.0 wrap=none font=code @text-muted
                                  if !empty(op.trace)
                                    text op.trace size=12.0 font=code @text-muted
                                  text op.payload size=13.5 @text-fg
        palette:
          stack w=fill h=fill
            // THE CHANNEL MODAL. The artifact picks VISIBILITY here; the chat
            // module has no read-visibility concept at all — `CreateChannel`
            // carries a `PostPolicy` of Open or MembersOnly and nothing else — so
            // the segment picks the POSTING policy and says so, rather than
            // promising a privacy the wire cannot keep.
            // A SCRIM IS NOT A MODAL. A `box bg=scrim` tints the console and
            // captures nothing: the rail, the channel list and the composer
            // behind it all stayed live, and clicking the dim did nothing. The
            // `overlay` widget is the only thing here that takes the pointer
            // and closes on the backdrop.
            overlay when=channel_create_open dismiss=toggle_channel_create backdrop=scrim p=30.0 align-x=center align-y=center
              content
                space w=fill h=fill
              layer
                ModalShell title="Create a channel" width=418.0 #channel-modal
                  close:
                    button label="Close" disabled=(mutation_phase != "idle") w=26.0 h=26.0 p=0.0 @icon_action -> toggle_channel_create
                      box w=fill h=fill align-x=center align-y=center
                        text "×" size=14.0 wrap=none @text-muted
                      active bg=transparent text=muted border=transparent border-w=1.0 r=7.0
                      hovered bg=elevated text=fg
                      pressed bg=subtle text=fg
                  body:
                    col w=fill gap=13.0
                      text "The channel is created immediately — there is no proposal and no approval step." w=fill size=12.0 line-h=1.5 @text-caption
                      col w=fill gap=6.0
                        Eyebrow label="CHANNEL NAME" note=""
                        box w=fill pl=11.0 pr=11.0 pt=2.0 pb=2.0 bg=surface border=primary border-w=1.5 r=9.0
                          row w=fill gap=7.0 align=center
                            text "#" size=14.0 wrap=none font=code_medium @text-label
                            input "" #new-channel label="New channel name" <-> channel_draft hint="design-review" disabled=(loading || mutation_phase != "idle" || !connected) submit=create_channel_submit w=fill p=6.2 text-size=13.0 line-h=1.2 font=code @control
                              active bg=transparent border=transparent value=fg placeholder=hint selection=fg/18 border-w=0.0 r=0.0
                              hovered bg=transparent border=transparent
                              disabled value=muted
                      col w=fill gap=6.0
                        Eyebrow label="POSTING" note=""
                        row w=fill gap=8.0 align=center
                          if !channel_create_members_only
                            box w=fill pl=12.0 pr=12.0 pt=10.0 pb=10.0 bg=muted_bg border=primary border-w=1.5 r=9.0
                              text "Open · any member posts" size=12.0 wrap=none @text-accent_fg
                          if channel_create_members_only
                            button label="Open posting" w=fill p=0.0 @ghost_action -> toggle_channel_create_members_only
                              box w=fill pl=12.0 pr=12.0 pt=10.0 pb=10.0
                                text "Open · any member posts" size=12.0 wrap=none @text-accent_fg
                              active bg=surface text=fg border=border border-w=1.5 r=9.0
                              hovered bg=muted_bg text=fg border=control_line
                              pressed bg=elevated text=fg
                          if channel_create_members_only
                            box w=fill pl=12.0 pr=12.0 pt=10.0 pb=10.0 bg=muted_bg border=primary border-w=1.5 r=9.0
                              text "Members only · added members post" size=12.0 wrap=none @text-accent_fg
                          if !channel_create_members_only
                            button label="Members-only posting" w=fill p=0.0 @ghost_action -> toggle_channel_create_members_only
                              box w=fill pl=12.0 pr=12.0 pt=10.0 pb=10.0
                                text "Members only · added members post" size=12.0 wrap=none @text-accent_fg
                              active bg=surface text=fg border=border border-w=1.5 r=9.0
                              hovered bg=muted_bg text=fg border=control_line
                              pressed bg=elevated text=fg
                      row w=fill gap=8.0 align=center
                        button "Cancel" disabled=(mutation_phase != "idle") w=fill h=38.0 p=9.0 @secondary_action -> toggle_channel_create
                        button "Create →" disabled=(loading || mutation_phase != "idle" || !connected || empty(trim(channel_draft))) w=fill h=38.0 p=9.0 @primary_action -> create_channel_submit
            // THE TOAST HOST, mounted once for the whole app. It rides in this
            // full-window stack because WorkspaceTabs' overlay slots are `palette`
            // and `bell` and a slot takes one child; the palette below is a
            // top-anchored box, so the two never contend for the same pixels.
            if !empty(toast)
              box w=fill h=fill align-x=center align-y=end pb=26.0
                button label="Dismiss" p=0.0 @icon_action -> dismiss_toast
                  Toast.Confirm message=toast tone=toast_tone
                  active bg=transparent text=fg border=transparent border-w=1.0 r=10.0
                  hovered bg=transparent text=fg
                  pressed bg=transparent text=fg
            if palette_open
              box w=fill h=fill align-x=center pt=72.0 bg=scrim
                box w=540.0 p=10.0 bg=surface border=border border-w=1.0 r=14.0 shadow=shadow_modal shadow-y=24.0 shadow-blur=60.0
                  col w=fill gap=8.0
                    input "" #palette-input label="Search everything" <-> palette_draft change=palette_changed hint="Search messages and pages… (Esc closes)" submit=close_palette w=fill p=8.0 text-size=13.0 line-h=1.2 @control
                      active bg=muted_bg border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=9.0
                      hovered bg=elevated border=fg/21
                    if palette_searching
                      text "Searching…" size=12.5 @text-muted
                    if !empty(palette_chat_hits) || !empty(palette_page_hits)
                      scroll dir=vertical w=fill h=380.0
                        col w=fill gap=4.0
                          if !empty(palette_chat_hits)
                            box w=fill pl=4.0
                              text "MESSAGES" size=10.0 font=code_semibold @text-muted
                            col w=fill gap=1.0
                              for hit in palette_chat_hits
                                button label="Open message" w=fill p=6.0 @ghost_action -> open_chat_search_hit(hit.channel_id, hit.root_seq, hit.seq)
                                  col w=fill gap=1.0
                                    text hit.text size=13.0 wrap=none @text-fg
                                    text hit.meta size=11.0 wrap=none font=code_medium @text-muted
                                  active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
                                  hovered bg=row_hover text=fg
                                  pressed bg=accent
                          if !empty(palette_page_hits)
                            box w=fill pl=4.0
                              text "PAGES" size=10.0 font=code_semibold @text-muted
                            col w=fill gap=1.0
                              for hit in palette_page_hits
                                button label="Open page" w=fill p=6.0 @ghost_action -> open_page_search_hit(hit.page_id, hit.block_id)
                                  col w=fill gap=1.0
                                    text hit.text size=13.0 wrap=none @text-fg
                                    text hit.kind size=12.0 wrap=none font=code @text-muted
                                  active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
                                  hovered bg=row_hover text=fg
                                  pressed bg=accent
        bell:
          stack w=fill h=fill
            if bell_open
              button label="Close notifications" w=fill h=fill p=0.0 @icon_action -> close_bell
                space w=fill h=fill
                active bg=transparent border=transparent
            if bell_open
              box w=fill h=fill align-x=end align-y=start pt=44.0 pr=13.0
                box w=342.0 bg=surface border=border border-w=1.0 r=13.0 clip=true shadow=shadow_modal shadow-y=16.0 shadow-blur=40.0
                  col w=fill
                    box w=fill pl=13.0 pr=13.0 pt=11.0 pb=9.0
                      row w=fill gap=8.0 align=center
                        text "Alerts" size=12.5 wrap=none @text-primary
                        text bell_unread size=10.5 wrap=none font=code_medium @text-meta
                        text "unread" size=12.5 wrap=none @text-meta
                        space w=fill
                        button "Mark all read" disabled=(bell_unread <= 0) h=22.0 p=4.0 @ghost_action -> mark_bell_read_submit
                          active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
                          hovered bg=elevated text=brand
                          pressed bg=subtle text=brand
                    box w=fill h=1.0 bg=separator
                      space w=1.0 h=1.0
                    if empty(bell_items)
                      box w=fill p=26.0 align-x=center
                        text "Nothing yet — mentions and deliveries land here." size=12.0 @text-meta
                    if !empty(bell_items)
                      scroll dir=vertical w=fill h=290.0
                        col w=fill p=5.0 gap=1.0
                          for item in bell_items
                            BellRow item=item
