// NODE — the operator surface for this daemon: coherent status, standing,
// peers, logs and the code registry. It is a rail destination, not a Settings
// appendix; Settings links here for readers who start from workspace details.
//
// The live `block_height` register is deliberately NOT read here. It is the
// titlebar's liveness reading and carries no checkpoint. This screen's head and
// checkpoint come from one `NodeFacts` document so they can never describe two
// different instants.
component NodeScreen(node_key:str, node_data_dir:str, members_rows:[MemberRow], status:str, loading:bool, node_tab:str, module_rows:[ModuleRow], node_height:i64, node_checkpoint:i64, node_last_finalized:i64, node_reachable_label:str, node_quorum_label:str, node_version:str, node_root_hash:str, sync_line:str, node_phase_since:i64, node_sync_retries:i64, node_sync_failures:i64, node_sync_last_error:str, node_peers:[PeerRow], bind node_log_filter:str, wall_now:i64)
  emits
    select_node_tab(str)
    open_node_modules()
    node_log_filter_changed(str)
    copy_to_clipboard(str, str)
  scroll #node-body
    with
      dir=vertical
      w=fill
      h=fill
    col
      with
        w=fill
        p=22.0
        gap=18.0
      col w=fill gap=13.0
        row
          with
            w=fill
            gap=10.0
            align=center
          text "This node"
            with
              size=16.0
              wrap=none
              font=display
              @text-primary
          StatusPill degraded=connection_degraded(status) loading=loading
          space w=fill
        row gap=3.0 align=center
          button #node-overview-tab -> emit(select_node_tab, "overview")
            with
              label="Node overview"
              checked=(node_tab == "overview")
              p=0.0
              @ghost_action
            box px=15.0 py=0.0
              TabLabel
                with
                  label="Overview"
                  count=0
                  active=(node_tab == "overview")
            active bg=transparent text=muted border=transparent border-w=1.0 r=8.0
            hovered bg=row_hover text=fg
            pressed bg=elevated text=fg
          button #node-permissions-tab -> emit(select_node_tab, "permissions")
            with
              label="Node permissions"
              checked=(node_tab == "permissions")
              p=0.0
              @ghost_action
            box px=15.0 py=0.0
              TabLabel
                with
                  label="Permissions"
                  count=0
                  active=(node_tab == "permissions")
            active bg=transparent text=muted border=transparent border-w=1.0 r=8.0
            hovered bg=row_hover text=fg
            pressed bg=elevated text=fg
          button #node-activity-tab -> emit(select_node_tab, "activity")
            with
              label="Node activity"
              checked=(node_tab == "activity")
              p=0.0
              @ghost_action
            box px=15.0 py=0.0
              TabLabel
                with
                  label="Activity"
                  count=0
                  active=(node_tab == "activity")
            active bg=transparent text=muted border=transparent border-w=1.0 r=8.0
            hovered bg=row_hover text=fg
            pressed bg=elevated text=fg
          button #node-modules-tab -> emit(open_node_modules)
            with
              label="Node modules"
              checked=(node_tab == "modules")
              p=0.0
              @ghost_action
            box px=15.0 py=0.0
              TabLabel
                with
                  label="Modules"
                  count=len(module_rows)
                  active=(node_tab == "modules")
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
            LogTimeline.Frame
              with
                title="Log ring"
                description="Live node events retained in the in-memory ring."
              col w=fill gap=9.0
                row
                  with
                    w=fill
                    align=end
                  input "" #log-filter <-> node_log_filter
                    with
                      label="Filter logs"
                      change=emit(node_log_filter_changed, _)
                      hint="filter logs…"
                      w=200.0
                      p=6.2
                      text-size=13.0
                      line-h=1.2
                      @control
                    active bg=surface value=fg placeholder=hint selection=fg/18 border-w=1.0 r=8.0
                    hovered bg=muted_bg border=control_line
                box w=fill h=420.0
                  slot activity_log
          _
            col w=fill gap=13.0
              GroupLabel label="NODE"
              MemberFactRow label="public key" value=keep_str(!empty(node_key), node_key, "—")
              MemberFactRow label="data directory" value=keep_str(!empty(node_data_dir), node_data_dir, "—")
              button "Copy node key" -> emit(copy_to_clipboard, node_key, "Node key copied")
                with
                  disabled=empty(node_key)
                  h=28.0
                  p=7.0
                  @secondary_action
              GroupLabel label="NETWORK"
              // Three readings this node can actually prove. The artifact's
              // FINALITY (ms) and ROUND cards are omitted: /v1/status
              // publishes neither, and `view`/`quorum` are absent on a
              // non-validator rather than filled with a misleading zero.
              grid min-cell=170.0 gap=10.0
                StatCard
                  with
                    label="HEIGHT"
                    value=height_label_short(node_height)
                    note=""
                StatCard
                  with
                    label="CHECKPOINT"
                    value=height_label_short(node_checkpoint)
                    note=""
                StatCard
                  with
                    label="LAST FINALIZED"
                    value=relative_time(node_last_finalized, wall_now)
                    note=""
              if members_is_admin(members_rows)
                grid min-cell=170.0 gap=10.0
                  StatCard
                    with
                      label="VALIDATORS REACHED"
                      value=reading_pair(node_reachable_label, node_quorum_label)
                      note="of quorum"
              GroupCard
                col w=fill
                  NodeBuildRow version=node_version last=false
                  KeyValueRow
                    with
                      label="Phase"
                      value=reading_pair(sync_line, relative_time(node_phase_since, wall_now))
                      last=false
                  // CUMULATIVE, and labelled so. These two only ever climb —
                  // nothing in the node resets them — so a nonzero total is
                  // history, not a fault happening now. The row above says
                  // what is happening now.
                  KeyValueRow
                    with
                      label="Sync retries / failures, cumulative"
                      value=reading_pair(count_label(node_sync_retries), count_label(node_sync_failures))
                      last=empty(node_sync_last_error)
                  // The error SELF-CLEARS on the node the moment sync advances,
                  // so its presence is a fact about now: the last attempt
                  // failed and nothing has moved since.
                  if !empty(node_sync_last_error)
                    KeyValueRow
                      with
                        label="Last sync error"
                        value=node_sync_last_error
                        last=false
                  KeyValueRow
                    with
                      label="App hash"
                      value=node_root_hash
                      last=true
              if !empty(node_peers)
                col w=fill gap=9.0
                  GroupLabel label="PEERS"
                  GroupCard
                    col w=fill
                      for peer in node_peers
                        box
                          with
                            w=fill
                            px=15.0
                            py=11.0
                          row
                            with
                              w=fill
                              gap=8.0
                              align=center
                            if peer.live
                              Dot plate=7.0
                            if !peer.live
                              box
                                with
                                  w=7.0
                                  h=7.0
                                  bg=presence_off
                                  r=3.5
                                space w=1.0 h=1.0
                            text peer.key
                              with
                                w=fill
                                size=12.0
                                wrap=none
                                font=code
                                @text-fg
                            text peer.role
                              with
                                size=12.0
                                wrap=none
                                font=code
                                @text-muted
