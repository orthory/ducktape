// `This node` — the screen the artifact gives no rail seat, so these mount
// under Settings as the Overview / Permissions / Activity tabs.
//
// The artifact's copy asserts two authority axes — a node tier this machine
// picks, and an ADMIN grant quorum hands out on top of it. This product has
// ONE. `members_is_admin` (backend.rs) is literally
// `is_this_node && role == "validator"`, the same predicate as
// `member_tier(..) == "validator"`, and the tier is not a device setting: it is
// the valset row the chain wrote. So the artifact's ADMIN/MAINTAINER/VIEWER
// vocabulary is NOT adopted — every surface here names the real standing
// (validator / resident / guest) and never implies a local switch.
//
// Deliberately NOT here: Restart / Stop / Start (the app attaches to a node it
// does not supervise — rpc-client carries no lifecycle verb), the build string
// (only {height, public_key} is decoded off /v1/status), the RUN AS tier picker
// (no chain action moves this node between standings from here, and the only
// one it could submit is refused by the module — see the resident card), and
// the validator accept/decline pair (the artifact binds handlers for it but
// authored no geometry, so building it would be invention).

// ---------------------------------------------------------------- YOUR ACCESS

// One card per tier, each with the capability checklist for that tier. The
// artifact paints the admin and guest cards with a vertical gradient; iced has
// no gradient primitive, so each takes the gradient's TOP stop as a flat plate
// — the same substitution UnfinalizedFrame makes for the dashed border.
//
// `admin` is on the contract's signature and stays there, but nothing here
// branches on it: it is `is_this_node && role == "validator"`, which is the
// `"validator"` arm itself. A badge that split on it could only ever render one
// of its two faces.
//
// The `_` arm is NOT a guest card. Anything that is not one of the three
// standings means we do not know this node's, so the card says so rather than
// telling a validator's operator they are a read-only guest. NOTE: today
// `member_tier` folds "no row for this node" into `"guest"`, so an unanswered
// roster still lands on the guest card; it must answer `""` for an unmatched
// roster for this arm to catch the case it exists for.
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
                box px=7.0 py=2.0 bg=primary r=5.0
                  text "VALIDATOR · QUORUM SEAT" size=9.0 wrap=none font=code_semibold @text-primary_fg
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
              text "A quorum seat is granted and revoked by quorum only — this device cannot change it." w=fill size=12.0 line-h=1.5 @text-caption
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
                // NOT held, and the artifact's tick here is wrong for this
                // product: governance's `frozen_electorate` resolves the
                // submitter against `valset::members`, which is validators
                // only, so a resident's proposal is refused by the module.
                CapabilityCheck label="Propose modules & members" on=false
                CapabilityCheck label="Sign quorum · finalize" on=false
            col w=fill gap=13.0
              box w=fill h=1.0 bg=separator
                space w=1.0 h=1.0
              col w=fill gap=9.0
                text "VALIDATORS ONLY · QUORUM-GATED" size=9.0 wrap=none font=code_semibold @text-warning
                row w=fill wrap wrap-gap=7.0 gap=7.0
                  GatedChip label="Propose modules & members"
                  GatedChip label="Invite members"
                  GatedChip label="Change roles"
                  GatedChip label="Network settings"
                // The artifact's `Request validator role` CTA is NOT built. It
                // could only ever fail: the proposal it would open is refused
                // for any submitter without a validator seat, which is every
                // operator who can see this card. There is no message path from
                // here to a validator either, so the rule is stated instead of
                // a button that returns a rejection.
                GateNote reason="Only a validator may open a membership proposal." next="Ask a validator to propose this node for the validator set — this device cannot open it."
      "guest"
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
      _
        box w=fill pl=18.0 pr=18.0 pt=16.0 pb=16.0 bg=surface border=card_line border-w=1.0 r=13.0
          col w=fill gap=9.0
            row w=fill gap=7.0 align=center
              text "This node" size=14.0 wrap=none font=display @text-primary
              box px=7.0 py=2.0 bg=elevated r=5.0
                text "STANDING UNKNOWN" size=9.0 wrap=none font=code_semibold @text-meta
            text "The valset roster has not answered, so this node's standing is not known yet. Nothing is claimed until it does." w=fill size=12.0 line-h=1.5 @text-caption

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

// ----------------------------------------------------------------- PERMISSIONS

// The capability x tier table. The rows are static product truth; the only
// live thing is which column is tinted, which is why the whole thing takes one
// prop. 92px columns, exactly as authored. When `tier` is not one of the three
// standings — an unanswered roster — no column tints, which is the honest
// reading.
//
// `Propose modules & members` is FULL-off against the artifact: the governance
// module resolves a proposal's submitter against the validator set, so the
// resident column would be printing a claim the chain refuses.
component PermissionMatrix(tier:str)
  col #root w=fill gap=13.0
    box w=fill max-w=640.0
      text "Standing is one axis, not two: the row the validator set holds for this node is the whole of its authority, and quorum — not this device — writes it. The table is what each standing may do; this node's standing is highlighted." size=12.5 line-h=1.55 @text-muted
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
        MatrixRow label="Propose modules & members" v=true f=false l=false tier=tier
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
// terminal here. `source` names where the stream comes from; the caller fills
// the slot with its own filtered `for` over the ring.
//
// The artifact hangs a green liveness dot beside the NODE LOG title. It is not
// built: `node_logs` retries the ws `logs` topic forever and surfaces no
// failure, so the dot had no signal behind it and could not go dark — a green
// light over a dead stream is worse than none.
component NodeLogConsole(source:str)
  box #root w=fill h=fill bg=primary r=12.0 clip=true
    col w=fill h=fill pl=17.0 pr=17.0 pt=15.0 pb=15.0 gap=11.0
      row w=fill gap=8.0 align=center
        text "NODE LOG" size=9.0 wrap=none font=code_semibold @text-toast_fg
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
