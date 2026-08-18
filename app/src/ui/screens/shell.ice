// SHELL — the operator's two agent surfaces. Raw is a real terminal: the
// provider owns every byte and key. Chat is the durable headless lane: prompts
// become sagas, while their work and final answer read like the ai-chat app.

component ShellModeButton(label:str, value:ShellMode, selected:bool, disabled:bool) -> ShellMode
  col #root
    if selected
      button -> emit(value)
        with
          label=label
          checked=true
          disabled=disabled
          h=32.0
          p=8.0
        text label size=12.0 font=medium
        active bg=primary text=primary_fg border=primary border-w=1.0 r=8.0
        hovered bg=primary_hover text=primary_fg border=primary_hover
        pressed bg=primary text=primary_fg
        disabled bg=muted_bg text=disabled_fg border=transparent border-w=1.0 r=8.0
    if !selected
      button -> emit(value)
        with
          label=label
          checked=false
          disabled=disabled
          h=32.0
          p=8.0
        text label size=12.0 font=medium
        active bg=transparent text=muted border=transparent border-w=1.0 r=8.0
        hovered bg=row_hover text=fg border=border
        pressed bg=elevated text=fg
        disabled bg=muted_bg text=disabled_fg border=transparent border-w=1.0 r=8.0

// The COMPUTE band's one dropdown shape, worn by both of its questions: which
// credential signs the run, and which peer executes it.
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

component ShellProviderButton(label:str, value:str, selected:bool, disabled:bool) -> str
  col #root
    if selected
      button -> emit(value)
        with
          label=label
          checked=true
          disabled=disabled
          h=34.0
          p=8.0
        row gap=7.0 align=center
          box w=7.0 h=7.0 bg=brand r=3.5
            space w=1.0 h=1.0
          text label size=12.5 font=medium
        active bg=brand_bg text=brand border=brand_line border-w=1.0 r=8.0
        hovered bg=brand_wash text=brand border=brand
        disabled bg=muted_bg text=disabled_fg border=border border-w=1.0 r=8.0
    if !selected
      button -> emit(value)
        with
          label=label
          checked=false
          disabled=disabled
          h=34.0
          p=8.0
        row gap=7.0 align=center
          box w=7.0 h=7.0 bg=hint r=3.5
            space w=1.0 h=1.0
          text label size=12.5 font=medium
        active bg=surface text=muted border=control_line border-w=1.0 r=8.0
        hovered bg=row_hover text=fg border=control_line_hover
        disabled bg=muted_bg text=disabled_fg border=border border-w=1.0 r=8.0

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

component ShellAnswer(entry:AgentChatEntry, dark:bool) -> str
  col #root w=fill gap=7.0
    row gap=8.0 align=center
      AgentAvatar initials=agent_provider_initial(entry.provider) plate=24.0 ink=10.0
      text agent_provider_label(entry.provider)
        with
          size=11.5
          font=display
          @text-meta
    box pl=32.0 w=fill
      extern agent_markdown(entry.body, dark) #answer -> emit(_)

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

component ShellWelcome(provider:str) -> str
  col #root
    with
      w=fill
      py=56.0
      gap=14.0
      align=center
    AgentAvatar initials=agent_provider_initial(provider) plate=46.0 ink=18.0
    text "What are we working on?"
      with
        size=20.0
        font=display
        @text-primary
    box max-w=520.0
      text "Ask an agent on this network. The run is durable, its work streams here, and the committed answer stays in the conversation."
        with
          size=12.5
          line-h=1.5
          align-x=center
          @text-caption
    row gap=8.0 wrap
      button "Explain this system" @secondary_action -> emit("Explain this system's architecture and the most important execution path.")
      button "Inspect a failure" @secondary_action -> emit("Help me diagnose the most likely cause of the current failure.")

component ShellScreen(mode:ShellMode, provider:str, credential_options:[str], credential:str, host_node_options:[str], host_node:str, credentials_loading:bool, terminal:AgentTerminalSession, terminal_running:bool, terminal_busy:bool, terminal_title:str, terminal_error:str, entries:[AgentChatEntry], activity:[AgentActivity], bind draft:editor, chat_busy:bool, chat_status:str, chat_detail:str, live:str, chat_error:str, saga_id:str, connected:bool, dark:bool)
  emits
    shell_mode_changed(ShellMode)
    shell_provider_changed(str)
    shell_credential_changed(str)
    shell_host_node_changed(str)
    shell_credentials_refresh()
    shell_terminal_start()
    shell_terminal_stop()
    shell_composer_event(ComposerEvent)
    shell_chat_reset()
    shell_chat_suggest(str)
    shell_open_link(str)
  col #root w=fill h=fill
    // Calm chrome: one title line, one compact two-way mode switch. Provider
    // controls get their own lower band because they affect both surfaces.
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
          text "Raw provider terminal or durable agent conversation" size=11.5 @text-caption
        box
          with
            p=3.0
            bg=muted_bg
            border=border
            border-w=1.0
            r=11.0
          row gap=2.0
            ShellModeButton #raw-mode -> emit(shell_mode_changed, _)
              with
                label="Raw shell"
                value=ShellMode.raw
                selected=(mode == ShellMode.raw)
                disabled=(terminal_busy || terminal_running || chat_busy)
            ShellModeButton #chat-mode -> emit(shell_mode_changed, _)
              with
                label="Agent chat"
                value=ShellMode.chat
                selected=(mode == ShellMode.chat)
                disabled=(terminal_busy || terminal_running || chat_busy)
    box w=fill h=1.0 bg=separator
      space w=1.0 h=1.0
    box
      with
        w=fill
        px=22.0
        py=10.0
        bg=bg_wash
      col w=fill gap=7.0
        row w=fill gap=12.0 align=center
          text "COMPUTE" size=9.0 font=code_semibold @text-label
          ShellProviderButton #codex-provider -> emit(shell_provider_changed, _)
            with
              label="Codex"
              value="codex"
              selected=(provider == "codex")
              disabled=(terminal_running || terminal_busy || chat_busy)
          ShellProviderButton #claude-provider -> emit(shell_provider_changed, _)
            with
              label="Claude"
              value="claude"
              selected=(provider == "claude")
              disabled=(terminal_running || terminal_busy || chat_busy)
          box w=1.0 h=24.0 bg=separator
            space w=1.0 h=1.0
          text "CREDENTIAL" size=9.0 font=code_semibold @text-label
          ShellPick #credential -> emit(shell_credential_changed, _)
            with
              options=credential_options
              selected=credential
              hint="Choose credential"
              width=238.0
              disabled=(!connected || terminal_running || terminal_busy || chat_busy || credentials_loading || empty(credential_options))
          box w=1.0 h=24.0 bg=separator
            space w=1.0 h=1.0
          text "HOST" size=9.0 font=code_semibold @text-label
          ShellPick #host-node -> emit(shell_host_node_changed, _)
            with
              options=host_node_options
              selected=host_node
              hint="This node"
              width=200.0
              disabled=(!connected || terminal_running || terminal_busy || chat_busy || credentials_loading)
          space w=fill
          button "Refresh" disabled=(!connected || credentials_loading || terminal_busy || chat_busy) @ghost_action -> emit(shell_credentials_refresh)
        if credentials_loading
          row gap=7.0 align=center
            box w=6.0 h=6.0 bg=hint r=3.0
              space w=1.0 h=1.0
            text "Loading registered credentials…" size=10.5 @text-meta
        if !credentials_loading && empty(credential_options)
          row gap=7.0 align=center
            box w=6.0 h=6.0 bg=warning_dot r=3.0
              space w=1.0 h=1.0
            text "No credential is registered for this provider." size=10.5 @text-meta
            text agent_register_hint(provider)
              with
                size=10.5
                font=code
                @text-warning
    box w=fill h=1.0 bg=separator
      space w=1.0 h=1.0

    if !connected
      box w=fill h=fill align-x=center align-y=center
        EmptyState title="Not connected" description="Click the network name in the titlebar to pick or reconnect a network."

    if connected && mode == ShellMode.raw
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
                text "No active raw session" size=12.5 font=medium @text-fg
              text "Keystrokes and resize events pass directly to the provider PTY." size=10.5 @text-meta
            if !terminal_running
              button "Start session" disabled=(!connected || terminal_busy) @primary_action -> emit(shell_terminal_start)
            if terminal_running
              button "End session" @secondary_action -> emit(shell_terminal_stop)
        if !empty(terminal_error)
          box w=fill px=22.0 pt=10.0
            Alert.Destructive title="The terminal did not start" description=terminal_error
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
                  text "Start a session to use the provider's native terminal experience." size=12.0 @text-muted
                  space w=1.0 h=fill

    if connected && mode == ShellMode.chat
      col w=fill h=fill
        if !empty(chat_error)
          box w=fill px=22.0 pt=12.0
            Alert.Destructive title="That turn did not finish" description=chat_error
        scroll #transcript
          with
            w=fill
            h=fill
            anchor-y=end
          box w=fill px=22.0 py=26.0 align-x=center
            box w=fill max-w=780.0
              col w=fill gap=20.0
                if empty(entries) && !chat_busy
                  ShellWelcome #welcome provider=provider -> emit(shell_chat_suggest, _)
                keyed entry in entries by=entry.id #entries
                  with
                    w=fill
                    gap=20.0
                    virtual-row=64.0
                  col w=fill
                    if entry.role == "user"
                      ShellPrompt entry=entry
                    if entry.role == "assistant"
                      ShellAnswer entry=entry dark=dark -> emit(shell_open_link, _)
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
                          text "durable" size=9.5 font=code_medium @text-success
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
                    extern rich_composer(draft, agent_composer_hint(provider), (!connected || chat_busy || empty(credential)), 40.0, 150.0, 8.0) #draft -> emit(shell_composer_event, _)
                    button #send -> emit(shell_composer_event, composer_submit_event())
                      with
                        label="Send"
                        disabled=(!connected || chat_busy || empty(credential) || empty(trim(editor_text(draft))))
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
                row w=fill gap=8.0 align=center
                  text agent_credential_caption(provider, credential)
                    with
                      size=10.5
                      font=code
                      @text-meta
                  space w=fill
                  if !empty(entries)
                    button "New chat" disabled=chat_busy @ghost_action -> emit(shell_chat_reset)
                  if !chat_busy
                    text "Enter to send · Shift+Enter for a new line" size=10.0 @text-hint
