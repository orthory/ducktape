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
//    to handlers/lifecycle.ice — this app has exactly one `subscribe` block.

on pop_huddle
  huddle_popped = true

on dock_huddle
  huddle_popped = false

// The panel and the huddle's channel are the same conversation, so opening one
// docks the other. `choose_channel` owns the whole channel-switch reset; this
// hands it the huddle's channel through a native `Task::done` rather than
// copying twenty lines of that reset into a second place.
on huddle_go_channel
  return if empty(huddle_channel)
  huddle_popped = false
  flow
    from done huddle_channel
    done -> choose_channel _

// Leaving always leaves THE HUDDLE'S channel, which is not always the one on
// screen: the docked pill and the popped panel follow you onto every screen,
// and `active_channel` under them can be anything. `leave_huddle_submit` in
// handlers/chat.ice leaves `active_channel`, which is right only for the
// channel-header ✕.
on leave_huddle_here
  return if loading || mutation_phase != "idle" || !huddle_joined || empty(huddle_channel)
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = "huddle"
  error = ""
  run leave_huddle(connected_rpc, password, huddle_channel) -> chat_acked _ | mutation_failed _
