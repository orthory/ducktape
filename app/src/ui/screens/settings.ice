// SETTINGS — this device's preferences, account custody and workspace
// lifecycle. Node operations have their own rail surface; the workspace card
// links there instead of duplicating its facts here. See
// `screens/roster.ice` for the screen contract: no app state is reachable from
// here, so every reading is a prop and every act leaves as a named event that
// `view.ice` routes back to the handler of the same name.
component SettingsScreen(account_name:str, network_name:str, connected_rpc:str, account_ceremony_phase:str, account_ceremony_qr:str, account_ceremony_detail:str, account_ceremony_left:str, settings_key_state:str, settings_key_path:str, settings_open_tabs:i64, members_rows:[MemberRow], members_answered:bool, account_number:str, bind account_name_draft:str, account_renaming:bool, account_exists:bool, account_keys:i64, account_key_rows:[AccountKeyRow], account_busy:bool, bind account_create_draft:str, bind account_key_draft:str, bind account_key_label_draft:str, account_ticket:str, bind account_join_draft:str, appearance:Appearance, desktop_notifications:bool, password:str, status:str, loading:bool, connected:bool, mutation_phase:MutationPhase)
  emits
    select_shell_tab(ShellTab)
    reconnect()
    switch_network
    settings_unlock_submit(str)
    lock_session
    account_name_draft_changed(str)
    account_rename_submit()
    account_create_draft_changed(str)
    account_create_submit()
    account_key_draft_changed(str)
    account_key_label_draft_changed(str)
    account_key_add_submit()
    account_join_draft_changed(str)
    account_key_join_submit()
    account_key_remove(str)
    account_passkey_submit()
    account_passkey_desktop()
    account_ceremony_cancel()
    account_wallet_submit()
    account_login_submit()
    copy_to_clipboard(str, str)
    settings_clear_tabs()
    forget_workspace_submit()
    set_appearance_light()
    set_appearance_dark()
    set_desktop_notifications(bool)
  state
    key_pw = ""
    // WHICH GROUP IS ON SCREEN. Instance state, never app state: the pane is
    // chrome. Nothing outside this screen reads it, nothing loads when it
    // changes, and a reader who leaves Settings has no pane — so a field in
    // `core.ice` would be a value the app carries around for one widget.
    settings_pane:SettingsPane = SettingsPane.general
  on pick_pane(picked)
    settings_pane = picked
  scroll #settings-body
    with
      dir=vertical
      w=fill
      h=fill
    col
      with
        w=fill
        // The cards are label/value rows, not prose: past ~860px the value
        // drifts a whole hand-span from the label it belongs to. The grid this
        // replaced answered the same width with a second column, which is what
        // split identity from its keys.
        max-w=860.0
        p=22.0
        gap=18.0
      col w=fill gap=13.0
        text "Settings"
          with
            size=16.0
            wrap=none
            font=display
            @text-primary
        // ONE ROW OF GROUPS, the same shape `This node` wears — a tab strip
        // over a single `match`. Eight cards in one scroll had no way in
        // except the wheel: `Forget this network` was nine group-heights below
        // the fold, and the theme switch sat in the same list as it.
        row gap=3.0 align=center
          button #settings-general-tab -> pick_pane(SettingsPane.general)
            with
              w=shrink
              label="General"
              checked=(settings_pane == SettingsPane.general)
              p=0.0
              @ghost_action
            box px=15.0 py=0.0
              TabLabel
                with
                  label="General"
                  count=0
                  active=(settings_pane == SettingsPane.general)
            active bg=transparent text=muted border=transparent border-w=1.0 r=8.0
            hovered bg=row_hover text=fg
            pressed bg=elevated text=fg
          button #settings-network-tab -> pick_pane(SettingsPane.network)
            with
              w=shrink
              label="Network"
              checked=(settings_pane == SettingsPane.network)
              p=0.0
              @ghost_action
            box px=15.0 py=0.0
              TabLabel
                with
                  label="Network"
                  count=0
                  active=(settings_pane == SettingsPane.network)
            active bg=transparent text=muted border=transparent border-w=1.0 r=8.0
            hovered bg=row_hover text=fg
            pressed bg=elevated text=fg
          button #settings-account-tab -> pick_pane(SettingsPane.account)
            with
              w=shrink
              label="Account"
              checked=(settings_pane == SettingsPane.account)
              p=0.0
              @ghost_action
            box px=15.0 py=0.0
              TabLabel
                with
                  label="Account"
                  count=account_keys
                  active=(settings_pane == SettingsPane.account)
            active bg=transparent text=muted border=transparent border-w=1.0 r=8.0
            hovered bg=row_hover text=fg
            pressed bg=elevated text=fg
          button #settings-security-tab -> pick_pane(SettingsPane.security)
            with
              w=shrink
              label="Security"
              checked=(settings_pane == SettingsPane.security)
              p=0.0
              @ghost_action
            box px=15.0 py=0.0
              TabLabel
                with
                  label="Security"
                  count=0
                  active=(settings_pane == SettingsPane.security)
            active bg=transparent text=muted border=transparent border-w=1.0 r=8.0
            hovered bg=row_hover text=fg
            pressed bg=elevated text=fg
          button #settings-danger-tab -> pick_pane(SettingsPane.danger)
            with
              w=shrink
              label="Danger zone"
              checked=(settings_pane == SettingsPane.danger)
              p=0.0
              @ghost_action
            box px=15.0 py=0.0
              TabLabel
                with
                  label="Danger zone"
                  count=0
                  active=(settings_pane == SettingsPane.danger)
            active bg=transparent text=muted border=transparent border-w=1.0 r=8.0
            hovered bg=row_hover text=fg
            pressed bg=elevated text=fg
      match settings_pane
        // THIS DEVICE'S OWN PREFERENCES — the three groups that change nothing
        // on the network and travel with no key.
        SettingsPane.general
          col w=fill gap=18.0
            col w=fill gap=9.0
              GroupLabel label="APPEARANCE"
              box
                with
                  w=fill
                  p=15.0
                  bg=surface
                  border=card_line
                  border-w=1.0
                  r=11.0
                row
                  with
                    w=fill
                    gap=9.0
                    align=center
                  col w=fill gap=3.0
                    text "Theme"
                      with
                        size=12.5
                        wrap=none
                        @text-fg
                    match appearance
                      Appearance.system
                        text "Following the system appearance." size=11.0 @text-caption
                      Appearance.light
                        text "Pinned for this device." size=11.0 @text-caption
                      Appearance.dark
                        text "Pinned for this device." size=11.0 @text-caption
                  space w=fill
                  if appearance == Appearance.light
                    button "Light" -> emit(set_appearance_light)
                      with
                        checked=true
                        h=28.0
                        p=6.0
                        @primary_action
                  if appearance != Appearance.light
                    button "Light" -> emit(set_appearance_light)
                      with
                        checked=false
                        h=28.0
                        p=6.0
                        @secondary_action
                  if appearance == Appearance.dark
                    button "Dark" -> emit(set_appearance_dark)
                      with
                        checked=true
                        h=28.0
                        p=6.0
                        @primary_action
                  if appearance != Appearance.dark
                    button "Dark" -> emit(set_appearance_dark)
                      with
                        checked=false
                        h=28.0
                        p=6.0
                        @secondary_action
            col w=fill gap=9.0
              GroupLabel label="NOTIFICATIONS"
              box
                with
                  w=fill
                  p=15.0
                  bg=surface
                  border=card_line
                  border-w=1.0
                  r=11.0
                row
                  with
                    w=fill
                    gap=9.0
                    align=center
                  col w=fill gap=3.0
                    text "Mentions and direct messages"
                      with
                        size=12.5
                        wrap=none
                        @text-fg
                    if desktop_notifications
                      text "A desktop banner when you are named, or written to directly." size=11.0 @text-caption
                    if !desktop_notifications
                      text "Silent — the bell is the only notice." size=11.0 @text-caption
                  space w=fill
                  if desktop_notifications
                    button "On" -> emit(set_desktop_notifications, true)
                      with
                        checked=true
                        h=28.0
                        p=6.0
                        @primary_action
                  if !desktop_notifications
                    button "On" -> emit(set_desktop_notifications, true)
                      with
                        checked=false
                        h=28.0
                        p=6.0
                        @secondary_action
                  if !desktop_notifications
                    button "Off" -> emit(set_desktop_notifications, false)
                      with
                        checked=true
                        h=28.0
                        p=6.0
                        @primary_action
                  if desktop_notifications
                    button "Off" -> emit(set_desktop_notifications, false)
                      with
                        checked=false
                        h=28.0
                        p=6.0
                        @secondary_action
            // NO PREFERENCES GROUP. `Change receipts` was a placebo: every
            // finality mark in the app — FinalityChip, the chat tick, the
            // merge stamp — renders unconditionally, so the switch wrote a
            // value nothing read. It also painted ON from the state default
            // and flipped itself OFF a beat later, because the loader
            // answers `false` for an absent key. The group comes back the
            // day the marks are actually gated on it.
            col w=fill gap=9.0
              GroupLabel label="THIS DEVICE"
              box
                with
                  w=fill
                  bg=surface
                  border=card_line
                  border-w=1.0
                  r=11.0
                  clip=true
                col w=fill
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
                      col w=fill gap=1.0
                        text "Open page tabs" size=12.5 @text-accent_fg
                        text "Preferences persist per endpoint in app-prefs.json beside the user key."
                          with
                            size=12.5
                            @text-meta
                      text settings_open_tabs
                        with
                          size=12.0
                          wrap=none
                          font=code_medium
                          @text-secondary_fg
                      button "Forget tabs" -> emit(settings_clear_tabs)
                        with
                          h=28.0
                          p=5.0
                          @secondary_action
        SettingsPane.network
          col w=fill gap=18.0
            col w=fill gap=9.0
              GroupLabel label="NETWORK"
              GroupCard
                col w=fill
                  KeyValueRow
                    with
                      label="Workspace"
                      value=network_name
                      last=false
                  KeyValueRow
                    with
                      label="Endpoint"
                      value=connected_rpc
                      last=false
                  // The artifact's last NETWORK row: the roster reading with an
                  // inline accent link onto the Members screen.
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
                      text "Members"
                        with
                          size=12.5
                          wrap=none
                          @text-accent_fg
                      space w=fill
                      text members_summary(connected, members_rows)
                        with
                          size=12.0
                          wrap=none
                          font=code_medium
                          @text-secondary_fg
                      button "manage" -> emit(select_shell_tab, ShellTab.members)
                        with
                          h=22.0
                          p=0.0
                          @ghost_action
                        active bg=transparent text=brand border=transparent border-w=1.0 r=6.0
                        hovered bg=elevated text=brand
                        pressed bg=subtle text=brand
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
                      text "Node"
                        with
                          size=12.5
                          wrap=none
                          @text-accent_fg
                      space w=fill
                      text status
                        with
                          size=12.0
                          wrap=none
                          font=code_medium
                          @text-secondary_fg
                      button "view" -> emit(select_shell_tab, ShellTab.node)
                        with
                          h=22.0
                          p=0.0
                          @ghost_action
                        active bg=transparent text=brand border=transparent border-w=1.0 r=6.0
                        hovered bg=elevated text=brand
                        pressed bg=subtle text=brand
                  // The connection's two acts. There is no endpoint field: the
                  // launch window's picker owns WHICH network; this card only
                  // retries the picked one or goes back to the list.
                  box
                    with
                      w=fill
                      px=15.0
                      py=13.0
                    row
                      with
                        w=fill
                        gap=9.0
                        align=center
                      if connection_degraded(status)
                        Badge.Destructive label=status
                      if !connection_degraded(status)
                        Badge.Success label=status
                      space w=fill
                      button "Reconnect" -> emit(reconnect)
                        with
                          disabled=(loading || (mutation_phase != MutationPhase.idle && mutation_phase != MutationPhase.recovering))
                          h=28.0
                          p=6.0
                          @secondary_action
                      button "Switch network" -> emit(switch_network)
                        with
                          disabled=(mutation_phase != MutationPhase.idle)
                          h=28.0
                          p=6.0
                          @secondary_action
        // WHO YOU ARE, AND EVERY KEY THAT SPEAKS FOR YOU — one pane, because
        // the key list is only readable next to the account it hangs off.
        SettingsPane.account
          col w=fill gap=18.0
            col w=fill gap=9.0
              GroupLabel label="YOUR IDENTITY"
              box
                with
                  w=fill
                  p=15.0
                  bg=surface
                  border=card_line
                  border-w=1.0
                  r=11.0
                row
                  with
                    w=fill
                    gap=13.0
                    align=center
                  PersonAvatar
                    with
                      initials=initial_of(account_name)
                      plate=40.0
                      ink=14.0
                  // clip: the key line is four `wrap=none` runs over a 64-hex
                  // key, so it cannot shrink — without this it paints over the
                  // rename controls in the next column.
                  col
                    with
                      w=fill
                      gap=3.0
                      clip=true
                    row
                      with
                        w=fill
                        gap=7.0
                        align=center
                      if !empty(account_name)
                        text account_name
                          with
                            size=13.5
                            wrap=none
                            font=display
                            @text-fg
                      if empty(account_name)
                        text "(unnamed)"
                          with
                            size=13.5
                            wrap=none
                            @text-muted
                      // The badge carries STANDING on this network — validator,
                      // resident, guest — not whether a local key is bound. The
                      // artifact's ADMIN/MAINTAINER words name authority this
                      // chain does not grant, so the app keeps its own. An empty
                      // tier is not a standing — it is the roster not having
                      // answered — so it says so instead of drawing a bare pill,
                      // but only once the roster HAS answered: `member_tier`
                      // returns "" for "no answer yet" and for "answered, not
                      // listed" alike, and the load starts after hydration clears
                      // `loading`, so an ungated alarm fires on every cold start.
                      if members_is_admin(members_rows)
                        Badge.Secondary label=member_tier(members_rows)
                      if !members_is_admin(members_rows) && !empty(member_tier(members_rows))
                        Badge.Outline label=member_tier(members_rows)
                      if empty(member_tier(members_rows)) && members_answered
                        Badge.Outline label="standing unknown"
                    // The number line says WHICH account this key belongs to and
                    // that the key lives on this device — the custody clause the
                    // artifact carries.
                    row
                      with
                        w=fill
                        gap=5.0
                        align=center
                      text account_number
                        with
                          size=10.5
                          wrap=none
                          font=code_medium
                          @text-hint
                      // The separator belongs to the NUMBER, which a key outside
                      // every account does not have: `load_account` answers "" for
                      // every field until the key belongs to one, and drawn
                      // unconditionally the dot led the line — `· validator
                      // keypair on this device`. Every other separator in the
                      // console is gated by the run it introduces (forge.ice's
                      // repo-count dot says so in its own comment); this one was
                      // the exception.
                      if !empty(account_number)
                        text "·"
                          with
                            size=10.5
                            wrap=none
                            font=code_medium
                            @text-hint
                      // Same empty answer the badge above guards on: an unanswered
                      // roster has no standing to name here.
                      if !empty(member_tier(members_rows))
                        text member_tier(members_rows)
                          with
                            size=10.5
                            wrap=none
                            font=code_medium
                            @text-hint
                      text "keypair on this device"
                        with
                          size=10.5
                          wrap=none
                          font=code_medium
                          @text-hint
                  col gap=5.0
                    row
                      with
                        w=fill
                        h=28.0
                        gap=5.0
                        align=center
                      input "" #account-rename <-> account_name_draft
                        with
                          label="New display name"
                          change=emit(account_name_draft_changed, _)
                          hint="rename account…"
                          disabled=account_renaming
                          w=150.0
                          p=5.0
                          text-size=13.0
                          line-h=1.2
                          @control
                        active bg=elevated border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                        hovered bg=elevated border=fg/21
                        disabled bg=muted_bg/54 value=muted
                      button "Rename" -> emit(account_rename_submit)
                        with
                          disabled=(account_renaming || empty(trim(account_name_draft)))
                          h=28.0
                          p=5.0
                          @secondary_action
                    // NO ACCOUNT YET: this key founds one, or joins one another
                    // device minted a ticket for. Both are user-signed frames;
                    // both need the signing seat, so they wait on `password`.
                    if !account_exists
                      row
                        with
                          w=fill
                          h=28.0
                          gap=5.0
                          align=center
                        input "" #account-create <-> account_create_draft
                          with
                            label="Account name"
                            change=emit(account_create_draft_changed, _)
                            hint="name your account…"
                            disabled=account_busy
                            w=150.0
                            p=5.0
                            text-size=13.0
                            line-h=1.2
                            @control
                          active bg=elevated border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                          hovered bg=elevated border=fg/21
                          disabled bg=muted_bg/54 value=muted
                        button "Create account" -> emit(account_create_submit)
                          with
                            disabled=(account_busy || empty(password) || empty(trim(account_create_draft)))
                            h=28.0
                            p=5.0
                            @secondary_action
                      row
                        with
                          w=fill
                          h=28.0
                          gap=5.0
                          align=center
                        input "" #account-join <-> account_join_draft
                          with
                            label="Add-key ticket"
                            change=emit(account_join_draft_changed, _)
                            hint="paste a ticket from a member device…"
                            disabled=account_busy
                            w=150.0
                            p=5.0
                            text-size=13.0
                            line-h=1.2
                            @control
                          active bg=elevated border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                          hovered bg=elevated border=fg/21
                          disabled bg=muted_bg/54 value=muted
                        button "Join" -> emit(account_key_join_submit)
                          with
                            disabled=(account_busy || empty(password) || empty(trim(account_join_draft)))
                            h=28.0
                            p=5.0
                            @secondary_action
                      // …or a passkey registered on a member device consents in
                      // the browser — no ticket to carry across.
                      row
                        with
                          w=fill
                          h=28.0
                          gap=5.0
                          align=center
                        text "…or let a passkey from another device admit this one."
                          with
                            w=fill
                            size=12.0
                            @text-meta
                        button "Log in with a passkey" -> emit(account_login_submit)
                          with
                            disabled=(account_busy || empty(password))
                            h=28.0
                            p=5.0
                            @secondary_action
                    // ACCOUNT FACTS, ONLY WHEN THERE IS AN ACCOUNT. With the key
                    // in no account, `load_account` returns zeros for every field,
                    // and the card printed `0 keys` one line under "· validator
                    // keypair on this device" — a count of the account's keys
                    // read as a count of this device's, and the two contradicted
                    // each other in the same card. `account_exists` is the fact
                    // that tells them apart, and it was already in state gating
                    // Rename.
                    if account_exists
                      row gap=8.0 align=center
                        text account_keys
                          with
                            size=12.0
                            wrap=none
                            font=code
                            @text-meta
                        text "keys"
                          with
                            size=12.5
                            wrap=none
                            @text-meta
                        space w=fill
                        button "Copy number" -> emit(copy_to_clipboard, account_number, "Number copied")
                          with
                            disabled=empty(account_number)
                            h=28.0
                            p=7.0
                            @secondary_action
            // THE ACCOUNT'S KEYS — every device, wallet or passkey on it. A row
            // per association; `Remove` is member-gated by the module and
            // last-key-gated here (`account_keys <= 1`) so the card never offers
            // the one removal consensus refuses.
            if account_exists
              col w=fill gap=9.0
                GroupLabel label="ACCOUNT KEYS"
                GroupCard
                  col w=fill
                    for row in account_key_rows
                      box
                        with
                          w=fill
                          px=15.0
                          py=11.0
                        row
                          with
                            w=fill
                            gap=10.0
                            align=center
                          Badge.Outline label=row.scheme
                          col
                            with
                              w=fill
                              gap=2.0
                              clip=true
                            if !empty(row.label)
                              text row.label
                                with
                                  size=12.5
                                  wrap=none
                                  @text-fg
                            if empty(row.label)
                              text "(unlabeled)"
                                with
                                  size=12.5
                                  wrap=none
                                  @text-muted
                            text row.pubkey
                              with
                                size=10.5
                                wrap=none
                                font=code
                                @text-hint
                          button "Remove" -> emit(account_key_remove, row.pubkey)
                            with
                              disabled=(account_busy || empty(password) || account_keys <= 1)
                              h=26.0
                              p=5.0
                              @secondary_action
                    // ADD A DEVICE: paste the other device's key, mint the ticket
                    // it joins with (`ducktape account key add`, in the app).
                    box
                      with
                        w=fill
                        px=15.0
                        py=13.0
                      col w=fill gap=7.0
                        text "Add a device"
                          with
                            size=12.5
                            wrap=none
                            @text-accent_fg
                        row
                          with
                            w=fill
                            gap=7.0
                            align=center
                          input "" #account-key <-> account_key_draft
                            with
                              label="Other device's public key"
                              change=emit(account_key_draft_changed, _)
                              hint="paste its ed25519 key (hex)…"
                              disabled=account_busy
                              w=fill
                              p=5.0
                              text-size=12.5
                              line-h=1.2
                              @control
                            active bg=elevated border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                            hovered bg=elevated border=fg/21
                            disabled bg=muted_bg/54 value=muted
                          input "" #account-key-label <-> account_key_label_draft
                            with
                              label="Key label"
                              change=emit(account_key_label_draft_changed, _)
                              hint="label…"
                              disabled=account_busy
                              w=120.0
                              p=5.0
                              text-size=12.5
                              line-h=1.2
                              @control
                            active bg=elevated border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                            hovered bg=elevated border=fg/21
                            disabled bg=muted_bg/54 value=muted
                          button "Mint ticket" -> emit(account_key_add_submit)
                            with
                              disabled=(account_busy || empty(password) || empty(trim(account_key_draft)))
                              h=28.0
                              p=5.0
                              @secondary_action
                        // …or a passkey: from the phone (the card shows a QR),
                        // or in this computer's browser; or an Ethereum wallet
                        // in the browser. Each signs its own admission (the
                        // label above names it).
                        row
                          with
                            w=fill
                            gap=7.0
                            align=center
                          text "…or a passkey:"
                            with
                              w=fill
                              size=12.0
                              @text-meta
                          button "On your phone" #account-passkey-qr -> emit(account_passkey_submit)
                            with
                              disabled=(account_busy || empty(password))
                              h=28.0
                              p=5.0
                              @secondary_action
                          button "In this browser" -> emit(account_passkey_desktop)
                            with
                              disabled=(account_busy || empty(password))
                              h=28.0
                              p=5.0
                              @secondary_action
                          button "Link a wallet" -> emit(account_wallet_submit)
                            with
                              disabled=(account_busy || empty(password))
                              h=28.0
                              p=5.0
                              @secondary_action
                        CeremonyPlate #account-ceremony
                          with
                            phase=account_ceremony_phase
                            qr=account_ceremony_qr
                            detail=account_ceremony_detail
                            left=account_ceremony_left
                          forward
                            account_ceremony_cancel
                        if !empty(account_ticket)
                          row
                            with
                              w=fill
                              gap=7.0
                              align=center
                            text "Ticket minted — paste it on the other device."
                              with
                                w=fill
                                size=12.0
                                @text-meta
                            button "Copy ticket" -> emit(copy_to_clipboard, account_ticket, "Ticket copied")
                              with
                                h=26.0
                                p=5.0
                                @secondary_action
        SettingsPane.security
          col w=fill gap=18.0
            col w=fill gap=9.0
              GroupLabel label="IDENTITY KEY"
              GroupCard
                col w=fill
                  KeyValueRow
                    with
                      label="Key state"
                      value=settings_key_state
                      last=false
                  KeyValueRow
                    with
                      label="Key path"
                      value=settings_key_path
                      last=false
                  // The session's signing seat. Locked = no password held; the
                  // unlock VERIFIES against user.key (`user key unlock`) before
                  // anything is stored — the old CONNECTION field stored blind.
                  box
                    with
                      w=fill
                      px=15.0
                      py=13.0
                    col w=fill
                      if empty(password)
                        row
                          with
                            w=fill
                            gap=9.0
                            align=center
                          input "" #key-password <-> key_pw
                            with
                              label="Key password"
                              secure=true
                              hint="unlock signing…"
                              disabled=(mutation_phase != MutationPhase.idle)
                              submit=emit(settings_unlock_submit, key_pw)
                              w=fill
                              p=6.2
                              text-size=13.0
                              line-h=1.2
                              @control
                            active bg=muted_bg border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=9.0
                            hovered bg=elevated border=fg/21
                            disabled bg=muted_bg/54 value=muted
                          button "Unlock" -> emit(settings_unlock_submit, key_pw)
                            with
                              disabled=(mutation_phase != MutationPhase.idle || empty(key_pw))
                              h=28.0
                              p=6.0
                              @secondary_action
                      if !empty(password)
                        row
                          with
                            w=fill
                            gap=9.0
                            align=center
                          text "Signing unlocked for this session."
                            with
                              w=fill
                              size=12.5
                              @text-meta
                          button "Lock" -> emit(lock_session)
                            with
                              h=28.0
                              p=6.0
                              @secondary_action
        // A PLACE YOU GO. The forget is the one act here that cannot be taken
        // back from this screen, so it is not something a reader scrolls past
        // on the way to the theme switch.
        SettingsPane.danger
          col w=fill gap=18.0
            col w=fill gap=9.0
              // The one warmed eyebrow in the console: #c79a8a, not the #bdbbb1
              // every other group label wears.
              text "DANGER ZONE"
                with
                  size=9.0
                  wrap=none
                  font=code_semibold
                  @text-danger_label
              box
                with
                  w=fill
                  p=15.0
                  bg=danger_zone_bg
                  border=danger_zone_line
                  border-w=1.0
                  r=11.0
                row
                  with
                    w=fill
                    gap=13.0
                    align=center
                  col w=fill gap=2.0
                    text "Forget this network"
                      with
                        size=12.5
                        wrap=none
                        font=medium
                        @text-accent_fg
                    text "Drops this network from THIS DEVICE's list and returns to the network picker. A running node stays running, nothing on the network changes, and no key is destroyed."
                      with
                        size=10.5
                        @text-meta
                  button "Forget network" -> emit(forget_workspace_submit)
                    with
                      disabled=(!connected || mutation_phase != MutationPhase.idle)
                      h=32.0
                      p=8.0
                      @icon_action
                    active bg=danger_solid text=brand_fg border=danger_solid border-w=1.0 r=8.0
                    hovered bg=danger_solid_hover text=brand_fg border=danger_solid_hover
                    pressed bg=danger_solid_hover text=brand_fg
