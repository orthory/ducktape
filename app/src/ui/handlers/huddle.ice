// The huddle plane's own handlers.
//
// TWO FACTS SHAPE THIS FILE:
//
// 1. POPPING IS A WINDOW, NOT A BOOL. `pop_huddle`/`dock_huddle` open and
//    close the daemon's third window and touch no chain; `huddle_win` holding
//    an id IS "the huddle is popped out".
// 2. THE ELAPSED CLOCK IS A LOCAL SESSION FACT. It is counted by a 1 Hz tick on
//    THIS machine into `huddle_now`, never derived from a module record's time:
//    a consensus write is stamped with the block HEIGHT on a validator network
//    (bin/noded/src/index.rs) and with unix MILLIS on a single-writer node, so
//    subtracting `huddle_joined_at` from a chain value would print nonsense.
//    The tick subscription and the handler that keeps `huddle_now` fresh belong
//    to handlers/lifecycle.ice — this app has exactly one `subscribe` block,
//    and the `huddle_joined_at = huddle_now` stamp belongs to the join ack in
//    handlers/chat.ice, the one place that knows THIS process saw the join.
//    A process that finds itself already on the roster — after a restart, or
//    because another device joined for this key — never observed a start
//    instant and has none to invent: the surfaces below take `elapsed` as a
//    STRING precisely so that case can pass "" and render LIVE with no clock.

// One call-session event: fold the status prose, the peer beacons, and — on
// every event — re-steer the fan-out set to the roster's current node keys
// (an idle no-op between roster changes, and exactly what the fresh session
// needs the moment it connects).
on call_event(event)
  call_status = call_status_after(call_status, event)
  call_muted = keep_bool(event.kind == "connecting", false, call_muted)
  call_camera = keep_bool(event.kind == "connecting", false, call_camera)
  call_peers = apply_call_peer(call_peers, event)
  call_video_live = call_video_live_after(call_peers, call_camera)
  call_steered = call_recipients(huddle_recipient_nodes(huddle_roster))

on toggle_call_mute
  call_muted = call_set_muted(!call_muted)

on toggle_call_camera
  call_camera = call_set_camera(!call_camera)
  call_video_live = call_video_live_after(call_peers, call_camera)

on video_tick
  call_frame_generation = latest_frame_generation()

// POPPING OPENS A REAL WINDOW, and the window's existence IS the popped
// state — there is no `huddle_popped` bool to keep in step with it. Docking
// closes it; so does the OS close button, which lands in `window_was_closed`
// (handlers/lifecycle.ice) and clears `huddle_win` there. The open-or-raise
// split lives in the VIEW: the LIVE pill's `popped` prop routes its click to
// `focus_huddle` while the window is up, so this guard is a belt, not a path.
on pop_huddle
  return if huddle_win != none
  task window open huddle -> huddle_opened _

on focus_huddle
  task window focus target=window_target(huddle_win)

on huddle_opened(id)
  huddle_win = some(id)

// A huddle that ends WITHOUT this window (another device leaves, the node
// drops the seat) closes it from the folds: every `huddle_joined` fold in
// handlers/chat.ice and lifecycle.ice ends with a
// `window_target_unless(huddle_joined, huddle_win)` close — a no-op while
// she is still in, the window's end the moment she is not.
on dock_huddle
  task window close target=window_target(huddle_win)

// The panel and the huddle's channel are the same conversation, so opening one
// docks the other. `choose_channel` owns the whole channel-switch reset; this
// hands it the huddle's channel through a native `Task::done` rather than
// copying twenty lines of that reset into a second place.
//
// BOTH routes into here are cross-screen — the docked pill rides every screen
// and the panel floats over Forge/Files/Settings — so this is a screen jump,
// not just a channel switch, and it sets `shell_tab` exactly like
// `open_chat_search_hit`. It also carries `choose_channel`'s OWN busy guard: on
// a loading or mid-mutation app that handler early-returns, and docking the
// panel for a switch that will not happen is the worse half of the failure.
on huddle_go_channel
  return if loading || mutation_phase != "idle" || empty(huddle_channel)
  shell_tab = "chat"
  flow
    from done huddle_channel
    done -> choose_channel _

// THE ONLY WAY OUT OF A HUDDLE. Leaving always leaves THE HUDDLE'S channel,
// which is not always the one on screen: the docked pill and the popped panel
// follow you onto every screen, and `active_channel` under them can be
// anything. In the channel-header pill the two coincide — that pill only draws
// when `huddle_channel == active_channel` — so every leave control in the app
// routes here and there is no second leave handler to keep in step.
on leave_huddle_here
  return if loading || mutation_phase != "idle" || !huddle_joined || empty(huddle_channel)
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = "huddle"
  call_status = ""
  call_muted = false
  call_camera = false
  call_video_live = false
  call_peers = []
  error = ""
  // Leaving is the one thing that ends the huddle window: it has nothing left
  // to show. A window task is terminal, so the leave call rides beside it.
  parallel
    task window close target=window_target(huddle_win)
    run leave_huddle(connected_rpc, password, huddle_channel) -> chat_acked _ | mutation_failed _
