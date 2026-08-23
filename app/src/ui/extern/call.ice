// The huddle call boundary — `crate::call` is the `/v1/call/ws` client: mic
// capture in, mixed playout out, control as json text. The SESSION IS THE
// SUBSCRIPTION: lifecycle.ice runs `call_session` while `huddle_joined`
// holds, so joining starts media and leaving tears it down by dropping the
// stream — there is no imperative stop.
//
// `call_set_muted` reaches the running session through the module's parked
// handles and is an honest no-op when none runs. The fan-out set is NOT an
// extern: the session polls the huddle's on-chain roster itself, because the
// peer whose arrival should push it is the one peer admission won't let
// through until it moves (see `crate::backend::huddle_fanout_nodes`).
extern crate::call
  CallEvent(kind:str, message:str, peer:str, muted:bool, camera_on:bool, sharing:bool)
  stream call_session(rpc:str, channel_id:str) -> CallEvent
  sync call_set_muted(muted:bool) -> bool
  pure call_status_after(current:str, event:CallEvent) -> str
  pure apply_call_peer(peers:[CallEvent], event:CallEvent) -> [CallEvent]
  HuddleTileRow(person:HuddleParticipant, muted:bool)
  pure huddle_tile_rows(roster:[HuddleParticipant], peers:[CallEvent], local_muted:bool) -> [HuddleTileRow]
  pure call_video_live_after(peers:[CallEvent], camera:bool, sharing:bool) -> bool
  pure huddle_stage_peer(peers:[CallEvent], local_sharing:bool) -> str

// The video leg — `crate::video`: capture/encode on its own thread, decoded
// peer frames in a store the surfaces read. Both surfaces are SELF-REDRAWING
// widgets: they repaint their own window at the capture cadence via per-window
// redraw requests — no tick, no state, no app rebuild.
//
// ONE SOURCE, TWO TOGGLES. The camera and the screen are two sources for one
// stream, so each toggle hands back BOTH readings: starting a share stops the
// camera, and the view has two buttons to draw from one answer.
extern crate::video
  VideoSource(camera:bool, sharing:bool)
  sync call_use_camera(on:bool) -> VideoSource
  sync call_use_screen(on:bool) -> VideoSource
  component call_video_tiles(staged:&str) -> unit
  component call_video_stage(peer:&str) -> unit
