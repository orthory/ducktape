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
  MessagingSeat(participant:str, role:str, you:bool)
  MessagingBinding(present:bool, device:str, credential:str, detached:bool)
  MessagingMessage(seq:i64, sender:str, recipient:str, kind:str, body:str, body_bytes:i64, shown_bytes:i64, references:str, reply_to:i64, task:str, task_attempt:i64, delivery:str, delivery_reason:str, mine:bool, expires_at:i64, admitted_at:i64)
  MessagingProps(participant:str, conversation:str, network:str, topic:str, roster:[MessagingSeat], binding:MessagingBinding, messages:[MessagingMessage], may_read:bool, may_send:bool, denied:str, error:str, history_gap:bool, floor_seq:i64, from_seq:i64, next_seq:i64, page_size:i64, more_before:bool, more_after:bool, undelivered:i64, queued_bytes:i64, max_body_bytes:i64, loading:bool, answered:bool, sending:bool, send_error:str, sent_seq:i64, visibility:str)
  AgentsProps(rows:[AgentRow], capabilities:[str], actions:[str], account:str, committed:i64, connected:bool, answered:bool, dark:bool, messaging:MessagingProps)
  stream props() -> AgentsProps ! HostError
  pure agents_summary(connected:bool, rows:&[AgentRow]) -> str
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
  pure pick_option(condition:bool, then:&str, or:&str?) -> str?
  pure empty_caps() -> AgentCaps
  pure pick_list(condition:bool, then:&[str], or:&[str]) -> [str]
  pure pick_caps(condition:bool, then:&AgentCaps, or:&AgentCaps) -> AgentCaps
  pure pick_skills(condition:bool, then:&[AgentSkill], or:&[AgentSkill]) -> [AgentSkill]
  pure valid_agent_id(id:&str) -> bool
  pure empty_messaging() -> MessagingProps
  pure pick_int(condition:bool, then:i64, or:i64) -> i64
  pure delivery_label(state:&str) -> str
  pure delivery_note(state:&str) -> str
  pure delivery_unsettled(state:&str) -> bool
  pure kind_label(kind:&str) -> str
  pure body_note(body_bytes:i64, shown_bytes:i64) -> str
  pure task_note(task:&str, attempt:i64) -> str
  pure denied_note(denied:&str) -> str
  pure seat_role(role:&str) -> str
  pure binding_note(binding:&MessagingBinding) -> str
  pure mailbox_note(undelivered:i64, queued_bytes:i64) -> str
  pure page_note(messages:&[MessagingMessage], from_seq:i64, next_seq:i64) -> str
  pure compose_kinds(reply_to:i64) -> [str]
  pure body_refusal(body:&str, max_body_bytes:i64) -> str
  pure send_ready(may_send:bool, sending:bool, recipient:&str?, kind:&str?, body:&str, max_body_bytes:i64) -> bool
  pure recipients(roster:&[MessagingSeat]) -> [str]
  pure older_from(from_seq:i64, floor_seq:i64, page_size:i64) -> i64
  pure newer_from(from_seq:i64, page_size:i64, next_seq:i64) -> i64
  pure reply_note(reply_to:i64) -> str
  pure same_scope(network:&str, participant:&str, conversation:&str, other_network:&str, other_participant:&str, other_conversation:&str) -> bool
  pure open_conversation(participant:&str, conversation:&str) -> bool
  pure page_messages(from_seq:i64, newest:bool) -> bool
  pure send_message(kind:&str, recipient:&str, body:&str, reply_to:i64) -> bool
  pure status(agent_id:&str, paused:bool) -> bool
  pure save(agent_id:&str, display_name:&str, capability:&str, allowed_actions:&[str], caps:&AgentCaps, skills:&[AgentSkill]) -> bool
  pure register(agent_id:&str, display_name:&str, capability:&str, allowed_actions:&[str], caps:&AgentCaps, skills:&[AgentSkill]) -> bool

state
  active_palette:palette[AppTheme] = AppTheme.app
  rows:[AgentRow] = []
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
  // WHICH PANE: "register" is the record list above, "messages" is the
  // collaboration a person watches. One screen, two subsections — an agent's
  // record and an agent's messages are the same object read two ways.
  agents_pane = "register"
  // the panel's whole reading, exactly as the app authenticated it
  messaging:MessagingProps = empty_messaging()
  // THE EXPLICIT ASSOCIATION: who the viewer acts as, and what they open.
  // Nothing here scans, discovers or attaches on its own.
  open_participant = ""
  open_conversation_id = ""
  // the composer
  msg_kind:str? = some("notice")
  msg_recipient:str? = none
  msg_body = ""
  msg_reply_to:i64 = 0
  // THE SCOPE THE DRAFT WAS WRITTEN FOR, captured when a send leaves. An
  // answer about another network, participant or conversation never clears
  // text written for this one — and a failed send keeps it whole.
  draft_network = ""
  draft_participant = ""
  draft_conversation = ""
  last_sent_seq:i64 = 0

on mount
  stream every props() -> props_changed _ | props_failed _

// The register moved. A bumped `committed` means the app signed a write off
// these drafts: the New form closes onto the agent it registered, and an
// open record re-seeds from its fresh row — the drafts were consumed, the
// row is now the truth.
on props_changed(next)
  rows = next.rows
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
  // A LANDED SEND CLEARS ONLY ITS OWN DRAFT. `sent_seq` moves when the network
  // admitted this panel's send; the scope check is what stops an answer about
  // conversation A from clearing a message being written for B after a switch.
  let landed = next.messaging.sent_seq > last_sent_seq
  let draft_landed = landed && same_scope(draft_network, draft_participant, draft_conversation, next.messaging.network, next.messaging.participant, next.messaging.conversation)
  last_sent_seq = next.messaging.sent_seq
  // OPENING A DIFFERENT CONVERSATION EMPTIES THE COMPOSER. Text written for
  // one roster is not silently retargeted at another.
  let switched = messaging.conversation != next.messaging.conversation || messaging.participant != next.messaging.participant || messaging.network != next.messaging.network
  msg_body = pick_str(draft_landed || switched, "", msg_body)
  msg_reply_to = pick_int(draft_landed || switched, 0, msg_reply_to)
  msg_recipient = pick_option(switched, "", msg_recipient)
  msg_kind = pick_option(switched, "notice", msg_kind)
  messaging = next.messaging
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

// ---- the messaging panel ----------------------------------------------------

on choose_pane(next_pane)
  agents_pane = next_pane

// OPEN ONE CONVERSATION, EXPLICITLY. The ids are the reader's own: the network
// resolves them under the key this device already holds, and a refusal is
// shown as a refusal. Nothing is scanned and no provider is contacted.
on submit_open
  sent = open_conversation(open_participant, open_conversation_id)

// Closing is the same intent with nothing named — the app drops the panel's
// whole reading rather than leaving a stale one on screen.
on close_conversation
  sent = open_conversation("", "")

on pick_message_kind(value)
  msg_kind = some(value)

on pick_recipient(value)
  msg_recipient = some(value)

on reply_to_message(seq)
  msg_reply_to = seq

// `result` exists as a kind only while replying; dropping the reply drops it.
on clear_reply
  msg_reply_to = 0
  msg_kind = pick_option(or_empty(msg_kind) == "result", "notice", msg_kind)

on submit_message
  draft_network = messaging.network
  draft_participant = messaging.participant
  draft_conversation = messaging.conversation
  sent = send_message(or_empty(msg_kind), or_empty(msg_recipient), msg_body, msg_reply_to)

on page_older
  sent = page_messages(older_from(messaging.from_seq, messaging.floor_seq, messaging.page_size), false)

on page_newer
  sent = page_messages(newer_from(messaging.from_seq, messaging.page_size, messaging.next_seq), false)

on page_newest
  sent = page_messages(0, true)

// A history gap is not a page to skip: resync from the retained floor rather
// than advancing a cursor past events that no longer exist.
on resync_history
  sent = page_messages(messaging.floor_seq, false)

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
          // The two readings of the same agents: their records, and their
          // messages. One screen — the panel is a subsection, not a shell.
          if agents_pane != "register"
            button -> choose_pane("register")
              with
                label="Show the agent register"
                h=28.0
                p=5.0
                @outline_action
              text "Register" size=12.0
          if agents_pane != "messages"
            button -> choose_pane("messages")
              with
                label="Show agent messages"
                h=28.0
                p=5.0
                @outline_action
              text "Messages" size=12.0
          // registering needs an account to control the new agent; a device
          // without one is offered nothing rather than a refusal later
          if agents_pane == "register" && connected && !empty(account)
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
        if agents_pane == "register"
          text "The registry records who may act, what they may do, and under whose grant — every entry here is on chain. The acting itself is recorded separately, as each agent's runs."
            with
              w=fill
              size=12.0
              @text-caption
        if agents_pane == "messages"
          text "Messages are immutable records with a separate delivery state per recipient. A delivery state says what a provider's input interface did with an input — never that a model read it, understood it, acted on it, or that a task was claimed or finished."
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
      if agents_pane == "register" && connected && empty(rows) && answered && !creating
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
      if agents_pane == "register" && connected && (!empty(rows) || creating)
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
                    for action in actions
                      checkbox action #action(action) checked=has(draft_actions, action) disabled=!can_edit -> toggle_action(action, _)
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
      // THE MESSAGES PANE. A conversation is opened EXPLICITLY, read under
      // this device's own key, and shown with what the network actually said —
      // including that it refused. A refusal is never an empty list, and a
      // delivery state is never dressed up as work.
      if agents_pane == "messages" && connected
        scroll #messages-body
          with
            dir=vertical
            w=fill
            h=fill
          col
            with
              w=fill
              p=18.0
              gap=13.0
            if !empty(messaging.visibility)
              box #visibility
                with
                  w=fill
                  px=12.0
                  py=9.0
                  bg=elevated
                  r=8.0
                text messaging.visibility
                  with
                    w=fill
                    size=11.5
                    @text-caption
            // OPEN ONE, BY NAME. There is no directory of conversations to
            // browse: a reader names the participant it acts as and the
            // conversation it opens, and the network answers for that pair or
            // refuses it.
            if empty(messaging.conversation)
              col #open-form
                with
                  w=fill
                  gap=8.0
                text "Open a conversation"
                  with
                    size=13.0
                    @text-fg
                    @font-semibold
                text "Name the participant you are acting as and the conversation to open. Both are resolved on this network under this device's key — nothing is discovered by scanning, and no provider session is contacted."
                  with
                    w=fill
                    size=11.5
                    @text-caption
                input "" #open-participant <-> open_participant
                  with
                    label="Acting participant"
                    hint="the participant id you own…"
                    w=fill
                    p=7.0
                    text-size=13.0
                    line-h=1.2
                    @control
                  active bg=elevated border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                  hovered bg=elevated border=fg/21
                  disabled bg=muted_bg/54 value=muted
                input "" #open-conversation <-> open_conversation_id
                  with
                    label="Conversation"
                    hint="the conversation id…"
                    w=fill
                    p=7.0
                    text-size=13.0
                    line-h=1.2
                    @control
                  active bg=elevated border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                  hovered bg=elevated border=fg/21
                  disabled bg=muted_bg/54 value=muted
                button -> submit_open
                  with
                    label="Open conversation"
                    disabled=(empty(trim(open_participant)) || empty(trim(open_conversation_id)) || messaging.loading)
                    p=9.0
                    @primary_action
                  text "Open" size=12.5
            // WHAT IS OPEN, UNDER WHOSE IDENTITY, ON WHICH NETWORK. The ids
            // mean nothing without the network: the same name on another chain
            // is another conversation.
            if !empty(messaging.conversation)
              col w=fill gap=5.0
                row
                  with
                    w=fill
                    gap=9.0
                    align=center
                  text messaging.conversation #conversation-id
                    with
                      size=14.0
                      @text-fg
                      @font-semibold
                  if !empty(messaging.topic)
                    text messaging.topic
                      with
                        size=12.0
                        @text-meta
                  space w=fill
                  button -> close_conversation
                    with
                      label="Close conversation"
                      w=24.0
                      h=24.0
                      p=0.0
                    text "×" size=16.0 @text-meta
                row w=fill gap=6.0 align=center
                  text "as"
                    with
                      size=10.5
                      @text-hint
                      @font-mono
                  text messaging.participant
                    with
                      size=10.5
                      @text-fg
                      @font-mono
                  text "· network"
                    with
                      size=10.5
                      @text-hint
                      @font-mono
                  text messaging.network #network-id
                    with
                      size=10.5
                      @text-hint
                      @font-mono
            // A REFUSAL IS SHOWN AS A REFUSAL. Neither of these plates may be
            // replaced by an empty conversation.
            if !empty(messaging.denied)
              box #denied
                with
                  w=fill
                  px=12.0
                  py=9.0
                  bg=warning_bg
                  border=warning_line
                  border-w=1.0
                  r=8.0
                col w=fill gap=3.0
                  text denied_note(messaging.denied)
                    with
                      w=fill
                      size=12.0
                      @text-warning
                  text messaging.denied
                    with
                      size=10.0
                      @text-warning
                      @font-mono
            if !empty(messaging.error)
              box #messaging-error
                with
                  w=fill
                  px=12.0
                  py=9.0
                  bg=warning_bg
                  border=warning_line
                  border-w=1.0
                  r=8.0
                text messaging.error
                  with
                    w=fill
                    size=12.0
                    @text-danger
            if messaging.loading
              text "Reading…" #loading size=11.5 @text-hint @font-mono
            // WHO IS ON IT, WHAT THIS DEVICE HOLDS, AND WHAT IS QUEUED.
            if !empty(messaging.conversation) && messaging.may_read
              col w=fill gap=6.0
                text "Participants"
                  with
                    size=12.5
                    @text-fg
                    @font-semibold
                for seat in messaging.roster
                  row w=fill gap=6.0 align=center
                    text seat.participant
                      with
                        size=12.0
                        @text-fg
                        @font-mono
                    text seat_role(seat.role)
                      with
                        size=10.5
                        @text-hint
                        @font-mono
                    if seat.you
                      text "you"
                        with
                          size=10.0
                          @text-secondary_fg
                          @font-mono
                          @font-semibold
                text binding_note(messaging.binding) #binding-note
                  with
                    w=fill
                    size=11.5
                    @text-caption
                text mailbox_note(messaging.undelivered, messaging.queued_bytes) #mailbox-note
                  with
                    size=11.0
                    @text-hint
                    @font-mono
            // A GAP IS A GAP. The cursor fell below the retained floor, so the
            // only honest move is to resync from the floor — never to advance
            // past events that no longer exist.
            if messaging.history_gap
              box #history-gap
                with
                  w=fill
                  px=12.0
                  py=9.0
                  bg=warning_bg
                  border=warning_line
                  border-w=1.0
                  r=8.0
                col w=fill gap=6.0
                  text "This conversation was pruned past the page you asked for. Older events no longer exist; resync from the retained floor."
                    with
                      w=fill
                      size=12.0
                      @text-warning
                  button -> resync_history
                    with
                      label="Resync from the retained floor"
                      h=26.0
                      p=5.0
                      @outline_action
                    text "Resync" size=11.5
            // THE PAGE, SAID OUT LOUD: what is on screen against the
            // conversation's own tip, so a tail is never hidden in silence.
            if !empty(messaging.conversation) && messaging.may_read && !messaging.history_gap
              col w=fill gap=8.0
                row w=fill gap=6.0 align=center
                  text page_note(messaging.messages, messaging.from_seq, messaging.next_seq) #page-note
                    with
                      size=11.0
                      @text-hint
                      @font-mono
                  space w=fill
                  if messaging.more_before
                    button -> page_older
                      with
                        label="Older messages"
                        h=26.0
                        p=5.0
                        @outline_action
                      text "Older" size=11.5
                  if messaging.more_after
                    button -> page_newer
                      with
                        label="Newer messages"
                        h=26.0
                        p=5.0
                        @outline_action
                      text "Newer" size=11.5
                  button -> page_newest
                    with
                      label="Newest messages"
                      h=26.0
                      p=5.0
                      @outline_action
                    text "Newest" size=11.5
                if empty(messaging.messages) && messaging.answered
                  box #no-messages
                    with
                      w=fill
                      p=22.0
                      align-x=center
                      border=border
                      border-w=1.0
                      r=10.0
                    text "No messages in the retained range this page covers."
                      with
                        size=12.5
                        @text-meta
                for message in messaging.messages
                  col
                    with
                      w=fill
                      gap=5.0
                      px=13.0
                      py=11.0
                    row w=fill gap=7.0 align=center
                      text message.sender
                        with
                          size=12.5
                          @text-fg
                          @font-semibold
                      text "→"
                        with
                          size=11.0
                          @text-hint
                      text message.recipient
                        with
                          size=12.0
                          @text-meta
                          @font-mono
                      box
                        with
                          px=7.0
                          py=2.0
                          bg=elevated
                          r=5.0
                        text kind_label(message.kind)
                          with
                            size=10.0
                            @text-secondary_fg
                            @font-mono
                            @font-semibold
                      space w=fill
                      text message.seq
                        with
                          size=10.0
                          @text-hint
                          @font-mono
                    // DELIVERY, AND WHAT IT DOES NOT MEAN. The chip is the
                    // committed state; the sentence beside it is the limit.
                    row w=fill gap=6.0 align=center
                      if !delivery_unsettled(message.delivery)
                        box
                          with
                            px=8.0
                            py=3.0
                            bg=success_bg
                            border=success_line
                            border-w=1.0
                            r=6.0
                          text delivery_label(message.delivery)
                            with
                              size=9.0
                              @text-success
                              @font-mono
                              @font-semibold
                      if delivery_unsettled(message.delivery)
                        box
                          with
                            px=8.0
                            py=3.0
                            bg=warning_bg
                            border=warning_line
                            border-w=1.0
                            r=6.0
                          text delivery_label(message.delivery)
                            with
                              size=9.0
                              @text-warning
                              @font-mono
                              @font-semibold
                      if !empty(message.delivery_reason)
                        text message.delivery_reason
                          with
                            size=10.0
                            @text-hint
                            @font-mono
                    text delivery_note(message.delivery)
                      with
                        w=fill
                        size=10.5
                        @text-caption
                    text message.body
                      with
                        w=fill
                        size=12.5
                        @text-fg
                    if !empty(body_note(message.body_bytes, message.shown_bytes))
                      text body_note(message.body_bytes, message.shown_bytes)
                        with
                          size=10.0
                          @text-hint
                          @font-mono
                    if !empty(message.references)
                      text message.references
                        with
                          w=fill
                          size=10.5
                          @text-hint
                          @font-mono
                    if !empty(message.task)
                      text task_note(message.task, message.task_attempt)
                        with
                          w=fill
                          size=10.5
                          @text-warning
                          @font-mono
                    if message.reply_to > 0
                      text reply_note(message.reply_to)
                        with
                          size=10.0
                          @text-hint
                          @font-mono
                    if messaging.may_send
                      row w=fill gap=6.0 align=center
                        space w=fill
                        button -> reply_to_message(message.seq)
                          with
                            label="Reply to this message"
                            h=24.0
                            p=4.0
                            @outline_action
                          text "Reply" size=11.0
                  box
                    with
                      w=fill
                      h=1.0
                      bg=muted_bg
                    space w=1.0 h=1.0
            // THE COMPOSER. One recipient per send — group delivery is
            // per-recipient messages under the same conversation. An observer
            // sees this pane and is told why it cannot send.
            if !empty(messaging.conversation) && messaging.may_read && !messaging.may_send
              text "You hold an observer seat on this conversation: you receive it and do not send on it."
                with
                  w=fill
                  size=11.5
                  @text-caption
            if !empty(messaging.conversation) && messaging.may_send
              col #composer
                with
                  w=fill
                  gap=7.0
                text "Send"
                  with
                    size=12.5
                    @text-fg
                    @font-semibold
                text "A task update names a task and the attempt it addresses, which an attached service holds and this panel does not — compose one from the service."
                  with
                    w=fill
                    size=11.0
                    @text-caption
                row w=fill gap=6.0 align=center
                  pick recipients(messaging.roster) msg_recipient #recipient -> pick_recipient _
                    with
                      hint="recipient…"
                      w=fill
                  pick compose_kinds(msg_reply_to) msg_kind #kind -> pick_message_kind _
                    with
                      hint="kind"
                      w=150.0
                if msg_reply_to > 0
                  row w=fill gap=6.0 align=center
                    text reply_note(msg_reply_to)
                      with
                        size=11.0
                        @text-hint
                        @font-mono
                    space w=fill
                    button -> clear_reply
                      with
                        label="Clear the reply target"
                        h=24.0
                        p=4.0
                        @outline_action
                      text "Clear reply" size=11.0
                input "" #message-body <-> msg_body
                  with
                    label="Message body"
                    hint="what to send…"
                    disabled=messaging.sending
                    w=fill
                    p=8.0
                    text-size=13.0
                    line-h=1.3
                    @control
                  active bg=elevated border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                  hovered bg=elevated border=fg/21
                  disabled bg=muted_bg/54 value=muted
                // OVERSIZED INPUT IS REFUSED BEFORE ADMISSION, never truncated
                // into a message the sender did not write.
                if !empty(body_refusal(msg_body, messaging.max_body_bytes))
                  text body_refusal(msg_body, messaging.max_body_bytes) #body-refusal
                    with
                      w=fill
                      size=11.0
                      @text-danger
                // A FAILED OR STALE SEND KEEPS THE DRAFT. Nothing is appended
                // to the list on the way out — a message appears when the
                // network says it was admitted.
                if !empty(messaging.send_error)
                  text messaging.send_error #send-error
                    with
                      w=fill
                      size=11.5
                      @text-danger
                button -> submit_message
                  with
                    label="Send message"
                    disabled=(!send_ready(messaging.may_send, messaging.sending, msg_recipient, msg_kind, msg_body, messaging.max_body_bytes))
                    w=fill
                    p=10.0
                    @primary_action
                  text "Send" size=12.5
