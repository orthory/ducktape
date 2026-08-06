// The roster detail panel: one member record, drawn from what the valset and
// the agent registry actually hold — a 56px header, a centred identity block,
// machine fact rows, then the writes this node is allowed to attempt.
//
// THE CHAIN'S OWN WORDS, NOT THE ARTIFACT'S. The handoff prints ADMIN /
// MAINTAINER / VIEWER. This chain grants validator / resident / agent and has
// no admin or maintainer tier at all, so every label here is the chain's. The
// artifact's `needs quorum to change` caption over an agent's grants is the
// same class of error in the other direction — registry mutation is
// OWNER-gated (`AgentMsg::PauseAgent`/`ResumeAgent`), never quorum-gated.
//
// PRESENCE IS TWO DIFFERENT FACTS, and the panel must not conflate them.
// `MemberRow.live` is a mesh liveness bit for a human member and the registry's
// `status == "active"` for an agent (backend.rs load_members), so an agent's
// strip reads active/paused and a person's reads live/offline. The artifact's
// idle and invited states have no source anywhere and are not drawn.
//
// NO AGENT REGISTRY PANEL HERE. The 322px `AgentDetail` record (skills, granted
// capabilities, allowed actions, recent runs) was built against `AgentRow` +
// `RunRow` but nothing could ever open it: the Agents screen mounts `AgentCard`
// only, `agents_selected` has no reader and there is no `open_agent` route. A
// component no code path reaches is dead code, so it is deleted rather than
// left reading as delivered — restore it from 9de754551 the moment the agents
// screen grows a per-agent route.

// The 312px member record. `admin` is members_is_admin — this node holds a
// quorum seat — and it gates the membership proposals, not the whole panel:
// a non-admin still sees WHY the writes are refused.
component MemberDetail(member:MemberRow, admin:bool)
  // Every route out of this panel is a named event, so the panel stays closed
  // over the app. The names are the app handlers the screen routes them back
  // to, and the payloads are those handlers' own arities — an empty key closes
  // the panel, `agent_set_status` carries the DESIRED paused state, and
  // `gov_propose` carries the action then the target key.
  emits
    open_member(str)
    copy_to_clipboard(str, str)
    agent_set_status(str, bool)
    gov_propose(str, str)
  row #root h=fill
    box
      with
        w=1.0
        h=fill
        bg=separator
      space w=1.0 h=1.0
    box
      with
        w=312.0
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
                wrap=none
                font=display
                @text-fg
            button -> emit(open_member, "")
              with
                label="Close member"
                w=24.0
                h=24.0
                p=0.0
                @icon_action
              text "×"
                with
                  size=16.0
                  wrap=none
                  font=code_medium
                  @text-meta
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
                PrincipalAvatar
                  with
                    initials=initials_of(member.label)
                    is_agent=true
                    plate=54.0
                    ink=16.0
                    ring=""
              if !member.is_agent
                PrincipalAvatar
                  with
                    initials=initial_of(member.label)
                    is_agent=false
                    plate=54.0
                    ink=20.0
                    ring=""
              box pt=11.0
                text member.label
                  with
                    size=16.0
                    wrap=none
                    font=display
                    @text-fg
              box pt=5.0
                row gap=6.0 align=center
                  MemberPresence live=member.live is_agent=member.is_agent
                  RoleMarker role=member.role
            col
              with
                w=fill
                pt=18.0
                gap=8.0
              // an agent's `key` is its REGISTRY ID (`reviewer-bot`), not a
              // key at all — it is what `agent_set_status` addresses. Only a
              // human member's row carries a node public key.
              if member.is_agent
                MemberFactRow label="agent id" value=member.key
              if !member.is_agent
                MemberFactRow label="public key" value=member.key
              // `model` would be a lie: this is `AgentRecord.capability`, an
              // open-set dispatch tag; which binary and which model serve it
              // are host policy the chain never sees.
              if !empty(member.model)
                MemberFactRow label="capability" value=member.model
            col
              with
                w=fill
                pt=16.0
                gap=8.0
              // your own row's only action: the key you hand someone to invite you
              if member.is_this_node
                button -> emit(copy_to_clipboard, member.key, "Key copied")
                  with
                    label="Copy your key"
                    w=fill
                    @secondary_action
                    @px-12px
                    @py-10px
                    @rounded-9px
                  PanelActionLabel
                    with
                      label="Copy your key"
                      tag=""
                      danger=false
              // an agent is paused and resumed by its OWNER, immediately — no ballot
              if member.is_agent && member.live
                button -> emit(agent_set_status, member.key, true)
                  with
                    label="Pause agent"
                    w=fill
                    @secondary_action
                    @px-12px
                    @py-10px
                    @rounded-9px
                  PanelActionLabel
                    with
                      label="Pause agent"
                      tag=""
                      danger=false
              if member.is_agent && !member.live
                button -> emit(agent_set_status, member.key, false)
                  with
                    label="Resume agent"
                    w=fill
                    @secondary_action
                    @px-12px
                    @py-10px
                    @rounded-9px
                  PanelActionLabel
                    with
                      label="Resume agent"
                      tag=""
                      danger=false
              // STATED AS THE RULE, NOT AS A REFUSAL. `MemberRow` carries no
              // ownership bit, so this panel cannot tell the owner from anyone
              // else and must not accuse the one signer the button works for.
              if member.is_agent
                GateNote
                  with
                    reason="Pause and resume are owner-gated writes."
                    next="The registry accepts them only from the signer that registered this agent."
              // membership moves are ballots: this opens the proposal, it does not settle it
              if admin && !member.is_agent && member.role == "resident"
                button -> emit(gov_propose, "add_validator", member.key)
                  with
                    label="Promote to validator"
                    w=fill
                    @secondary_action
                    @px-12px
                    @py-10px
                    @rounded-9px
                  PanelActionLabel
                    with
                      label="Promote to validator"
                      tag="needs quorum"
                      danger=false
              if admin && !member.is_agent && member.role == "validator" && !member.is_this_node
                button -> emit(gov_propose, "remove_validator", member.key)
                  with
                    label="Remove from the validator set"
                    w=fill
                    @secondary_action
                    @px-12px
                    @py-10px
                    @rounded-9px
                    @border-alert_line
                  PanelActionLabel
                    with
                      label="Remove from the validator set"
                      tag="needs quorum"
                      danger=true
              if !admin && !member.is_this_node && !member.is_agent
                GateNote
                  with
                    reason="Only a validator node may open a membership proposal."
                    next="This node holds no quorum seat, so the network refuses the write."

// A panel button's face: the label, and the right-aligned marker that says the
// write will open a ballot rather than settle on the spot.
component PanelActionLabel(label:str, tag:str, danger:bool)
  row #root
    with
      w=fill
      gap=8.0
      align=center
    if danger
      text label
        with
          w=fill
          size=12.0
          wrap=none
          font=display
          @text-alert_fg
    if !danger
      text label
        with
          w=fill
          size=12.0
          wrap=none
          font=display
          @text-accent_fg
    if tag != ""
      text tag
        with
          size=9.0
          wrap=none
          font=code_semibold
          @text-label

// Two words per principal, never four. A person is live or offline on the
// mesh; an agent is active or paused in the registry — the same word the
// Agents screen stamps on its card, so one record never reads two ways.
component MemberPresence(live:bool, is_agent:bool)
  row #root gap=6.0 align=center
    if live
      PulseDot plate=7.0 tone="success"
    if !live
      box
        with
          w=7.0
          h=7.0
          bg=presence_off
          r=3.5
        space w=1.0 h=1.0
    if is_agent && live
      text "active"
        with
          size=12.0
          wrap=none
          @text-input
    if is_agent && !live
      text "paused"
        with
          size=12.0
          wrap=none
          @text-input
    if !is_agent && live
      text "live"
        with
          size=12.0
          wrap=none
          @text-input
    if !is_agent && !live
      text "offline"
        with
          size=12.0
          wrap=none
          @text-input

// A bordered machine-fact line. The value takes the rest of the row and wraps,
// because a full 64-hex public key does not fit a 312px panel on one line.
component MemberFactRow(label:str, value:str)
  box #root
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
      text label
        with
          size=11.0
          wrap=none
          font=code_medium
          @text-meta
      text value
        with
          w=fill
          size=11.0
          font=code_medium
          @text-secondary_fg
