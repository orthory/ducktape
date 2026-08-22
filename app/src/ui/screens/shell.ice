// SHELL — one question the operator actually has ("run an agent on this
// network"), two surfaces that answer it, and no machine vocabulary between
// them.
//
// WHAT THIS SCREEN USED TO ASK, AND WHY IT STOPPED. A permanent COMPUTE band
// asked four questions on every frame — provider, credential, host, refresh —
// of which the CLI only ever asked two: `--cred` DECIDES the provider
// (`agent_cli::resolve_provider` refuses one that contradicts it), so the
// provider buttons were a second answer the operator had to keep consistent by
// hand. The band is now a setup the operator opens, and the header carries the
// one line it settled on.
//
// WHAT A RUN IS, SAID ONCE. A task is a saga: it is submitted, pinned to a
// node, retried up to three times and COMMITTED — it outlives this window. So
// its work belongs to the turn it produced (not to a live-only panel wiped by
// the next prompt), its failure belongs to the turn (not to a banner over the
// whole transcript), and leaving it is "stop watching", never "cancel".

component ShellSurfaceButton(label:str, value:ShellSurface, selected:bool, live:bool) -> ShellSurface
  col #root
    if selected
      button -> emit(value)
        with
          label=label
          checked=true
          h=32.0
          p=8.0
        row gap=6.0 align=center
          text label size=12.0 font=medium
          if live
            box w=6.0 h=6.0 bg=success_dot r=3.0
              space w=1.0 h=1.0
        active bg=primary text=primary_fg border=primary border-w=1.0 r=8.0
        hovered bg=primary_hover text=primary_fg border=primary_hover
        pressed bg=primary text=primary_fg
    if !selected
      button -> emit(value)
        with
          label=label
          checked=false
          h=32.0
          p=8.0
        row gap=6.0 align=center
          text label size=12.0 font=medium
          if live
            box w=6.0 h=6.0 bg=success_dot r=3.0
              space w=1.0 h=1.0
        active bg=transparent text=muted border=transparent border-w=1.0 r=8.0
        hovered bg=row_hover text=fg border=border
        pressed bg=elevated text=fg

// The affordance beside the summary, weighted BELOW it: the line is the
// reading, this is only the way to change it. A `@ghost_action` here put a
// semibold 12.5px "Change" next to an 11.5px caption and won the row.
component ShellSetupToggle(open:bool)
  emits
    shell_setup_toggled
  button #root -> emit(shell_setup_toggled)
    with
      label="Change who runs this work and where"
      p=4.0
    row align=center
      if open
        text "Done" size=11.5 font=medium
      if !open
        text "Change" size=11.5 font=medium
    active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
    hovered bg=row_hover text=fg border=border
    pressed bg=elevated text=fg border=border

// The setup's one dropdown shape, worn by both of its questions: who runs the
// work, and where.
component ShellPick(options:[str], selected:str, hint:str, width:f64, disabled:bool) -> str
  col #root
    if !disabled
      pick options some(selected) -> emit(_)
        with
          hint=hint
          w=width
          p=8.0
          text-size=12.5
        active text=fg handle=muted bg=surface border=control_line border-w=1.0 r=8.0
        hovered text=fg handle=fg bg=surface border=control_line_hover
        opened text=fg handle=fg bg=surface border=brand_line
        opened-hovered text=fg handle=fg bg=surface border=brand
        menu text=fg selected-text=fg selected-bg=selected_row bg=surface border=border border-w=1.0 r=10.0 shadow=shadow_popover shadow-y=6.0 shadow-blur=18.0
        handle dynamic
          closed code="▼" size=11.0
          open code="▲" size=11.0
    if disabled
      box
        with
          w=width
          h=34.0
          px=9.0
          align-y=center
          bg=muted_bg
          border=border
          border-w=1.0
          r=8.0
        row w=fill gap=8.0 align=center
          if empty(selected)
            text hint size=12.5 @text-disabled_fg
          if !empty(selected)
            text selected size=12.5 @text-disabled_fg
          space w=fill
          text "▼" size=10.0 @text-disabled_fg

component ShellPrompt(entry:AgentChatEntry)
  row #root w=fill gap=12.0
    space w=fill h=1.0
    box
      with
        max-w=560.0
        px=15.0
        py=11.0
        bg=muted_bg
        border=border
        border-w=1.0
        r=14.0
      text entry.body
        with
          size=13.5
          line-h=1.48
          wrap=word
          @text-fg

component ShellActivityRow(row:AgentActivity)
  row #root w=fill gap=10.0 align=start
    if row.status == "done"
      box w=20.0 h=20.0 align-x=center align-y=center bg=success_bg r=6.0
        text "✓" size=11.0 font=code_medium @text-success
    if row.status != "done"
      box w=20.0 h=20.0 align-x=center align-y=center bg=warning_bg r=6.0
        text "◌" size=11.0 font=code_medium @text-warning
    col w=fill gap=2.0
      text row.title size=12.0 font=medium @text-fg
      if !empty(row.detail)
        text row.detail
          with
            size=11.0
            font=code
            wrap=word-or-glyph
            @text-meta

// WHAT IT DID, kept with WHAT IT SAID. Folded by default because a settled
// answer is the reading; one click is the whole cost of the audit trail that
// used to be deleted on arrival.
component ShellSteps(entry:AgentChatEntry, open:bool) -> i64
  col #root w=fill gap=8.0 pl=32.0
    button -> emit(entry.id)
      with
        label="Show what the agent did"
        p=0.0
        @ghost_action
      row gap=6.0 align=center
        if open
          text "▾" size=10.0 font=code @text-meta
        if !open
          text "▸" size=10.0 font=code @text-meta
        text entry.steps_label size=11.0 font=code @text-meta
    if open
      col w=fill gap=10.0 pl=4.0
        for step in entry.steps
          ShellActivityRow row=step

component ShellAnswer(entry:AgentChatEntry, open:bool, dark:bool)
  emits
    open_link(str)
    toggle_steps(i64)
  col #root w=fill gap=7.0
    row gap=8.0 align=center
      AgentAvatar initials=agent_provider_initial(entry.provider) plate=24.0 ink=10.0
      text agent_provider_label(entry.provider)
        with
          size=11.5
          font=display
          @text-meta
      if entry.status == "failed"
        Badge.Destructive label="did not finish"
    if entry.status == "done"
      box pl=32.0 w=fill
        extern agent_markdown(entry.body, dark) #answer -> emit(open_link, _)
    if entry.status == "failed"
      box pl=32.0 w=fill
        Alert.Destructive title="That run did not finish" description=entry.body
    if !empty(entry.steps)
      ShellSteps entry=entry open=open -> emit(toggle_steps, _)

// A DETACHED TURN IS NOT A DEAD ONE. The saga is still executing on the node it
// was pinned to; this plate is the address back to it, which is the whole
// difference between a durable run and a lost one.
component ShellDetached(entry:AgentChatEntry, open:bool, connected:bool)
  emits
    reopen_run
    discard_run
    toggle_steps(i64)
  col #root w=fill gap=7.0
    row gap=8.0 align=center
      AgentAvatar initials=agent_provider_initial(entry.provider) plate=24.0 ink=10.0
      text agent_provider_label(entry.provider)
        with
          size=11.5
          font=display
          @text-meta
    box pl=32.0 w=fill
      box
        with
          w=fill
          px=15.0
          py=13.0
          bg=muted_bg
          border=border
          border-w=1.0
          r=11.0
        col w=fill gap=10.0
          text "Still running on the network"
            with
              size=12.5
              font=medium
              @text-fg
          text "You stopped watching this run. It keeps going, retries on failure and commits its answer — reopen it to read the result here."
            with
              size=11.0
              line-h=1.45
              wrap=word
              @text-meta
          row w=fill gap=8.0 align=center
            text agent_run_label(entry.saga_id) size=10.5 font=code @text-meta
            space w=fill
            button "Discard" @ghost_action -> emit(discard_run)
            button "Reopen" disabled=!connected @secondary_action -> emit(reopen_run)
    if !empty(entry.steps)
      ShellSteps entry=entry open=open -> emit(toggle_steps, _)

component ShellWelcome(provider:str, host_node:str)
  col #root
    with
      w=fill
      py=56.0
      gap=14.0
      align=center
    AgentAvatar initials=agent_provider_initial(provider) plate=46.0 ink=18.0
    text "What should the agent do?"
      with
        size=20.0
        font=display
        @text-primary
    box max-w=520.0
      text agent_task_blurb(host_node)
        with
          size=12.5
          line-h=1.5
          align-x=center
          @text-caption

// NO CREDENTIAL IS NOT AN EMPTY LIST, it is one instruction. The screen used to
// answer it with a welcome mat and two suggestion chips that filled a composer
// whose send button could never light up.
component ShellNoCredential(provider:str)
  col #root
    with
      w=fill
      py=56.0
      gap=12.0
      align=center
    text "No credential is registered here"
      with
        size=16.0
        font=display
        @text-primary
    box max-w=520.0
      text "A durable task spends a provider subscription, so it needs one. A terminal still opens without it — the provider will ask you to sign in inside the session."
        with
          size=12.5
          line-h=1.5
          align-x=center
          @text-caption
    box
      with
        px=12.0
        py=8.0
        bg=muted_bg
        border=border
        border-w=1.0
        r=8.0
      text agent_register_hint(provider) size=11.0 font=code @text-warning

component ShellScreen(surface:ShellSurface, setup_open:bool, identity_options:[str], identity:str, provider:str, credential:str, host_node_options:[str], host_node:str, credentials_loading:bool, terminal:AgentTerminalSession, terminal_running:bool, terminal_busy:bool, terminal_title:str, terminal_error:str, entries:[AgentChatEntry], activity:[AgentActivity], bind draft:editor, chat_busy:bool, chat_status:str, chat_detail:str, live:str, saga_id:str, steps_open:i64, detached_saga:str, connected:bool, dark:bool)
  emits
    shell_surface_changed(ShellSurface)
    shell_setup_toggled()
    shell_identity_changed(str)
    shell_host_node_changed(str)
    shell_credentials_refresh()
    shell_terminal_start()
    shell_terminal_stop()
    shell_composer_event(ComposerEvent)
    shell_chat_reset()
    shell_chat_detach()
    shell_chat_reopen()
    shell_chat_discard()
    shell_chat_steps_toggled(i64)
    shell_open_link(str)
  col #root w=fill h=fill
    // ONE header line. The surface switch is never disabled — a run in flight
    // is a reason to keep its inputs stable, not a reason to trap the operator
    // on the surface that started it, and the terminal's dot keeps saying it is
    // live from the other side.
    box
      with
        w=fill
        px=22.0
        py=12.0
        bg=surface
      row w=fill gap=14.0 align=center
        col w=fill gap=2.0
          row gap=9.0 align=center
            Icon name="code-slash" tone="primary" px=18.0
            text "Shell" size=16.0 font=display @text-primary
          row gap=5.0 align=center
            text agent_run_line(identity, host_node) size=11.5 @text-caption
            ShellSetupToggle #setup-toggle open=setup_open
              forward
                shell_setup_toggled
        box
          with
            p=3.0
            bg=muted_bg
            border=border
            border-w=1.0
            r=11.0
          row gap=2.0
            ShellSurfaceButton #tasks-surface -> emit(shell_surface_changed, _)
              with
                label="Tasks"
                value=ShellSurface.tasks
                selected=(surface == ShellSurface.tasks)
                live=chat_busy
            ShellSurfaceButton #terminal-surface -> emit(shell_surface_changed, _)
              with
                label="Terminal"
                value=ShellSurface.terminal
                selected=(surface == ShellSurface.terminal)
                live=terminal_running
    box w=fill h=1.0 bg=separator
      space w=1.0 h=1.0

    // THE SETUP, open on request and while nothing is picked. Two questions,
    // both of them ones the node will actually be asked.
    if setup_open || empty(identity)
      col w=fill
        box #setup
          with
            w=fill
            px=22.0
            py=12.0
            bg=bg_wash
          col w=fill gap=9.0
            row w=fill gap=12.0 align=center
              text "WHO RUNS IT" size=9.0 font=code_semibold @text-label
              ShellPick #identity -> emit(shell_identity_changed, _)
                with
                  options=identity_options
                  selected=identity
                  hint="Choose a credential"
                  width=252.0
                  disabled=(!connected || credentials_loading || empty(identity_options))
              box w=1.0 h=24.0 bg=separator
                space w=1.0 h=1.0
              text "WHERE" size=9.0 font=code_semibold @text-label
              ShellPick #host-node -> emit(shell_host_node_changed, _)
                with
                  options=host_node_options
                  selected=host_node
                  hint="This node"
                  width=200.0
                  disabled=(!connected || credentials_loading)
              space w=fill
              button "Refresh" disabled=(!connected || credentials_loading) @ghost_action -> emit(shell_credentials_refresh)
            if credentials_loading
              row gap=7.0 align=center
                box w=6.0 h=6.0 bg=hint r=3.0
                  space w=1.0 h=1.0
                text "Reading registered credentials and announcing peers…" size=10.5 @text-meta
            if !empty(agent_host_grant_note(host_node, credential))
              row gap=7.0 align=center
                box w=6.0 h=6.0 bg=warning_dot r=3.0
                  space w=1.0 h=1.0
                text agent_host_grant_note(host_node, credential) size=10.5 @text-meta
        box w=fill h=1.0 bg=separator
          space w=1.0 h=1.0

    if !connected
      box w=fill h=fill align-x=center align-y=center
        EmptyState title="Not connected" description="Click the network name in the titlebar to pick or reconnect a network."

    if connected && surface == ShellSurface.terminal
      col w=fill h=fill
        box
          with
            w=fill
            px=22.0
            py=10.0
            bg=surface
          row w=fill gap=10.0 align=center
            if terminal_running
              box w=8.0 h=8.0 bg=success_dot r=4.0
                space w=1.0 h=1.0
            if !terminal_running
              box w=8.0 h=8.0 bg=presence_off r=4.0
                space w=1.0 h=1.0
            col w=fill gap=2.0
              if !empty(terminal_title)
                text terminal_title size=12.5 font=medium @text-fg
              if empty(terminal_title)
                text "No session open" size=12.5 font=medium @text-fg
              text agent_terminal_note(provider, credential) size=10.5 @text-meta
            if !terminal_running
              button "Open session" disabled=terminal_busy @primary_action -> emit(shell_terminal_start)
            if terminal_running
              button "Close session" @secondary_action -> emit(shell_terminal_stop)
        if !empty(terminal_error)
          box w=fill px=22.0 pt=10.0
            Alert.Destructive title="The session did not open" description=terminal_error
        box
          with
            w=fill
            h=fill
            p=14.0
            bg=terminal_bg
          box
            with
              w=fill
              h=fill
              bg=terminal_bg
              border=terminal_line
              border-w=1.0
              r=10.0
              clip=true
              p=6.0
            col w=fill h=fill
              if terminal_running
                extern agent_terminal_surface(terminal) #terminal
              if !terminal_running
                col
                  with
                    w=fill
                    h=fill
                    gap=9.0
                    align=center
                  space w=1.0 h=fill
                  text "▸_" size=22.0 font=code @text-muted
                  text "Open a session to work in the provider's own terminal." size=12.0 @text-muted
                  space w=1.0 h=fill

    if connected && surface == ShellSurface.tasks
      col w=fill h=fill
        scroll #transcript
          with
            w=fill
            h=fill
            anchor-y=end
          box w=fill px=22.0 py=26.0 align-x=center
            box w=fill max-w=780.0
              col w=fill gap=20.0
                if empty(entries) && !chat_busy && empty(credential)
                  ShellNoCredential #no-credential provider=provider
                if empty(entries) && !chat_busy && !empty(credential)
                  ShellWelcome #welcome provider=provider host_node=host_node
                keyed entry in entries by=entry.id #entries
                  with
                    w=fill
                    gap=20.0
                    virtual-row=64.0
                  col w=fill
                    if entry.role == "user"
                      ShellPrompt entry=entry
                    if entry.role != "user" && entry.status == "detached"
                      ShellDetached entry=entry open=(steps_open == entry.id) connected=connected
                        events
                          reopen_run -> emit(shell_chat_reopen)
                          discard_run -> emit(shell_chat_discard)
                          toggle_steps -> emit(shell_chat_steps_toggled, _)
                    if entry.role != "user" && entry.status != "detached"
                      ShellAnswer entry=entry open=(steps_open == entry.id) dark=dark
                        events
                          open_link -> emit(shell_open_link, _)
                          toggle_steps -> emit(shell_chat_steps_toggled, _)
                if chat_busy
                  box #work
                    with
                      w=fill
                      px=15.0
                      py=13.0
                      bg=muted_bg
                      border=border
                      border-w=1.0
                      r=11.0
                    col w=fill gap=10.0
                      row w=fill gap=9.0 align=center
                        text "◌" size=13.0 @text-warning
                        col w=fill gap=1.0
                          text chat_status size=12.5 font=medium @text-fg
                          if !empty(chat_detail)
                            text chat_detail size=10.5 @text-meta
                        if !empty(saga_id)
                          button "Stop watching" @ghost_action -> emit(shell_chat_detach)
                      keyed row in activity by=row.id #activity
                        with
                          w=fill
                          gap=8.0
                        lazy row as settled
                          ShellActivityRow row=settled
                  if chat_busy
                    extern agent_markdown(live, dark) #live-answer -> emit(shell_open_link, _)
        box w=fill h=1.0 bg=separator
          space w=1.0 h=1.0
        box
          with
            w=fill
            px=22.0
            py=14.0
            bg=surface
          box w=fill align-x=center
            box w=fill max-w=780.0
              col w=fill gap=8.0
                box
                  with
                    w=fill
                    px=7.0
                    py=6.0
                    bg=muted_bg
                    border=border
                    border-w=1.0
                    r=15.0
                  row w=fill gap=6.0 align=center
                    extern rich_composer(draft, agent_composer_hint(provider), (!connected || chat_busy || empty(credential) || !empty(detached_saga)), 40.0, 150.0, 8.0) #draft -> emit(shell_composer_event, _)
                    button #send -> emit(shell_composer_event, composer_submit_event())
                      with
                        label="Send"
                        disabled=(!connected || chat_busy || empty(credential) || !empty(detached_saga) || empty(trim(editor_text(draft))))
                        w=32.0
                        h=32.0
                      // Regular weight, deliberately — see the note on the
                      // message toolbar in components/chat.ice: a semibold
                      // string label sends every non-ASCII glyph down
                      // cosmic-text's walk-every-face fallback path.
                      text "↑"
                        with
                          size=12.5
                          font=ui
                      active bg=primary text=primary_fg r=16.0
                      hovered bg=primary_hover text=primary_fg r=16.0
                      disabled bg=disabled text=disabled_fg r=16.0
                // A DISABLED SEND SAYS WHY. The three reasons it can be off are
                // three different things to do next, and the operator used to
                // get a grey circle for all of them.
                row w=fill gap=8.0 align=center
                  if empty(credential)
                    text "Pick a credential above to send a task." size=10.5 @text-meta
                  if !empty(credential) && !empty(detached_saga)
                    text "Reopen or discard the run above before sending another." size=10.5 @text-meta
                  if !empty(credential) && empty(detached_saga) && chat_busy
                    text "A task is running — stop watching it to send another." size=10.5 @text-meta
                  space w=fill
                  // A reset would take the detached run's id with it, and that
                  // id is the only way back to a saga that is still executing.
                  // Reopen or Discard says which — a "New chat" click does not.
                  if !empty(entries)
                    button "New chat" disabled=(chat_busy || !empty(detached_saga)) @ghost_action -> emit(shell_chat_reset)
                  if !chat_busy
                    text "Enter to send · Shift+Enter for a new line" size=10.0 @text-hint
