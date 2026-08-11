// The product patterns the Design System states as law, in ONE place each,
// so that no screen ever spells them itself. The finality stamp, the refusal
// plate, and the human-vs-agent shape all live here.
//
// ONE SANCTIONED SUBSTITUTION — settled, do not re-litigate per screen:
//
// GRADIENT -> FLAT SURFACE. The console is deliberately opaque; every
// rgba()+backdrop-filter plate in the glass file is painted at its value from
// the designer's own non-glass variant.

// THE FINALITY STAMP. `✓ finalized · h N` is the only wording that may claim
// proof — no ✓ without a height behind it. An IN-FLIGHT write is not a phase
// on this chip anymore: the chat timeline shows it as a quiet right-edge dot
// (MessageContents) and settles into a transient ✓, so an unsettled row reads
// as a normal message, not a restyled one.
component FinalityChip(height:i64)
  col #root
    box
      with
        px=7.0
        py=2.0
        bg=final_bg
        border=final_line
        border-w=1.0
        r=5.0
      row gap=4.0 align=center
        text "✓ finalized"
          with
            size=9.0
            wrap=none
            font=code_semibold
            @text-success_tick
        if height > 0
          text "·"
            with
              size=9.0
              wrap=none
              font=code_semibold
              @text-success_tick
        if height > 0
          text "h"
            with
              size=9.0
              wrap=none
              font=code_semibold
              @text-success_tick
        if height > 0
          text height
            with
              size=9.0
              wrap=none
              font=code_semibold
              @text-success_tick

// PERMISSION GATING SHOWS ITS REASON. Never hide a control and never disable
// one silently: say why it is refused and what unlocks it. `reason` is the
// refusal, `next` is the move that clears it.
component GateNote(reason:str, next:str)
  box #root
    with
      w=fill
      px=13.0
      py=11.0
      bg=warning_bg_lit
      border=warning_line
      border-w=1.0
      r=9.0
    row
      with
        w=fill
        gap=8.0
        align=start
      col pt=4.0
        box
          with
            w=6.0
            h=6.0
            bg=warning_dot
            r=3.0
          space w=1.0 h=1.0
      col w=fill gap=2.0
        text reason
          with
            w=fill
            size=12.0
            line-h=1.45
            @text-warning
        if next != ""
          text next
            with
              w=fill
              size=12.0
              line-h=1.45
              @text-caption

// HUMAN VS AGENT IS A SHAPE, EVERYWHERE. A person is a circle, a machine is a
// rounded square. The rule is never mixed and never carried by colour alone.
// `ring` hangs the plate on a band for the stacks that need to lift off their
// surface: "paper" against a card, "rail" against the navigation rail.
component PrincipalAvatar(initials:str, is_agent:bool, plate:f64, ink:f64, ring:str)
  col #root
    match ring
      "paper"
        box
          with
            p=1.5
            bg=surface
            r=(plate / 2.0 + 1.5)
          PrincipalPlate
            with
              initials
              is_agent
              plate
              ink
      "rail"
        box
          with
            p=1.5
            bg=rail
            r=(plate / 2.0 + 1.5)
          PrincipalPlate
            with
              initials
              is_agent
              plate
              ink
      _
        PrincipalPlate
          with
            initials
            is_agent
            plate
            ink

component PrincipalPlate(initials:str, is_agent:bool, plate:f64, ink:f64)
  col #root
    if is_agent
      AgentPlate
        with
          initials
          plate
          ink
    if !is_agent
      HumanPlate
        with
          initials
          plate
          ink

// The 18px sidebar plates sit on a lighter step than the 24px and up ones —
// at that size the roster plate would otherwise read as a filled dot.
component HumanPlate(initials:str, plate:f64, ink:f64)
  col #root
    if plate <= 18.0
      box
        with
          w=plate
          h=plate
          align-x=center
          align-y=center
          bg=avatar_bg_sm
          r=(plate / 2.0)
        text initials
          with
            size=ink
            wrap=none
            font=display
            @text-avatar_fg_sm
    if plate > 18.0
      box
        with
          w=plate
          h=plate
          align-x=center
          align-y=center
          bg=avatar_bg
          r=(plate / 2.0)
        text initials
          with
            size=ink
            wrap=none
            font=display
            @text-muted

// The square's radius steps with the plate — 6 at 18, 7 at 24, 8 at 28-32,
// 9 at 34, 10 at 40 and up. A fixed radius is the bug this replaces.
// ponytail: the ladder tops out at 10; the artifact's 54px detail plate wants
// 13, add a step when a call site actually asks for one.
component AgentPlate(initials:str, plate:f64, ink:f64)
  col #root
    if plate >= 40.0
      AgentSquare
        with
          initials
          plate
          ink
          radius=10.0
    if plate >= 33.0 && plate < 40.0
      AgentSquare
        with
          initials
          plate
          ink
          radius=9.0
    if plate >= 25.0 && plate < 33.0
      AgentSquare
        with
          initials
          plate
          ink
          radius=8.0
    if plate >= 22.0 && plate < 25.0
      AgentSquare
        with
          initials
          plate
          ink
          radius=7.0
    if plate < 22.0
      AgentSquare
        with
          initials
          plate
          ink
          radius=6.0

component AgentSquare(initials:str, plate:f64, ink:f64, radius:f64)
  box #root
    with
      w=plate
      h=plate
      align-x=center
      align-y=center
      bg=primary
      r=radius
    text initials
      with
        size=ink
        wrap=none
        font=code_semibold
        @text-toast_fg

// The machine marker in its one sanctioned form: what an agent is doing right
// now, on a wash plate with a hairline. `live` lights the pulse.
//
// This is NOT the AGENT badge and does not duplicate it: the badge (RoleMarker
// in kit.ice, and the same word in dm.ice and chat.ice) says a principal IS a
// machine, which is identity; this says what that machine is DOING, which is
// activity. Both appear on the artifact's agent row, together.
//
// `AgentChip` (a pulse dot + the run's own summary, on the agent row) was built
// here and is DELETED. It was waiting on a join the product does not have: the
// label is the artifact's `#142 reanalyzing`, which names a RUN, and `AgentRow`
// carries no run. `load_agent_runs` -> `[RunRow{agent_id, running, summary}]` is
// declared in backend.ice and is `run` from nowhere; the Agents screen loops
// `agents_rows` straight into `AgentCard` with no selection and no detail pane,
// so there is no surface holding one agent's runs to hang it from, and the
// loader is per-agent — a list of N agents would need N calls.
// It could NOT honestly be fed from what the row does carry: `AgentRow.status`
// is already painted by the StatusBadge active/paused mapping, so a chip over
// it would be the same fact twice, and `AgentRow.live` with a constant label is
// a liveness signal with nothing behind it.
// The values to keep for whoever lands the join: a 10px `card_wash` chip on a
// `separator` hairline, r7, the label at 10.5 `font=code_medium`
// `@text-secondary_fg`, and the pulse dot ONLY while the run is running.
// Its dot survives below — `PulseDot` is the console's one breathing mark and
// three live surfaces draw it.

// The one dot the console breathes with — w4-motion-kit binds it to `pulse`.
// The same severity ladder as PulseDot, held still — for a row that has been
// read but still carries the severity it arrived with.
component StillDot(plate:f64, tone:str)
  col #root
    match tone
      "warning"
        box
          with
            w=plate
            h=plate
            bg=warning_dot/55
            r=(plate / 2.0)
          space w=1.0 h=1.0
      "danger"
        box
          with
            w=plate
            h=plate
            bg=danger_dot/55
            r=(plate / 2.0)
          space w=1.0 h=1.0
      "info"
        box
          with
            w=plate
            h=plate
            bg=info_dot/55
            r=(plate / 2.0)
          space w=1.0 h=1.0
      _
        box
          with
            w=plate
            h=plate
            bg=success_dot/55
            r=(plate / 2.0)
          space w=1.0 h=1.0

component PulseDot(plate:f64, tone:str)
  col #root
    match tone
      "warning"
        box
          with
            w=plate
            h=plate
            bg=warning_dot
            r=(plate / 2.0)
          space w=1.0 h=1.0
      "danger"
        box
          with
            w=plate
            h=plate
            bg=danger_dot
            r=(plate / 2.0)
          space w=1.0 h=1.0
      "info"
        box
          with
            w=plate
            h=plate
            bg=info_dot
            r=(plate / 2.0)
          space w=1.0 h=1.0
      _
        box
          with
            w=plate
            h=plate
            bg=success_dot
            r=(plate / 2.0)
          space w=1.0 h=1.0
