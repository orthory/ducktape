// The huddle call boundary — `crate::call` is the `/v1/call/ws` client: mic
// capture in, mixed playout out, control as json text. The SESSION IS THE
// SUBSCRIPTION: lifecycle.ice runs `call_session` while `huddle_joined`
// holds, so joining starts media and leaving tears it down by dropping the
// stream — there is no imperative stop.
//
// `call_set_muted`/`call_recipients` reach the running session through the
// module's parked handles and are honest no-ops when none runs.
extern crate::call
  CallEvent(kind:str, message:str, peer:str, muted:bool, camera_on:bool, sharing:bool)
  stream call_session(rpc:str, channel_id:str) -> CallEvent
  sync call_set_muted(muted:bool) -> bool
  sync call_recipients(nodes:[str]) -> bool
  pure call_status_after(current:str, event:CallEvent) -> str
  pure apply_call_peer(peers:[CallEvent], event:CallEvent) -> [CallEvent]
  pure call_peer_muted(peers:[CallEvent], node:str) -> bool
  pure call_video_live_after(peers:[CallEvent], camera:bool) -> bool

// The camera leg — `crate::video`: capture/encode on its own thread, decoded
// peer frames in a store the tile strip reads. `call_video_tiles` is a
// SELF-REDRAWING widget: it repaints its own window at the capture cadence
// via per-window redraw requests — no tick, no state, no app rebuild.
extern crate::video
  sync call_set_camera(on:bool) -> bool
  component call_video_tiles() -> unit
