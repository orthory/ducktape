// The two ROSTER screens: who may act on this network, and what the registry
// says they may do.
//
// A screen is a component like any other: shared readings arrive as props,
// interaction-local state stays here, and only application effects leave as
// named events.

component MembersScreen(rows:[MemberRow], admin:bool, connected:bool, answered:bool)
  lifetime retained
  emits
    copy_to_clipboard(str, str)
    agent_set_status(str, bool)
    gov_propose(str, str)
  state
    filter:MembersFilter = MembersFilter.all
    selected = ""
  on pick_members_filter(next)
    filter = next
  on open_member(key)
    selected = key
  row w=fill h=fill
    col w=fill h=fill
      // THE SUBTITLE FOLDS `rows`, not the valset queries — same list, same
      // split as the Humans / Agents chips below it, so the header and the
      // strip can never disagree about what is on this screen.
      ScreenHeader title="Members" meta=members_summary(connected, rows)
        // NO INVITE BUTTON YET. `mint_invite` exists in the backend, but nothing
        // routes the mint itself, so the button would open a modal with no act
        // in it.
        space w=1.0 h=1.0
      // All / Humans / Agents / Validators. `filter_members` owns the
      // predicate so the strip and the list can never disagree.
      col w=fill
        // EVERY CHIP CARRIES A COUNT, and a count is a reading. With the node
        // down these fold a roster nobody fetched, so the strip stands down
        // with the list it filters rather than offering `All 2` over a plate
        // that says the network is unreachable.
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
              button -> pick_members_filter(MembersFilter.all)
                with
                  label="Show every member"
                  checked=(filter == MembersFilter.all)
                  p=0.0
                  @ghost_action
                FilterChip
                  with
                    label="All"
                    count=len(rows)
                    selected=(filter == MembersFilter.all)
                active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
                hovered bg=row_hover text=fg
                pressed bg=elevated text=fg
              button -> pick_members_filter(MembersFilter.humans)
                with
                  label="Show people only"
                  checked=(filter == MembersFilter.humans)
                  p=0.0
                  @ghost_action
                FilterChip
                  with
                    label="Humans"
                    count=len(filter_members(rows, MembersFilter.humans))
                    selected=(filter == MembersFilter.humans)
                active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
                hovered bg=row_hover text=fg
                pressed bg=elevated text=fg
              button -> pick_members_filter(MembersFilter.agents)
                with
                  label="Show agents only"
                  checked=(filter == MembersFilter.agents)
                  p=0.0
                  @ghost_action
                FilterChip
                  with
                    label="Agents"
                    count=len(filter_members(rows, MembersFilter.agents))
                    selected=(filter == MembersFilter.agents)
                active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
                hovered bg=row_hover text=fg
                pressed bg=elevated text=fg
              button -> pick_members_filter(MembersFilter.validators)
                with
                  label="Show validators only"
                  checked=(filter == MembersFilter.validators)
                  p=0.0
                  @ghost_action
                FilterChip
                  with
                    label="Validators"
                    count=len(filter_members(rows, MembersFilter.validators))
                    selected=(filter == MembersFilter.validators)
                active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
                hovered bg=row_hover text=fg
                pressed bg=elevated text=fg
              space w=fill
        box
          with
            w=fill
            h=1.0
            bg=separator
          space w=1.0 h=1.0
      // NOT CONNECTED IS NOT EMPTY. `answered` is "the roster replied", and it
      // stays true across a node going down — so the plate below went on
      // claiming nobody is on this network off a roster read minutes ago and
      // now unreadable. The header and the filter strip stay: they are how the
      // reader gets back out.
      if !connected
        box
          with
            w=fill
            h=fill
            p=22.0
          EmptyState
            with
              title="Not connected"
              description="Click the network name in the titlebar to pick or reconnect a network."
      if connected && empty(filter_members(rows, filter)) && answered
        box
          with
            w=fill
            h=fill
            p=22.0
          EmptyPlate
            with
              message="No members here yet — validators, residents and registered agents appear as they join."
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
            for member in filter_members(rows, filter)
              col w=fill
                if member.key == selected
                  button -> open_member(member.key)
                    with
                      label="Open member"
                      description=member.label
                      w=fill
                      p=0.0
                      @ghost_action
                    MemberRowCard member=member
                    active bg=selected_row text=fg border=transparent border-w=1.0 r=9.0
                    hovered bg=selected_row text=fg
                    pressed bg=subtle text=fg
                if member.key != selected
                  button -> open_member(member.key)
                    with
                      label="Open member"
                      description=member.label
                      w=fill
                      p=0.0
                      @ghost_action
                    MemberRowCard member=member
                    active bg=transparent text=fg border=transparent border-w=1.0 r=9.0
                    hovered bg=row_hover text=fg
                    pressed bg=elevated text=fg
    if connected && !empty(selected)
      for member in rows
        if member.key == selected
          MemberDetail member=member admin=admin
            events
              open_member -> open_member _
            forward
              copy_to_clipboard
              agent_set_status
              gov_propose

component AgentsScreen(rows:[AgentRow], connected:bool, answered:bool)
  col w=fill h=fill
    ScreenHeader title="Agents" meta=agents_summary(connected, rows)
      space w=1.0 h=1.0
    // The registry explainer. The artifact states the whole model in this
    // one strip and the English UI never did: the registry is the record of
    // WHO may do WHAT under WHICH grant, and the doing itself is recorded
    // separately as that agent's runs.
    col w=fill
      box
        with
          w=fill
          pl=22.0
          pr=22.0
          pt=12.0
          pb=10.0
        text "The registry records who may act, what they may do, and under whose grant — every entry here is on chain. The acting itself is recorded separately, as each agent's runs."
          with
            w=fill
            size=12.0
            line-h=1.5
            @text-caption
      box
        with
          w=fill
          h=1.0
          bg=separator
        space w=1.0 h=1.0
    // NOT CONNECTED IS NOT EMPTY — the registry lives on chain and nothing
    // read it. The explainer strip above stays (it states the MODEL, not a
    // reading); the plate that says the register is empty does not.
    if !connected
      box
        with
          w=fill
          h=fill
          p=22.0
        EmptyState
          with
            title="Not connected"
            description="Click the network name in the titlebar to pick or reconnect a network."
    if connected && empty(rows) && answered
      box
        with
          w=fill
          h=fill
          p=22.0
        EmptyPlate
          with
            message="No model agents configured — models appear here with their capability and grants."
    if connected && !empty(rows)
      scroll #agents-body
        with
          dir=vertical
          w=fill
          h=fill
        col
          with
            w=fill
            p=18.0
            gap=11.0
          for agent in rows
            AgentCard agent=agent
