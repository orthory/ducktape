// SETTINGS — the workspace's own facts and the two acts that change them
// (reconnect, rename), plus THIS NODE, which the rail has no seat for. See
// `screens/roster.ice` for the screen contract: no app state is reachable from
// here, so every reading is a prop and every act leaves as a named event that
// `view.ice` routes back to the handler of the same name.
//
// The `settings_*` / `node_*` / `account_*` / `members_*` prefixes are KEPT.
// They are not redundant with the screen name — this one surface carries four
// fact families, and the prefix is what says which loader a reading came from.
// `settings_height` (the facts reading) and `block_height` (the live head) are
// two different numbers and would collide as one word.
component SettingsScreen(account_name:str, connected_rpc:str, settings_endpoint:str, settings_node_key:str, settings_height:i64, settings_data_dir:str, settings_key_state:str, settings_key_path:str, settings_open_tabs:i64, members_rows:[MemberRow], members_validators:i64, members_residents:i64, account_id:str, bind account_name_draft:str, account_renaming:bool, account_members:i64, account_nodes:i64, appearance:str, bind rpc:str, bind password:str, status:str, loading:bool, connected:bool, mutation_phase:str, node_tab:str, module_rows:[ModuleRow], block_height:i64, node_checkpoint:i64, node_last_finalized:i64, node_reachable_label:str, node_quorum_label:str, node_version:str, node_root_hash:str, node_peers:[PeerRow], bind node_log_filter:str, node_log_lines:[NodeLogLine])
  emits
    select_shell_tab(str)
    reconnect()
    account_name_draft_changed(str)
    account_rename_submit()
    copy_to_clipboard(str, str)
    settings_clear_tabs()
    forget_workspace_submit()
    select_node_tab(str)
    open_node_modules()
    node_log_filter_changed(str)
    set_appearance_light()
    set_appearance_dark()
  scroll dir=vertical w=fill h=fill
    col w=fill p=22.0 gap=18.0
      text "Settings" size=16.0 wrap=none font=display @text-primary
      grid min-cell=420.0 gap=22.0
        col w=fill gap=9.0
          GroupLabel label="NETWORK"
          GroupCard
            col w=fill
              KeyValueRow label="Workspace" value=network_label(account_name, connected_rpc) last=false
              KeyValueRow label="Endpoint" value=settings_endpoint last=false
              KeyValueRow label="Node key" value=settings_node_key last=false
              KeyValueRow label="Block height" value=height_label(settings_height) last=false
              KeyValueRow label="Data directory" value=settings_data_dir last=false
              // The artifact's last NETWORK row: the roster reading with an
              // inline accent link onto the Members screen.
              box w=fill px=15.0 py=13.0
                row w=fill gap=10.0 align=center
                  text "Members" size=12.5 wrap=none @text-accent_fg
                  space w=fill
                  text members_summary(members_validators, members_residents) size=12.0 wrap=none font=code_medium @text-secondary_fg
                  button "manage" h=22.0 p=0.0 @ghost_action -> emit(select_shell_tab, "members")
                    active bg=transparent text=brand border=transparent border-w=1.0 r=6.0
                    hovered bg=elevated text=brand
                    pressed bg=subtle text=brand
        col w=fill gap=9.0
          GroupLabel label="CONNECTION"
          box w=fill p=15.0 bg=surface border=card_line border-w=1.0 r=11.0
            col w=fill gap=9.0
              input "" #rpc label="RPC endpoint" <-> rpc hint="Node URL" disabled=(loading || (mutation_phase != "idle" && mutation_phase != "recovering")) submit=emit(reconnect) w=fill p=6.2 text-size=13.0 line-h=1.2 @control
                active bg=muted_bg border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=9.0
                hovered bg=elevated border=fg/21
                disabled bg=muted_bg/54 value=muted
              input "" #password label="Local key password" secure=true <-> password hint="Key password" disabled=(loading || mutation_phase != "idle") w=fill p=6.2 text-size=13.0 line-h=1.2 @control
                active bg=muted_bg border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=9.0
                hovered bg=elevated border=fg/21
                disabled bg=muted_bg/54 value=muted
              row w=fill gap=9.0 align=center
                if connection_degraded(status)
                  Badge.Destructive label=status
                if !connection_degraded(status)
                  Badge.Success label=status
                space w=fill
                button "Connect" disabled=(loading || (mutation_phase != "idle" && mutation_phase != "recovering")) h=32.0 p=7.0 @primary_action -> emit(reconnect)
        col w=fill gap=9.0
          GroupLabel label="APPEARANCE"
          box w=fill p=15.0 bg=surface border=card_line border-w=1.0 r=11.0
            row w=fill gap=9.0 align=center
              col w=fill gap=3.0
                text "Theme" size=12.5 wrap=none @text-fg
                if empty(appearance)
                  text "Following the system appearance." size=11.0 @text-caption
                if !empty(appearance)
                  text "Pinned for this device." size=11.0 @text-caption
              space w=fill
              if appearance == "light"
                button "Light" h=28.0 p=6.0 @primary_action -> emit(set_appearance_light)
              if appearance != "light"
                button "Light" h=28.0 p=6.0 @secondary_action -> emit(set_appearance_light)
              if appearance == "dark"
                button "Dark" h=28.0 p=6.0 @primary_action -> emit(set_appearance_dark)
              if appearance != "dark"
                button "Dark" h=28.0 p=6.0 @secondary_action -> emit(set_appearance_dark)
        col w=fill gap=9.0
          GroupLabel label="YOUR IDENTITY"
          box w=fill p=15.0 bg=surface border=card_line border-w=1.0 r=11.0
            row w=fill gap=13.0 align=center
              PersonAvatar initials=initial_of(account_name) plate=40.0 ink=14.0
              // clip: the key line is four `wrap=none` runs over a 64-hex
              // key, so it cannot shrink — without this it paints over the
              // rename controls in the next column.
              col w=fill gap=3.0 clip=true
                row w=fill gap=7.0 align=center
                  if !empty(account_name)
                    text account_name size=13.5 wrap=none font=display @text-fg
                  if empty(account_name)
                    text "(unnamed)" size=13.5 wrap=none @text-muted
                  // The badge carries STANDING on this network — validator,
                  // resident, guest — not whether a local key is bound. The
                  // artifact's ADMIN/MAINTAINER words name authority this
                  // chain does not grant, so the app keeps its own.
                  if members_is_admin(members_rows)
                    Badge.Secondary label=member_tier(members_rows)
                  if !members_is_admin(members_rows)
                    Badge.Outline label=member_tier(members_rows)
                // The key line says WHICH keypair this is and that it lives
                // on this device — the custody clause the artifact carries.
                row w=fill gap=5.0 align=center
                  text account_id size=10.5 wrap=none font=code_medium @text-hint
                  text "·" size=10.5 wrap=none font=code_medium @text-hint
                  text member_tier(members_rows) size=10.5 wrap=none font=code_medium @text-hint
                  text "keypair on this device" size=10.5 wrap=none font=code_medium @text-hint
              col gap=5.0
                row w=fill h=28.0 gap=5.0 align=center
                  input "" #account-rename label="New display name" <-> account_name_draft change=emit(account_name_draft_changed, _) hint="rename account…" disabled=account_renaming w=150.0 p=5.0 text-size=13.0 line-h=1.2 @control
                    active bg=elevated border=fg/16 value=fg placeholder=muted selection=fg/18 border-w=1.0 r=7.0
                    hovered bg=elevated border=fg/21
                    disabled bg=muted_bg/54 value=muted
                  button "Rename" disabled=(account_renaming || empty(trim(account_name_draft))) h=28.0 p=5.0 @secondary_action -> emit(account_rename_submit)
                row gap=8.0 align=center
                  text account_members size=12.0 wrap=none font=code @text-meta
                  text "keys" size=12.5 wrap=none @text-meta
                  text account_nodes size=12.0 wrap=none font=code @text-meta
                  text "nodes" size=12.5 wrap=none @text-meta
                  space w=fill
                  button "Copy key" disabled=empty(account_id) h=28.0 p=7.0 @secondary_action -> emit(copy_to_clipboard, account_id, "Key copied")
        col w=fill gap=9.0
          GroupLabel label="IDENTITY KEY"
          GroupCard
            col w=fill
              KeyValueRow label="Key state" value=settings_key_state last=false
              KeyValueRow label="Key path" value=settings_key_path last=true
        // NO PREFERENCES GROUP. `Change receipts` was a placebo: every
        // finality mark in the app — FinalityChip, the chat tick, the
        // merge stamp — renders unconditionally, so the switch wrote a
        // value nothing read. It also painted ON from the state default
        // and flipped itself OFF a beat later, because the loader
        // answers `false` for an absent key. The group comes back the
        // day the marks are actually gated on it.
        col w=fill gap=9.0
          GroupLabel label="THIS DEVICE"
          box w=fill bg=surface border=card_line border-w=1.0 r=11.0 clip=true
            col w=fill
              box w=fill px=15.0 py=13.0
                row w=fill gap=10.0 align=center
                  col w=fill gap=1.0
                    text "Open page tabs" size=12.5 @text-accent_fg
                    text "Preferences persist per endpoint in app-prefs.json beside the user key." size=12.5 @text-meta
                  text settings_open_tabs size=12.0 wrap=none font=code_medium @text-secondary_fg
                  button "Forget tabs" h=28.0 p=5.0 @secondary_action -> emit(settings_clear_tabs)
        col w=fill gap=9.0
          // The one warmed eyebrow in the console: #c79a8a, not the #bdbbb1
          // every other group label wears.
          text "DANGER ZONE" size=9.0 wrap=none font=code_semibold @text-danger_label
          box w=fill p=15.0 bg=danger_zone_bg border=danger_zone_line border-w=1.0 r=11.0
            row w=fill gap=13.0 align=center
              col w=fill gap=2.0
                text "Leave this workspace" size=12.5 wrap=none font=medium @text-accent_fg
                text "Removes the workspace from THIS DEVICE and returns to onboarding. Nothing on the network changes and no key is destroyed." size=10.5 @text-meta
              button "Leave workspace" disabled=(!connected || mutation_phase != "idle") h=32.0 p=8.0 @icon_action -> emit(forget_workspace_submit)
                active bg=danger_solid text=brand_fg border=danger_solid border-w=1.0 r=8.0
                hovered bg=danger_solid_hover text=brand_fg border=danger_solid_hover
                pressed bg=danger_solid_hover text=brand_fg
      // ── THIS NODE ──────────────────────────────────────────────────────
      // The rail has eight seats and none of them is Node: the artifact puts
      // the node's own facts under Settings, reached from the rail footer.
      // This is that relocation, kept whole — Overview / Permissions /
      // Activity, with the log console under Activity.
      //
      // MODULES IS A TAB, NOT A SEAT. The artifact's own Modules screen
      // hangs off a ninth rail capsule; this rail has eight and the
      // campaign closed that question. The module set is a fact about
      // THIS NODE — which code consensus is executing here — so it sits
      // beside the node's other facts and takes no seat from anyone.
      box w=fill h=1.0 bg=separator
        space w=1.0 h=1.0
      col w=fill gap=13.0
        row w=fill gap=10.0 align=center
          text "This node" size=16.0 wrap=none font=display @text-primary
          StatusPill degraded=connection_degraded(status) loading=loading
          space w=fill
        row gap=3.0 align=center
          button label="Node overview" p=0.0 @ghost_action -> emit(select_node_tab, "overview")
            box px=15.0 py=0.0
              TabLabel label="Overview" count=0 active=(node_tab == "overview")
            active bg=transparent text=muted border=transparent border-w=1.0 r=8.0
            hovered bg=row_hover text=fg
            pressed bg=elevated text=fg
          button label="Node permissions" p=0.0 @ghost_action -> emit(select_node_tab, "permissions")
            box px=15.0 py=0.0
              TabLabel label="Permissions" count=0 active=(node_tab == "permissions")
            active bg=transparent text=muted border=transparent border-w=1.0 r=8.0
            hovered bg=row_hover text=fg
            pressed bg=elevated text=fg
          button label="Node activity" p=0.0 @ghost_action -> emit(select_node_tab, "activity")
            box px=15.0 py=0.0
              TabLabel label="Activity" count=0 active=(node_tab == "activity")
            active bg=transparent text=muted border=transparent border-w=1.0 r=8.0
            hovered bg=row_hover text=fg
            pressed bg=elevated text=fg
          button label="Node modules" p=0.0 @ghost_action -> emit(open_node_modules)
            box px=15.0 py=0.0
              TabLabel label="Modules" count=len(module_rows) active=(node_tab == "modules")
            active bg=transparent text=muted border=transparent border-w=1.0 r=8.0
            hovered bg=row_hover text=fg
            pressed bg=elevated text=fg
        match node_tab
          "modules"
            ModulesPanel rows=module_rows
          "permissions"
            col w=fill gap=18.0
              NodeAccessCard tier=member_tier(members_rows) admin=members_is_admin(members_rows)
              PermissionMatrix tier=member_tier(members_rows)
          "activity"
            col w=fill gap=9.0
              row w=fill gap=10.0 align=center
                GroupLabel label="LOG RING"
                space w=fill
                input "" #log-filter label="Filter logs" <-> node_log_filter change=emit(node_log_filter_changed, _) hint="filter logs…" w=200.0 p=6.2 text-size=13.0 line-h=1.2 @control
                  active bg=surface border=border value=fg placeholder=hint selection=fg/18 border-w=1.0 r=8.0
                  hovered bg=muted_bg border=control_line
              box w=fill h=420.0
                NodeLogConsole source=settings_endpoint
                  col w=fill gap=5.0
                    if empty(node_log_lines)
                      text "Waiting for the node's log ring…" size=12.0 wrap=none font=code @text-input
                    for line in filter_log_lines(node_log_lines, node_log_filter)
                      LogLine parts=split_log_line(line.line)
          _
            col w=fill gap=13.0
              GroupLabel label="NETWORK"
              // Three readings this node can actually prove. The artifact's
              // FINALITY (ms) and ROUND cards are omitted: /v1/status
              // publishes neither, and `view`/`quorum` are absent on a
              // non-validator rather than filled with a misleading zero.
              grid min-cell=170.0 gap=10.0
                StatCard label="HEIGHT" value=height_label_short(block_height) note=""
                StatCard label="CHECKPOINT" value=height_label_short(node_checkpoint) note=""
                StatCard label="LAST FINALIZED" value=relative_time(node_last_finalized) note=""
              if members_is_admin(members_rows)
                grid min-cell=170.0 gap=10.0
                  StatCard label="VALIDATORS REACHED" value=reading_pair(node_reachable_label, node_quorum_label) note="of quorum"
              GroupCard
                col w=fill
                  NodeBuildRow version=node_version last=false
                  KeyValueRow label="App hash" value=node_root_hash last=true
              if !empty(node_peers)
                col w=fill gap=9.0
                  GroupLabel label="PEERS"
                  GroupCard
                    col w=fill
                      for peer in node_peers
                        box w=fill px=15.0 py=11.0
                          row w=fill gap=8.0 align=center
                            if peer.live
                              Dot plate=7.0
                            if !peer.live
                              box w=7.0 h=7.0 bg=presence_off r=3.5
                                space w=1.0 h=1.0
                            text peer.key w=fill size=12.0 wrap=none font=code @text-fg
                            text peer.height size=12.0 wrap=none font=code @text-muted
