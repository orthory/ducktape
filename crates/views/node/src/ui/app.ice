// NODE, as a module-owned view: the operator surface for this daemon —
// coherent status, standing, peers, logs and the code registry — drawn from
// the one facts document the desktop app pushes. The screen body is the
// app's own (screens/node.ice before the port), with the two StatCard grids
// as rows; the live log ring is a host surface in the Activity slot.
app NodeView
  title "Node"
  palette active_palette
  id "dev.ducktape.view.node"
  text-size 13.5

use "../../../../../app/src/ui/theme.ice"
use "../../../../../app/src/ui/ducktape-ui/recipes.ice"
use "log-timeline.ice"
use "../../../../../app/src/ui/components/icon.ice"
use "node.ice"
use "kit.ice"

enum NodeTab
  overview
  permissions
  activity
  modules

extern crate::host
  HostError(message:str)
  PeerRow(key:str, role:str, live:bool)
  ModuleRow(id:str, category:str, root:str, code_hash:str, pending_hash:str, activation_height:i64, readiness:i64, ready:bool)
  NodeProps(node_key:str, node_data_dir:str, tier:str, admin:bool, status:str, loading:bool, module_rows:[ModuleRow], node_height:i64, node_checkpoint:i64, node_last_finalized:i64, node_reachable_label:str, node_quorum_label:str, node_version:str, node_root_hash:str, sync_line:str, node_phase_since:i64, node_sync_retries:i64, node_sync_failures:i64, node_sync_last_error:str, node_peers:[PeerRow], wall_now:i64, connected:bool, dark:bool)
  stream props() -> NodeProps ! HostError
  pure copy(text:&str, label:&str) -> bool
  pure show_tab(tab:NodeTab) -> bool
  pure log_filter(filter:&str) -> bool
  pure icon(name:&str) -> bytes
  pure connection_degraded(status:&str) -> bool
  pure reading_pair(left:&str, right:&str) -> str
  pure count_label(count:i64) -> str
  pure keep_str(loaded:bool, next:&str, current:&str) -> str
  pure initial_of(name:&str) -> str
  pure height_label_short(height:i64) -> str
  pure relative_time(unix_seconds:i64, wall_now:i64) -> str
  // The live log ring: the host draws its own retained timeline here.
  component node_log_timeline() -> unit

state
  active_palette:palette[AppTheme] = AppTheme.app
  node_key = ""
  node_data_dir = ""
  tier = ""
  admin = false
  status = ""
  loading = false
  node_tab:NodeTab = NodeTab.overview
  module_rows:[ModuleRow] = []
  node_height:i64 = -1
  node_checkpoint:i64 = -1
  node_last_finalized:i64 = -1
  node_reachable_label = "—"
  node_quorum_label = "—"
  node_version = ""
  node_root_hash = ""
  sync_line = ""
  node_phase_since:i64 = -1
  node_sync_retries:i64 = 0
  node_sync_failures:i64 = 0
  node_sync_last_error = ""
  node_peers:[PeerRow] = []
  node_log_filter = ""
  wall_now:i64 = 0
  connected = false
  host_error = ""
  // a write's acknowledgement — `host::notify` returns nothing to bind
  sent = false

on mount
  stream every props() -> props_changed _ | props_failed _

on props_changed(next)
  node_key = next.node_key
  node_data_dir = next.node_data_dir
  tier = next.tier
  admin = next.admin
  status = next.status
  loading = next.loading
  module_rows = next.module_rows
  node_height = next.node_height
  node_checkpoint = next.node_checkpoint
  node_last_finalized = next.node_last_finalized
  node_reachable_label = next.node_reachable_label
  node_quorum_label = next.node_quorum_label
  node_version = next.node_version
  node_root_hash = next.node_root_hash
  sync_line = next.sync_line
  node_phase_since = next.node_phase_since
  node_sync_retries = next.node_sync_retries
  node_sync_failures = next.node_sync_failures
  node_sync_last_error = next.node_sync_last_error
  node_peers = next.node_peers
  wall_now = next.wall_now
  connected = next.connected
  active_palette = AppTheme.app
  return if !next.dark
  active_palette = AppTheme.app_dark

on props_failed(error)
  host_error = error.message

on select_node_tab(next)
  node_tab = next
  sent = show_tab(next)

on open_node_modules
  node_tab = NodeTab.modules
  sent = show_tab(NodeTab.modules)

on node_log_filter_changed(next)
  node_log_filter = next
  sent = log_filter(next)

on copy_to_clipboard(text, label)
  sent = copy(text, label)

view
  box #root
    with
      w=fill
      h=fill
      bg=bg
    col w=fill h=fill
      if !connected
        col
          with
            w=fill
            h=fill
            align=center
          space h=fill
          text "Not connected" size=13.0 @text-muted
          space h=fill
      if connected
        NodeScreen wall_now=wall_now node_log_filter<->node_log_filter #node
          with
            node_key
            node_data_dir
            tier
            admin
            status
            loading
            node_tab
            module_rows
            node_height
            node_checkpoint
            node_last_finalized
            node_reachable_label
            node_quorum_label
            node_version
            node_root_hash
            sync_line
            node_phase_since
            node_sync_retries
            node_sync_failures
            node_sync_last_error
            node_peers
          events
            select_node_tab -> select_node_tab _
            open_node_modules -> open_node_modules
            node_log_filter_changed -> node_log_filter_changed _
            copy_to_clipboard -> copy_to_clipboard _ _
          activity_log:
            extern node_log_timeline() #node-log-timeline

component NodeScreen(node_key:str, node_data_dir:str, tier:str, admin:bool, status:str, loading:bool, node_tab:NodeTab, module_rows:[ModuleRow], node_height:i64, node_checkpoint:i64, node_last_finalized:i64, node_reachable_label:str, node_quorum_label:str, node_version:str, node_root_hash:str, sync_line:str, node_phase_since:i64, node_sync_retries:i64, node_sync_failures:i64, node_sync_last_error:str, node_peers:[PeerRow], bind node_log_filter:str, wall_now:i64)
  emits
    select_node_tab(NodeTab)
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
          button #node-overview-tab -> emit(select_node_tab, NodeTab.overview)
            with
              label="Node overview"
              checked=(node_tab == NodeTab.overview)
              p=0.0
              @ghost_action
            box px=15.0 py=0.0
              TabLabel
                with
                  label="Overview"
                  count=0
                  active=(node_tab == NodeTab.overview)
            active bg=transparent text=muted border=transparent border-w=1.0 r=8.0
            hovered bg=row_hover text=fg
            pressed bg=elevated text=fg
          button #node-permissions-tab -> emit(select_node_tab, NodeTab.permissions)
            with
              label="Node permissions"
              checked=(node_tab == NodeTab.permissions)
              p=0.0
              @ghost_action
            box px=15.0 py=0.0
              TabLabel
                with
                  label="Permissions"
                  count=0
                  active=(node_tab == NodeTab.permissions)
            active bg=transparent text=muted border=transparent border-w=1.0 r=8.0
            hovered bg=row_hover text=fg
            pressed bg=elevated text=fg
          button #node-activity-tab -> emit(select_node_tab, NodeTab.activity)
            with
              label="Node activity"
              checked=(node_tab == NodeTab.activity)
              p=0.0
              @ghost_action
            box px=15.0 py=0.0
              TabLabel
                with
                  label="Activity"
                  count=0
                  active=(node_tab == NodeTab.activity)
            active bg=transparent text=muted border=transparent border-w=1.0 r=8.0
            hovered bg=row_hover text=fg
            pressed bg=elevated text=fg
          button #node-modules-tab -> emit(open_node_modules)
            with
              label="Node modules"
              checked=(node_tab == NodeTab.modules)
              p=0.0
              @ghost_action
            box px=15.0 py=0.0
              TabLabel
                with
                  label="Modules"
                  count=len(module_rows)
                  active=(node_tab == NodeTab.modules)
            active bg=transparent text=muted border=transparent border-w=1.0 r=8.0
            hovered bg=row_hover text=fg
            pressed bg=elevated text=fg
        match node_tab
          NodeTab.modules
            ModulesPanel rows=module_rows
          NodeTab.permissions
            col w=fill gap=18.0
              NodeAccessCard tier=tier admin=admin
              PermissionMatrix tier=tier
          NodeTab.activity
            LogTimeline.Frame
              with
                title="Log ring"
                description="Live node events retained in the in-memory ring."
              col w=fill gap=9.0
                row w=fill align=end
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
          NodeTab.overview
            col w=fill gap=13.0
              GroupLabel label="NODE"
              MemberFactRow label="public key" value=keep_str(!empty(node_key), node_key, "—")
              MemberFactRow
                with
                  label="data directory"
                  value=keep_str(!empty(node_data_dir), node_data_dir, "—")
              button "Copy node key" -> emit(copy_to_clipboard, node_key, "Node key copied")
                with
                  disabled=empty(node_key)
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
              if admin
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
