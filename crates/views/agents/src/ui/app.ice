// AGENTS, as a module-owned view: the register the host pushes, listed, with
// the one record the reader opened beside it as an editor. The record is the
// row: name, executor, the action grant, the resource caps, the curated
// skills and the standing, every one editable by the account that controls
// it and read-only for everyone else. Drafts are the view's; a save hands the
// app the whole record and the app signs it. The plates are the kit's shapes
// spelled flat in the wire's vocabulary (no named fonts, `wrap=none`, line
// heights or component uses cross the tree wire); the theme file is the
// desktop app's own.
app AgentsView
  title "Agents"
  palette active_palette
  id "dev.ducktape.view.agents"
  text-size 13.5

use "../../../../../app/src/ui/theme.ice"
use "../../../../../app/src/ui/ducktape-ui/recipes.ice"

extern crate::host
  HostError(message:str)
  AgentSkill(name:str, source_prefix:str, source_snapshot:str, always:bool)
  AgentCaps(forge_read:[str], forge_push:[str], duckfs_read:[str], duckfs_write:[str], tools:[str], secrets:[str], pages_write:[str], subagent_budget:i64)
  AgentRow(id:str, name:str, initials:str, capability:str, status:str, owner_handle:str, controller:str, live:bool, allowed_actions:[str], caps:AgentCaps, skills:[AgentSkill])
  RunRow(run_id:str, dispatch_id:str, agent_id:str, agent_name:str, origin:str, state:str, dispatched:str, settled:str, attempt:i64, holder:str, actions:i64, degraded:bool, reason:str, output_ref:str, pr_number:i64)
  JournalEntry(height:str, kind:str, summary:str, status:str, targets:[RunLink])
  RunLink(relation:str, kind:str, label:str, url:str)
  RunJournal(dispatch_id:str, entries:[JournalEntry], links:[RunLink])
  LiveActivity(label:str, done:bool)
  LiveRun(present:bool, status:str, activity:[LiveActivity], answer_preview:str)
  AgentsProps(rows:[AgentRow], runs:[RunRow], open_run:str, opened:i64, journal:RunJournal, live:LiveRun, capabilities:[str], actions:[str], account:str, committed:i64, connected:bool, answered:bool, dark:bool)
  stream props() -> AgentsProps ! HostError
  pure agents_summary(connected:bool, rows:&[AgentRow]) -> str
  pure runs_summary(runs:&[RunRow]) -> str
  pure run_named(runs:&[RunRow], run_id:&str) -> RunRow
  pure run_at(runs:&[RunRow], dispatch_id:&str) -> RunRow
  pure empty_journal() -> RunJournal
  pure empty_live() -> LiveRun
  pure empty_run() -> RunRow
  pure link_glyph(kind:&str) -> str
  pure journal_width_after_delta(width:f64, delta:f64, viewport:f64) -> f64
  pure open_run(dispatch_id:&str) -> bool
  pure open_link(url:&str) -> bool
  pure cap_count(caps:&AgentCaps) -> i64
  pure skill_count(skills:&[AgentSkill]) -> i64
  pure row_named(rows:&[AgentRow], id:&str) -> AgentRow
  pure editable(connected:bool, account:&str, controller:&str) -> bool
  pure has(list:&[str], item:&str) -> bool
  pure with_flag(list:&[str], item:&str, on:bool) -> [str]
  pure cap_kinds() -> [str]
  pure caps_with(caps:&AgentCaps, kind:&str, entry:&str) -> AgentCaps
  pure caps_without(caps:&AgentCaps, kind:&str, entry:&str) -> AgentCaps
  pure caps_with_budget(caps:&AgentCaps, text:&str) -> AgentCaps
  pure budget_text(budget:i64) -> str
  pure with_skill(skills:&[AgentSkill], name:&str, source_prefix:&str, source_snapshot:&str, always:bool) -> [AgentSkill]
  pure without_skill(skills:&[AgentSkill], name:&str) -> [AgentSkill]
  pure skill_loaded(skills:&[AgentSkill], name:&str, always:bool) -> [AgentSkill]
  pure skill_mode(always:bool) -> str
  pure library_prefix(name:&str) -> str
  pure capability_options(announced:&[str], current:&str) -> [str]
  pure some_str(value:&str) -> str?
  pure or_empty(value:&str?) -> str
  pure pick_str(condition:bool, then:&str, or:&str) -> str
  pure pick_capability(condition:bool, then:&str, or:&str?) -> str?
  pure empty_caps() -> AgentCaps
  pure pick_list(condition:bool, then:&[str], or:&[str]) -> [str]
  pure pick_caps(condition:bool, then:&AgentCaps, or:&AgentCaps) -> AgentCaps
  pure pick_skills(condition:bool, then:&[AgentSkill], or:&[AgentSkill]) -> [AgentSkill]
  pure valid_agent_id(id:&str) -> bool
  pure pane_note(pane:&str) -> str
  pure status(agent_id:&str, paused:bool) -> bool
  pure save(agent_id:&str, display_name:&str, capability:&str, allowed_actions:&[str], caps:&AgentCaps, skills:&[AgentSkill]) -> bool
  pure register(agent_id:&str, display_name:&str, capability:&str, allowed_actions:&[str], caps:&AgentCaps, skills:&[AgentSkill]) -> bool

state
  active_palette:palette[AppTheme] = AppTheme.app
  rows:[AgentRow] = []
  runs:[RunRow] = []
  journal:RunJournal = empty_journal()
  live:LiveRun = empty_live()
  // which panel the reader is on: the registry (who may act) or the runs
  // tracker (what they did, and how it settled). Mutually exclusive by
  // construction — one value, and every panel's content is gated on it.
  panel = "registry"
  // the run open in the tracker, by dispatch id; "" is none — and its row,
  // as the register last listed it. The app owns which run is open (a chat
  // hint, a bell or a duck://run link opens one from another tab), so the
  // register's `open_run` is the truth and a press here is the request.
  open_run = ""
  journal_width = 400.0
  viewport_width = 1280.0
  expanded_receipt = ""
  open_row:RunRow = empty_run()
  // the doors the app has opened a run through, counted, as the register
  // last carried it; a bump is a door pressed since
  opened:i64 = 0
  capabilities:[str] = []
  actions:[str] = []
  account = ""
  committed:i64 = 0
  connected = false
  answered = false
  host_error = ""
  // the record open in the editor, by registry id; "" is none
  selected = ""
  // the New agent form is open instead of a record
  creating = false
  // whether the signing account controls the open record
  can_edit = false
  // the open record's standing, as the register last said it
  selected_status = ""
  // the editor's drafts — the reader's own until a commit consumes them
  draft_id = ""
  draft_name = ""
  draft_capability:str? = none
  draft_actions:[str] = []
  draft_caps:AgentCaps = empty_caps()
  draft_budget = ""
  draft_skills:[AgentSkill] = []
  // the "add a grant" row
  cap_kind:str? = some("forge_read")
  cap_entry = ""
  // the "add a skill" row
  skill_name = ""
  skill_prefix = ""
  skill_snapshot = ""
  skill_always = false
  // a write's acknowledgement — `host::notify` returns nothing to bind, and
  // the host's answer arrives as the next register
  sent = false

on mount
  stream every props() -> props_changed _ | props_failed _

on journal_resized(dx, _dy)
  journal_width = journal_width_after_delta(journal_width, -dx, viewport_width)

on viewport_changed(width, _height)
  viewport_width = width
  journal_width = journal_width_after_delta(journal_width, 0.0, width)

on toggle_receipt(value)
  expanded_receipt = pick_str(expanded_receipt != value, value, "")

// The register moved. A bumped `committed` means the app signed a write off
// these drafts: the New form closes onto the agent it registered, and an
// open record re-seeds from its fresh row — the drafts were consumed, the
// row is now the truth.
on props_changed(next)
  rows = next.rows
  runs = next.runs
  journal = next.journal
  live = next.live
  open_run = next.open_run
  open_row = run_at(runs, open_run)
  // A DOOR LANDS THE READER ON THE TRACKER. A run opened from another tab —
  // a chat hint, a bell, a link — is shown, whichever panel the reader was
  // on; afterwards the run stays open while they look elsewhere. So the panel
  // moves on the door, never on the run's name, and a door onto the run
  // already open lands here again.
  let door_pressed = next.opened != opened && !empty(next.open_run)
  opened = next.opened
  panel = pick_str(door_pressed, "runs", panel)
  capabilities = next.capabilities
  actions = next.actions
  account = next.account
  connected = next.connected
  answered = next.answered
  let consumed = next.committed != committed
  committed = next.committed
  selected = pick_str(consumed && creating, draft_id, selected)
  creating = creating && !consumed
  let row = row_named(rows, selected)
  can_edit = editable(connected, account, row.controller)
  selected_status = row.status
  draft_name = pick_str(consumed, row.name, draft_name)
  draft_capability = pick_capability(consumed, row.capability, draft_capability)
  draft_actions = pick_list(consumed, row.allowed_actions, draft_actions)
  draft_caps = pick_caps(consumed, row.caps, draft_caps)
  draft_budget = pick_str(consumed, budget_text(row.caps.subagent_budget), draft_budget)
  draft_skills = pick_skills(consumed, row.skills, draft_skills)
  active_palette = AppTheme.app
  return if !next.dark
  active_palette = AppTheme.app_dark

on props_failed(error)
  host_error = error.message

// Open a record: its row seeds every draft.
on open_agent(id)
  let row = row_named(rows, id)
  selected = id
  creating = false
  can_edit = editable(connected, account, row.controller)
  selected_status = row.status
  draft_id = row.id
  draft_name = row.name
  draft_capability = some_str(row.capability)
  draft_actions = row.allowed_actions
  draft_caps = row.caps
  draft_budget = budget_text(row.caps.subagent_budget)
  draft_skills = row.skills
  cap_entry = ""
  skill_name = ""
  skill_prefix = ""
  skill_snapshot = ""
  skill_always = false

// Open the New agent form: empty drafts, the signing account as controller.
on open_new
  selected = ""
  creating = true
  can_edit = connected && !empty(account)
  draft_id = ""
  draft_name = ""
  draft_capability = none
  draft_actions = []
  draft_caps = empty_caps()
  draft_budget = ""
  draft_skills = []
  cap_entry = ""
  skill_name = ""
  skill_prefix = ""
  skill_snapshot = ""
  skill_always = false

on close_editor
  selected = ""
  creating = false

// ONE SELECTOR FOR THREE MUTUALLY EXCLUSIVE PANELS. Three zero-arg handlers
// would be three places to forget a panel; the value IS the panel, and the
// header's three buttons are the only callers.
on choose_panel(next)
  panel = next

// Open a run: the app is asked for its journal, which arrives as the next
// register under this run's dispatch id.
on open_run_row(run_id)
  expanded_receipt = ""
  open_row = run_named(runs, run_id)
  open_run = open_row.dispatch_id
  sent = open_run(open_row.dispatch_id)

on close_run
  expanded_receipt = ""
  open_run = ""
  open_row = empty_run()
  live = empty_live()
  sent = open_run("")

// A chip pressed: the app's open plane warps to the place it names.
on open_place(url)
  sent = open_link(url)

on pick_capability_option(value)
  draft_capability = some(value)

on toggle_action(action, on)
  draft_actions = with_flag(draft_actions, action, on)

on pick_cap_kind(kind)
  cap_kind = some(kind)

on add_cap
  draft_caps = caps_with(draft_caps, or_empty(cap_kind), cap_entry)
  cap_entry = ""

on remove_cap(kind, entry)
  draft_caps = caps_without(draft_caps, kind, entry)

on set_skill_always(on)
  skill_always = on

on add_skill
  draft_skills = with_skill(draft_skills, skill_name, pick_str(empty(trim(skill_prefix)), library_prefix(skill_name), skill_prefix), skill_snapshot, skill_always)
  skill_name = ""
  skill_prefix = ""
  skill_snapshot = ""
  skill_always = false

on remove_skill(name)
  draft_skills = without_skill(draft_skills, name)

on load_skill(name, always)
  draft_skills = skill_loaded(draft_skills, name, always)

on set_status(agent_id, paused)
  sent = status(agent_id, paused)

on submit_save
  sent = save(selected, draft_name, or_empty(draft_capability), draft_actions, caps_with_budget(draft_caps, draft_budget), draft_skills)

on submit_register
  sent = register(draft_id, draft_name, or_empty(draft_capability), draft_actions, caps_with_budget(draft_caps, draft_budget), draft_skills)

view
  box #root
    with
      w=fill
      h=fill
      bg=bg
    col w=fill h=fill
      sensor show=viewport_changed resize=viewport_changed
        space w=fill h=0.0
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
          if panel == "registry"
            text agents_summary(connected, rows) #meta
              with
                size=12.0
                @text-hint
                @font-mono
          if panel == "runs"
            text runs_summary(runs) #runs-meta
              with
                size=12.0
                @text-hint
                @font-mono
          space w=fill
          // ONE SCREEN, TWO READINGS OF THE SAME AGENTS: their records (who
          // may act) and their runs (what they did, and how it settled). One
          // selector, mutually exclusive content — a run is a registry entry
          // in motion.
          //
          // Disabled rather than hidden on the panel you are on: a hidden row
          // is a row that is MISSING, and a reader cannot see what the screen
          // has by looking at what it is not showing.
          if connected
            button -> choose_panel("registry")
              with
                label="Registry"
                h=28.0
                p=5.0
                disabled=(panel == "registry")
                @outline_action
              text "Registry" size=12.0
          if connected
            button -> choose_panel("runs")
              with
                label="Runs"
                h=28.0
                p=5.0
                disabled=(panel == "runs")
                @outline_action
              text "Runs" size=12.0
          // registering needs an account to control the new agent; a device
          // without one is offered nothing rather than a refusal later
          if connected && !empty(account) && panel == "registry"
            button -> open_new
              with
                label="New agent"
                h=28.0
                p=5.0
                @secondary_action
              text "New agent" size=12.0
      box
        with
          w=fill
          h=1.0
          bg=separator
        space w=1.0 h=1.0
      // The explainer for the pane on screen: the model, not a reading, so it
      // stays with the node down.
      box
        with
          w=fill
          px=22.0
          pt=12.0
          pb=10.0
        text pane_note(panel)
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
      if connected && panel == "runs"
        row w=fill h=fill
          if empty(runs)
            box
              with
                w=fill
                h=fill
                p=22.0
              box #no-runs
                with
                  w=fill
                  p=30.0
                  align-x=center
                  border=border
                  border-w=1.0
                  r=12.0
                text "No runs yet — every dispatch of an agent lands here with its journal."
                  with
                    size=13.0
                    @text-meta
          if !empty(runs)
            scroll #runs-body
              with
                dir=vertical
                w=fill
                h=fill
              col
                with
                  w=fill
                  p=18.0
                  gap=11.0
                // A run row: who ran, what it answered, where it stands and
                // how far it got. The row opens its journal; its accessible
                // name is the run id.
                for run in runs
                  col w=fill
                    button -> open_run_row(run.run_id)
                      with
                        label=run.run_id
                        w=fill
                        p=0.0
                      box
                        with
                          w=fill
                          pl=14.0
                          pr=14.0
                          pt=11.0
                          pb=11.0
                        row
                          with
                            w=fill
                            gap=13.0
                            align=center
                          col w=fill gap=3.0
                            row
                              with
                                w=fill
                                gap=8.0
                                align=center
                              text run.agent_name
                                with
                                  size=13.5
                                  @text-fg
                                  @font-semibold
                              text run.origin
                                with
                                  w=fill
                                  size=11.0
                                  @text-meta
                                  @font-mono
                            row
                              with
                                w=fill
                                gap=5.0
                                align=center
                              text run.dispatched
                                with
                                  size=10.5
                                  @text-hint
                                  @font-mono
                              text "·"
                                with
                                  size=10.5
                                  @text-hint
                                  @font-mono
                              text run.actions
                                with
                                  size=10.5
                                  @text-meta
                                  @font-mono
                                  @font-medium
                              text "actions"
                                with
                                  size=10.5
                                  @text-meta
                                  @font-mono
                                  @font-medium
                              if run.pr_number > 0
                                text "· PR #"
                                  with
                                    size=10.5
                                    @text-hint
                                    @font-mono
                              if run.pr_number > 0
                                text run.pr_number
                                  with
                                    size=10.5
                                    @text-hint
                                    @font-mono
                          // Standing, in the journal's own words: in flight
                          // on a warning plate, accepted on a success one,
                          // refused or failed on a danger one.
                          if run.state == "dispatched" || run.state == "running"
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
                                text run.state
                                  with
                                    size=9.0
                                    @text-warning
                                    @font-mono
                                    @font-semibold
                          if run.state == "accepted"
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
                                text run.state
                                  with
                                    size=9.0
                                    @text-success
                                    @font-mono
                                    @font-semibold
                          if run.state == "rejected" || run.state == "failed"
                            box
                              with
                                px=8.0
                                py=3.0
                                bg=danger_bg
                                border=danger_line
                                border-w=1.0
                                r=6.0
                              text run.state
                                with
                                  size=9.0
                                  @text-danger
                                  @font-mono
                                  @font-semibold
                      active bg=bg
                      hovered bg=row_hover
                    box
                      with
                        w=fill
                        h=1.0
                        bg=muted_bg
                      space w=1.0 h=1.0
          // THE JOURNAL: the open run's lifecycle, fact by fact, beside the
          // list. Read-only — a run is history the moment it is written.
          if !empty(open_run)
            resize-handle #journal-resize drag=journal_resized cursor=resize-horizontal
              box #journal-divider
                with
                  w=10.0
                  h=fill
                  bg=sidebar
                  align-x=center
                box w=2.0 h=fill bg=separator
                  space w=2.0 h=1.0
            box #journal
              with
                w=journal_width
                h=fill
                bg=surface
                border=border
                border-w=1.0
              scroll
                with
                  dir=vertical
                  w=fill
                  h=fill
                col
                  with
                    w=fill
                    p=18.0
                    gap=12.0
                  row
                    with
                      w=fill
                      gap=10.0
                      align=center
                    text open_row.agent_name
                      with
                        size=14.0
                        @text-fg
                        @font-semibold
                    text open_row.state
                      with
                        size=11.0
                        @text-meta
                        @font-mono
                    space w=fill
                    button -> close_run
                      with
                        label="Close journal"
                        w=24.0
                        h=24.0
                        p=0.0
                      text "×" size=16.0 @text-meta
                  text open_row.origin
                    with
                      w=fill
                      size=11.0
                      @text-meta
                      @font-mono
                  // THE RUN'S IDENTIFIERS, folded behind a disclosure: the
                  // dispatch id the run is addressed by (`duck://run/<id>`)
                  // and the blob its output landed in. Each is LABELED — a
                  // bare hash under a button called "Run diagnostics" read as
                  // a command that answered with a digest. A disclosure wears
                  // the ghost face, never the default button face.
                  button -> toggle_receipt(open_run)
                    with
                      label="Run details"
                      expanded=(expanded_receipt == open_run)
                      p=4.0
                      @ghost_action
                    row gap=6.0 align=center
                      if expanded_receipt == open_run
                        text "▾" size=10.0 @text-meta
                      if expanded_receipt != open_run
                        text "▸" size=10.0 @text-meta
                      text "Details" size=11.0 @text-meta
                  if expanded_receipt == open_run
                    col w=fill gap=4.0
                      row w=fill gap=8.0 align=center
                        text "dispatch"
                          with
                            w=64.0
                            size=10.0
                            @text-label
                            @font-mono
                        text open_run
                          with
                            w=fill
                            size=10.0
                            @text-meta
                            @font-mono
                      if !empty(open_row.output_ref)
                        row w=fill gap=8.0 align=center
                          text "output"
                            with
                              w=64.0
                              size=10.0
                              @text-label
                              @font-mono
                          text open_row.output_ref
                            with
                              w=fill
                              size=10.0
                              @text-meta
                              @font-mono
                  // THE RUN AS IT RUNS: its status, the steps it has taken
                  // and the answer forming, off the node's live reading. The
                  // chat stream only hints that a run is working under its
                  // message; this is where the progress is drawn.
                  if live.present
                    box #live
                      with
                        w=fill
                        px=12.0
                        py=9.0
                        bg=warning_bg
                        border=warning_line
                        border-w=1.0
                        r=8.0
                      col w=fill gap=4.0
                        text live.status
                          with
                            w=fill
                            size=12.0
                            @text-fg
                            @font-medium
                        for act in live.activity
                          row w=fill gap=5.0 align=center
                            if act.done
                              text "✓"
                                with
                                  size=11.0
                                  @text-meta
                                  @font-mono
                            if !act.done
                              text "…"
                                with
                                  size=11.0
                                  @text-meta
                                  @font-mono
                            text act.label
                              with
                                w=fill
                                size=11.0
                                @text-meta
                        if !empty(live.answer_preview)
                          text live.answer_preview
                            with
                              w=fill
                              size=12.0
                              @text-meta
                  // RELEVANT: where the run came from and every place it
                  // touched, folded from its journal's receipts — the thread
                  // that summoned it, the pages and blocks it wrote, the PR
                  // it opened, the runs it delegated. Each chip is a duck://
                  // address the app's open plane warps to; a place the
                  // protocol cannot address yet draws as a label alone.
                  if journal.dispatch_id == open_run && !empty(journal.links)
                    text "Relevant"
                      with
                        size=12.5
                        @text-fg
                        @font-semibold
                    col
                      with
                        w=fill
                        gap=6.0
                      for link in journal.links
                        if !empty(link.url)
                          button -> open_place(link.url)
                            with
                              label=link.label
                              w=fill
                              p=0.0
                              @outline_action
                            RunChip link=link
                        if empty(link.url)
                          RunChip link=link
                  if !empty(open_row.reason)
                    box
                      with
                        w=fill
                        px=12.0
                        py=9.0
                        bg=danger_bg
                        border=danger_line
                        border-w=1.0
                        r=8.0
                      text open_row.reason
                        with
                          w=fill
                          size=12.0
                          @text-danger
                  text "Journal"
                    with
                      size=12.5
                      @text-fg
                      @font-semibold
                  if journal.dispatch_id != open_run
                    text "Reading the journal…"
                      with
                        size=12.0
                        @text-caption
                  if journal.dispatch_id == open_run && empty(journal.entries)
                    text "This run's journal has no entries yet — the fold may still be catching up to the chain."
                      with
                        w=fill
                        size=12.0
                        @text-caption
                  if journal.dispatch_id == open_run
                    for entry in journal.entries
                      row w=fill gap=8.0
                        text entry.height
                          with
                            size=10.5
                            @text-hint
                            @font-mono
                        col w=fill gap=2.0
                          text entry.kind
                            with
                              size=11.0
                              @text-fg
                              @font-mono
                              @font-semibold
                          text entry.summary
                            with
                              w=fill
                              size=12.0
                              @text-meta
                          if !empty(entry.status)
                            text entry.status size=11.0 @text-meta
                          for target in entry.targets
                            if !empty(target.url)
                              button -> open_place(target.url)
                                with
                                  label=target.label
                                  w=fill
                                  p=0.0
                                  @outline_action
                                RunChip link=target
                            if empty(target.url)
                              RunChip link=target
      if connected && panel == "registry" && empty(rows) && answered && !creating
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
      if connected && panel == "registry" && (!empty(rows) || creating)
        row w=fill h=fill
          if !empty(rows)
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
                // who owns it, and whether it is live. The row opens its
                // record; its accessible name is the agent's own.
                for agent in rows
                  col w=fill
                    button -> open_agent(agent.id)
                      with
                        label=agent.name
                        w=fill
                        p=0.0
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
                              text skill_count(agent.skills)
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
                              text cap_count(agent.caps)
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
                      active bg=bg
                      hovered bg=row_hover
                    box
                      with
                        w=fill
                        h=1.0
                        bg=muted_bg
                      space w=1.0 h=1.0
          // THE EDITOR: the open record, or the New agent form, beside the
          // list. Every control is live for the controller and read-only for
          // anyone else — the record is still worth reading whole.
          if !empty(selected) || creating
            box #editor
              with
                w=400.0
                h=fill
                bg=surface
                border=border
                border-w=1.0
              scroll
                with
                  dir=vertical
                  w=fill
                  h=fill
                col
                  with
                    w=fill
                    p=18.0
                    gap=14.0
                  row
                    with
                      w=fill
                      gap=10.0
                      align=center
                    if creating
                      text "New agent"
                        with
                          size=14.0
                          @text-fg
                          @font-semibold
                    if !creating
                      text draft_name
                        with
                          size=14.0
                          @text-fg
                          @font-semibold
                    space w=fill
                    button -> close_editor
                      with
                        label="Close editor"
                        w=24.0
                        h=24.0
                        p=0.0
                      text "×" size=16.0 @text-meta
                  if !can_edit && !creating
                    box
                      with
                        w=fill
                        px=12.0
                        py=9.0
                        bg=warning_bg
                        border=warning_line
                        border-w=1.0
                        r=8.0
                      text "Only this agent's controller can change its record. You are reading it."
                        with
                          w=fill
                          size=12.0
                          @text-warning
                  // Standing: paused agents engage no new runs; pausing does
                  // not cancel work already dispatched.
                  if !creating
                    row
                      with
                        w=fill
                        gap=8.0
                        align=center
                      text "Standing"
                        with
                          size=12.5
                          @text-fg
                          @font-semibold
                      space w=fill
                      text selected_status
                        with
                          size=11.0
                          @text-meta
                          @font-mono
                      if can_edit && selected_status == "active"
                        button -> set_status(selected, true)
                          with
                            label="Pause agent"
                            h=26.0
                            p=5.0
                            @outline_action
                          text "Pause" size=11.5
                      if can_edit && selected_status == "paused"
                        button -> set_status(selected, false)
                          with
                            label="Resume agent"
                            h=26.0
                            p=5.0
                            @outline_action
                          text "Resume" size=11.5
                  // Identity. The id is the agent's address (`<id>@agents.duck`),
                  // fixed at registration; the display name is the record's.
                  col w=fill gap=6.0
                    text "Identity"
                      with
                        size=12.5
                        @text-fg
                        @font-semibold
                    if creating
                      input "" #agent-id <-> draft_id
                        with
                          label="Agent id"
                          hint="a-dns-label, e.g. chiefduck"
                          w=fill
                          p=7.0
                          text-size=13.0
                          line-h=1.2
                          @control
                        active bg=elevated border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                        hovered bg=elevated border=fg/21
                        disabled bg=muted_bg/54 value=muted
                    if creating && !empty(draft_id) && !valid_agent_id(draft_id)
                      text "An agent id is a lowercase DNS label: a-z, 0-9 and hyphens, no hyphen at either end."
                        with
                          w=fill
                          size=11.0
                          @text-danger
                    if !creating
                      text draft_id
                        with
                          size=11.0
                          @text-meta
                          @font-mono
                    input "" #agent-name <-> draft_name
                      with
                        label="Display name"
                        hint="display name…"
                        disabled=!can_edit
                        w=fill
                        p=7.0
                        text-size=13.0
                        line-h=1.2
                        @control
                      active bg=elevated border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                      hovered bg=elevated border=fg/21
                      disabled bg=muted_bg/54 value=muted
                  // Executor: the capability tag its runs dispatch on. The
                  // network's announced tags, plus the record's own when no
                  // node announces it today.
                  col w=fill gap=6.0
                    text "Executor"
                      with
                        size=12.5
                        @text-fg
                        @font-semibold
                    text "The capability tag this agent's runs are dispatched on — a node announcing it runs them."
                      with
                        w=fill
                        size=11.0
                        @text-caption
                    if can_edit
                      pick capability_options(capabilities, or_empty(draft_capability)) draft_capability #agent-capability -> pick_capability_option _
                        with
                          hint="pick a capability…"
                          w=fill
                    if !can_edit
                      text or_empty(draft_capability)
                        with
                          size=12.0
                          @text-fg
                          @font-mono
                  // The action grant: the whole vocabulary, ticked.
                  col w=fill gap=6.0
                    text "Actions"
                      with
                        size=12.5
                        @text-fg
                        @font-semibold
                    text "Every write this agent may propose. An unticked action is refused at the registry."
                      with
                        w=fill
                        size=11.0
                        @text-caption
                    // "*" is every action the catalog knows today and every
                    // one added later; the registry keeps it as the whole
                    // grant, so the individual ticks read as implied.
                    checkbox "every action (*)" #action-every checked=has(draft_actions, "*") disabled=!can_edit -> toggle_action("*", _)
                    for action in actions
                      checkbox action #action(action) checked=(has(draft_actions, action) || has(draft_actions, "*")) disabled=(!can_edit || has(draft_actions, "*")) -> toggle_action(action, _)
                  // Resource caps: exact repos, duckfs prefixes, page ids ("*"
                  // is every page), tool ids, vault refs, and the peer-call
                  // budget.
                  col w=fill gap=6.0
                    text "Grants"
                      with
                        size=12.5
                        @text-fg
                        @font-semibold
                    text "Forge repos by name, duckfs prefixes, page ids (\"*\" for every page), tool ids and vault refs. Empty denies everything."
                      with
                        w=fill
                        size=11.0
                        @text-caption
                    for entry in draft_caps.forge_read
                      row w=fill gap=6.0 align=center
                        text "forge read"
                          with
                            size=10.5
                            @text-hint
                            @font-mono
                        text entry
                          with
                            size=12.0
                            @text-fg
                            @font-mono
                        space w=fill
                        if can_edit
                          button -> remove_cap("forge_read", entry)
                            with
                              label="Remove grant"
                              w=22.0
                              h=22.0
                              p=0.0
                            text "×" size=14.0 @text-meta
                    for entry in draft_caps.forge_push
                      row w=fill gap=6.0 align=center
                        text "forge push"
                          with
                            size=10.5
                            @text-hint
                            @font-mono
                        text entry
                          with
                            size=12.0
                            @text-fg
                            @font-mono
                        space w=fill
                        if can_edit
                          button -> remove_cap("forge_push", entry)
                            with
                              label="Remove grant"
                              w=22.0
                              h=22.0
                              p=0.0
                            text "×" size=14.0 @text-meta
                    for entry in draft_caps.duckfs_read
                      row w=fill gap=6.0 align=center
                        text "duckfs read"
                          with
                            size=10.5
                            @text-hint
                            @font-mono
                        text entry
                          with
                            size=12.0
                            @text-fg
                            @font-mono
                        space w=fill
                        if can_edit
                          button -> remove_cap("duckfs_read", entry)
                            with
                              label="Remove grant"
                              w=22.0
                              h=22.0
                              p=0.0
                            text "×" size=14.0 @text-meta
                    for entry in draft_caps.duckfs_write
                      row w=fill gap=6.0 align=center
                        text "duckfs write"
                          with
                            size=10.5
                            @text-hint
                            @font-mono
                        text entry
                          with
                            size=12.0
                            @text-fg
                            @font-mono
                        space w=fill
                        if can_edit
                          button -> remove_cap("duckfs_write", entry)
                            with
                              label="Remove grant"
                              w=22.0
                              h=22.0
                              p=0.0
                            text "×" size=14.0 @text-meta
                    for entry in draft_caps.tools
                      row w=fill gap=6.0 align=center
                        text "tool"
                          with
                            size=10.5
                            @text-hint
                            @font-mono
                        text entry
                          with
                            size=12.0
                            @text-fg
                            @font-mono
                        space w=fill
                        if can_edit
                          button -> remove_cap("tools", entry)
                            with
                              label="Remove grant"
                              w=22.0
                              h=22.0
                              p=0.0
                            text "×" size=14.0 @text-meta
                    for entry in draft_caps.secrets
                      row w=fill gap=6.0 align=center
                        text "secret"
                          with
                            size=10.5
                            @text-hint
                            @font-mono
                        text entry
                          with
                            size=12.0
                            @text-fg
                            @font-mono
                        space w=fill
                        if can_edit
                          button -> remove_cap("secrets", entry)
                            with
                              label="Remove grant"
                              w=22.0
                              h=22.0
                              p=0.0
                            text "×" size=14.0 @text-meta
                    for entry in draft_caps.pages_write
                      row w=fill gap=6.0 align=center
                        text "pages write"
                          with
                            size=10.5
                            @text-hint
                            @font-mono
                        text entry
                          with
                            size=12.0
                            @text-fg
                            @font-mono
                        space w=fill
                        if can_edit
                          button -> remove_cap("pages_write", entry)
                            with
                              label="Remove grant"
                              w=22.0
                              h=22.0
                              p=0.0
                            text "×" size=14.0 @text-meta
                    if can_edit
                      row w=fill gap=6.0 align=center
                        pick cap_kinds() cap_kind #cap-kind -> pick_cap_kind _
                          with
                            hint="kind"
                            w=130.0
                        input "" #cap-entry <-> cap_entry
                          with
                            label="Grant entry"
                            hint="repo, prefix, page id…"
                            w=fill
                            p=7.0
                            text-size=12.5
                            line-h=1.2
                            @control
                          active bg=elevated border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                          hovered bg=elevated border=fg/21
                          disabled bg=muted_bg/54 value=muted
                        button -> add_cap
                          with
                            label="Add grant"
                            disabled=empty(trim(cap_entry))
                            h=28.0
                            p=5.0
                            @secondary_action
                          text "Add" size=12.0
                    row w=fill gap=6.0 align=center
                      text "Peer-call budget" size=12.0 @text-fg
                      space w=fill
                      input "" #agent-budget <-> draft_budget
                        with
                          label="Subagent budget"
                          hint="0"
                          disabled=!can_edit
                          w=70.0
                          p=7.0
                          text-size=12.5
                          line-h=1.2
                          @control
                        active bg=elevated border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                        hovered bg=elevated border=fg/21
                        disabled bg=muted_bg/54 value=muted
                  // Curated skills: an always-loaded skill is the persona,
                  // assembled into every run's context; an on-demand one is
                  // indexed and read when the task calls for it.
                  col w=fill gap=6.0
                    text "Skills"
                      with
                        size=12.5
                        @text-fg
                        @font-semibold
                    text "Always-loaded skills are the agent's persona; on-demand skills are indexed and read when a task needs them. A pinned snapshot freezes the source; unpinned follows the committed head."
                      with
                        w=fill
                        size=11.0
                        @text-caption
                    for skill in draft_skills
                      row w=fill gap=6.0 align=center
                        col w=fill gap=2.0
                          text skill.name
                            with
                              size=12.5
                              @text-fg
                              @font-semibold
                          text skill.source_prefix
                            with
                              size=10.5
                              @text-hint
                              @font-mono
                          if !empty(skill.source_snapshot)
                            text skill.source_snapshot
                              with
                                size=10.0
                                @text-hint
                                @font-mono
                        if can_edit && skill.always
                          button -> load_skill(skill.name, false)
                            with
                              label="Load on demand"
                              h=24.0
                              p=4.0
                              @outline_action
                            text skill_mode(skill.always) size=10.5
                        if can_edit && !skill.always
                          button -> load_skill(skill.name, true)
                            with
                              label="Load always"
                              h=24.0
                              p=4.0
                              @outline_action
                            text skill_mode(skill.always) size=10.5
                        if !can_edit
                          text skill_mode(skill.always)
                            with
                              size=10.5
                              @text-meta
                              @font-mono
                        if can_edit
                          button -> remove_skill(skill.name)
                            with
                              label="Remove skill"
                              w=22.0
                              h=22.0
                              p=0.0
                            text "×" size=14.0 @text-meta
                    if can_edit
                      col w=fill gap=6.0
                        input "" #skill-name <-> skill_name
                          with
                            label="Skill name"
                            hint="skill name (its mount directory)…"
                            w=fill
                            p=7.0
                            text-size=12.5
                            line-h=1.2
                            @control
                          active bg=elevated border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                          hovered bg=elevated border=fg/21
                          disabled bg=muted_bg/54 value=muted
                        input "" #skill-prefix <-> skill_prefix
                          with
                            label="Skill source prefix"
                            hint="/shared/skills/<name> when left empty"
                            w=fill
                            p=7.0
                            text-size=12.5
                            line-h=1.2
                            @control
                          active bg=elevated border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                          hovered bg=elevated border=fg/21
                          disabled bg=muted_bg/54 value=muted
                        input "" #skill-snapshot <-> skill_snapshot
                          with
                            label="Skill snapshot pin"
                            hint="snapshot id to pin (optional)"
                            w=fill
                            p=7.0
                            text-size=12.5
                            line-h=1.2
                            @control
                          active bg=elevated border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                          hovered bg=elevated border=fg/21
                          disabled bg=muted_bg/54 value=muted
                        row w=fill gap=6.0 align=center
                          checkbox "load always (persona)" #skill-always checked=skill_always -> set_skill_always(_)
                          space w=fill
                          button -> add_skill
                            with
                              label="Add skill"
                              disabled=empty(trim(skill_name))
                              h=28.0
                              p=5.0
                              @secondary_action
                            text "Add skill" size=12.0
                  // The one write: the whole record, signed by the app.
                  if can_edit && !creating
                    button -> submit_save
                      with
                        label="Save agent"
                        disabled=(empty(trim(draft_name)) || empty(or_empty(draft_capability)))
                        w=fill
                        p=10.0
                        @primary_action
                      text "Save" size=12.5
                  if creating
                    button -> submit_register
                      with
                        label="Register agent"
                        disabled=(!valid_agent_id(draft_id) || empty(trim(draft_name)) || empty(or_empty(draft_capability)))
                        w=fill
                        p=10.0
                        @primary_action
                      text "Register" size=12.5

// ONE PLACE A RUN TOUCHED, as a chip: the glyph of its kind, `from` when it
// is where the run was called from, and the place's own label.
component RunChip(link:RunLink)
  box
    with
      w=fill
      clip=true
      px=9.0
      py=4.0
      r=13.0
    row w=fill gap=6.0 align=center
      text link_glyph(link.kind)
        with
          size=11.5
          @text-meta
          @font-medium
      if link.relation == "from"
        text "from"
          with
            size=11.5
            @text-hint
            @font-medium
      text link.label
        with
          w=fill
          size=11.5
          @text-fg
          @font-medium
