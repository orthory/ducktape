// The two roster detail panels: a member record and an agent registry record.
// Both are the same shape — a 56px header, a centred identity block, machine
// fact rows, then the writes this node is actually allowed to attempt.
//
// THE CHAIN'S OWN WORDS, NOT THE ARTIFACT'S. The handoff prints ADMIN /
// MAINTAINER / VIEWER. This chain grants validator / resident / agent and has
// no admin or maintainer tier at all, so every label here is the chain's. The
// artifact's `needs quorum to change` caption over an agent's grants is the
// same class of error in the other direction — registry mutation is
// OWNER-gated (`AgentMsg::PauseAgent`/`ResumeAgent`), never quorum-gated — so
// that caption reads `only the owner can change`.
//
// PRESENCE IS BINARY HERE. The artifact draws four presence states (online,
// idle, invited, offline); the mesh reports one bit, `live`. idle and invited
// have no source, so they are not drawn.

// ── MEMBER ────────────────────────────────────────────────────────────────

// The 312px member record. `admin` is members_is_admin — this node holds a
// quorum seat — and it gates the membership proposals, not the whole panel:
// a non-admin still sees WHY the writes are refused.
component MemberDetail(member:MemberRow, admin:bool)
  row #root h=fill
    box w=1.0 h=fill bg=separator
      space w=1.0 h=1.0
    box w=312.0 h=fill bg=sidebar
      col w=fill h=fill
        box w=fill h=56.0 px=16.0
          row w=fill h=fill gap=8.0 align=center
            text "Member" w=fill size=13.0 wrap=none font=display @text-fg
            button label="Close member" w=24.0 h=24.0 p=0.0 @icon_action -> open_member("")
              text "×" size=16.0 wrap=none font=code_medium @text-meta
              active bg=transparent r=6.0
              hovered bg=separator
        scroll dir=vertical w=fill h=fill
          col w=fill pl=16.0 pr=16.0 pt=18.0 pb=18.0 gap=0.0
            col w=fill gap=0.0 align=center
              if member.is_agent
                PrincipalAvatar initials=initials_of(member.label) is_agent=true plate=54.0 ink=16.0 ring=""
              if !member.is_agent
                PrincipalAvatar initials=initial_of(member.label) is_agent=false plate=54.0 ink=20.0 ring=""
              box pt=11.0
                text member.label size=16.0 wrap=none font=display @text-fg
              box pt=5.0
                row gap=6.0 align=center
                  MemberPresence live=member.live
                  RoleMarker role=member.role
            col w=fill pt=18.0 gap=8.0
              MemberFactRow label="public key" value=member.key
              if !empty(member.model)
                MemberFactRow label="model" value=member.model
            col w=fill pt=16.0 gap=8.0
              // your own row's only action: the key you hand someone to invite you
              if member.is_this_node
                button label="Copy your key" w=fill @secondary_action px-12px py-10px rounded-9px -> copy_to_clipboard(member.key, "Key copied")
                  PanelActionLabel label="Copy your key" tag="" danger=false
              // an agent is paused and resumed by its OWNER, immediately — no ballot
              if member.is_agent && member.live
                button label="Pause agent" w=fill @secondary_action px-12px py-10px rounded-9px -> agent_set_status(member.key, true)
                  PanelActionLabel label="Pause agent" tag="" danger=false
              if member.is_agent && !member.live
                button label="Resume agent" w=fill @secondary_action px-12px py-10px rounded-9px -> agent_set_status(member.key, false)
                  PanelActionLabel label="Resume agent" tag="" danger=false
              if member.is_agent
                GateNote reason="Only the agent's owner may pause or resume it." next="The registry refuses the write from any other signer."
              // membership moves are ballots: this opens the proposal, it does not settle it
              if admin && !member.is_agent && member.role == "resident"
                button label="Promote to validator" w=fill @secondary_action px-12px py-10px rounded-9px -> gov_propose("add_validator", member.key)
                  PanelActionLabel label="Promote to validator" tag="needs quorum" danger=false
              if admin && !member.is_agent && member.role == "validator" && !member.is_this_node
                button label="Remove from the validator set" w=fill @secondary_action px-12px py-10px rounded-9px border-alert_line -> gov_propose("remove_validator", member.key)
                  PanelActionLabel label="Remove from the validator set" tag="needs quorum" danger=true
              if !admin && !member.is_this_node && !member.is_agent
                GateNote reason="Only a validator node may open a membership proposal." next="This node holds no quorum seat, so the network refuses the write."

// A panel button's face: the label, and the right-aligned marker that says the
// write will open a ballot rather than settle on the spot.
component PanelActionLabel(label:str, tag:str, danger:bool)
  row #root w=fill gap=8.0 align=center
    if danger
      text label w=fill size=12.0 wrap=none font=display @text-alert_fg
    if !danger
      text label w=fill size=12.0 wrap=none font=display @text-accent_fg
    if tag != ""
      text tag size=9.0 wrap=none font=code_semibold @text-label

// live or offline — the only two the mesh can tell apart.
component MemberPresence(live:bool)
  row #root gap=6.0 align=center
    if live
      PulseDot plate=7.0 tone="success"
    if !live
      box w=7.0 h=7.0 bg=presence_off r=3.5
        space w=1.0 h=1.0
    if live
      text "live" size=12.0 wrap=none @text-input
    if !live
      text "offline" size=12.0 wrap=none @text-input

// A bordered machine-fact line. The value takes the rest of the row and wraps,
// because a full 64-hex public key does not fit a 312px panel on one line.
component MemberFactRow(label:str, value:str)
  box #root w=fill px=12.0 py=9.0 bg=surface border=card_line border-w=1.0 r=8.0
    row w=fill gap=10.0 align=start
      text label size=11.0 wrap=none font=code_medium @text-meta
      text value w=fill size=11.0 font=code_medium @text-secondary_fg

// ── AGENT ─────────────────────────────────────────────────────────────────

// The 322px registry record. `mine` is AgentRow.is_mine — this node signed the
// registration — and it is the ONLY gate on Pause/Resume, which is owner-gated.
component AgentDetail(agent:AgentRow, runs:[RunRow], mine:bool)
  row #root h=fill
    box w=1.0 h=fill bg=separator
      space w=1.0 h=1.0
    box w=322.0 h=fill bg=sidebar
      col w=fill h=fill
        box w=fill h=56.0 px=16.0
          row w=fill h=fill gap=6.0 align=center
            text "Registry record" w=fill size=13.0 wrap=none font=display @text-fg
            // `joined` is a consensus stamp, so it prints as the height it is —
            // this chain has no wall clock to render the artifact's `3d ago`.
            if agent.created_at > 0
              text "joined" size=10.5 wrap=none font=code_medium @text-label
            if agent.created_at > 0
              text height_label(agent.created_at) size=10.5 wrap=none font=code_medium @text-label
        scroll dir=vertical w=fill h=fill
          col w=fill p=16.0 gap=0.0
            row w=fill gap=11.0 align=center
              AgentAvatar initials=agent.initials plate=44.0 ink=14.0
              col w=fill gap=2.0
                text agent.name size=16.0 wrap=none font=display @text-fg
                row gap=4.0 align=center
                  text "owner" size=11.0 wrap=none font=code_medium @text-meta
                  OwnerHandle handle=agent.owner_handle
            row w=fill pt=13.0 gap=7.0 align=center
              AgentStatusChip status=agent.status
              box px=8.0 py=3.0 bg=elevated r=5.0
                text agent.capability size=10.5 wrap=none font=code_medium @text-secondary_fg
            // the live strip is the agent's OWN unsettled run — nothing invented
            for run in runs
              if run.running
                AgentLiveStrip run_id=run.run_id
            if !empty(agent.skills)
              box w=fill pt=17.0
                Eyebrow label="SKILLS" note=""
            if !empty(agent.skills)
              col w=fill pt=8.0 gap=6.0
                for skill in agent.skills
                  SkillRow skill=skill
            if !empty(agent.caps)
              box w=fill pt=17.0
                Eyebrow label="GRANTED CAPABILITIES" note="only the owner can change"
            if !empty(agent.caps)
              row w=fill pt=8.0 gap=6.0 wrap wrap-gap=6.0
                for cap in agent.caps
                  CapChip cap=cap
            if !empty(agent.allowed_actions)
              box w=fill pt=17.0
                Eyebrow label="ALLOWED ACTIONS" note=""
            if !empty(agent.allowed_actions)
              row w=fill pt=8.0 gap=6.0 wrap wrap-gap=6.0
                for action in agent.allowed_actions
                  box px=8.0 py=4.0 bg=surface border=border border-w=1.0 r=7.0
                    text action size=10.5 wrap=none font=code_medium @text-secondary_fg
            row w=fill pt=8.0 gap=6.0 wrap wrap-gap=6.0
              CapCountBox value=agent.tools label="tools"
              CapCountBox value=agent.secrets label="secrets"
              // subagent_budget is a CONCURRENCY ceiling, never a headcount
              ConcurrencyBox value=agent.subagent_budget
            if !empty(runs)
              box w=fill pt=17.0
                Eyebrow label="RECENT RUNS" note=""
            if !empty(runs)
              col w=fill pt=8.0 gap=8.0
                for run in runs
                  RunCard run=run
            col w=fill pt=16.0 gap=8.0
              if mine && agent.status == "active"
                button label="Pause agent" w=fill @secondary_action px-12px py-10px rounded-9px -> agent_set_status(agent.id, true)
                  PanelActionLabel label="Pause agent" tag="" danger=false
              if mine && agent.status != "active"
                button label="Resume agent" w=fill @secondary_action px-12px py-10px rounded-9px -> agent_set_status(agent.id, false)
                  PanelActionLabel label="Resume agent" tag="" danger=false
              if !mine
                GateNote reason="Only the agent's owner may pause or resume it." next="The registry refuses the write from any other signer."

// What the agent is doing right now, taken from its own unsettled run. The
// ring is the app's spinner at rest — AgentDetail's frozen signature carries
// no `spin`, so there is no animation value to turn it with.
component AgentLiveStrip(run_id:str)
  col #root w=fill pt=11.0
    box w=fill px=11.0 py=9.0 bg=surface border=card_line border-w=1.0 r=9.0
      row w=fill gap=8.0 align=center
        SpinRing px=13.0 spin=0.0
        text "running" size=12.0 wrap=none @text-secondary_fg
        text run_id w=fill size=10.5 wrap=none font=code_medium @text-label

// A curated skill and how it loads: ALWAYS is resident in the agent's context,
// ON DEMAND is fetched only when it is asked for.
component SkillRow(skill:AgentSkill)
  box #root w=fill px=11.0 py=8.0 bg=surface border=card_line border-w=1.0 r=8.0
    row w=fill gap=8.0 align=center
      text skill.name w=fill size=12.0 wrap=none font=code_medium @text-accent_fg
      if skill.always
        box px=6.0 py=2.0 bg=success_bg r=4.0
          text "ALWAYS" size=9.0 wrap=none font=code_semibold @text-success
      if !skill.always
        box px=6.0 py=2.0 bg=elevated r=4.0
          text "ON DEMAND" size=9.0 wrap=none font=code_semibold @text-input

// One granted capability in the `CapRequest` vocabulary: the grant name in the
// brand ink, the resource it names in a lighter one.
component CapChip(cap:AgentCap)
  box #root px=8.0 py=4.0 bg=brand_wash border=brand_line border-w=1.0 r=7.0
    row gap=4.0 align=center
      text cap.label size=10.0 wrap=none font=code_semibold @text-brand
      if !empty(cap.arg)
        text cap.arg size=10.5 wrap=none font=code_medium @text-input

// `{n} tools` / `{n} secrets` — a count and the noun it counts.
component CapCountBox(value:i64, label:str)
  box #root px=8.0 py=4.0 border=separator border-w=1.0 r=6.0
    row gap=4.0 align=center
      text value size=10.5 wrap=none font=code_medium @text-meta
      text label size=10.5 wrap=none font=code_medium @text-meta

// `max {n} concurrent` — the peer-call ceiling, stated as a ceiling.
component ConcurrencyBox(value:i64)
  box #root px=8.0 py=4.0 border=separator border-w=1.0 r=6.0
    row gap=4.0 align=center
      text "max" size=10.5 wrap=none font=code_medium @text-meta
      text value size=10.5 wrap=none font=code_medium @text-meta
      text "concurrent" size=10.5 wrap=none font=code_medium @text-meta

// One run: its id, whether it is still running, and the block it was created
// at. No summary line — `RunRecord` carries no summary — and no `N tool calls`
// line, because it carries no tool counter either.
component RunCard(run:RunRow)
  box #root w=fill px=12.0 py=10.0 bg=surface border=card_line border-w=1.0 r=10.0
    row w=fill gap=8.0 align=center
      text run.run_id size=10.5 wrap=none font=code_medium @text-accent_fg
      if run.running
        box px=6.0 py=1.0 bg=warning_bg border=warning_line border-w=1.0 r=5.0
          row gap=5.0 align=center
            PulseDot plate=5.0 tone="warning"
            text "RUNNING" size=9.0 wrap=none font=code_semibold @text-warning
      if !run.running
        box px=6.0 py=1.0 bg=final_bg border=final_line border-w=1.0 r=5.0
          text "✓ DONE" size=9.0 wrap=none font=code_semibold @text-success_tick
      space w=fill
      if run.created_at > 0
        text height_label(run.created_at) size=9.0 wrap=none font=code_semibold @text-label
