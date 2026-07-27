// THE PRE-CONSOLE PHASE FLOW. Five screens in front of the shell: welcome,
// create, provisioning, live, join. `phase` is the single discriminant and
// "console" is the only value that renders the console instead of this column.
//
// MOUNT (view.ice owns the branch):
//
//   if phase != "console"
//     OnboardingPhase phase=phase name=onboarding_name node_api=canonical_endpoint(rpc)
//       invite=invite_link steps=provision_steps step_index=provision_index
//       height=block_height peers_live=… peers_total=… tier=member_tier(members_rows)
//       error=onboarding_error busy=(mutation_phase != "idle")
//
// THREE THINGS THE ARTIFACT DRAWS THAT THIS APP REFUSES TO DRAW:
//
// 1. THE 80px QR BLOCK IS DROPPED. ui-lang has a native `qr` widget, but its
//    payload is a STATIC literal (`qr_payload = string | bytes`) and a
//    runtime-minted invite can never be one. The artifact's own block is 25
//    hardcoded cells from `((i*7+3)%5 < 2)`, i.e. decoration, not a code.
//    Shipping either would be a beautiful dead pixel.
// 2. THE `↗` SHARE BUTTON IS DROPPED. The artifact binds it to the SAME
//    `copyInvite` handler as `Copy link`; two controls, one behaviour. One
//    button ships, and no OS share sheet is invented to justify the second.
// 3. PROVISIONING DOES NOT AUTO-ADVANCE ON A TIMER. The artifact's
//    `runProvision()` is a fake 850ms clock. This app is a strict CLIENT: it
//    cannot start a node daemon, so steps 4-5 are a real `/v1/status` poll and
//    a stalled node gets a visible `blocked` step carrying the command that
//    fixes it, not a spinner that lies.

// The centred column on the flat second surface. iced has no radial gradient,
// so the artifact's `radial-gradient(#fdfdfb -> #f7f6f2)` lands as the flat
// `bg_wash` step — the sanctioned substitution, same as every other plate.
component OnboardingPhase(phase:str, name:str, node_api:str, invite:str, steps:[ProvisionStep], step_index:i64, height:i64, peers_live:i64, peers_total:i64, tier:str, error:str, busy:bool)
  box #root w=fill h=fill p=30.0 align-x=center align-y=center bg=bg_wash
    col gap=0.0
      match phase
        "welcome"
          WelcomeScreen
        "create"
          CreateScreen node_api=node_api busy=busy error=error
        "provisioning"
          ProvisioningScreen name=name steps=steps step_index=step_index error=error
        "live"
          LiveScreen name=name invite=invite height=height peers_live=peers_live peers_total=peers_total tier=tier busy=busy error=error
        "join"
          JoinScreen busy=busy error=error
        _
          col gap=0.0
            space w=1.0 h=1.0

// The 430px column every screen sits in, with the `‹ STEP 1 / 3` control that
// doubles as the step label. `step` empty hides it, `title` empty hides the
// heading — Welcome and Live carry their own hero instead.
component OnboardingShell(phase:str, title:str, step:str)
  col #root w=430.0 gap=0.0
    if step != ""
      OnboardingStepLabel phase=phase step=step
    if title != ""
      box w=fill pt=16.0
        text title w=fill size=20.0 wrap=none font=display @text-primary
    slot

// On Create and Join the step label IS the way back. On Provisioning there is
// no way back — the workspace already exists on disk — so it is plain text.
component OnboardingStepLabel(phase:str, step:str)
  col #root
    if phase == "provisioning"
      row gap=8.0 align=center
        text step size=11.0 wrap=none font=code_medium @text-meta
    if phase != "provisioning"
      button label="Back" @ghost_action px-0px py-0px rounded-6px -> go_welcome
        row gap=8.0 align=center
          text "‹" size=14.0 wrap=none @text-meta
          text step size=11.0 wrap=none font=code_medium @text-meta

// WELCOME. The brand plate, what this is, and the only two ways in. The footer
// says where the key lives — and it does NOT repeat the artifact's `node boots
// locally`, because this app never boots one.
component WelcomeScreen()
  col #root w=430.0 gap=0.0 align=center
    box w=50.0 h=50.0 align-x=center align-y=center bg=primary r=13.0
      text "D" size=22.0 wrap=none font=display @text-toast_fg
    box pt=18.0
      text "Welcome to Ducktape" size=22.0 wrap=none font=display @text-primary
    box w=fill pt=6.0
      col w=fill gap=0.0 align=center
        text "People and agents work on one shared record." size=13.5 line-h=1.55 @text-caption
        text "Chat, docs, code and approvals in one place." size=13.5 line-h=1.55 @text-caption
    box w=fill pt=28.0
      button label="Create a workspace" w=fill @primary_action px-17px py-15px rounded-11px -> go_create
        RouteCardBody title="Create a workspace" note="Registers a workspace on this device and generates your admin keypair." primary=true
    box w=fill pt=11.0
      button label="Join with an invite" w=fill @outline_action px-17px py-15px rounded-11px border-control_line -> go_join
        RouteCardBody title="Join with an invite" note="Materializes this device's node from an invite blob." primary=false
    box w=fill pt=24.0
      col w=fill gap=0.0 align=center
        text "workspace and admin key are created on-device" size=10.5 wrap=none font=code_medium @text-icon_idle
        text "nothing leaves this machine without your signature" size=10.5 wrap=none font=code_medium @text-icon_idle

// One route card's interior: the label, its trailing chevron, and the line
// that says what the route actually does. `primary` is the ink-filled card.
component RouteCardBody(title:str, note:str, primary:bool)
  col #root w=fill gap=3.0
    if primary
      row w=fill gap=8.0 align=center
        text title size=13.5 wrap=none font=display @text-primary_fg
        space w=fill
        text "→" size=13.5 wrap=none @text-caption
    if !primary
      row w=fill gap=8.0 align=center
        text title size=13.5 wrap=none font=display @text-accent_fg
        space w=fill
        text "→" size=13.5 wrap=none @text-chevron_idle
    if primary
      text note w=fill size=12.0 line-h=1.4 @text-ink_soft
    if !primary
      text note w=fill size=12.0 line-h=1.4 @text-meta

// CREATE. One field and three facts about what the field will produce. The
// facts are the Console file's set — the Onboarding file's `p2p port 7420` and
// `quorum 4 of 6 validators` are both false against this code.
component CreateScreen(node_api:str, busy:bool, error:str)
  state
    draft = ""
  col #root w=430.0 gap=0.0
    OnboardingShell phase="create" title="Create a workspace" step="STEP 1 / 3"
      col w=fill gap=0.0
        box w=fill pt=5.0
          text "A workspace is registered under ~/.ducktape and an admin keypair is generated on this device." w=fill size=13.0 line-h=1.5 @text-caption
        box w=fill pt=22.0
          text "WORKSPACE NAME" size=10.0 wrap=none font=code_semibold @text-label
        box w=fill pt=8.0
          box w=fill px=14.0 py=12.0 bg=surface border=primary border-w=1.5 r=10.0
            row w=fill gap=6.0 align=center
              text "#" size=14.0 wrap=none font=code_medium @text-label
              input "" #create-name label="Workspace name" <-> draft hint="acme-research" disabled=busy submit=create_network_submit(draft) w=fill p=0.0 text-size=14.0 line-h=1.2 font=code_medium @control
                active bg=transparent border=transparent value=fg placeholder=label selection=fg/18 border-w=0.0 r=0.0
                disabled value=hint
        box w=fill pt=18.0
          text "ADVANCED" size=10.0 wrap=none font=code_semibold @text-label
        box w=fill pt=8.0
          col w=fill gap=7.0
            WorkspaceDirRow slug=network_slug(draft)
            AdvancedRow label="node api" value=node_api
            AdvancedRow label="join policy" value="invite + approval"
        box w=fill pt=22.0
          button label="Create network" disabled=(busy || empty(trim(draft))) w=fill @primary_action px-0px py-13px rounded-10px -> create_network_submit(draft)
            text "Create network →" w=fill size=13.5 wrap=none align-x=center font=display @text-primary_fg
        OnboardingError message=error

// One ADVANCED row: the fact's name on the left, the value the node will
// actually use on the right. Values are READ, never retyped.
component AdvancedRow(label:str, value:str)
  box #root w=fill px=13.0 py=10.0 bg=surface border=border border-w=1.0 r=9.0
    row w=fill gap=10.0 align=center
      text label size=12.0 wrap=none font=code @text-meta
      space w=fill
      text value size=12.0 wrap=none font=code @text-secondary_fg

// The workspace-dir row is the one ADVANCED value assembled from two runs —
// Ice has no string concatenation, so the home prefix and the live slug are
// two text nodes rather than one fabricated string.
component WorkspaceDirRow(slug:str)
  box #root w=fill px=13.0 py=10.0 bg=surface border=border border-w=1.0 r=9.0
    row w=fill gap=10.0 align=center
      text "workspace dir" size=12.0 wrap=none font=code @text-meta
      space w=fill
      text "~/.ducktape/" size=12.0 wrap=none font=code @text-secondary_fg
      text slug size=12.0 wrap=none font=code @text-secondary_fg

// PROVISIONING. Five segments of bar and the step the node is actually on.
// The app does NOT supervise the daemon, so this screen never claims progress
// it has not observed: it renders exactly what `provision_progress` emitted.
component ProvisioningScreen(name:str, steps:[ProvisionStep], step_index:i64, error:str)
  col #root w=430.0 gap=0.0
    OnboardingShell phase="provisioning" title="" step="STEP 2 / 3"
      col w=fill gap=0.0
        box w=fill pt=13.0
          row w=fill gap=6.0 align=center
            text "Setting up" size=20.0 wrap=none font=display @text-primary
            text name w=fill size=20.0 wrap=none font=display @text-primary
        box w=fill pt=18.0
          box w=fill h=5.0 bg=subtle r=3.0 clip=true
            row w=fill h=fill gap=0.0
              ProgressCell filled=(step_index >= 1)
              ProgressCell filled=(step_index >= 2)
              ProgressCell filled=(step_index >= 3)
              ProgressCell filled=(step_index >= 4)
              ProgressCell filled=(step_index >= 5)
        box w=fill pt=22.0
          col w=fill gap=14.0
            for step in steps
              ProvisionRow step=step spin=0.0
        box w=fill pt=26.0
          col w=fill gap=0.0 align=center
            text "the console opens as soon as the node answers" size=11.0 wrap=none font=code_medium @text-label
        OnboardingError message=error

// One fifth of the 5px bar. Five segments, not a percentage, because the
// stream reports a step index and nothing finer.
component ProgressCell(filled:bool)
  col #root w=fill h=fill
    if filled
      box w=fill h=fill bg=primary
        space w=1.0 h=1.0
    if !filled
      box w=fill h=fill bg=subtle
        space w=1.0 h=1.0

// One checklist row. `blocked` is the state the artifact never drew: the node
// did not come up, and the label the stream carries IS the command that starts
// it — so the row becomes a refusal plate rather than a spinner that lies.
//
// `spin` is carried per the frozen signature and deliberately unconsumed: the
// artifact's running marker is a CSS-keyframe arc with a transparent top edge,
// which iced cannot stroke inside a border. The marker is a solid amber ring.
component ProvisionRow(step:ProvisionStep, spin:f64)
  col #root w=fill gap=0.0
    if step.state == "blocked"
      GateNote reason=step.label next="This app is a client — it attaches to a node you start, it never starts one."
    if step.state != "blocked"
      row w=fill gap=12.0 align=center
        ProvisionMark state=step.state
        ProvisionLabel label=step.label state=step.state

// done ✓ on the success plate, running on an amber ring, pending on the
// `pending_line` ring — the artifact's dashed outline at its own hex, solid,
// because iced's Border carries no dash.
component ProvisionMark(state:str)
  col #root
    match state
      "done"
        box w=19.0 h=19.0 align-x=center align-y=center bg=success_bg border=success_line border-w=1.0 r=9.5
          text "✓" size=10.0 wrap=none font=code_semibold @text-success
      "running"
        box w=19.0 h=19.0 bg=transparent border=warning_dot border-w=2.0 r=9.5
          space w=1.0 h=1.0
      _
        box w=19.0 h=19.0 bg=transparent border=pending_line border-w=1.0 r=9.5
          space w=1.0 h=1.0

// A reached step reads forward; an unreached one recedes.
component ProvisionLabel(label:str, state:str)
  col #root w=fill
    if state == "pending"
      text label w=fill size=13.5 line-h=1.45 @text-hint
    if state != "pending"
      text label w=fill size=13.5 line-h=1.45 @text-accent_fg

// LIVE. The hero, the reading, and the one thing this screen is for: an invite
// another device can use.
component LiveScreen(name:str, invite:str, height:i64, peers_live:i64, peers_total:i64, tier:str, busy:bool, error:str)
  col #root w=430.0 gap=0.0
    OnboardingShell phase="live" title="" step=""
      col w=fill gap=0.0
        row w=fill gap=10.0 align=center
          box w=24.0 h=24.0 align-x=center align-y=center bg=success_bg border=success_line border-w=1.0 r=12.0
            text "✓" size=12.0 wrap=none font=code_medium @text-success
          text "Your network is live" w=fill size=20.0 wrap=none font=display @text-primary
        // the chain id, because "live" is meaningless without which chain
        box w=fill pt=8.0
          text name w=fill size=11.0 wrap=none font=code_medium @text-meta
        box w=fill pt=12.0
          LiveStatusStrip height=height peers_live=peers_live peers_total=peers_total tier=tier
        box w=fill pt=20.0
          text "INVITE A NODE" size=10.0 wrap=none font=code_semibold @text-label
        box w=fill pt=9.0
          col w=fill gap=9.0
            box w=fill px=12.0 py=10.0 bg=muted_bg border=border border-w=1.0 r=9.0
              InviteValue invite=invite
            button label="Copy invite" disabled=(busy || empty(invite)) w=fill @primary_action px-0px py-9px rounded-9px -> copy_onboarding_invite
              text "Copy link" w=fill size=12.0 wrap=none align-x=center font=display @text-primary_fg
        box w=fill pt=14.0
          text "Only a device holding this invite can join, and a member still has to approve it." w=fill size=12.0 line-h=1.55 @text-caption
        box w=fill pt=24.0
          button label="Open console" w=fill @primary_action px-0px py-13px rounded-10px -> enter_console
            text "Open console →" w=fill size=13.5 wrap=none align-x=center font=display @text-primary_fg
        OnboardingError message=error

// The invite is one opaque `🦆<base64>` blob — there is no `ducktape://` URI
// and no slug inside it. Until it is minted the box says so rather than
// showing a plausible-looking fake.
component InviteValue(invite:str)
  col #root w=fill
    if empty(invite)
      text "minting…" w=fill size=11.0 wrap=none font=code_medium @text-hint
    if !empty(invite)
      text invite w=fill size=11.0 wrap=none font=code_medium @text-secondary_fg

// The reading under the hero. The artifact says `you are admin`; this product
// has no admin role, so it prints the genesis seat the roster actually reports
// and prints nothing at all when the roster has not answered yet.
component LiveStatusStrip(height:i64, peers_live:i64, peers_total:i64, tier:str)
  box #root w=fill px=12.0 py=9.0 bg=muted_bg border=separator border-w=1.0 r=8.0
    row w=fill gap=8.0 align=center
      box w=7.0 h=7.0 bg=success_dot r=3.5
        space w=1.0 h=1.0
      text "node reachable" size=11.0 wrap=none font=code_medium @text-secondary_fg
      if height >= 0
        text "· h" size=11.0 wrap=none font=code_medium @text-secondary_fg
      if height >= 0
        text height size=11.0 wrap=none font=code_medium @text-secondary_fg
      text "· peers" size=11.0 wrap=none font=code_medium @text-secondary_fg
      text peers_live size=11.0 wrap=none font=code_medium @text-secondary_fg
      text "/" size=11.0 wrap=none font=code_medium @text-secondary_fg
      text peers_total size=11.0 wrap=none font=code_medium @text-secondary_fg
      if tier != ""
        text "· you are" size=11.0 wrap=none font=code_medium @text-secondary_fg
      if tier != ""
        text tier size=11.0 wrap=none font=code_medium @text-primary

// JOIN. One field for the blob, and an honest account of what happens next —
// which is NOT the artifact's `handshake ready · 2 bootstrap peers`. Nothing in
// this app can decode an invite, so no reading is claimed; the card states the
// node's own join ladder instead, including the wait the artifact never draws.
component JoinScreen(busy:bool, error:str)
  state
    blob = ""
  col #root w=430.0 gap=0.0
    OnboardingShell phase="join" title="Join a network" step="BACK"
      col w=fill gap=0.0
        box w=fill pt=5.0
          text "Paste an invite to materialize this device's node, download the finalized history, verify it, and ask to join." w=fill size=13.0 line-h=1.5 @text-caption
        box w=fill pt=20.0
          text "INVITE BLOB" size=10.0 wrap=none font=code_semibold @text-label
        box w=fill pt=8.0
          box w=fill px=14.0 py=12.0 bg=surface border=primary border-w=1.5 r=10.0
            input "" #join-invite label="Invite blob" <-> blob hint="🦆AAAA…" disabled=busy submit=join_network_submit(blob) w=fill p=0.0 text-size=12.0 line-h=1.2 font=code @control
              active bg=transparent border=transparent value=fg placeholder=label selection=fg/18 border-w=0.0 r=0.0
              disabled value=hint
        box w=fill pt=18.0
          box w=fill p=13.0 bg=muted_bg border=separator border-w=1.0 r=10.0
            col w=fill gap=10.0
              JoinPhaseRow phase_name="parked" note="waits until a member approves this device" tone="next"
              JoinPhaseRow phase_name="admitted" note="the network accepts the node" tone="later"
              JoinPhaseRow phase_name="synced" note="finalized history downloaded and verified" tone="later"
              JoinPhaseRow phase_name="promoted" note="the console opens on live state" tone="later"
        box w=fill pt=22.0
          button label="Join network" disabled=(busy || empty(trim(blob))) w=fill @primary_action px-0px py-13px rounded-10px -> join_network_submit(blob)
            text "Join →" w=fill size=13.5 wrap=none align-x=center font=display @text-primary_fg
        OnboardingError message=error

// One rung of the join ladder, in the node's own vocabulary — these are the
// four `JoinStateView.phase` values, which exist so the console can render
// them verbatim.
component JoinPhaseRow(phase_name:str, note:str, tone:str)
  row #root w=fill gap=9.0 align=center
    if tone == "next"
      box w=6.0 h=6.0 bg=warning_dot r=3.0
        space w=1.0 h=1.0
    if tone != "next"
      box w=6.0 h=6.0 bg=pending_line r=3.0
        space w=1.0 h=1.0
    text phase_name size=11.0 wrap=none font=code_medium @text-secondary_fg
    text "·" size=11.0 wrap=none font=code_medium @text-hint
    text note w=fill size=11.0 font=code_medium @text-hint

// A refusal on this column is never a dead end: the workspace is already on
// disk, so the screen keeps its controls and says what went wrong.
component OnboardingError(message:str)
  col #root w=fill
    if !empty(message)
      box w=fill pt=14.0
        GateNote reason=message next="Nothing was lost — the workspace stays on disk. Fix the cause and try again."
