// The two ROSTER screens: who may act on this network, and what the registry
// says they may do.
//
// A screen is a component like any other, which means it cannot reach app state
// — every reading it draws arrives as a prop, and every act it offers leaves as
// a named event that `view.ice` routes back to the handler of the same name.
// That is the whole contract; the bodies below are the ones that used to sit
// inline in the view's `members:` and `agents:` slots, unchanged.

component MembersScreen(rows:[MemberRow], validators:i64, residents:i64, filter:str, selected:str, admin:bool, connected:bool, answered:bool)
  emits
    pick_members_filter(str)
    open_member(str)
    copy_to_clipboard(str, str)
    agent_set_status(str, bool)
    gov_propose(str, str)
  row w=fill h=fill
    col w=fill h=fill
      ScreenHeader title="Members" meta=members_summary(connected, validators, residents)
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
              button -> emit(pick_members_filter, "all")
                with
                  label="Show every member"
                  p=0.0
                  @ghost_action
                FilterChip
                  with
                    label="All"
                    count=len(rows)
                    selected=(filter == "all")
                active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
                hovered bg=row_hover text=fg
                pressed bg=elevated text=fg
              button -> emit(pick_members_filter, "humans")
                with
                  label="Show people only"
                  p=0.0
                  @ghost_action
                FilterChip
                  with
                    label="Humans"
                    count=len(filter_members(rows, "humans"))
                    selected=(filter == "humans")
                active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
                hovered bg=row_hover text=fg
                pressed bg=elevated text=fg
              button -> emit(pick_members_filter, "agents")
                with
                  label="Show agents only"
                  p=0.0
                  @ghost_action
                FilterChip
                  with
                    label="Agents"
                    count=len(filter_members(rows, "agents"))
                    selected=(filter == "agents")
                active bg=transparent text=fg border=transparent border-w=1.0 r=8.0
                hovered bg=row_hover text=fg
                pressed bg=elevated text=fg
              button -> emit(pick_members_filter, "validators")
                with
                  label="Show validators only"
                  p=0.0
                  @ghost_action
                FilterChip
                  with
                    label="Validators"
                    count=len(filter_members(rows, "validators"))
                    selected=(filter == "validators")
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
        scroll
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
                  button -> emit(open_member, member.key)
                    with
                      label="Open member"
                      description=member.label
                      w=fill
                      p=0.0
                      @ghost_action
                    MemberRowCard member=member
                    active bg=elevated text=fg border=transparent border-w=1.0 r=9.0
                    hovered bg=elevated text=fg
                    pressed bg=subtle text=fg
                if member.key != selected
                  button -> emit(open_member, member.key)
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
            forward
              open_member
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
            message="No agents registered — a registered agent appears here with its capability and grants."
    if connected && !empty(rows)
      scroll
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
