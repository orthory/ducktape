// The repeated pieces of the canonical console: the shapes that appear on more
// than one screen. Anything used once stays inline in `view.ice`.

// An all-caps section eyebrow — 9px mono over the widest tracking in the scale.
component GroupLabel(label:str)
  text label #root
    with
      size=9.0
      wrap=none
      font=code_semibold
      @text-label

// The bordered card a settings/detail group lives in. Children draw their own
// separators, so the card only owns the outline and the clip.
component GroupCard()
  box #root
    with
      w=fill
      bg=surface
      border=card_line
      border-w=1.0
      r=11.0
      clip=true
    col w=fill
      slot

// One label/value line inside a GroupCard. `last` drops the rule so the card's
// own border finishes the stack.
component KeyValueRow(label:str, value:str, last:bool)
  col #root w=fill
    box
      with
        w=fill
        px=15.0
        py=13.0
      row
        with
          w=fill
          gap=10.0
          align=center
        text label
          with
            size=12.5
            wrap=none
            @text-accent_fg
        space w=fill
        text value
          with
            size=12.0
            wrap=none
            font=code_medium
            @text-secondary_fg
    if !last
      box
        with
          w=fill
          h=1.0
          bg=elevated
        space w=1.0 h=1.0

// The two shape-named entry points into `PrincipalAvatar`. The shape rule and
// the radius ladder live there — these only spell the discriminant, so a call
// site that already knows it is drawing a person keeps reading as one.
component PersonAvatar(initials:str, plate:f64, ink:f64)
  col #root
    PrincipalAvatar
      with
        initials
        is_agent=false
        plate
        ink
        ring=""

component AgentAvatar(initials:str, plate:f64, ink:f64)
  col #root
    PrincipalAvatar
      with
        initials
        is_agent=true
        plate
        ink
        ring=""

// The status dot that precedes a machine reading.
component Dot(plate:f64)
  box #root
    with
      w=plate
      h=plate
      bg=success_dot
      r=(plate / 2.0)
    space w=1.0 h=1.0

// The tinted count chip the artifact pins beside a screen title.
component CountChip(label:str)
  box #root
    with
      px=8.0
      py=3.0
      bg=brand_bg
      r=6.0
    text label
      with
        size=11.0
        wrap=none
        font=code_medium
        @text-brand

// A dashed empty plate — what a screen shows when its list is legitimately
// empty, as opposed to not loaded yet.
component EmptyPlate(message:str)
  box #root
    with
      w=fill
      p=30.0
      align-x=center
      bg=transparent
      border=border
      border-w=1.0
      r=12.0
    text message size=13.0 @text-meta

// The screen-level heading pair used by the padded screens (Approvals,
// Settings, Explorer) that have no 56px header bar.
component ScreenTitle(title:str, detail:str)
  col #root w=fill gap=3.0
    text title
      with
        size=16.0
        wrap=none
        font=display
        @text-primary
    if detail != ""
      box w=fill max-w=620.0
        text detail
          with
            size=12.5
            line-h=1.5
            @text-caption

// A roster row: 32px plate, name over key line, role marker, standing on the
// right. Shape carries authorship — a person is round, an agent is a square.
component MemberRowCard(member:MemberRow)
  col #root w=fill
    box
      with
        w=fill
        pl=14.0
        pr=14.0
        pt=12.0
        pb=12.0
      row
        with
          w=fill
          gap=12.0
          align=center
        if member.is_agent
          PrincipalAvatar
            with
              initials=initials_of(member.label)
              is_agent=true
              plate=32.0
              ink=10.0
              ring=""
        if !member.is_agent
          PrincipalAvatar
            with
              initials=initial_of(member.label)
              is_agent=false
              plate=32.0
              ink=12.0
              ring=""
        col w=fill gap=2.0
          row gap=6.0 align=center
            text member.label
              with
                size=13.5
                wrap=none
                font=display
                @text-fg
            // the artifact writes `you` in SANS medium and the key line in mono
            // regular. The type-scale guard fixes the SIZE only and asserts no
            // size→face pairing (main.rs), so the face here is a free choice;
            // these steps are the nearest ones on the scale.
            if member.is_this_node
              text "you"
                with
                  size=9.5
                  wrap=none
                  font=display
                  @text-meta
          text member.key
            with
              size=10.5
              wrap=none
              font=code_medium
              @text-hint
        RoleMarker role=member.role
    box
      with
        w=fill
        h=1.0
        bg=muted_bg
      space w=1.0 h=1.0

// The role marker's tone is its meaning: ink for authority, accent for a
// machine principal, hairline for everyone else.
component RoleMarker(role:str)
  row #root gap=6.0 align=center
    match role
      "validator"
        box
          with
            px=7.0
            py=3.0
            bg=primary
            r=5.0
          text "VALIDATOR"
            with
              size=9.0
              wrap=none
              font=code_semibold
              @text-primary_fg
      "agent"
        box
          with
            px=7.0
            py=3.0
            bg=brand_bg
            border=brand_line
            border-w=1.0
            r=5.0
          text "AGENT"
            with
              size=9.0
              wrap=none
              font=code_semibold
              @text-brand
      "resident"
        box
          with
            px=7.0
            py=3.0
            bg=surface
            border=control_line
            border-w=1.0
            r=5.0
          text "RESIDENT"
            with
              size=9.0
              wrap=none
              font=code_semibold
              @text-input
      _
        box
          with
            px=7.0
            py=3.0
            bg=warning_bg
            border=warning_line
            border-w=1.0
            r=5.0
          text role
            with
              size=9.0
              wrap=none
              font=code_semibold
              @text-warning

// An agent registry row: who it is, what capability it holds, who owns it, and
// whether it is live. The artifact lists agents — it does not card them.
component AgentCard(agent:AgentRow)
  col #root w=fill
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
        PrincipalAvatar
          with
            initials=agent.initials
            is_agent=true
            plate=34.0
            ink=11.0
            ring=""
        col w=fill gap=3.0
          row
            with
              w=fill
              gap=8.0
              align=center
            text agent.name
              with
                size=13.5
                wrap=none
                font=display
                @text-fg
            box
              with
                px=7.0
                py=2.0
                bg=elevated
                r=5.0
              text agent.capability
                with
                  size=10.0
                  wrap=none
                  font=code_semibold
                  @text-secondary_fg
          // what it may do, counted — never a comma-joined dump of grant names
          row
            with
              w=fill
              gap=5.0
              align=center
            text len(agent.skills)
              with
                size=10.5
                wrap=none
                font=code_medium
                @text-meta
            text "skills ·"
              with
                size=10.5
                wrap=none
                font=code_medium
                @text-meta
            text len(agent.caps)
              with
                size=10.5
                wrap=none
                font=code_medium
                @text-meta
            text "grants · owner"
              with
                size=10.5
                wrap=none
                font=code_medium
                @text-meta
            OwnerHandle handle=agent.owner_handle
        AgentStatusChip status=agent.status
    box
      with
        w=fill
        h=1.0
        bg=muted_bg
      space w=1.0 h=1.0

// A member handle always wears its sigil; an unowned record says so rather
// than leaving the slot blank.
component OwnerHandle(handle:str)
  row #root gap=0.0 align=center
    if empty(handle)
      text "unowned"
        with
          size=10.5
          wrap=none
          font=code_medium
          @text-hint
    if !empty(handle)
      text "@"
        with
          size=10.5
          wrap=none
          font=code_medium
          @text-hint
    if !empty(handle)
      text handle
        with
          size=10.5
          wrap=none
          font=code_medium
          @text-hint

// Standing, in the registry's own words, printed the way a status reads:
// upper-case, because the wire hands us `active` / `paused` in snake case.
component AgentStatusChip(status:str)
  col #root
    match status
      "active"
        box
          with
            px=8.0
            py=3.0
            bg=success_bg
            border=success_line
            border-w=1.0
            r=6.0
          row gap=5.0 align=center
            PulseDot plate=5.0 tone="success"
            text "ACTIVE"
              with
                size=9.0
                wrap=none
                font=code_semibold
                @text-success
      "paused"
        box
          with
            px=8.0
            py=3.0
            bg=warning_bg
            border=warning_line
            border-w=1.0
            r=6.0
          row gap=5.0 align=center
            PulseDot plate=5.0 tone="warning"
            text "PAUSED"
              with
                size=9.0
                wrap=none
                font=code_semibold
                @text-warning
      _
        box
          with
            px=8.0
            py=3.0
            bg=warning_bg
            border=warning_line
            border-w=1.0
            r=6.0
          row gap=5.0 align=center
            PulseDot plate=5.0 tone="warning"
            text status
              with
                size=9.0
                wrap=none
                font=code_semibold
                @text-warning

// An open proposal: what it does, who opened it, and how close the electorate
// is to settling it. The dots are the tally — the number is the confirmation.
component ProposalCard(proposal:ProposalRow, busy:bool)
  emits
    gov_vote(str, bool)
    gov_execute(str)
  box #root
    with
      w=fill
      p=16.0
      bg=surface
      border=border
      border-w=1.0
      r=12.0
    col w=fill gap=5.0
      row
        with
          w=fill
          gap=7.0
          align=center
        ProposalKindPill action=proposal.action tone=proposal_kind_tone(proposal.action)
        text proposal.id
          with
            w=fill
            size=14.0
            wrap=none
            font=display
            @text-primary
      // one meta line: who opened it, when it lapses, and what it actually does
      row
        with
          w=fill
          gap=4.0
          align=center
        text "proposed by"
          with
            size=12.0
            wrap=none
            @text-caption
        row gap=0.0 align=center
          text "@"
            with
              size=12.0
              wrap=none
              @text-secondary_fg
          text proposal.proposer
            with
              size=12.0
              wrap=none
              @text-secondary_fg
        // the contract signed `expires_in(deadline)`; what landed is
        // `expires_in_blocks(deadline_height, height)`, and this card has no
        // chain height to feed it. Print the deadline height honestly until
        // the screen threads one in — never a relative phrase we cannot compute.
        text "· expires at h"
          with
            size=12.0
            wrap=none
            @text-caption
        text proposal.deadline
          with
            size=12.0
            wrap=none
            font=code_medium
            @text-secondary_fg
        if !empty(proposal.detail)
          text "·"
            with
              size=12.0
              wrap=none
              @text-caption
        if !empty(proposal.detail)
          text proposal.detail
            with
              w=fill
              size=12.0
              wrap=none
              font=code_medium
              @text-secondary_fg
      // the dots count the frozen voting rule, not the electorate
      row
        with
          w=fill
          gap=13.0
          align=center
          pt=9.0
        row gap=5.0 align=center
          for seat in quorum_dots(proposal.approvals, proposal.required_yes)
            QuorumDot filled=seat.filled
        TallyReading
          with
            label=tally_label(proposal.approvals, proposal.required_yes)
            tone=tally_tone(proposal.approvals, proposal.required_yes)
        text tally_note(proposal.approvals, proposal.required_yes)
          with
            size=12.0
            wrap=none
            @text-meta
        if proposal.rejections > 0
          text proposal.rejections
            with
              size=12.0
              wrap=none
              font=code_medium
              @text-danger
        if proposal.rejections > 0
          text "against"
            with
              size=12.0
              wrap=none
              @text-caption
        space w=fill
        // the artifact's slot holds exactly two buttons; Settle appears only
        // once the rule is met, because that is the only moment it can succeed
        row gap=8.0 align=center
          button -> emit(gov_vote, proposal.id, false)
            with
              label="Reject"
              disabled=busy
              @outline_action
              @px-15px
              @py-7px
              @text-secondary_fg
              @border-control_line
              @rounded-8px
            text "Reject"
              with
                size=12.0
                wrap=none
                font=display
                @text-secondary_fg
          if proposal.approvals < proposal.required_yes
            button -> emit(gov_vote, proposal.id, true)
              with
                label="Approve"
                disabled=busy
                @primary_action
                @px-17px
                @py-7px
                @rounded-8px
              text approve_label(proposal.approvals, proposal.required_yes)
                with
                  size=12.0
                  wrap=none
                  font=display
                  @text-primary_fg
          if proposal.approvals >= proposal.required_yes
            button -> emit(gov_execute, proposal.id)
              with
                label="Settle"
                disabled=busy
                @secondary_action
                @px-17px
                @py-7px
                @rounded-8px
              text "Settle →"
                with
                  size=12.0
                  wrap=none
                  font=display
                  @text-secondary_fg

// ACCESS-class proposals wear the terracotta pair; everything else is neutral.
// Two tones, no border — the plate is the whole signal.
component ProposalKindPill(action:str, tone:str)
  col #root
    if tone == "access"
      box
        with
          px=6.0
          py=2.0
          bg=brand_bg
          r=4.0
        text action
          with
            size=9.0
            wrap=none
            font=code_semibold
            @text-brand
    if tone != "access"
      box
        with
          px=6.0
          py=2.0
          bg=elevated
          r=4.0
        text action
          with
            size=9.0
            wrap=none
            font=code_semibold
            @text-avatar_fg_sm

// `3 / 4` in one mono run: grey until one signature from quorum, then green.
component TallyReading(label:str, tone:str)
  col #root
    if tone == "near"
      text label
        with
          size=12.0
          wrap=none
          font=code_semibold
          @text-success
    if tone != "near"
      text label
        with
          size=12.0
          wrap=none
          font=code_semibold
          @text-meta

// An unfilled seat is the unfinalized ring at dot scale — 1.5px, `presence_off`.
component QuorumDot(filled:bool)
  col #root
    if filled
      box
        with
          w=13.0
          h=13.0
          bg=success_dot
          r=6.5
        space w=1.0 h=1.0
    if !filled
      box
        with
          w=13.0
          h=13.0
          bg=surface
          border=presence_off
          border-w=1.5
          r=6.5
        space w=1.0 h=1.0

// A settled proposal: a tick, the title, and the tally it closed on. No
// mid-row action chip — the closing tally is the whole right-hand meta.
component SettledProposalRow(proposal:ProposalRow)
  box #root
    with
      w=fill
      px=15.0
      py=13.0
      bg=surface
      border=separator
      border-w=1.0
      r=10.0
    row
      with
        w=fill
        gap=11.0
        align=center
      box
        with
          w=19.0
          h=19.0
          align-x=center
          align-y=center
          bg=success_bg
          border=success_line
          border-w=1.0
          r=9.5
        text "✓"
          with
            size=9.0
            wrap=none
            font=code_semibold
            @text-success
      text proposal.id
        with
          size=13.0
          wrap=none
          font=medium
          @text-muted
      space w=fill
      row gap=5.0 align=center
        text proposal.status
          with
            size=11.0
            wrap=none
            font=code_medium
            @text-meta
        text "·"
          with
            size=11.0
            wrap=none
            font=code_medium
            @text-meta
        text tally_label(proposal.approvals, proposal.required_yes)
          with
            size=11.0
            wrap=none
            font=code_medium
            @text-meta
        // NO ✓ WITHOUT A HEIGHT BEHIND IT — the row draws a finality tick, so it
        // states the block it settled in. `settle_heights` folds it off the
        // module's own settle op; a row whose op predates that fold has 0 and
        // prints nothing rather than `h 0`.
        if proposal.settled_height > 0
          text "·"
            with
              size=11.0
              wrap=none
              font=code_medium
              @text-meta
        if proposal.settled_height > 0
          text height_label_short(proposal.settled_height)
            with
              size=11.0
              wrap=none
              font=code_medium
              @text-hint

// One alert: a severity dot, the title, the source that raised it, the body,
// and the block it landed in. Unread rows sit on a warmer plate than read ones
// AND pulse; a read row keeps the same severity colour, held still. `height` is
// a BLOCK, so it prints as one — this chain publishes no wall clock.
component BellRow(item:BellItem)
  col #root w=fill
    if item.read
      box
        with
          w=fill
          pl=9.0
          pr=9.0
          pt=9.0
          pb=10.0
          r=9.0
          bg=transparent
        BellBody item=item
    if !item.read
      box
        with
          w=fill
          pl=9.0
          pr=9.0
          pt=9.0
          pb=10.0
          r=9.0
          bg=unread_wash
        BellBody item=item

component BellBody(item:BellItem)
  row #root
    with
      w=fill
      gap=9.0
      align=start
    // The dot carries SEVERITY, which is what `bell_severity` was written for
    // and what the titlebar badge already keys on — a row that colours by read
    // state instead says the same blue for a failure and a mention. Read vs
    // unread is the plate and the pulse; the hue is the severity either way.
    if item.read
      StillDot plate=7.0 tone=bell_severity(item.kind)
    if !item.read
      PulseDot plate=7.0 tone=bell_severity(item.kind)
    col w=fill gap=3.0
      row
        with
          w=fill
          gap=7.0
          align=center
        text bell_title(item.kind)
          with
            w=fill
            size=12.0
            wrap=none
            @text-primary
        if item.height > 0
          text height_label_short(item.height)
            with
              size=9.5
              wrap=none
              font=code_medium
              @text-hint
        box
          with
            px=4.0
            py=1.0
            bg=info_bg
            border=info_line
            border-w=1.0
            r=4.0
          text item.source
            with
              size=9.0
              wrap=none
              font=code_semibold
              @text-info
      text item.body
        with
          w=fill
          size=12.0
          line-h=1.45
          @text-input

component StatusBadge(label:str)
  row align=center
    match label
      "active"
        Badge.Success label=label
      "paused"
        Badge.Warning label=label
      "open"
        Badge.Success label=label
      "closed"
        Badge.Destructive label=label
      "merged"
        Badge.Success label=label
      "passed"
        Badge.Success label=label
      "rejected"
        Badge.Destructive label=label
      "applied"
        Badge.Success label=label
      "discarded"
        Badge.Warning label=label
      _
        Badge.Outline label=label
