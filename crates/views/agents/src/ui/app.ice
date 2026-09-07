// AGENTS, as a module-owned view: the registry rows the host pushes, listed.
// The plates are the kit's shapes spelled flat in the wire's vocabulary (no
// named fonts, `wrap=none`, line heights or component uses cross the tree
// wire); the theme file is the desktop app's own.
app AgentsView
  title "Agents"
  palette active_palette
  id "dev.ducktape.view.agents"
  text-size 13.5

use "../../../../../app/src/ui/theme.ice"

extern crate::host
  HostError(message:str)
  AgentRow(id:str, name:str, initials:str, capability:str, status:str, owner_handle:str, live:bool, skill_count:i64, cap_count:i64)
  AgentsProps(rows:[AgentRow], connected:bool, answered:bool, dark:bool)
  stream props() -> AgentsProps ! HostError
  pure agents_summary(connected:bool, rows:&[AgentRow]) -> str

state
  active_palette:palette[AppTheme] = AppTheme.app
  rows:[AgentRow] = []
  connected = false
  answered = false
  host_error = ""

on mount
  stream every props() -> props_changed _ | props_failed _

on props_changed(next)
  rows = next.rows
  connected = next.connected
  answered = next.answered
  active_palette = AppTheme.app
  return if !next.dark
  active_palette = AppTheme.app_dark

on props_failed(error)
  host_error = error.message

view
  box #root
    with
      w=fill
      h=fill
      bg=bg
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
          text "Agents" #title
            with
              size=16.0
              @text-primary
              @font-semibold
          text agents_summary(connected, rows) #meta
            with
              size=12.0
              @text-hint
              @font-mono
          space w=fill
      box
        with
          w=fill
          h=1.0
          bg=separator
        space w=1.0 h=1.0
      // The registry explainer: the model, not a reading, so it stays with
      // the node down.
      box
        with
          w=fill
          px=22.0
          pt=12.0
          pb=10.0
        text "The registry records who may act, what they may do, and under whose grant — every entry here is on chain. The acting itself is recorded separately, as each agent's runs."
          with
            w=fill
            size=12.0
            @text-caption
      box
        with
          w=fill
          h=1.0
          bg=separator
        space w=1.0 h=1.0
      if !empty(host_error)
        text host_error #host-error size=12.0 @text-danger
      // NOT CONNECTED IS NOT EMPTY — the registry lives on chain and nothing
      // read it.
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
      if connected && empty(rows) && answered
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
            text "No model agents configured — models appear here with their capability and grants."
              with
                size=13.0
                @text-meta
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
            // An agent registry row: who it is, what capability it holds,
            // who owns it, and whether it is live.
            for agent in rows
              col w=fill
                box
                  with
                    w=fill
                    pl=14.0
                    pr=14.0
                    pt=13.0
                    pb=13.0
                  row
                    with
                      w=fill
                      gap=13.0
                      align=center
                    box
                      with
                        w=34.0
                        h=34.0
                        align-x=center
                        align-y=center
                        bg=primary
                        r=9.0
                      text agent.initials
                        with
                          size=11.0
                          @text-toast_fg
                          @font-mono
                          @font-semibold
                    col w=fill gap=3.0
                      row
                        with
                          w=fill
                          gap=8.0
                          align=center
                        text agent.name
                          with
                            size=13.5
                            @text-fg
                            @font-semibold
                        box
                          with
                            px=7.0
                            py=2.0
                            bg=elevated
                            r=5.0
                          text agent.capability
                            with
                              size=10.0
                              @text-secondary_fg
                              @font-mono
                              @font-semibold
                      // what it may do, counted — never a comma-joined dump
                      // of grant names
                      row
                        with
                          w=fill
                          gap=5.0
                          align=center
                        text agent.skill_count
                          with
                            size=10.5
                            @text-meta
                            @font-mono
                            @font-medium
                        text "skills ·"
                          with
                            size=10.5
                            @text-meta
                            @font-mono
                            @font-medium
                        text agent.cap_count
                          with
                            size=10.5
                            @text-meta
                            @font-mono
                            @font-medium
                        text "grants · owner"
                          with
                            size=10.5
                            @text-meta
                            @font-mono
                            @font-medium
                        if empty(agent.owner_handle)
                          text "unowned"
                            with
                              size=10.5
                              @text-hint
                              @font-mono
                              @font-medium
                        if !empty(agent.owner_handle)
                          text "@"
                            with
                              size=10.5
                              @text-hint
                              @font-mono
                              @font-medium
                        if !empty(agent.owner_handle)
                          text agent.owner_handle
                            with
                              size=10.5
                              @text-hint
                              @font-mono
                              @font-medium
                    // Standing, in the registry's own words: active on a
                    // success plate, anything else on a warning one.
                    if agent.status == "active"
                      box
                        with
                          px=8.0
                          py=3.0
                          bg=success_bg
                          border=success_line
                          border-w=1.0
                          r=6.0
                        row gap=5.0 align=center
                          box
                            with
                              w=5.0
                              h=5.0
                              bg=success_dot
                              r=2.5
                            space w=1.0 h=1.0
                          text "ACTIVE"
                            with
                              size=9.0
                              @text-success
                              @font-mono
                              @font-semibold
                    if agent.status == "paused"
                      box
                        with
                          px=8.0
                          py=3.0
                          bg=warning_bg
                          border=warning_line
                          border-w=1.0
                          r=6.0
                        row gap=5.0 align=center
                          box
                            with
                              w=5.0
                              h=5.0
                              bg=warning_dot
                              r=2.5
                            space w=1.0 h=1.0
                          text "PAUSED"
                            with
                              size=9.0
                              @text-warning
                              @font-mono
                              @font-semibold
                    if agent.status != "active" && agent.status != "paused"
                      box
                        with
                          px=8.0
                          py=3.0
                          bg=warning_bg
                          border=warning_line
                          border-w=1.0
                          r=6.0
                        row gap=5.0 align=center
                          box
                            with
                              w=5.0
                              h=5.0
                              bg=warning_dot
                              r=2.5
                            space w=1.0 h=1.0
                          text agent.status
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
