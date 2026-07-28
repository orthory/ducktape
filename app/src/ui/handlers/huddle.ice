// The huddle plane's own handlers.
//
// TWO FACTS SHAPE THIS FILE:
//
// 1. POPPING IS VIEW STATE. `pop_huddle`/`dock_huddle` write one bool and touch
//    no chain — the same pure view toggles the artifact uses.
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

on pop_huddle
  huddle_popped = true

on dock_huddle
  huddle_popped = false

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
  huddle_popped = false
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
  error = ""
  run leave_huddle(connected_rpc, password, huddle_channel) -> chat_acked _ | mutation_failed _
