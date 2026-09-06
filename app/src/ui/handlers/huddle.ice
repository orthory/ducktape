// The huddle plane's own handlers.
//
// TWO FACTS SHAPE THIS FILE:
//
// 1. THE CALL IS A WINDOW, NOT A CARD. Joining opens the daemon's third
//    window and touches no chain; `huddle_win` holding an id IS "the window is
//    up". Closing it is not leaving — only `leave_huddle_here` ends a call.
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

// One call-session event: fold the status prose and the peer beacons.
//
// THE FAN-OUT SET IS NOT STEERED FROM HERE, and that is load-bearing. It used
// to be re-pushed on every event, which meant it moved only when a peer beacon
// arrived — and a beacon from a peer outside the set is dropped at our own
// demux, so the joiner who should have re-steered it was the one peer that
// could never reach us. The live session polls the huddle's on-chain roster
// itself (`crate::call::steer_recipients`); the roster in THIS state belongs
// to the channel on screen, which is not always the one being huddled in.
on call_event(event)
  call_status = call_status_after(call_status, event)
  call_muted = keep_bool(event.kind == "connecting", false, call_muted)
  call_camera = keep_bool(event.kind == "connecting", false, call_camera)
  call_sharing = keep_bool(event.kind == "connecting", false, call_sharing)
  call_peers = apply_call_peer(call_peers, event)
  huddle_rows = huddle_tile_rows(huddle_roster, call_peers, call_muted)
  call_video_live = call_video_live_after(call_peers, call_camera, call_sharing)
  huddle_stage = huddle_stage_peer(call_peers, call_sharing)

// The panel draws its button only while joined; the tray row is always there.
on toggle_call_mute
  return if !huddle_joined
  call_muted = call_set_muted(!call_muted)
  huddle_rows = huddle_tile_rows(huddle_roster, call_peers, call_muted)

// THE TWO VIDEO TOGGLES ARE ONE SOURCE. Each hands back both readings because
// starting either one ends the other — the camera and the shared screen are
// two sources for a participant's ONE video stream, and a beacon that claimed
// both would leave the far end guessing which picture it was looking at.
on toggle_call_camera
  let source = call_use_camera(!call_camera)
  call_camera = source.camera
  call_sharing = source.sharing
  call_video_live = call_video_live_after(call_peers, call_camera, call_sharing)
  huddle_stage = huddle_stage_peer(call_peers, call_sharing)

on toggle_call_screen
  let source = call_use_screen(!call_sharing)
  call_camera = source.camera
  call_sharing = source.sharing
  call_video_live = call_video_live_after(call_peers, call_camera, call_sharing)
  huddle_stage = huddle_stage_peer(call_peers, call_sharing)

// THE CALL IS A WINDOW, and this is the one way to put it in front of you:
// open it if it is gone, raise it if it is behind something. One discriminant,
// one branch — the same decision the status item's Open row makes, which is
// why they share `WindowSummon`.
//
// CLOSING THAT WINDOW IS NOT LEAVING. A call is a session and the window is a
// view of it, so the OS close button (and ⌘W) only clears `huddle_win` in
// `window_was_closed`; the media session stays subscribed on `huddle_joined`.
// The channel's LIVE pill and the tray's huddle row both route here, so a
// closed window is always one click from being back.
on show_huddle
  let summon = huddle_summon(huddle_win)
  match summon
    WindowSummon.open
      task window open huddle -> huddle_opened _
    WindowSummon.raise
      task window focus target=window_target(huddle_win)

on huddle_opened(id)
  huddle_win = some(id)

// A huddle that ends WITHOUT this window (another device leaves, the node
// drops the seat) closes it from the folds: every `huddle_joined` fold in
// handlers/chat.ice and lifecycle.ice ends with a
// `window_target_unless(huddle_joined, huddle_win)` close — a no-op while
// she is still in, the window's end the moment she is not.

// The panel's channel link puts the console on the huddle's channel.
// `choose_channel` owns the whole channel-switch reset; this hands it the
// huddle's channel through a native `Task::done` rather than copying twenty
// lines of that reset into a second place.
//
// The route in is cross-screen — the huddle window floats over
// Forge/Files/Settings — so this is a screen jump, not just a channel switch,
// and it sets `shell_tab` exactly like `open_chat_search_hit`. It also
// carries `choose_channel`'s OWN busy guard: on a loading or mid-mutation app
// that handler early-returns, and jumping tabs for a switch that will not
// happen is the worse half of the failure.
on huddle_go_channel
  return if loading || mutation_phase != MutationPhase.idle || empty(huddle_channel)
  shell_tab = ShellTab.chat
  // Same tab-move rule as `select_shell_tab`. `choose_channel` below retires it
  // again; the rule is uniform on purpose, so no reader has to prove the flow
  // reaches a second retire before trusting this one.
  flow
    from done huddle_channel
    done -> choose_channel _

// THE ONLY WAY OUT OF A HUDDLE. Leaving always leaves THE HUDDLE'S channel,
// which is not always the one on screen: the huddle window follows you onto
// every screen, and `active_channel` under it can be
// anything. In the channel-header pill the two coincide — that pill only draws
// when `huddle_channel == active_channel` — so every leave control in the app
// routes here and there is no second leave handler to keep in step.
on leave_huddle_here
  return if loading || mutation_phase != MutationPhase.idle || !huddle_joined || empty(huddle_channel)
  hydration_generation = hydration_generation + 1
  hydration_retry_attempt = 0
  mutation_phase = MutationPhase.huddle
  call_status = ""
  call_muted = false
  call_camera = false
  call_sharing = false
  call_video_live = false
  huddle_stage = ""
  call_peers = []
  // Keep the retained roster visible if the leave is refused. The peer and
  // local mute state above are already reset, so rebuild the same projection
  // every other writer of those sources rebuilds.
  huddle_rows = huddle_tile_rows(huddle_roster, call_peers, call_muted)
  error = ""
  // Leaving is the one thing that ends the huddle window: it has nothing left
  // to show. A window task is terminal, so the leave call rides beside it.
  parallel
    task window close target=window_target(huddle_win)
    run every leave_huddle(connected_rpc, password, huddle_channel) -> huddle_left _ | mutation_failed _

// THE LEAVE'S OWN ACK, and it has to be its own: the huddle used to end on the
// resync that follows, and a resync loads the channel ON SCREEN. `Leave` lives
// in the popped huddle window, which floats over every other room — so the
// resync that was supposed to notice she is gone was reading a conversation
// that never knew she was there. Now the write that took her off the roster is
// what takes the huddle off this device. `mutation_failed` still leaves
// everything standing, which is why nothing here is done optimistically.
on huddle_left(_result)
  huddle_joined = false
  huddle_roster = []
  huddle_rows = []
  huddle_channel = ""
  huddle_channel_name = ""
  huddle_joined_at = 0
  mutation_phase = MutationPhase.idle
  error = ""
