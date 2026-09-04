// APPROVALS, AS THE MODULE DRAWS THEM. The same register the app's native
// GovernanceScreen shows, read through the app: one `query.governance` for
// the proposals, the colour mode as a stream, and a refresh item whenever the
// app's generation for this module moves. Read-only on purpose — a vote is a
// signed submit, and the pilot stops at the read side.
app Approvals
  title "Approvals"
  palette active_palette
  id "dev.ducktape.governance.view"
  text-size 13.5
  window
    size 960 640

use "theme.ice"

extern crate::host
  HostError(message:str)
  Proposal(id:str, open:bool, status:str, action:str, detail:str, approvals:i64, rejections:i64, required_yes:i64, electorate:i64, deadline:i64)
  load_proposals() -> [Proposal] ! HostError
  stream theme_changes() -> str ! HostError
  stream refreshes() -> bool ! HostError
  pure summary(rows:&[Proposal]) -> str
  pure tally(row:&Proposal) -> str
  pure status_label(row:&Proposal) -> str
  pure has_detail(row:&Proposal) -> bool

state
  rows:[Proposal] = []
  error = ""
  loaded = false
  active_palette:palette[ApprovalsTheme] = ApprovalsTheme.light

on mount
  parallel
    stream every theme_changes() -> themed _ | host_failed _
    stream every refreshes() -> refresh _ | host_failed _
    run every load_proposals() -> proposals_loaded _ | host_failed _

on themed(mode)
  active_palette = ApprovalsTheme.light
  return if mode != "dark"
  active_palette = ApprovalsTheme.dark

on refresh(_moved)
  run replace lane=proposals load_proposals() -> proposals_loaded _ | host_failed _

on proposals_loaded(next)
  rows = next
  loaded = true
  error = ""

on host_failed(cause)
  error = cause.message

view
  box #app w=fill h=fill bg=bg
    col w=fill h=fill
      row #header w=fill p=16.0 gap=10.0 align=center
        text "Approvals" size=18.0 font=strong @text-fg
        text summary(rows) size=12.0 @text-muted
        space w=fill
        text "module view · wasm" size=11.0 @text-primary
      scroll #body dir=vertical w=fill h=fill
        col w=fill p=22.0 gap=12.0
          if error != ""
            text error size=12.5 @text-danger
          if loaded && empty(rows)
            text "Nothing to decide." size=12.5 @text-muted
          for row in rows
            box w=fill bg=surface border=border border-w=1.0 r=10.0 p=14.0
              col gap=6.0
                row gap=8.0 align=center
                  text row.action size=13.5 font=strong @text-fg
                  text status_label(row) size=11.0 @text-muted
                if has_detail(row)
                  text row.detail size=12.5 @text-fg
                text tally(row) size=12.0 @text-muted
