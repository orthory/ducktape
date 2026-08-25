// THE LAUNCH WINDOW'S COLUMN. `hub_step` is the single discriminant:
// (create | unlock) -> [reveal | restore] -> networks -> [join ->
// provisioning -> live]. The console never renders here — it lives in its
// own window, opened on a network pick.
//
// THERE IS NO CREATE-NETWORK ROUTE. Founding a network is an operator act on
// the node (`ducktape node init`). This app attaches to a node somebody
// already runs; the only way in from here is an invite.
//
// Every route out of this column is a NAMED EVENT, and each event carries the
// exact name and arity of the app handler it lands on — a component route may
// only resolve local handlers and declared emissions, so an app handler is
// never named inside this file.

component HubColumn(step:HubStep, key_state:str, networks:[HubNetwork], selected:str, hidden:i64, name:str, invite:str, reveal:str, steps:[ProvisionStep], step_index:i64, height:i64, tier:str, error:str, busy:bool, restore_empty:bool, join_empty:bool)
  emits
    unlock_submit(str)
    login_skip
    create_submit(str)
    reveal_confirm
    go_restore
    go_login
    restore_submit(str)
    pick_network(str)
    open_network_submit
    forget_network_submit(str, str)
    connect_remote_submit(str)
    restore_hidden_submit
    go_join
    go_networks
    join_network_submit
    copy_onboarding_invite
    enter_console
  box #root
    with
      w=fill
      h=fill
      p=26.0
      align-x=center
      align-y=center
      bg=bg_wash
    col gap=0.0
      match step
        HubStep.unlock
          UnlockScreen #unlock
            with
              key_state
              busy
              error
            forward
              unlock_submit
              login_skip
              go_restore
        HubStep.create
          CreateScreen busy=busy error=error
            forward
              create_submit
              go_restore
        HubStep.reveal
          RevealScreen words=reveal
            forward
              reveal_confirm
        HubStep.restore
          RestoreScreen
            with
              busy=busy
              error=error
              phrase_empty=restore_empty
            forward
              restore_submit
              go_login
            phrase:
              slot restore_phrase?
        HubStep.networks
          NetworksScreen #networks
            with
              networks
              selected
              hidden
              busy
              error
            forward
              pick_network
              open_network_submit
              forget_network_submit
              connect_remote_submit
              restore_hidden_submit
              go_join
        HubStep.provisioning
          ProvisioningScreen
            with
              name
              steps
              step_index
              error
        HubStep.live
          LiveScreen
            with
              name
              invite
              height
              peers_live=0
              peers_total=0
              tier
              busy
              error
            forward
              go_networks
              copy_onboarding_invite
              enter_console
        HubStep.join
          JoinScreen
            with
              busy=busy
              error=error
              invite_empty=join_empty
            forward
              go_networks
              join_network_submit
            invite:
              slot join_invite?
        HubStep.loading
          col gap=0.0 align=center
            text "…"
              with
                size=13.5
                wrap=none
                @text-hint

// The brand plate every sign-in screen opens with.
component HubBrand(title:str, caption:str)
  col #root
    with
      w=fill
      gap=0.0
      align=center
    box
      with
        w=50.0
        h=50.0
        align-x=center
        align-y=center
        bg=primary
        r=13.0
      text "D"
        with
          size=22.0
          wrap=none
          font=display
          @text-toast_fg
    box pt=18.0
      text title
        with
          size=22.0
          wrap=none
          font=display
          @text-primary
    if caption != ""
      box w=fill pt=6.0
        text caption
          with
            w=fill
            size=13.5
            line-h=1.55
            align-x=center
            @text-caption

// UNLOCK. Returning device: the password that opens this device's user.key
// becomes the session's signing password. Reads never need it, so the quiet
// way past a forgotten password stays one click.
component UnlockScreen(key_state:str, busy:bool, error:str)
  emits
    unlock_submit(str)
    login_skip
    go_restore
  state
    pw = ""
  col #root w=428.0 gap=0.0
    HubBrand title="Welcome back" caption="Unlock this device's identity to sign what you do."
    if key_state == "encrypted"
      col w=fill gap=0.0
        box w=fill pt=26.0
          text "PASSWORD"
            with
              size=10.0
              wrap=none
              font=code_semibold
              @text-label
        box w=fill pt=8.0
          box
            with
              w=fill
              px=14.0
              py=12.0
              bg=surface
              border=primary
              border-w=1.5
              r=10.0
            input "" #unlock-password <-> pw
              with
                label="Key password"
                hint="••••••••"
                secure=true
                disabled=busy
                submit=emit(unlock_submit, pw)
                w=fill
                p=0.0
                text-size=13.0
                line-h=1.2
                font=code
                @control
              active bg=transparent border=transparent value=fg placeholder=label selection=fg/18 border-w=0.0 r=0.0
              disabled value=hint
        box w=fill pt=16.0
          button -> emit(unlock_submit, pw)
            with
              label="Unlock"
              disabled=(busy || empty(pw))
              w=fill
              @primary_action
              @px-0px
              @py-13px
              @rounded-10px
            text "Unlock →"
              with
                w=fill
                size=13.5
                wrap=none
                align-x=center
                font=display
                @text-primary_fg
    if key_state != "encrypted"
      box w=fill pt=22.0
        GateNote
          with
            reason="This device's user key is not usable for signing."
            next="`ducktape user key status` explains; restore from the recovery phrase or continue read-only."
    box w=fill pt=18.0
      col
        with
          w=fill
          gap=8.0
          align=center
        button "Restore from recovery phrase" -> emit(go_restore)
          with
            disabled=busy
            h=26.0
            p=5.0
            @ghost_action
          active bg=transparent text=muted r=7.0
          hovered bg=fg/9 text=fg
          pressed bg=fg/14
        button "Continue read-only" -> emit(login_skip)
          with
            disabled=busy
            h=26.0
            p=5.0
            @ghost_action
          active bg=transparent text=muted r=7.0
          hovered bg=fg/9 text=fg
          pressed bg=fg/14
    OnboardingError message=error

// CREATE. First run: mint this device's identity under a password. The
// authoritative floor lives in Rust (`password_problem` mirrors the CLI's
// 8-char minimum); the button stays dead until the pair is acceptable.
component CreateScreen(busy:bool, error:str)
  emits
    create_submit(str)
    go_restore
  state
    pw = ""
    pw2 = ""
  col #root w=428.0 gap=0.0
    HubBrand
      with
        title="Create your identity"
        caption="One key, generated on this device. A password seals it; 24 recovery words back it up."
    box w=fill pt=26.0
      text "PASSWORD"
        with
          size=10.0
          wrap=none
          font=code_semibold
          @text-label
    box w=fill pt=8.0
      box
        with
          w=fill
          px=14.0
          py=12.0
          bg=surface
          border=primary
          border-w=1.5
          r=10.0
        input "" #create-password <-> pw
          with
            label="New password"
            hint="at least 8 characters"
            secure=true
            disabled=busy
            w=fill
            p=0.0
            text-size=13.0
            line-h=1.2
            font=code
            @control
          active bg=transparent border=transparent value=fg placeholder=label selection=fg/18 border-w=0.0 r=0.0
          disabled value=hint
    box w=fill pt=10.0
      box
        with
          w=fill
          px=14.0
          py=12.0
          bg=surface
          border=primary
          border-w=1.5
          r=10.0
        input "" #create-confirm <-> pw2
          with
            label="Confirm password"
            hint="again"
            secure=true
            disabled=busy
            submit=emit(create_submit, pw)
            w=fill
            p=0.0
            text-size=13.0
            line-h=1.2
            font=code
            @control
          active bg=transparent border=transparent value=fg placeholder=label selection=fg/18 border-w=0.0 r=0.0
          disabled value=hint
    if !empty(pw2) && password_problem(pw, pw2) != ""
      box w=fill pt=10.0
        text password_problem(pw, pw2)
          with
            w=fill
            size=12.0
            line-h=1.4
            @text-danger_fg
    box w=fill pt=16.0
      button -> emit(create_submit, pw)
        with
          label="Create identity"
          disabled=(busy || empty(pw) || password_problem(pw, pw2) != "")
          w=fill
          @primary_action
          @px-0px
          @py-13px
          @rounded-10px
        text "Create →"
          with
            w=fill
            size=13.5
            wrap=none
            align-x=center
            font=display
            @text-primary_fg
    box w=fill pt=18.0
      col
        with
          w=fill
          gap=0.0
          align=center
        button "Restore from recovery phrase" -> emit(go_restore)
          with
            disabled=busy
            h=26.0
            p=5.0
            @ghost_action
          active bg=transparent text=muted r=7.0
          hovered bg=fg/9 text=fg
          pressed bg=fg/14
    box w=fill pt=20.0
      col
        with
          w=fill
          gap=0.0
          align=center
        text "this device's key is generated on-device"
          with
            size=10.5
            wrap=none
            font=code_medium
            @text-icon_idle
        text "nothing leaves this machine without your signature"
          with
            size=10.5
            wrap=none
            font=code_medium
            @text-icon_idle
    OnboardingError message=error

// REVEAL. The one time the 24 words exist on screen. No copy button on
// purpose: a clipboard outlives this screen, paper does not leak.
component RevealScreen(words:str)
  emits
    reveal_confirm
  col #root w=428.0 gap=0.0
    HubBrand
      with
        title="Your recovery phrase"
        caption="Write these 24 words down, in order. They are the ONLY way back into this identity."
    box w=fill pt=22.0
      box
        with
          w=fill
          p=14.0
          bg=muted_bg
          border=border
          border-w=1.0
          r=10.0
        text words
          with
            w=fill
            size=13.0
            line-h=1.7
            font=code
            @text-accent_fg
    box w=fill pt=14.0
      text "Anyone holding these words IS this identity. They are shown once and never stored."
        with
          w=fill
          size=12.0
          line-h=1.55
          @text-caption
    box w=fill pt=20.0
      button -> emit(reveal_confirm)
        with
          label="I saved them"
          w=fill
          @primary_action
          @px-0px
          @py-13px
          @rounded-10px
        text "I saved them — continue →"
          with
            w=fill
            size=13.5
            wrap=none
            align-x=center
            font=display
            @text-primary_fg

// RESTORE. 24 words in, a new password around them, the same pubkey out.
component RestoreScreen(busy:bool, error:str, phrase_empty:bool)
  emits
    restore_submit(str)
    go_login
  state
    pw = ""
  col #root w=428.0 gap=0.0
    button -> emit(go_login)
      with
        label="Back"
        @ghost_action
        @px-0px
        @py-0px
        @rounded-6px
      row gap=8.0 align=center
        text "‹"
          with
            size=14.0
            wrap=none
            @text-meta
        text "BACK"
          with
            size=11.0
            wrap=none
            font=code_medium
            @text-meta
    box w=fill pt=16.0
      text "Restore your identity"
        with
          w=fill
          size=20.0
          wrap=none
          font=display
          @text-primary
    box w=fill pt=6.0
      text "Paste the 24 recovery words, pick a new password for this device."
        with
          w=fill
          size=13.0
          line-h=1.5
          @text-caption
    box w=fill pt=20.0
      text "RECOVERY PHRASE"
        with
          size=10.0
          wrap=none
          font=code_semibold
          @text-label
    box w=fill pt=8.0
      box
        with
          w=fill
          px=14.0
          py=12.0
          bg=surface
          border=primary
          border-w=1.5
          r=10.0
        slot phrase
    box w=fill pt=14.0
      text "NEW PASSWORD"
        with
          size=10.0
          wrap=none
          font=code_semibold
          @text-label
    box w=fill pt=8.0
      box
        with
          w=fill
          px=14.0
          py=12.0
          bg=surface
          border=primary
          border-w=1.5
          r=10.0
        input "" #restore-password <-> pw
          with
            label="New password"
            hint="at least 8 characters"
            secure=true
            disabled=busy
            submit=emit(restore_submit, pw)
            w=fill
            p=0.0
            text-size=13.0
            line-h=1.2
            font=code
            @control
          active bg=transparent border=transparent value=fg placeholder=label selection=fg/18 border-w=0.0 r=0.0
          disabled value=hint
    box w=fill pt=18.0
      button -> emit(restore_submit, pw)
        with
          label="Restore"
          disabled=(busy || phrase_empty || empty(pw))
          w=fill
          @primary_action
          @px-0px
          @py-13px
          @rounded-10px
        text "Restore →"
          with
            w=fill
            size=13.5
            wrap=none
            align-x=center
            font=display
            @text-primary_fg
    OnboardingError message=error

// NETWORKS. The launch window's home: every network this device knows —
// workspaces on disk and saved remote endpoints — most recently used first.
// An empty list is the old welcome screen wearing its real name.
component NetworksScreen(networks:[HubNetwork], selected:str, hidden:i64, busy:bool, error:str)
  emits
    pick_network(str)
    open_network_submit
    forget_network_submit(str, str)
    go_join
    connect_remote_submit(str)
    restore_hidden_submit
  state
    remote = ""
  col #root w=428.0 gap=0.0
    if empty(networks)
      col
        with
          w=fill
          gap=0.0
          align=center
        HubBrand
          with
            title="Welcome to Ducktape"
            caption="People and agents work on one shared record. Chat, docs, code and approvals in one place."
        box w=fill pt=28.0
          button #join-cta -> emit(go_join)
            with
              label="Join with an invite"
              disabled=busy
              w=fill
              @primary_action
              @px-17px
              @py-15px
              @rounded-11px
            col w=fill gap=3.0
              row
                with
                  w=fill
                  gap=8.0
                  align=center
                text "Join with an invite"
                  with
                    size=13.5
                    wrap=none
                    font=display
                    @text-primary_fg
                space w=fill
                text "→"
                  with
                    size=13.5
                    wrap=none
                    @text-caption
              text "Materializes this device's node from an invite."
                with
                  w=fill
                  size=12.0
                  line-h=1.4
                  @text-ink_soft
        box w=fill pt=24.0
          col
            with
              w=fill
              gap=0.0
              align=center
            text "founding a network is `ducktape node init` on the node"
              with
                size=10.5
                wrap=none
                font=code_medium
                @text-icon_idle
    if !empty(networks)
      col w=fill gap=0.0
        text "Choose a network"
          with
            w=fill
            size=20.0
            wrap=none
            font=display
            @text-primary
        box w=fill pt=6.0
          text "Local workspaces on this device and saved remote endpoints."
            with
              w=fill
              size=13.0
              line-h=1.5
              @text-caption
        box w=fill pt=16.0
          scroll
            with
              dir=vertical
              w=fill
              h=360.0
            col w=fill gap=8.0
              for row in networks
                NetworkRow
                  with
                    row
                    selected=(row.id == selected)
                    busy
                  forward
                    pick_network
                    forget_network_submit
        box w=fill pt=18.0
          button -> emit(open_network_submit)
            with
              label="Open network"
              disabled=(busy || empty(selected))
              w=fill
              @primary_action
              @px-0px
              @py-13px
              @rounded-10px
            text "Open →"
              with
                w=fill
                size=13.5
                wrap=none
                align-x=center
                font=display
                @text-primary_fg
        box w=fill pt=12.0
          col
            with
              w=fill
              gap=0.0
              align=center
            button "Join another network with an invite" -> emit(go_join)
              with
                disabled=busy
                h=26.0
                p=5.0
                @ghost_action
              active bg=transparent text=muted r=7.0
              hovered bg=fg/9 text=fg
              pressed bg=fg/14
        // A remote node this device holds no workspace for — Enter connects,
        // and a successful connect is what saves it as a remote row.
        box w=fill pt=10.0
          box
            with
              w=fill
              px=14.0
              py=10.0
              bg=surface
              border=border
              border-w=1.0
              r=10.0
            input "" #remote-endpoint <-> remote
              with
                label="Remote node endpoint"
                hint="connect a remote node… (http://host:port)"
                disabled=busy
                submit=emit(connect_remote_submit, remote)
                w=fill
                p=0.0
                text-size=12.0
                line-h=1.2
                font=code
                @control
              active bg=transparent border=transparent value=fg placeholder=label selection=fg/18 border-w=0.0 r=0.0
              disabled value=hint
    // Forgetting is not a one-way door: every hidden local network comes
    // back with one click. Lives OUTSIDE the empty/non-empty branch —
    // forgetting the ONLY network empties the list, and that is exactly
    // when the door must stay visible.
    if hidden > 0
      box w=fill pt=10.0
        col
          with
            w=fill
            gap=0.0
            align=center
          button "Restore hidden networks" #restore-hidden -> emit(restore_hidden_submit)
            with
              disabled=busy
              h=24.0
              p=4.0
              @ghost_action
            active bg=transparent text=muted r=7.0
            hovered bg=fg/9 text=fg
            pressed bg=fg/14
    OnboardingError message=error

// One network row: the liveness dot, the name, where it lives, and — while
// selected — the honest state line and the forget control.
component NetworkRow(row:HubNetwork, selected:bool, busy:bool)
  emits
    pick_network(str)
    forget_network_submit(str, str)
  col #root w=fill gap=0.0
    if selected
      col w=fill gap=0.0
        button -> emit(pick_network, row.id)
          with
            label=row.name
            checked=selected
            w=fill
            p=0.0
            @icon_action
          box
            with
              w=fill
              px=13.0
              pt=11.0
              pb=11.0
            col w=fill gap=4.0
              row
                with
                  w=fill
                  gap=9.0
                  align=center
                NetworkDot probed=row.probed live=row.live
                text row.name
                  with
                    w=fill
                    size=13.5
                    wrap=none
                    font=display
                    @text-primary
                text row.kind
                  with
                    size=9.5
                    wrap=none
                    font=code_semibold
                    @text-label
              row
                with
                  w=fill
                  gap=9.0
                  align=center
                text row.endpoint
                  with
                    w=fill
                    size=11.0
                    wrap=none
                    font=code_medium
                    @text-meta
                if row.height >= 0
                  text height_label_short(row.height)
                    with
                      size=11.0
                      wrap=none
                      font=code_medium
                      @text-meta
              if row.probed && !row.live
                text network_run_hint(row)
                  with
                    w=fill
                    size=11.0
                    wrap=none
                    font=code_medium
                    @text-hint
          active bg=selected_row text=fg border=primary border-w=1.5 r=11.0
          hovered bg=selected_row text=fg
          pressed bg=rail_hover text=fg
        box
          with
            w=fill
            pt=4.0
            align-x=end
          button "Forget" -> emit(forget_network_submit, row.id, row.kind)
            with
              disabled=busy
              h=22.0
              p=4.0
              @ghost_action
            active bg=transparent text=muted border=transparent border-w=1.0 r=6.0
            hovered bg=danger_bg text=fg
            pressed bg=danger_bg text=fg
    if !selected
      button -> emit(pick_network, row.id)
        with
          label=row.name
          checked=selected
          w=fill
          p=0.0
          @icon_action
        box
          with
            w=fill
            px=13.0
            pt=11.0
            pb=11.0
          col w=fill gap=4.0
            row
              with
                w=fill
                gap=9.0
                align=center
              NetworkDot probed=row.probed live=row.live
              text row.name
                with
                  w=fill
                  size=13.5
                  wrap=none
                  font=display
                  @text-primary
              text row.kind
                with
                  size=9.5
                  wrap=none
                  font=code_semibold
                  @text-label
            row
              with
                w=fill
                gap=9.0
                align=center
              text row.endpoint
                with
                  w=fill
                  size=11.0
                  wrap=none
                  font=code_medium
                  @text-meta
        active bg=surface text=muted border=border border-w=1.0 r=11.0
        hovered bg=subtle text=fg
        pressed bg=rail_hover text=fg

// The row's liveness reading: measured-live, measured-dead, or not answered
// yet — three states, never a guess.
component NetworkDot(probed:bool, live:bool)
  col #root
    if probed && live
      box
        with
          w=8.0
          h=8.0
          bg=success_dot
          r=4.0
        space w=1.0 h=1.0
    if probed && !live
      box
        with
          w=8.0
          h=8.0
          bg=transparent
          border=pending_line
          border-w=2.0
          r=4.0
        space w=1.0 h=1.0
    if !probed
      box
        with
          w=8.0
          h=8.0
          bg=subtle
          r=4.0
        space w=1.0 h=1.0

// PROVISIONING. Five segments of bar and the step the node is actually on.
// The app does NOT supervise the daemon, so this screen never claims progress
// it has not observed: it renders exactly what `provision_progress` emitted.
component ProvisioningScreen(name:str, steps:[ProvisionStep], step_index:i64, error:str)
  col #root w=428.0 gap=0.0
    row gap=8.0 align=center
      text "STEP 2 / 3"
        with
          size=11.0
          wrap=none
          font=code_medium
          @text-meta
    box w=fill pt=13.0
      row
        with
          w=fill
          gap=6.0
          align=center
        text "Setting up"
          with
            size=20.0
            wrap=none
            font=display
            @text-primary
        text name
          with
            w=fill
            size=20.0
            wrap=none
            font=display
            @text-primary
    box w=fill pt=18.0
      box
        with
          w=fill
          h=5.0
          bg=subtle
          r=3.0
          clip=true
        row
          with
            w=fill
            h=fill
            gap=0.0
          ProgressCell filled=(step_index >= 1)
          ProgressCell filled=(step_index >= 2)
          ProgressCell filled=(step_index >= 3)
          ProgressCell filled=(step_index >= 4)
          ProgressCell filled=(step_index >= 5)
    box w=fill pt=22.0
      col w=fill gap=14.0
        for step in steps
          ProvisionRow step=step
    box w=fill pt=26.0
      col
        with
          w=fill
          gap=0.0
          align=center
        text "the console opens as soon as the node answers"
          with
            size=11.0
            wrap=none
            font=code_medium
            @text-label
    OnboardingError message=error

// One fifth of the 5px bar. Five segments, not a percentage, because the
// stream reports a step index and nothing finer.
component ProgressCell(filled:bool)
  col #root w=fill h=fill
    if filled
      box
        with
          w=fill
          h=fill
          bg=primary
        space w=1.0 h=1.0
    if !filled
      box
        with
          w=fill
          h=fill
          bg=subtle
        space w=1.0 h=1.0

// One checklist row. `blocked` is the state the artifact never drew: the node
// did not come up, and the label the stream carries IS the command that starts
// it — so the row becomes a refusal plate rather than a spinner that lies.
// Reads each of the step's String fields exactly once, in exclusive `match`
// arms: two `if` blocks both reading `step.label` move the same String twice
// and the generated Rust will not compile. Copy fields have no such limit.
component ProvisionRow(step:ProvisionStep)
  col #root w=fill gap=0.0
    match step.state
      "blocked"
        GateNote
          with
            reason=step.label
            next="This app is a client — it attaches to a node you start, it never starts one."
      "done"
        ProvisionLine
          with
            tone="done"
            label=step.label
            dim=false
      "running"
        ProvisionLine
          with
            tone="running"
            label=step.label
            dim=false
      _
        ProvisionLine
          with
            tone="pending"
            label=step.label
            dim=true

// The mark and its label, one row. Split out so `tone` reaches the mark and
// the label's dimming without either component reading a String twice.
component ProvisionLine(tone:str, label:str, dim:bool)
  row #root
    with
      w=fill
      gap=12.0
      align=center
    ProvisionMark state=tone
    ProvisionLabel label=label dim=dim

// done ✓ on the success plate, running on an amber ring, pending on the
// `pending_line` ring — the artifact's dashed outline at its own hex, solid,
// because iced's Border carries no dash.
component ProvisionMark(state:str)
  col #root
    match state
      "done"
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
              size=10.0
              wrap=none
              font=code_semibold
              @text-success
      "running"
        box
          with
            w=19.0
            h=19.0
            bg=transparent
            border=warning_dot
            border-w=2.0
            r=9.5
          space w=1.0 h=1.0
      _
        box
          with
            w=19.0
            h=19.0
            bg=transparent
            border=pending_line
            border-w=1.0
            r=9.5
          space w=1.0 h=1.0

// A reached step reads forward; an unreached one recedes.
component ProvisionLabel(label:str, dim:bool)
  col #root w=fill
    match dim
      true
        text label
          with
            w=fill
            size=13.5
            line-h=1.45
            @text-hint
      _
        text label
          with
            w=fill
            size=13.5
            line-h=1.45
            @text-accent_fg

// LIVE. The hero, the reading, and the one thing this screen is for: an invite
// another device can use.
component LiveScreen(name:str, invite:str, height:i64, peers_live:i64, peers_total:i64, tier:str, busy:bool, error:str)
  emits
    go_networks
    copy_onboarding_invite
    enter_console
  col #root w=428.0 gap=0.0
    button -> emit(go_networks)
      with
        label="Back"
        @ghost_action
        @px-0px
        @py-0px
        @rounded-6px
      row gap=8.0 align=center
        text "‹"
          with
            size=14.0
            wrap=none
            @text-meta
        text "NETWORKS"
          with
            size=11.0
            wrap=none
            font=code_medium
            @text-meta
    box w=fill pt=16.0
      row
        with
          w=fill
          gap=10.0
          align=center
        box
          with
            w=24.0
            h=24.0
            align-x=center
            align-y=center
            bg=success_bg
            border=success_line
            border-w=1.0
            r=12.0
          text "✓"
            with
              size=12.0
              wrap=none
              font=code_medium
              @text-success
        text "Your network is live"
          with
            w=fill
            size=20.0
            wrap=none
            font=display
            @text-primary
    // the chain id, because "live" is meaningless without which chain
    box w=fill pt=8.0
      text name
        with
          w=fill
          size=11.0
          wrap=none
          font=code_medium
          @text-meta
    box w=fill pt=12.0
      LiveStatusStrip
        with
          height
          peers_live
          peers_total
          tier
    box w=fill pt=20.0
      text "INVITE A NODE"
        with
          size=10.0
          wrap=none
          font=code_semibold
          @text-label
    box w=fill pt=9.0
      col w=fill gap=9.0
        box
          with
            w=fill
            px=12.0
            py=10.0
            bg=muted_bg
            border=border
            border-w=1.0
            r=9.0
          InviteValue invite=invite
        button -> emit(copy_onboarding_invite)
          with
            label="Copy invite"
            disabled=(busy || empty(invite))
            w=fill
            @primary_action
            @px-0px
            @py-9px
            @rounded-9px
          text "Copy invite"
            with
              w=fill
              size=12.0
              wrap=none
              align-x=center
              font=display
              @text-primary_fg
    box w=fill pt=14.0
      // The blob IS the admission decision: minting signs a single-use bearer token
      // that whoever redeems it first spends automatically through the join gate —
      // no member approval follows, so promising one made a forwardable credential
      // read as gated. The window is the other half of the terms: the handler mints
      // with `mint_invite(.., 7)` and the TTL is signed INSIDE the blob, so a holder
      // on day 8 is refused with nothing here left to re-open it.
      text "Whoever holds this invite can join — it is single-use and expires 7 days from minting, so send it to one device."
        with
          w=fill
          size=12.0
          line-h=1.55
          @text-caption
    box w=fill pt=24.0
      button -> emit(enter_console)
        with
          label="Open console"
          w=fill
          @primary_action
          @px-0px
          @py-13px
          @rounded-10px
        text "Open console →"
          with
            w=fill
            size=13.5
            wrap=none
            align-x=center
            font=display
            @text-primary_fg
    OnboardingError message=error

// The invite is one opaque `🦆<base64>` blob — there is no `ducktape://` URI
// and no slug inside it. Until it is minted the box says so rather than
// showing a plausible-looking fake.
component InviteValue(invite:str)
  col #root w=fill
    if empty(invite)
      text "minting…"
        with
          w=fill
          size=11.0
          wrap=none
          font=code_medium
          @text-hint
    if !empty(invite)
      // `word-or-glyph`, because the invite is a several-hundred-character
      // duck-emoji + base64 token carrying no space: word wrapping has nothing
      // to break on, so the run's minimum intrinsic width became the whole blob
      // and it drew straight past this 428px card in a 480x680 window that does
      // not scroll. Same ruling MemberFactRow made for public keys — the box
      // grows taller and the value stays readable end to end, which is the
      // point of the screen: an invite you cannot read in full is not an invite.
      text invite
        with
          w=fill
          size=11.0
          wrap=word-or-glyph
          font=code_medium
          @text-secondary_fg

// The reading under the hero. The artifact says `you are admin`; this product
// has no admin role, so it prints the genesis seat the roster actually reports
// and prints nothing at all when the roster has not answered yet.
component LiveStatusStrip(height:i64, peers_live:i64, peers_total:i64, tier:str)
  box #root
    with
      w=fill
      px=12.0
      py=9.0
      bg=muted_bg
      border=separator
      border-w=1.0
      r=8.0
    row
      with
        w=fill
        gap=8.0
        align=center
      box
        with
          w=7.0
          h=7.0
          bg=success_dot
          r=3.5
        space w=1.0 h=1.0
      text "node reachable"
        with
          size=11.0
          wrap=none
          font=code_medium
          @text-secondary_fg
      if height >= 0
        text "· h"
          with
            size=11.0
            wrap=none
            font=code_medium
            @text-secondary_fg
      if height >= 0
        text height
          with
            size=11.0
            wrap=none
            font=code_medium
            @text-secondary_fg
      // GUARDED, like the height and tier runs beside it. Nothing measures the
      // gossip reading — the caller passes literal zeros — and an unguarded run
      // printed `· peers 0 / 0` as a measured fact on a healthy node. The
      // segment drops until a peer count has a source.
      if peers_total > 0
        text "· peers"
          with
            size=11.0
            wrap=none
            font=code_medium
            @text-secondary_fg
      if peers_total > 0
        text peers_live
          with
            size=11.0
            wrap=none
            font=code_medium
            @text-secondary_fg
      if peers_total > 0
        text "/"
          with
            size=11.0
            wrap=none
            font=code_medium
            @text-secondary_fg
      if peers_total > 0
        text peers_total
          with
            size=11.0
            wrap=none
            font=code_medium
            @text-secondary_fg
      if tier != ""
        text "· you are"
          with
            size=11.0
            wrap=none
            font=code_medium
            @text-secondary_fg
      if tier != ""
        text tier
          with
            size=11.0
            wrap=none
            font=code_medium
            @text-primary

// JOIN. One field for the blob, and an honest account of what happens next.
// Nothing in this app can decode an invite, so no reading is claimed; the
// card states the node's own join ladder instead, including the wait.
component JoinScreen(busy:bool, error:str, invite_empty:bool)
  emits
    go_networks
    join_network_submit
  col #root w=428.0 gap=0.0
    button -> emit(go_networks)
      with
        label="Back"
        @ghost_action
        @px-0px
        @py-0px
        @rounded-6px
      row gap=8.0 align=center
        text "‹"
          with
            size=14.0
            wrap=none
            @text-meta
        text "NETWORKS"
          with
            size=11.0
            wrap=none
            font=code_medium
            @text-meta
    box w=fill pt=16.0
      text "Join a network"
        with
          w=fill
          size=20.0
          wrap=none
          font=display
          @text-primary
    box w=fill pt=5.0
      text "Paste an invite to materialize this device's node, download the finalized history, verify it, and ask to join."
        with
          w=fill
          size=13.0
          line-h=1.5
          @text-caption
    box w=fill pt=20.0
      // `INVITE`, not `INVITE BLOB`. Every other eyebrow on these screens is
      // plain English — PASSWORD, RECOVERY PHRASE, NETWORKS — and this one sat
      // under a heading that had just called the same thing "an invite". "Blob"
      // is what the wire calls it, not what the person pasting it calls it.
      text "INVITE"
        with
          size=10.0
          wrap=none
          font=code_semibold
          @text-label
    box w=fill pt=8.0
      box
        with
          w=fill
          px=14.0
          py=12.0
          bg=surface
          border=primary
          border-w=1.5
          r=10.0
        slot invite
    box w=fill pt=18.0
      box
        with
          w=fill
          p=13.0
          bg=muted_bg
          border=separator
          border-w=1.0
          r=10.0
        col w=fill gap=10.0
          JoinPhaseRow
            with
              phase_name="parked"
              note="waits for the invite to be redeemed through the join gate"
              tone="next"
          JoinPhaseRow
            with
              phase_name="admitted"
              note="the network accepts the node"
              tone="later"
          JoinPhaseRow
            with
              phase_name="synced"
              note="finalized history downloaded and verified"
              tone="later"
          JoinPhaseRow
            with
              phase_name="promoted"
              note="the console opens on live state"
              tone="later"
    box w=fill pt=22.0
      button -> emit(join_network_submit)
        with
          label="Join network"
          disabled=(busy || invite_empty)
          w=fill
          @primary_action
          @px-0px
          @py-13px
          @rounded-10px
        text "Join →"
          with
            w=fill
            size=13.5
            wrap=none
            align-x=center
            font=display
            @text-primary_fg
    OnboardingError message=error

// One rung of the join ladder, in the node's own vocabulary — these are the
// four `JoinStateView.phase` values, which exist so the console can render
// them verbatim.
component JoinPhaseRow(phase_name:str, note:str, tone:str)
  row #root
    with
      w=fill
      gap=9.0
      align=center
    if tone == "next"
      box
        with
          w=6.0
          h=6.0
          bg=warning_dot
          r=3.0
        space w=1.0 h=1.0
    if tone != "next"
      box
        with
          w=6.0
          h=6.0
          bg=pending_line
          r=3.0
        space w=1.0 h=1.0
    text phase_name
      with
        size=11.0
        wrap=none
        font=code_medium
        @text-secondary_fg
    text "·"
      with
        size=11.0
        wrap=none
        font=code_medium
        @text-hint
    text note
      with
        w=fill
        size=11.0
        font=code_medium
        @text-hint

// A refusal on this column is never a dead end: the screen keeps its
// controls and says what went wrong.
component OnboardingError(message:str)
  col #root w=fill
    if !empty(message)
      box w=fill pt=14.0
        GateNote reason=message next="Nothing was lost. Fix the cause and try again."
