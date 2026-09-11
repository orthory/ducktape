// MEMBERS, as a module-owned view. The kernel pushes session facts only
// (`session()` — one item per change); the roster is read here through
// `rpc.status` / `rpc.peers` / `rpc.query`, re-read on every valset block
// (`rpc.live`), filtered and listed, with the one record the reader opened
// beside it — and its writes leave as `op.submit` the kernel signs. The
// chain's own words throughout — validator / resident / agent — and the
// plates are the kit's shapes spelled flat in the wire's vocabulary (no
// named fonts, `wrap=none`, line heights or component uses cross the tree
// wire); the theme file is the desktop app's own.
app MembersView
  title "Members"
  palette active_palette
  id "dev.ducktape.view.members"
  text-size 13.5

use "../../../../../app/src/ui/theme.ice"

enum MembersFilter
  all
  humans
  agents
  validators

extern crate::host
  MemberRow(key:str, label:str, role:str, is_this_node:bool, is_agent:bool, model:str, live:bool)
  Session(connected:bool, admin:bool, dark:bool)
  SessionItem(next:Session, error:str)
  RosterItem(rows:[MemberRow], height:i64, error:str)
  ActItem(key:str, error:str)
  subscription session() -> SessionItem
  // the roster, read by this view: once per connection, then again on
  // every valset block
  subscription roster(connection:i64) -> RosterItem
  // every write's outcome, as the kernel answers it
  subscription acts() -> ActItem
  pure connection_serial_after(was_connected:bool, connected:bool, serial:i64) -> i64
  pure members_summary(connected:bool, rows:&[MemberRow]) -> str
  pure filter_members(rows:&[MemberRow], filter:MembersFilter) -> [MemberRow]
  pure initials_of(name:&str) -> str
  pure initial_of(name:&str) -> str
  pure member_width_after_delta(width:f64, delta:f64, viewport:f64) -> f64
  sync copy(text:&str, label:&str) -> bool
  sync agent_status(agent_id:&str, paused:bool) -> bool
  sync propose(action:&str, key:&str, height:i64) -> bool

state
  active_palette:palette[AppTheme] = AppTheme.app
  rows:[MemberRow] = []
  admin = false
  connected = false
  // moves when the session comes up: the roster is read afresh
  connection_serial:i64 = 0
  answered = false
  host_error = ""
  filter:MembersFilter = MembersFilter.all
  selected = ""
  // the node height the roster was read at — what a ballot's proposal id
  // is minted from
  height:i64 = 0
  // the member a signed write is in flight for, or empty
  acting = ""
  // a clipboard write's acknowledgement — an intent returns nothing to bind
  sent = false
  viewport_width = 1280.0
  member_width = 312.0

// Subscriptions, not mount tasks, so a replacement restored from this
// view's state asks for the session and the roster again on its own.
subscribe
  session() -> session_arrived _
  roster(connection_serial) when connected -> roster_arrived _
  acts() -> act_done _

on session_arrived(item)
  host_error = item.error
  return if !empty(item.error)
  let next = item.next
  connection_serial = connection_serial_after(connected, next.connected, connection_serial)
  admin = next.admin
  connected = next.connected
  active_palette = AppTheme.app
  return if !next.dark
  active_palette = AppTheme.app_dark

on roster_arrived(item)
  host_error = item.error
  answered = true
  return if !empty(item.error)
  rows = item.rows
  height = item.height

on act_done(item)
  acting = ""
  host_error = item.error

on pick_filter(next)
  filter = next

on open_member(key)
  selected = key

on copy_key(text, label)
  sent = copy(text, label)

on member_resized(dx, _dy)
  member_width = member_width_after_delta(member_width, -dx, viewport_width)

on viewport_changed(width, _height)
  viewport_width = width
  member_width = member_width_after_delta(member_width, 0.0, width)

// One signed write at a time: the kernel's answer frees the next.
on set_agent_status(agent_id, paused)
  return if !connected || !empty(acting)
  acting = agent_id
  let _sent = agent_status(agent_id, paused)

// A membership move is a BALLOT: this opens the proposal, the network
// settles it on the Approvals screen.
on open_ballot(action, key)
  return if !connected || !admin || !empty(acting)
  acting = key
  let _sent = propose(action, key, height)

view
  stack #root w=fill h=fill
    box w=fill h=fill bg=bg
      space w=1.0 h=1.0
    sensor show=viewport_changed resize=viewport_changed
      space w=fill h=fill
    row w=fill h=fill
      col w=fill h=fill
        box
          with
            w=fill
            h=56.0
            px=22.0
          row
            with
              w=fill
              h=fill
              gap=10.0
              align=center
            text "Members" #title
              with
                size=16.0
                @text-primary
                @font-semibold
            text members_summary(connected, rows) #meta
              with
                size=12.0
                @text-hint
                @font-mono
            space w=fill
        // All / Humans / Agents / Validators. Every chip carries a count, and
        // a count is a reading, so the strip stands down with the node.
        if connected
          box
            with
              w=fill
              pl=22.0
              pr=22.0
              pt=12.0
              pb=12.0
            row
              with
                w=fill
                gap=7.0
                align=center
              if filter == MembersFilter.all
                button label="Show every member" p=0.0 -> pick_filter(MembersFilter.all)
                  box
                    with
                      px=11.0
                      py=6.0
                      bg=primary
                      border=primary
                      border-w=1.0
                      r=8.0
                    row gap=6.0 align=center
                      text "All" size=12.0 @text-primary_fg
                      text len(rows)
                        with
                          size=10.0
                          @text-meta
                          @font-mono
                          @font-semibold
                  active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
                  hovered bg=row_hover text=fg
              if filter != MembersFilter.all
                button label="Show every member" p=0.0 -> pick_filter(MembersFilter.all)
                  box
                    with
                      px=11.0
                      py=6.0
                      bg=surface
                      border=border
                      border-w=1.0
                      r=8.0
                    row gap=6.0 align=center
                      text "All" size=12.0 @text-fg
                      text len(rows)
                        with
                          size=10.0
                          @text-meta
                          @font-mono
                          @font-semibold
                  active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
                  hovered bg=row_hover text=fg
              if filter == MembersFilter.humans
                button label="Show people only" p=0.0 -> pick_filter(MembersFilter.humans)
                  box
                    with
                      px=11.0
                      py=6.0
                      bg=primary
                      border=primary
                      border-w=1.0
                      r=8.0
                    row gap=6.0 align=center
                      text "Humans" size=12.0 @text-primary_fg
                      text len(filter_members(rows, MembersFilter.humans))
                        with
                          size=10.0
                          @text-meta
                          @font-mono
                          @font-semibold
                  active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
                  hovered bg=row_hover text=fg
              if filter != MembersFilter.humans
                button label="Show people only" p=0.0 -> pick_filter(MembersFilter.humans)
                  box
                    with
                      px=11.0
                      py=6.0
                      bg=surface
                      border=border
                      border-w=1.0
                      r=8.0
                    row gap=6.0 align=center
                      text "Humans" size=12.0 @text-fg
                      text len(filter_members(rows, MembersFilter.humans))
                        with
                          size=10.0
                          @text-meta
                          @font-mono
                          @font-semibold
                  active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
                  hovered bg=row_hover text=fg
              if filter == MembersFilter.agents
                button label="Show agents only" p=0.0 -> pick_filter(MembersFilter.agents)
                  box
                    with
                      px=11.0
                      py=6.0
                      bg=primary
                      border=primary
                      border-w=1.0
                      r=8.0
                    row gap=6.0 align=center
                      text "Agents" size=12.0 @text-primary_fg
                      text len(filter_members(rows, MembersFilter.agents))
                        with
                          size=10.0
                          @text-meta
                          @font-mono
                          @font-semibold
                  active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
                  hovered bg=row_hover text=fg
              if filter != MembersFilter.agents
                button label="Show agents only" p=0.0 -> pick_filter(MembersFilter.agents)
                  box
                    with
                      px=11.0
                      py=6.0
                      bg=surface
                      border=border
                      border-w=1.0
                      r=8.0
                    row gap=6.0 align=center
                      text "Agents" size=12.0 @text-fg
                      text len(filter_members(rows, MembersFilter.agents))
                        with
                          size=10.0
                          @text-meta
                          @font-mono
                          @font-semibold
                  active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
                  hovered bg=row_hover text=fg
              if filter == MembersFilter.validators
                button label="Show validators only" p=0.0 -> pick_filter(MembersFilter.validators)
                  box
                    with
                      px=11.0
                      py=6.0
                      bg=primary
                      border=primary
                      border-w=1.0
                      r=8.0
                    row gap=6.0 align=center
                      text "Validators" size=12.0 @text-primary_fg
                      text len(filter_members(rows, MembersFilter.validators))
                        with
                          size=10.0
                          @text-meta
                          @font-mono
                          @font-semibold
                  active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
                  hovered bg=row_hover text=fg
              if filter != MembersFilter.validators
                button label="Show validators only" p=0.0 -> pick_filter(MembersFilter.validators)
                  box
                    with
                      px=11.0
                      py=6.0
                      bg=surface
                      border=border
                      border-w=1.0
                      r=8.0
                    row gap=6.0 align=center
                      text "Validators" size=12.0 @text-fg
                      text len(filter_members(rows, MembersFilter.validators))
                        with
                          size=10.0
                          @text-meta
                          @font-mono
                          @font-semibold
                  active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
                  hovered bg=row_hover text=fg
              space w=fill
        box
          with
            w=fill
            h=1.0
            bg=separator
          space w=1.0 h=1.0
        if !empty(host_error)
          text host_error #host-error size=12.0 @text-danger
        // NOT CONNECTED IS NOT EMPTY: `answered` stays true across a node
        // going down, so the empty plate never speaks for an unreadable roster.
        if !connected
          box #offline
            with
              w=fill
              h=fill
              p=22.0
              align-x=center
              align-y=center
            col
              with
                w=fill
                align=center
                gap=7.0
              box
                with
                  w=42.0
                  h=42.0
                  align-x=center
                  align-y=center
                  bg=surface
                  border=border
                  border-w=1.0
                  r=21.0
                text "◇" size=20.0 @text-primary
              text "Not connected"
                with
                  size=16.0
                  @text-fg
                  @font-semibold
              text "Click the network name in the titlebar to pick or reconnect a network."
                with
                  size=12.5
                  @text-caption
        if connected && empty(filter_members(rows, filter)) && answered
          box
            with
              w=fill
              h=fill
              p=22.0
            box #empty
              with
                w=fill
                p=30.0
                align-x=center
                border=border
                border-w=1.0
                r=12.0
              text "No members here yet — validators, residents and registered agents appear as they join."
                with
                  size=13.0
                  @text-meta
        if connected && !empty(filter_members(rows, filter))
          scroll #members-body
            with
              dir=vertical
              w=fill
              h=fill
            col
              with
                w=fill
                pl=12.0
                pr=12.0
                pt=6.0
                pb=6.0
                gap=1.0
              // A roster row: 32px plate, name over key line, role marker.
              // `this node` is the row whose key the client is attached to —
              // not "you"; the reader's own identity is a different key.
              for member in filter_members(rows, filter)
                // the open record's row carries a bar at its edge — the wire
                // has no conditional face, and one button beats two copies
                row w=fill align=center
                  if member.key == selected
                    box
                      with
                        w=3.0
                        h=44.0
                        bg=primary
                        r=1.5
                      space w=1.0 h=1.0
                  if member.key != selected
                    box w=3.0 h=44.0
                      space w=1.0 h=1.0
                  // the row's accessible name is the member's own
                  button -> open_member(member.key)
                    with
                      label=member.label
                      w=fill
                      p=0.0
                    col w=fill
                      box
                        with
                          w=fill
                          pl=14.0
                          pr=14.0
                          pt=12.0
                          pb=12.0
                        row
                          with
                            w=fill
                            gap=12.0
                            align=center
                          if member.is_agent
                            box
                              with
                                w=32.0
                                h=32.0
                                align-x=center
                                align-y=center
                                bg=primary
                                r=8.0
                              text initials_of(member.label)
                                with
                                  size=10.0
                                  @text-toast_fg
                                  @font-mono
                                  @font-semibold
                          if !member.is_agent
                            box
                              with
                                w=32.0
                                h=32.0
                                align-x=center
                                align-y=center
                                bg=avatar_bg
                                r=16.0
                              text initial_of(member.label) size=12.0 @text-avatar_fg
                          col w=fill gap=2.0
                            row gap=6.0 align=center
                              text member.label
                                with
                                  size=13.5
                                  @text-fg
                                  @font-semibold
                              if member.is_this_node
                                text "this node" size=9.5 @text-meta
                            text member.key
                              with
                                size=10.5
                                @text-hint
                                @font-mono
                          if member.role == "validator"
                            box
                              with
                                px=7.0
                                py=3.0
                                bg=primary
                                r=5.0
                              text "VALIDATOR"
                                with
                                  size=9.0
                                  @text-primary_fg
                                  @font-mono
                                  @font-semibold
                          if member.role == "agent"
                            box
                              with
                                px=7.0
                                py=3.0
                                bg=brand_bg
                                border=brand_line
                                border-w=1.0
                                r=5.0
                              text "AGENT"
                                with
                                  size=9.0
                                  @text-brand
                                  @font-mono
                                  @font-semibold
                          if member.role == "resident"
                            box
                              with
                                px=7.0
                                py=3.0
                                bg=surface
                                border=control_line
                                border-w=1.0
                                r=5.0
                              text "RESIDENT"
                                with
                                  size=9.0
                                  @text-input
                                  @font-mono
                                  @font-semibold
                          if member.role != "validator" && member.role != "agent" && member.role != "resident"
                            box
                              with
                                px=7.0
                                py=3.0
                                bg=warning_bg
                                border=warning_line
                                border-w=1.0
                                r=5.0
                              text member.role
                                with
                                  size=9.0
                                  @text-warning
                                  @font-mono
                                  @font-semibold
                      box
                        with
                          w=fill
                          h=1.0
                          bg=muted_bg
                        space w=1.0 h=1.0
                    active bg=transparent text=fg border=transparent border-w=1.0 r=9.0
                    hovered bg=row_hover text=fg
      // The resizable member record. `admin` gates the membership proposals, not
      // the panel: a non-admin still sees WHY the writes are refused.
      if connected && !empty(selected)
        for member in rows
          if member.key == selected
            resize-handle #member-resize drag=member_resized cursor=resize-horizontal
              box #member-divider
                with
                  w=10.0
                  h=fill
                  bg=sidebar
                  align-x=center
                box w=2.0 h=fill bg=separator
                  space w=2.0 h=1.0
            box #member
              with
                w=member_width
                h=fill
                bg=sidebar
              col w=fill h=fill
                box
                  with
                    w=fill
                    h=56.0
                    px=16.0
                  row
                    with
                      w=fill
                      h=fill
                      gap=8.0
                      align=center
                    text "Member"
                      with
                        w=fill
                        size=13.0
                        @text-fg
                    button -> open_member("")
                      with
                        label="Close member"
                        w=24.0
                        h=24.0
                        p=0.0
                      text "×" size=16.0 @text-meta
                      active bg=transparent r=6.0
                      hovered bg=separator
                scroll
                  with
                    dir=vertical
                    w=fill
                    h=fill
                  col
                    with
                      w=fill
                      pl=16.0
                      pr=16.0
                      pt=18.0
                      pb=18.0
                      gap=0.0
                    col
                      with
                        w=fill
                        gap=0.0
                        align=center
                      if member.is_agent
                        box
                          with
                            w=54.0
                            h=54.0
                            align-x=center
                            align-y=center
                            bg=primary
                            r=10.0
                          text initials_of(member.label)
                            with
                              size=16.0
                              @text-toast_fg
                              @font-mono
                              @font-semibold
                      if !member.is_agent
                        box
                          with
                            w=54.0
                            h=54.0
                            align-x=center
                            align-y=center
                            bg=avatar_bg
                            r=27.0
                          text initial_of(member.label) size=20.0 @text-avatar_fg
                      box pt=11.0
                        text member.label
                          with
                            size=16.0
                            @text-fg
                            @font-semibold
                      // Two words per principal: a person is live or offline
                      // on the mesh; an agent is active or paused in the
                      // registry.
                      box pt=5.0
                        row gap=6.0 align=center
                          if member.live
                            box
                              with
                                w=7.0
                                h=7.0
                                bg=success_dot
                                r=3.5
                              space w=1.0 h=1.0
                          if !member.live
                            box
                              with
                                w=7.0
                                h=7.0
                                bg=presence_off
                                r=3.5
                              space w=1.0 h=1.0
                          if member.is_agent && member.live
                            text "active" size=12.0 @text-input
                          if member.is_agent && !member.live
                            text "paused" size=12.0 @text-input
                          if !member.is_agent && member.live
                            text "live" size=12.0 @text-input
                          if !member.is_agent && !member.live
                            text "offline" size=12.0 @text-input
                          if member.role == "validator"
                            box
                              with
                                px=7.0
                                py=3.0
                                bg=primary
                                r=5.0
                              text "VALIDATOR"
                                with
                                  size=9.0
                                  @text-primary_fg
                                  @font-mono
                                  @font-semibold
                          if member.role == "agent"
                            box
                              with
                                px=7.0
                                py=3.0
                                bg=brand_bg
                                border=brand_line
                                border-w=1.0
                                r=5.0
                              text "AGENT"
                                with
                                  size=9.0
                                  @text-brand
                                  @font-mono
                                  @font-semibold
                          if member.role == "resident"
                            box
                              with
                                px=7.0
                                py=3.0
                                bg=surface
                                border=control_line
                                border-w=1.0
                                r=5.0
                              text "RESIDENT"
                                with
                                  size=9.0
                                  @text-input
                                  @font-mono
                                  @font-semibold
                    col
                      with
                        w=fill
                        pt=18.0
                        gap=8.0
                      // an agent's `key` is its REGISTRY ID, which is what
                      // the pause addresses; only a person's row is a node key
                      box
                        with
                          w=fill
                          px=12.0
                          py=9.0
                          bg=surface
                          border=card_line
                          border-w=1.0
                          r=8.0
                        row
                          with
                            w=fill
                            gap=10.0
                            align=start
                          if member.is_agent
                            text "agent id"
                              with
                                size=11.0
                                @text-meta
                                @font-mono
                          if !member.is_agent
                            text "public key"
                              with
                                size=11.0
                                @text-meta
                                @font-mono
                          text member.key
                            with
                              w=fill
                              size=11.0
                              @text-secondary_fg
                              @font-mono
                      if !empty(member.model)
                        box
                          with
                            w=fill
                            px=12.0
                            py=9.0
                            bg=surface
                            border=card_line
                            border-w=1.0
                            r=8.0
                          row
                            with
                              w=fill
                              gap=10.0
                              align=start
                            text "capability"
                              with
                                size=11.0
                                @text-meta
                                @font-mono
                            text member.model
                              with
                                w=fill
                                size=11.0
                                @text-secondary_fg
                                @font-mono
                    col
                      with
                        w=fill
                        pt=16.0
                        gap=8.0
                      // the attached node's VALIDATOR key — what an operator
                      // hands over to be admitted to a valset
                      if member.is_this_node
                        button -> copy_key(member.key, "Node key copied")
                          with
                            label="Copy this node's key"
                            w=fill
                            p=10.0
                          text "Copy this node's key"
                            with
                              w=fill
                              size=12.0
                              @text-accent_fg
                          active bg=surface border=control_line border-w=1.0 r=9.0
                          hovered bg=row_hover
                      // an agent is paused and resumed by its OWNER,
                      // immediately — no ballot
                      if member.is_agent && member.live
                        button -> set_agent_status(member.key, true)
                          with
                            label="Pause agent"
                            w=fill
                            p=10.0
                          text "Pause agent"
                            with
                              w=fill
                              size=12.0
                              @text-accent_fg
                          active bg=surface border=control_line border-w=1.0 r=9.0
                          hovered bg=row_hover
                      if member.is_agent && !member.live
                        button -> set_agent_status(member.key, false)
                          with
                            label="Resume agent"
                            w=fill
                            p=10.0
                          text "Resume agent"
                            with
                              w=fill
                              size=12.0
                              @text-accent_fg
                          active bg=surface border=control_line border-w=1.0 r=9.0
                          hovered bg=row_hover
                      if member.is_agent
                        box
                          with
                            w=fill
                            px=13.0
                            py=11.0
                            bg=warning_bg_lit
                            border=warning_line
                            border-w=1.0
                            r=9.0
                          col w=fill gap=3.0
                            text "Pause and resume are owner-gated writes." size=12.0 @text-fg
                            text "The model accepts changes from its program account or current controller."
                              with
                                size=11.5
                                @text-caption
                      // membership moves are ballots: this opens the
                      // proposal, it does not settle it
                      if admin && !member.is_agent && member.role == "resident"
                        button -> open_ballot("add_validator", member.key)
                          with
                            label="Promote to validator"
                            w=fill
                            p=10.0
                          row
                            with
                              w=fill
                              gap=8.0
                              align=center
                            text "Promote to validator"
                              with
                                w=fill
                                size=12.0
                                @text-accent_fg
                            text "needs quorum"
                              with
                                size=9.0
                                @text-label
                                @font-mono
                                @font-semibold
                          active bg=surface border=control_line border-w=1.0 r=9.0
                          hovered bg=row_hover
                      if admin && !member.is_agent && member.role == "validator" && !member.is_this_node
                        button -> open_ballot("remove_validator", member.key)
                          with
                            label="Remove from the validator set"
                            w=fill
                            p=10.0
                          row
                            with
                              w=fill
                              gap=8.0
                              align=center
                            text "Remove from the validator set"
                              with
                                w=fill
                                size=12.0
                                @text-alert_fg
                            text "needs quorum"
                              with
                                size=9.0
                                @text-label
                                @font-mono
                                @font-semibold
                          active bg=surface border=alert_line border-w=1.0 r=9.0
                          hovered bg=row_hover
                      if !admin && !member.is_this_node && !member.is_agent
                        box
                          with
                            w=fill
                            px=13.0
                            py=11.0
                            bg=warning_bg_lit
                            border=warning_line
                            border-w=1.0
                            r=9.0
                          col w=fill gap=3.0
                            text "Only a validator node may open a membership proposal."
                              with
                                size=12.0
                                @text-fg
                            text "This node holds no quorum seat, so the network refuses the write."
                              with
                                size=11.5
                                @text-caption
