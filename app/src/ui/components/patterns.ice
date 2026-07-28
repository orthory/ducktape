// The four product patterns the Design System states as law, in ONE place each,
// so that no screen ever spells them itself. Finality words, the unfinalized
// ring, the refusal plate, and the human-vs-agent shape all live here.
//
// TWO SANCTIONED SUBSTITUTIONS — settled, do not re-litigate per screen:
//
// 1. DASHED BORDER -> 1.5px SOLID `pending_line`. The artifact draws anything
//    not yet settled with a dashed outline. iced's Border is colour + width +
//    radius only; ui-lang's `canvas` can stroke a dash, but at the pinned rev
//    there is no `zstack`, `stack` cannot size a canvas from its sibling, and
//    UnfinalizedFrame's signature carries no width/height to feed one. So the
//    ring is solid at 1.5px in `pending_line` — the artifact's own dash hex.
// 2. GRADIENT -> FLAT SURFACE. The console is deliberately opaque; every
//    rgba()+backdrop-filter plate in the glass file is painted at its value
//    from the designer's own non-glass variant.

// THE FINALITY STORY. Three phases, the same words on every surface that
// carries a write: messages, reviews, merges, proposals, the inspector.
// `finalizing…` is optimistic, `batched` is bound into a merkle root, and
// `✓ finalized · h N` is the only one that may claim proof. No ✓ without a
// height behind it.
component FinalityChip(phase:str, height:i64)
  col #root
    match phase
      "finalizing"
        row gap=4.0 align=center
          // static ring, not a spinner — nothing drives state.ice `spin`, so a
          // turning arc would freeze mid-turn (see overlay.ice)
          box w=9.0 h=9.0 bg=transparent border=input border-w=1.5 r=4.5
            space w=1.0 h=1.0
          text "finalizing…" size=12.0 wrap=none @text-ink_soft
      "batched"
        box px=7.0 py=2.0 bg=subtle r=5.0
          text "batched" size=12.0 wrap=none font=code_semibold @text-strong_ink
      _
        box px=7.0 py=2.0 bg=final_bg border=final_line border-w=1.0 r=5.0
          row gap=4.0 align=center
            text "✓ finalized" size=9.0 wrap=none font=code_semibold @text-success_tick
            if height > 0
              text "·" size=9.0 wrap=none font=code_semibold @text-success_tick
            if height > 0
              text "h" size=9.0 wrap=none font=code_semibold @text-success_tick
            if height > 0
              text height size=9.0 wrap=none font=code_semibold @text-success_tick

// UNFINALIZED MEANS A RING. Wrap any card, row or bubble whose write has not
// settled; the ring is drawn over the child so the child keeps its own plate.
// The radius is the app's card radius — the frozen signature carries no
// radius prop, and every wrapped surface today is a 10-12px card.
component UnfinalizedFrame(pending:bool)
  stack #root w=fill
    slot
    if pending
      box w=fill h=fill bg=transparent border=pending_line border-w=1.5 r=10.0
        space w=1.0 h=1.0

// PERMISSION GATING SHOWS ITS REASON. Never hide a control and never disable
// one silently: say why it is refused and what unlocks it. `reason` is the
// refusal, `next` is the move that clears it.
component GateNote(reason:str, next:str)
  box #root w=fill px=13.0 py=11.0 bg=warning_bg_lit border=warning_line border-w=1.0 r=9.0
    row w=fill gap=8.0 align=start
      col pt=4.0
        box w=6.0 h=6.0 bg=warning_dot r=3.0
          space w=1.0 h=1.0
      col w=fill gap=2.0
        text reason w=fill size=12.0 line-h=1.45 @text-warning
        if next != ""
          text next w=fill size=12.0 line-h=1.45 @text-caption

// HUMAN VS AGENT IS A SHAPE, EVERYWHERE. A person is a circle, a machine is a
// rounded square. The rule is never mixed and never carried by colour alone.
// `ring` hangs the plate on a band for the stacks that need to lift off their
// surface: "paper" against a card, "rail" against the navigation rail.
component PrincipalAvatar(initials:str, is_agent:bool, plate:f64, ink:f64, ring:str)
  col #root
    match ring
      "paper"
        box p=1.5 bg=surface r=(plate / 2.0 + 1.5)
          PrincipalPlate initials=initials is_agent=is_agent plate=plate ink=ink
      "rail"
        box p=1.5 bg=rail r=(plate / 2.0 + 1.5)
          PrincipalPlate initials=initials is_agent=is_agent plate=plate ink=ink
      _
        PrincipalPlate initials=initials is_agent=is_agent plate=plate ink=ink

component PrincipalPlate(initials:str, is_agent:bool, plate:f64, ink:f64)
  col #root
    if is_agent
      AgentPlate initials=initials plate=plate ink=ink
    if !is_agent
      HumanPlate initials=initials plate=plate ink=ink

// The 18px sidebar plates sit on a lighter step than the 24px and up ones —
// at that size the roster plate would otherwise read as a filled dot.
component HumanPlate(initials:str, plate:f64, ink:f64)
  col #root
    if plate <= 18.0
      box w=plate h=plate align-x=center align-y=center bg=avatar_bg_sm r=(plate / 2.0)
        text initials size=ink wrap=none font=display @text-avatar_fg_sm
    if plate > 18.0
      box w=plate h=plate align-x=center align-y=center bg=avatar_bg r=(plate / 2.0)
        text initials size=ink wrap=none font=display @text-muted

// The square's radius steps with the plate — 6 at 18, 7 at 24, 8 at 28-32,
// 9 at 34, 10 at 40 and up. A fixed radius is the bug this replaces.
// ponytail: the ladder tops out at 10; the artifact's 54px detail plate wants
// 13, add a step when a call site actually asks for one.
component AgentPlate(initials:str, plate:f64, ink:f64)
  col #root
    if plate >= 40.0
      AgentSquare initials=initials plate=plate ink=ink radius=10.0
    if plate >= 33.0 && plate < 40.0
      AgentSquare initials=initials plate=plate ink=ink radius=9.0
    if plate >= 25.0 && plate < 33.0
      AgentSquare initials=initials plate=plate ink=ink radius=8.0
    if plate >= 22.0 && plate < 25.0
      AgentSquare initials=initials plate=plate ink=ink radius=7.0
    if plate < 22.0
      AgentSquare initials=initials plate=plate ink=ink radius=6.0

component AgentSquare(initials:str, plate:f64, ink:f64, radius:f64)
  box #root w=plate h=plate align-x=center align-y=center bg=primary r=radius
    text initials size=ink wrap=none font=code_semibold @text-toast_fg

// The machine marker in its one sanctioned form: what an agent is doing right
// now, on a wash plate with a hairline. `live` lights the pulse.
//
// This is NOT the AGENT badge and does not duplicate it: the badge (RoleMarker
// in kit.ice, and the same word in dm.ice and chat.ice) says a principal IS a
// machine, which is identity; this says what that machine is DOING, which is
// activity. Both appear on the artifact's agent row, together.
//
// NOT MOUNTED YET, for want of the join rather than the geometry. The label is
// the artifact's `#142 재분석 중` — a run, not an agent — and `AgentRow` carries
// no run. `load_agent_runs` -> `[RunRow{agent_id, running, summary}]` is landed
// in backend.rs and declared in backend.ice, and state.ice already holds
// `agent_runs_generation`, but no `agent_runs` list is bound and nothing joins
// a run to its agent. Whoever lands that binding mounts this on the agent row:
// `AgentChip label=<run summary> live=<run running>`. A chip fed a constant
// would be a fake liveness signal, so it stays dark until the join is real.
component AgentChip(label:str, live:bool)
  box #root px=10.0 py=4.0 bg=card_wash border=separator border-w=1.0 r=7.0
    row gap=6.0 align=center
      if live
        PulseDot plate=6.0 tone="success"
      text label size=10.5 wrap=none font=code_medium @text-secondary_fg

// The one dot the console breathes with — w4-motion-kit binds it to `pulse`.
component PulseDot(plate:f64, tone:str)
  col #root
    match tone
      "warning"
        box w=plate h=plate bg=warning_dot r=(plate / 2.0)
          space w=1.0 h=1.0
      "danger"
        box w=plate h=plate bg=danger_dot r=(plate / 2.0)
          space w=1.0 h=1.0
      "info"
        box w=plate h=plate bg=info_dot r=(plate / 2.0)
          space w=1.0 h=1.0
      _
        box w=plate h=plate bg=success_dot r=(plate / 2.0)
          space w=1.0 h=1.0
