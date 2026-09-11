// NODE, as a module-owned view on the KERNEL CONTRACT: the operator surface
// for this daemon — coherent status, standing, peers, logs and the code
// registry. The kernel pushes SESSION FACTS ONLY (`session()` — connected,
// dark, this seat's admin standing and tier, the app's connection reading,
// the workspace directory and the wall clock, the things no `/v1` route
// publishes). The node's own facts, its peers, its code registry and its
// LOG RING are read HERE through `rpc.status`, `rpc.peers`, `rpc.query` and
// `rpc.stream`, re-read on every `rpc.live` hit for the `block` plane; the
// live tracing filter leaves as one `rpc.admin` POST the kernel signs. The
// clipboard is the one intent left, because it is an OS door.
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
  LogRow(cursor:str, time:str, level:str, message:str)
  NodeFacts(node_key:str, node_height:i64, node_checkpoint:i64, node_last_finalized:i64, node_reachable_label:str, node_quorum_label:str, node_version:str, node_root_hash:str, sync_line:str, node_phase_since:i64, node_sync_retries:i64, node_sync_failures:i64, node_sync_last_error:str)
  Session(connected:bool, dark:bool, admin:bool, tier:str, status:str, data_dir:str, wall_now:i64)
  SessionItem(next:Session, error:str)
  FactsItem(facts:NodeFacts, error:str)
  PeersItem(rows:[PeerRow], error:str)
  ModulesItem(rows:[ModuleRow], error:str)
  LogItem(lines:[LogRow], error:str)
  ActItem(reply:str, error:str)
  subscription session() -> SessionItem
  // the node's own facts, read by this view: once per connection, then
  // again on every block
  subscription facts(connection:i64) -> FactsItem
  // the mesh sample and the code registry, on the same cadence — each held
  // only while the tab that draws it is open, because every peers sample
  // encodes the node's whole metrics registry
  subscription peers(connection:i64) -> PeersItem
  subscription modules(connection:i64) -> ModulesItem
  // the node's own log ring, off `rpc.stream`: one item per batch of frames
  subscription logs(connection:i64) -> LogItem
  // the live-filter write's outcome, as the kernel answers it
  subscription acts() -> ActItem
  pure connection_serial_after(was_connected:bool, connected:bool, serial:i64) -> i64
  pure empty_facts() -> NodeFacts
  pure push_logs(lines:&[LogRow], arrived:&[LogRow]) -> [LogRow]
  pure visible_log(lines:&[LogRow], filter:&str) -> [LogRow]
  pure log_note(held:i64, shown:i64) -> str
  // the one write this view signs through the kernel
  sync set_log_filter(filter:&str) -> bool
  pure copy(text:&str, label:&str) -> bool
  pure icon(name:&str) -> bytes
  pure connection_degraded(status:&str) -> bool
  pure reading_pair(left:&str, right:&str) -> str
  pure count_label(count:i64) -> str
  pure keep_str(loaded:bool, next:&str, current:&str) -> str
  pure initial_of(name:&str) -> str
  pure height_label_short(height:i64) -> str
  pure relative_time(unix_seconds:i64, wall_now:i64) -> str

state
  active_palette:palette[AppTheme] = AppTheme.app
  // the session, as the kernel pushes it
  node_data_dir = ""
  tier = ""
  admin = false
  status = ""
  wall_now:i64 = 0
  connected = false
  // moves when the session comes up: every reading is read afresh
  connection_serial:i64 = 0
  node_tab:NodeTab = NodeTab.overview
  // this view's own readings
  facts:NodeFacts = empty_facts()
  loading = true
  module_rows:[ModuleRow] = []
  node_peers:[PeerRow] = []
  // the node's log ring as this view holds it, and the substring the
  // console draws it through
  log_lines:[LogRow] = []
  node_log_filter = ""
  // the RUNNING node's tracing filter: the draft, and what the node said
  live_log_filter = ""
  live_filter_note = ""
  host_error = ""
  // a write's acknowledgement — `host::notify` returns nothing to bind
  sent = false

// Subscriptions, not mount tasks, so a replacement restored from this view's
// state asks for the session and its readings again on its own. The peers
// sample and the registry are held only while their tab draws them: leaving
// the tab stops the node's encode at the source.
subscribe
  session() -> session_arrived _
  facts(connection_serial) when connected -> facts_arrived _
  peers(connection_serial) when (connected && node_tab == NodeTab.overview) -> peers_arrived _
  modules(connection_serial) when (connected && node_tab == NodeTab.modules) -> modules_arrived _
  logs(connection_serial) when (connected && node_tab == NodeTab.activity) -> logs_arrived _
  acts() -> act_done _

// THE SESSION: what the kernel knows and this view cannot — whether there is
// a node, the colour mode, this seat's standing, the app's connection
// reading, the daemon's workspace directory and the clock.
on session_arrived(item)
  host_error = item.error
  return if !empty(item.error)
  let next = item.next
  connection_serial = connection_serial_after(connected, next.connected, connection_serial)
  connected = next.connected
  admin = next.admin
  tier = next.tier
  status = next.status
  node_data_dir = next.data_dir
  wall_now = next.wall_now
  active_palette = AppTheme.app
  return if !next.dark
  active_palette = AppTheme.app_dark

on facts_arrived(item)
  host_error = item.error
  loading = false
  return if !empty(item.error)
  facts = item.facts

on peers_arrived(item)
  host_error = item.error
  return if !empty(item.error)
  node_peers = item.rows

on modules_arrived(item)
  host_error = item.error
  return if !empty(item.error)
  module_rows = item.rows

// One batch of the ring's frames, already split into columns.
on logs_arrived(item)
  host_error = item.error
  log_lines = push_logs(log_lines, item.lines)

// The live-filter write's answer: the node's own reply, or its refusal.
on act_done(item)
  host_error = item.error
  live_filter_note = keep_str(empty(item.error), item.reply, item.error)

on select_node_tab(next)
  node_tab = next

on open_node_modules
  node_tab = NodeTab.modules

// The console's own substring filter over the ring this view holds.
on node_log_filter_changed(next)
  node_log_filter = next

on live_log_filter_changed(next)
  live_log_filter = next

// RETUNE THE RUNNING NODE. The route mutates the process, so the node admits
// only its operator; the kernel signs with the seated key and the answer
// arrives on `acts()`.
on apply_live_log_filter
  return if !admin || empty(live_log_filter)
  live_filter_note = ""
  sent = set_log_filter(live_log_filter)

on copy_to_clipboard(text, label)
  sent = copy(text, label)

view
  box #root
    with
      w=fill
      h=fill
      bg=bg
    col w=fill h=fill
      // A read this view could not make is said in place, above the screen
      // it belongs to — the app has no door left for it to come back through.
      if !empty(host_error)
        box w=fill px=22.0 pt=13.0
          text host_error #host-error size=12.0 @text-danger
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
        NodeScreen wall_now=wall_now node_log_filter<->node_log_filter live_log_filter<->live_log_filter #node
          with
            facts
            node_data_dir
            tier
            admin
            status
            loading
            node_tab
            module_rows
            node_peers
            log_lines
            live_filter_note
          events
            select_node_tab -> select_node_tab _
            open_node_modules -> open_node_modules
            node_log_filter_changed -> node_log_filter_changed _
            live_log_filter_changed -> live_log_filter_changed _
            apply_live_log_filter -> apply_live_log_filter
            copy_to_clipboard -> copy_to_clipboard _ _

component NodeScreen(facts:NodeFacts, node_data_dir:str, tier:str, admin:bool, status:str, loading:bool, node_tab:NodeTab, module_rows:[ModuleRow], node_peers:[PeerRow], log_lines:[LogRow], live_filter_note:str, bind node_log_filter:str, bind live_log_filter:str, wall_now:i64)
  emits
    select_node_tab(NodeTab)
    open_node_modules()
    node_log_filter_changed(str)
    live_log_filter_changed(str)
    apply_live_log_filter()
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
                row w=fill gap=9.0 align=end
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
                  space w=fill
                  // RETUNE THE RUNNING NODE — the `ducktape node log-filter`
                  // verb, on the screen the operator is already reading. The
                  // route mutates the process, so only this node's operator
                  // is admitted; the card is offered to nobody else.
                  if admin
                    input "" #live-log-filter <-> live_log_filter
                      with
                        label="Live tracing filter"
                        change=emit(live_log_filter_changed, _)
                        submit=emit(apply_live_log_filter)
                        hint="info,ducktape::join=debug"
                        w=260.0
                        p=6.2
                        text-size=13.0
                        line-h=1.2
                        @control
                      active bg=surface value=fg placeholder=hint selection=fg/18 border-w=1.0 r=8.0
                      hovered bg=muted_bg border=control_line
                  if admin
                    button "Retune" -> emit(apply_live_log_filter)
                      with
                        disabled=empty(live_log_filter)
                        p=7.0
                        @secondary_action
                if !empty(live_filter_note)
                  text live_filter_note size=12.0 @text-muted
                LogConsole
                  with
                    lines=visible_log(log_lines, node_log_filter)
                    note=log_note(len(log_lines), len(visible_log(log_lines, node_log_filter)))
          NodeTab.overview
            col w=fill gap=13.0
              GroupLabel label="NODE"
              MemberFactRow label="public key" value=keep_str(!empty(facts.node_key), facts.node_key, "—")
              MemberFactRow
                with
                  label="data directory"
                  value=keep_str(!empty(node_data_dir), node_data_dir, "—")
              button "Copy node key" -> emit(copy_to_clipboard, facts.node_key, "Node key copied")
                with
                  disabled=empty(facts.node_key)
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
                    value=height_label_short(facts.node_height)
                    note=""
                StatCard
                  with
                    label="CHECKPOINT"
                    value=height_label_short(facts.node_checkpoint)
                    note=""
                StatCard
                  with
                    label="LAST FINALIZED"
                    value=relative_time(facts.node_last_finalized, wall_now)
                    note=""
              if admin
                grid min-cell=170.0 gap=10.0
                  StatCard
                    with
                      label="VALIDATORS REACHED"
                      value=reading_pair(facts.node_reachable_label, facts.node_quorum_label)
                      note="of quorum"
              GroupCard
                col w=fill
                  NodeBuildRow version=facts.node_version last=false
                  KeyValueRow
                    with
                      label="Phase"
                      value=reading_pair(facts.sync_line, relative_time(facts.node_phase_since, wall_now))
                      last=false
                  // CUMULATIVE, and labelled so. These two only ever climb —
                  // nothing in the node resets them — so a nonzero total is
                  // history, not a fault happening now. The row above says
                  // what is happening now.
                  KeyValueRow
                    with
                      label="Sync retries / failures, cumulative"
                      value=reading_pair(count_label(facts.node_sync_retries), count_label(facts.node_sync_failures))
                      last=empty(facts.node_sync_last_error)
                  // The error SELF-CLEARS on the node the moment sync advances,
                  // so its presence is a fact about now: the last attempt
                  // failed and nothing has moved since.
                  if !empty(facts.node_sync_last_error)
                    KeyValueRow
                      with
                        label="Last sync error"
                        value=facts.node_sync_last_error
                        last=false
                  KeyValueRow
                    with
                      label="App hash"
                      value=facts.node_root_hash
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
