// `This node` — the screen the artifact gives no rail seat, so these mount
// under Settings as the Overview / Permissions / Activity tabs.
//
// The whole surface is organised around the split the artifact's copy asserts
// three separate times: the node TIER is what this machine runs, and ADMIN is
// what quorum granted it. One is local, the other is a chain fact, and the app
// has never said so anywhere.
//
// Deliberately NOT here: Restart / Stop / Start (the app attaches to a node it
// does not supervise — rpc-client carries no lifecycle verb), the build string
// (only {height, public_key} is decoded off /v1/status), the admin-holder
// summary card (no admin-holder query exists), and the validator
// accept/decline pair (the artifact binds handlers for it but authored no
// geometry, so building it would be invention).

// ---------------------------------------------------------------- YOUR ACCESS

// One card per tier, each with the capability checklist for that tier. The
// artifact paints the admin and guest cards with a vertical gradient; iced has
// no gradient primitive, so each takes the gradient's TOP stop as a flat plate
// — the same substitution UnfinalizedFrame makes for the dashed border.
component NodeAccessCard(tier:str, admin:bool)
  col #root w=fill gap=9.0
    GroupLabel label="YOUR ACCESS"
    match tier
      "validator"
        box w=fill pl=18.0 pr=18.0 pt=16.0 pb=16.0 bg=final_bg border=success_line border-w=1.0 r=13.0
          col w=fill gap=14.0
            col w=fill gap=3.0
              row w=fill gap=7.0 align=center
                text "This node" size=14.0 wrap=none font=display @text-primary
                if admin
                  box px=7.0 py=2.0 bg=primary r=5.0
                    text "ADMIN · VALIDATOR" size=9.0 wrap=none font=code_semibold @text-primary_fg
                if !admin
                  box px=7.0 py=2.0 bg=primary r=5.0
                    text "VALIDATOR" size=9.0 wrap=none font=code_semibold @text-primary_fg
              text "signs quorum · finalizes rounds · stores all history" size=10.5 wrap=none font=code_medium @text-caption
            col w=fill gap=9.0
              row w=fill gap=9.0
                CapabilityCheck label="Sign quorum & finalize rounds" on=true
                CapabilityCheck label="Invite members & assign roles" on=true
              row w=fill gap=9.0
                CapabilityCheck label="Install & remove modules" on=true
                CapabilityCheck label="Edit network settings" on=true
            col w=fill gap=13.0
              box w=fill h=1.0 bg=success_line
                space w=1.0 h=1.0
              text "Admin authority is transferred and revoked by quorum only." w=fill size=12.0 line-h=1.5 @text-caption
      "resident"
        box w=fill pl=18.0 pr=18.0 pt=16.0 pb=16.0 bg=surface border=card_line border-w=1.0 r=13.0
          col w=fill gap=14.0
            col w=fill gap=3.0
              row w=fill gap=7.0 align=center
                text "This node" size=14.0 wrap=none font=display @text-primary
                box px=7.0 py=2.0 bg=surface border=control_line border-w=1.0 r=5.0
                  text "RESIDENT · FULL NODE" size=9.0 wrap=none font=code_semibold @text-muted
              text "full node · stores all history · cannot sign quorum" size=10.5 wrap=none font=code_medium @text-caption
            col w=fill gap=9.0
              row w=fill gap=9.0
                CapabilityCheck label="Read & verify finality" on=true
                CapabilityCheck label="Send · react · thread" on=true
              row w=fill gap=9.0
                CapabilityCheck label="Propose modules & members" on=true
                CapabilityCheck label="Sign quorum · finalize" on=false
            col w=fill gap=13.0
              box w=fill h=1.0 bg=separator
                space w=1.0 h=1.0
              col w=fill gap=9.0
                text "ADMIN ONLY · QUORUM-GATED" size=9.0 wrap=none font=code_semibold @text-warning
                row w=fill wrap wrap-gap=7.0 gap=7.0
                  GatedChip label="Invite members"
                  GatedChip label="Change roles"
                  GatedChip label="Network settings"
                // the CTA does not grant anything — it opens the proposal that
                // asks the electorate to grant it, and the note below says so
                row w=fill pt=4.0
                  button label="Request validator role" p=0.0 @primary_action -> node_request_tier("add_validator")
                    box pl=14.0 pr=14.0 pt=8.0 pb=8.0
                      row gap=7.0 align=center
                        text "Request validator role" size=12.0 wrap=none font=display @text-primary_fg
                        text "→" size=12.0 wrap=none @text-caption
                    active bg=primary text=primary_fg border=transparent border-w=1.0 r=8.0
                    hovered bg=ink_hover text=primary_fg
                    pressed bg=ink_hover text=primary_fg
      _
        box w=fill pl=18.0 pr=18.0 pt=16.0 pb=16.0 bg=warning_bg_lit border=warning_line border-w=1.0 r=13.0
          col w=fill gap=14.0
            col w=fill gap=3.0
              row w=fill gap=7.0 align=center
                text "This node" size=14.0 wrap=none font=display @text-primary
                box px=7.0 py=2.0 bg=warning_bg border=warning_line border-w=1.0 r=5.0
                  text "GUEST · LIGHT NODE" size=9.0 wrap=none font=code_semibold @text-warning
              text "read-only · verifies finalized headers" size=10.5 wrap=none font=code_medium @text-caption
            col w=fill gap=9.0
              row w=fill gap=9.0
                CapabilityCheck label="Read & verify finality" on=true
                CapabilityCheck label="Read chat & threads" on=true
              row w=fill gap=9.0
                CapabilityCheck label="Read governance" on=true
                CapabilityCheck label="Browse Forge" on=true
            col w=fill gap=13.0
              box w=fill h=1.0 bg=warning_line
                space w=1.0 h=1.0
              col w=fill gap=9.0
                text "GUEST · NO SIGNING, NO CONTRIBUTION" size=9.0 wrap=none font=code_semibold @text-warning
                row w=fill wrap wrap-gap=7.0 gap=7.0
                  GatedChip label="Propose"
                  GatedChip label="Forge contribute & merge"
                  GatedChip label="Sign quorum"
                  GatedChip label="Invite"
                text "Contributing needs a resident invite · quorum grants resident and validator standing." w=fill size=12.0 line-h=1.5 @text-caption

// A capability the tier either holds or does not. The plate carries the whole
// state — a tick on the success plate, an en-dash on the idle one — and the
// label fades with it.
component CapabilityCheck(label:str, on:bool)
  col #root w=fill
    if on
      row w=fill gap=8.0 align=center
        box w=17.0 h=17.0 align-x=center align-y=center bg=success_bg r=8.5
          text "✓" size=9.0 wrap=none font=code_semibold @text-success
        text label w=fill size=12.0 @text-accent_fg
    if !on
      row w=fill gap=8.0 align=center
        box w=17.0 h=17.0 align-x=center align-y=center bg=elevated r=8.5
          text "–" size=9.0 wrap=none font=code_semibold @text-icon_idle
        text label w=fill size=12.0 @text-icon_idle

// What the tier may not do at all — locked, not merely absent from the list.
component GatedChip(label:str)
  box #root pl=10.0 pr=10.0 pt=5.0 pb=5.0 bg=card_wash border=separator border-w=1.0 r=7.0
    row gap=6.0 align=center
      Icon name="lock" tone="idle" px=11.0
      text label size=12.0 wrap=none @text-meta

// ---------------------------------------------------------------------- RUN AS

// The tier picker. Reading the tier is local; CHANGING it is not — every
// inactive card opens a governance proposal rather than flipping a switch, and
// the note under the row says exactly that. `Light` carries the artifact's own
// lock glyph because no chain action demotes a resident to a light node: there
// is GovAction::{AddValidator, AddResident, RemoveValidator} and nothing else.
component NodeRunAsPicker(tier:str)
  col #root w=fill gap=9.0
    row gap=9.0 align=center
      GroupLabel label="RUN AS"
      text "node tier · separate from admin authority" size=12.5 @text-label
    row w=fill gap=10.0 align=start
      RunAsCard label="Validator" badge="QUORUM" detail="joins rounds · signs quorum" action="add_validator" locked=false active=(tier == "validator")
      RunAsCard label="Full" badge="FULL SYNC" detail="stores history · may propose" action="add_resident" locked=false active=(tier == "resident")
      RunAsCard label="Light" badge="READ-ONLY" detail="verifies headers · read only" action="" locked=true active=(tier == "guest")
    row w=fill gap=7.0 align=center
      box w=5.0 h=5.0 bg=label r=2.5
        space w=1.0 h=1.0
      text "Changing this node's tier opens a proposal — quorum settles it, this device does not." w=fill size=12.5 @text-caption

component RunAsCard(label:str, badge:str, detail:str, action:str, locked:bool, active:bool)
  col #root w=fill
    if active
      box w=fill pl=15.0 pr=15.0 pt=13.0 pb=13.0 bg=primary border=primary border-w=1.5 r=12.0
        col w=fill gap=7.0
          row w=fill gap=7.0 align=center
            text label size=14.0 wrap=none font=display @text-primary_fg
            space w=fill
            box px=6.0 py=2.0 bg=panel_tile r=4.0
              text badge size=9.0 wrap=none font=code_semibold @text-toast_fg
          text detail w=fill size=12.0 @text-ink_soft
    if !active
      col w=fill
        if locked
          box w=fill pl=15.0 pr=15.0 pt=13.0 pb=13.0 bg=surface border=border border-w=1.5 r=12.0
            col w=fill gap=7.0
              row w=fill gap=7.0 align=center
                text label size=14.0 wrap=none font=display @text-accent_fg
                Icon name="lock" tone="label" px=11.0
                space w=fill
                box px=6.0 py=2.0 bg=elevated r=4.0
                  text badge size=9.0 wrap=none font=code_semibold @text-input
              text detail w=fill size=12.0 @text-meta
        if !locked
          button label=label w=fill p=0.0 @outline_action -> node_request_tier(action)
            col w=fill pl=15.0 pr=15.0 pt=13.0 pb=13.0 gap=7.0
              row w=fill gap=7.0 align=center
                text label size=14.0 wrap=none font=display @text-accent_fg
                space w=fill
                box px=6.0 py=2.0 bg=elevated r=4.0
                  text badge size=9.0 wrap=none font=code_semibold @text-input
              text detail w=fill size=12.0 @text-meta
            active bg=surface text=accent_fg border=border border-w=1.5 r=12.0
            hovered bg=card_wash_hover text=accent_fg border=control_line_hover border-w=1.5 r=12.0
            pressed bg=elevated text=accent_fg border=control_line_hover border-w=1.5 r=12.0

// ----------------------------------------------------------------- PERMISSIONS

// The capability x tier table. The rows are static product truth; the only
// live thing is which column is tinted, which is why the whole thing takes one
// prop. 92px columns, exactly as authored.
component PermissionMatrix(tier:str)
  col #root w=fill gap=13.0
    box w=fill max-w=640.0
      text "Ducktape has two authority axes — the node tier this device runs, and admin governance, which quorum grants. The table is the default for each tier; the tier this node runs is highlighted." size=12.5 line-h=1.55 @text-muted
    box w=fill max-w=640.0 bg=surface border=card_line border-w=1.0 r=12.0 clip=true
      col w=fill
        box w=fill bg=card_wash
          row w=fill align=center
            box w=fill pl=14.0 pr=14.0 pt=10.0 pb=10.0
              text "capability" size=12.5 @text-caption
            MatrixHead label="Validator" active=(tier == "validator")
            MatrixHead label="Full" active=(tier == "resident")
            MatrixHead label="Light" active=(tier == "guest")
        MatrixRow label="Read & verify finality" v=true f=true l=true tier=tier
        MatrixRow label="Send · react · thread" v=true f=true l=false tier=tier
        MatrixRow label="Propose modules & members" v=true f=true l=false tier=tier
        MatrixRow label="Sign quorum · finalize" v=true f=false l=false tier=tier

component MatrixHead(label:str, active:bool)
  col #root
    if active
      box w=92.0 pt=10.0 pb=10.0 align-x=center bg=tree_selected
        text label size=9.5 wrap=none font=display @text-strong_ink
    if !active
      box w=92.0 pt=10.0 pb=10.0 align-x=center bg=transparent
        text label size=9.5 wrap=none font=display @text-strong_ink

component MatrixRow(label:str, v:bool, f:bool, l:bool, tier:str)
  col #root w=fill
    box w=fill h=1.0 bg=elevated
      space w=1.0 h=1.0
    row w=fill align=center
      box w=fill pl=14.0 pr=14.0 pt=11.0 pb=11.0
        text label size=12.0 @text-accent_fg
      MatrixCell on=v active=(tier == "validator")
      MatrixCell on=f active=(tier == "resident")
      MatrixCell on=l active=(tier == "guest")

component MatrixCell(on:bool, active:bool)
  col #root
    if active
      box w=92.0 pt=11.0 pb=11.0 align-x=center bg=bg_wash
        MatrixTick on=on
    if !active
      box w=92.0 pt=11.0 pb=11.0 align-x=center bg=transparent
        MatrixTick on=on

component MatrixTick(on:bool)
  col #root
    if on
      text "✓" size=13.0 wrap=none font=display @text-success
    if !on
      text "−" size=13.0 wrap=none font=display @text-presence_off

// -------------------------------------------------------------------- ACTIVITY

// The log console is the one dark plate in the console: paper everywhere else,
// terminal here. `source` names where the stream comes from — the app reads the
// ring over the node's ws `logs` topic, so it prints the endpoint, NOT the
// `~/.ducktape/…/node.log` path the artifact writes, which the app never opens.
// The caller fills the slot with its own filtered `for` over the ring.
component NodeLogConsole(source:str)
  box #root w=fill h=fill bg=primary r=12.0 clip=true
    col w=fill h=fill pl=17.0 pr=17.0 pt=15.0 pb=15.0 gap=11.0
      row w=fill gap=8.0 align=center
        text "NODE LOG" size=9.0 wrap=none font=code_semibold @text-toast_fg
        PulseDot plate=6.0 tone="success"
        space w=fill
        text source size=10.5 wrap=none font=code_medium @text-input
      scroll dir=vertical w=fill h=fill bar=hidden
        col w=fill gap=5.0
          slot

// One ring line as three columns. `split_log_line` hands back an empty level
// for anything it cannot parse, and then the whole line rides in `message` —
// the console never drops a line it did not understand.
component LogLine(parts:LogParts)
  row #root w=fill gap=6.0 align=start
    text parts.time size=12.0 wrap=none font=code @text-avatar_fg_sm
    box w=38.0
      LogLevel level=parts.level
    text parts.message w=fill size=12.0 line-h=1.55 font=code @text-chevron_idle

// Severity is the only colour in the panel, so it is the only thing the eye
// catches while the ring scrolls.
component LogLevel(level:str)
  col #root
    match level
      "INFO"
        text "INFO" size=12.0 wrap=none font=code @text-agent_live
      "WARN"
        text "WARN" size=12.0 wrap=none font=code @text-warning_plate
      "ERROR"
        text "ERROR" size=12.0 wrap=none font=code @text-danger_soft
      _
        text level size=12.0 wrap=none font=code @text-input

// ------------------------------------------------------------------- THE ROUTE

// The one handler this file owns. Both the Request-validator CTA and the RUN AS
// picker land here: they are the same act — open the membership proposal that
// would move THIS node to that tier. `gov_voting` is already the governance
// busy latch, and `gov_acted` / `gov_act_failed` already clear it and reload.
on node_request_tier(action)
  return if !connected || !empty(gov_voting) || empty(settings_node_key)
  gov_voting = action
  run governance_propose(connected_rpc, password, action, settings_node_key) -> gov_acted _ | gov_act_failed _
