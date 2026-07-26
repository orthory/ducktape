// The repeated pieces of the canonical console: the shapes that appear on more
// than one screen. Anything used once stays inline in `view.ice`.

// An all-caps section eyebrow — 9px mono over the widest tracking in the scale.
component GroupLabel(label:str)
  text label #root size=9.0 wrap=none font=code_semibold @text-label

// The bordered card a settings/detail group lives in. Children draw their own
// separators, so the card only owns the outline and the clip.
component GroupCard()
  box #root w=fill bg=surface border=card_line border-w=1.0 r=11.0 clip=true
    col w=fill
      slot

// One label/value line inside a GroupCard. `last` drops the rule so the card's
// own border finishes the stack.
component KeyValueRow(label:str, value:str, last:bool)
  col #root w=fill
    box w=fill px=15.0 py=13.0
      row w=fill gap=10.0 align=center
        text label size=12.5 wrap=none @text-accent_fg
        space w=fill
        text value size=12.0 wrap=none font=code_medium @text-secondary_fg
    if !last
      box w=fill h=1.0 bg=elevated
        space w=1.0 h=1.0

// A row whose value is a caption rather than a machine value.
component NoteRow(label:str, note:str, last:bool)
  col #root w=fill
    box w=fill px=15.0 py=13.0
      col w=fill gap=1.0
        text label size=12.5 @text-accent_fg
        text note size=12.5 @text-meta
    if !last
      box w=fill h=1.0 bg=elevated
        space w=1.0 h=1.0

// The 26px round plate a human wears. Agents get the 7px-radius square below —
// the artifact never mixes the two.
component PersonAvatar(initials:str, plate:f64, ink:f64)
  box #root w=plate h=plate align-x=center align-y=center bg=avatar_bg r=(plate / 2.0)
    text initials size=ink wrap=none font=display @text-muted

component AgentAvatar(initials:str, plate:f64, ink:f64)
  box #root w=plate h=plate align-x=center align-y=center bg=primary r=7.0
    text initials size=ink wrap=none font=code_semibold @text-toast_fg

// The status dot that precedes a machine reading.
component Dot(plate:f64)
  box #root w=plate h=plate bg=success_dot r=(plate / 2.0)
    space w=1.0 h=1.0

// The tinted count chip the artifact pins beside a screen title.
component CountChip(label:str)
  box #root px=8.0 py=3.0 bg=brand_bg r=6.0
    text label size=11.0 wrap=none font=code_medium @text-brand

// A dashed empty plate — what a screen shows when its list is legitimately
// empty, as opposed to not loaded yet.
component EmptyPlate(message:str)
  box #root w=fill p=30.0 align-x=center bg=transparent border=border border-w=1.0 r=12.0
    text message size=13.0 @text-meta

// The screen-level heading pair used by the padded screens (Approvals,
// Settings, Explorer) that have no 56px header bar.
component ScreenTitle(title:str, detail:str)
  col #root w=fill gap=3.0
    text title size=16.0 wrap=none font=display @text-primary
    if detail != ""
      box w=fill max-w=620.0
        text detail size=12.5 line-h=1.5 @text-caption

// A roster row: 32px plate, name over key line, role marker, standing on the
// right. Shape carries authorship — a person is round, an agent is a square.
component MemberRowCard(member:MemberRow)
  col #root w=fill
    box w=fill pl=14.0 pr=14.0 pt=12.0 pb=12.0
      row w=fill gap=12.0 align=center
        if member.role == "agent"
          AgentAvatar initials=initial_of(member.label) plate=32.0 ink=10.0
        if member.role != "agent"
          PersonAvatar initials=initial_of(member.label) plate=32.0 ink=12.0
        col w=fill gap=2.0
          row gap=6.0 align=center
            text member.label size=13.5 wrap=none font=display @text-fg
            if member.is_this_node
              text "you" size=9.0 wrap=none font=code_semibold @text-meta
          text member.key size=10.0 wrap=none font=code_semibold @text-hint
        RoleMarker role=member.role
    box w=fill h=1.0 bg=muted_bg
      space w=1.0 h=1.0

// The role marker's tone is its meaning: ink for authority, accent for a
// machine principal, hairline for everyone else.
component RoleMarker(role:str)
  row #root gap=6.0 align=center
    match role
      "validator"
        box px=7.0 py=3.0 bg=primary r=5.0
          text "VALIDATOR" size=9.0 wrap=none font=code_semibold @text-primary_fg
      "agent"
        box px=7.0 py=3.0 bg=brand_bg border=brand_line border-w=1.0 r=5.0
          text "AGENT" size=9.0 wrap=none font=code_semibold @text-brand
      "resident"
        box px=7.0 py=3.0 bg=surface border=control_line border-w=1.0 r=5.0
          text "RESIDENT" size=9.0 wrap=none font=code_semibold @text-input
      _
        box px=7.0 py=3.0 bg=warning_bg border=warning_line border-w=1.0 r=5.0
          text role size=9.0 wrap=none font=code_semibold @text-warning

// An agent registry row: who it is, what capability it holds, who owns it, and
// whether it is live. The artifact lists agents — it does not card them.
component AgentCard(agent:AgentRow)
  col #root w=fill
    box w=fill pl=14.0 pr=14.0 pt=13.0 pb=13.0
      row w=fill gap=13.0 align=center
        AgentAvatar initials=initial_of(agent.name) plate=34.0 ink=11.0
        col w=fill gap=3.0
          row w=fill gap=8.0 align=center
            text agent.name size=13.5 wrap=none font=display @text-fg
            box px=7.0 py=2.0 bg=elevated r=5.0
              text agent.capability size=10.0 wrap=none font=code_semibold @text-secondary_fg
          if !empty(agent.actions)
            text agent.actions w=fill size=10.5 wrap=none font=code_medium @text-meta
          if empty(agent.actions)
            text "no actions granted" size=10.5 wrap=none font=code_medium @text-meta
        text agent.owner size=10.5 wrap=none font=code_medium @text-hint
        if agent.status == "active"
          box px=8.0 py=3.0 bg=success_bg border=success_line border-w=1.0 r=6.0
            row gap=5.0 align=center
              box w=5.0 h=5.0 bg=success_dot r=2.5
                space w=1.0 h=1.0
              text "ACTIVE" size=9.0 wrap=none font=code_semibold @text-success
        if agent.status != "active"
          box px=8.0 py=3.0 bg=warning_bg border=warning_line border-w=1.0 r=6.0
            row gap=5.0 align=center
              box w=5.0 h=5.0 bg=warning_dot r=2.5
                space w=1.0 h=1.0
              text agent.status size=9.0 wrap=none font=code_semibold @text-warning
    box w=fill h=1.0 bg=muted_bg
      space w=1.0 h=1.0

// An open proposal: what it does, who opened it, and how close the electorate
// is to settling it. The dots are the tally — the number is the confirmation.
component ProposalCard(proposal:ProposalRow, busy:bool)
  box #root w=fill p=16.0 bg=surface border=card_line border-w=1.0 r=12.0
    col w=fill gap=5.0
      row w=fill gap=7.0 align=center
        box px=6.0 py=2.0 bg=brand_bg border=brand_line border-w=1.0 r=4.0
          text proposal.action size=9.0 wrap=none font=code_semibold @text-brand
        text proposal.id w=fill size=13.5 wrap=none font=display @text-primary
      row w=fill gap=6.0 align=center
        text "proposed by" size=12.5 wrap=none @text-caption
        text proposal.proposer size=12.0 wrap=none font=code_medium @text-secondary_fg
        text "·" size=12.5 wrap=none @text-caption
        text "deadline" size=12.5 wrap=none @text-caption
        text proposal.deadline size=12.0 wrap=none font=code_medium @text-secondary_fg
      row w=fill gap=13.0 align=center pt=9.0
        row gap=5.0 align=center
          for seat in quorum_dots(proposal.approvals, proposal.electorate)
            QuorumDot filled=seat.filled
        text proposal.approvals size=12.0 wrap=none font=code_medium @text-success
        text "of" size=12.5 wrap=none @text-caption
        text proposal.electorate size=12.0 wrap=none font=code_medium @text-secondary_fg
        if proposal.rejections > 0
          text proposal.rejections size=12.0 wrap=none font=code_medium @text-danger
        if proposal.rejections > 0
          text "against" size=12.5 wrap=none @text-caption
        space w=fill
        button "Reject" disabled=busy h=30.0 p=7.0 @outline_action -> gov_vote(proposal.id, false)
        button "Approve" disabled=busy h=30.0 p=7.0 @primary_action -> gov_vote(proposal.id, true)
        button "Settle" disabled=busy h=30.0 p=7.0 @secondary_action -> gov_execute(proposal.id)

component QuorumDot(filled:bool)
  col #root
    if filled
      box w=13.0 h=13.0 bg=success_dot r=6.5
        space w=1.0 h=1.0
    if !filled
      box w=13.0 h=13.0 bg=surface border=avatar_bg border-w=1.0 r=6.5
        space w=1.0 h=1.0

// A settled proposal: a tick, the action, and the tally it closed on.
component SettledProposalRow(proposal:ProposalRow)
  box #root w=fill px=15.0 py=13.0 bg=surface border=separator border-w=1.0 r=10.0
    row w=fill gap=11.0 align=center
      box w=19.0 h=19.0 align-x=center align-y=center bg=success_bg border=success_line border-w=1.0 r=9.5
        text "✓" size=9.0 wrap=none font=code_semibold @text-success
      text proposal.id size=13.0 wrap=none font=medium @text-muted
      text proposal.action size=12.0 wrap=none font=code_medium @text-hint
      space w=fill
      text proposal.status size=11.0 wrap=none font=code_medium @text-meta


// One alert: a severity dot, the title with its severity marker, the body, and
// when it landed. Unread rows sit on a warmer plate than read ones.
component BellRow(item:BellItem)
  box #root w=fill pl=9.0 pr=9.0 pt=9.0 pb=10.0 r=9.0
    row w=fill gap=9.0 align=start
      if item.read
        box w=7.0 h=7.0 bg=avatar_bg r=3.5
          space w=1.0 h=1.0
      if !item.read
        box w=7.0 h=7.0 bg=info_dot r=3.5
          space w=1.0 h=1.0
      col w=fill gap=3.0
        row w=fill gap=7.0 align=center
          text item.kind w=fill size=12.0 wrap=none @text-primary
          box px=4.0 py=1.0 bg=info_bg border=info_line border-w=1.0 r=4.0
            text item.source size=9.0 wrap=none font=code_semibold @text-info
        text item.body w=fill size=12.0 line-h=1.45 @text-input
      text item.height size=9.5 wrap=none font=display @text-label
