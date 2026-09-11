// FORGE SPEAKS THE KERNEL CONTRACT. The repo namespace, one repo's branches
// and tracker, the open item with its patch, reviews and discussion, and the
// code browse's listing and file are the VIEW's own reads (crates/views/forge)
// — it asks the node through `rpc.query` / `rpc.view`, re-reads on every
// forge block, and a review or a merge leaves as `op.submit` the kernel
// signs. The app holds no forge state.
//
// What is left here is what a view cannot hold: the two OS doors (the
// clipboard and the app's one open plane) and the discussion note composer,
// docked under the view as a host surface because the editor it edits
// cannot cross the wire. A note carries its SCOPE, and the channel is read
// back out of that scope — nothing on this side remembers which item is
// open.
on forge_view_event(event)
  match forge_intent(event)
    ForgeIntent.open_link
      flow
        from done event_text(event, "url")
        done -> open_message_link _
    ForgeIntent.composer
      let scope = event_text(event, "scope")
      run every duck_echo_str(event_text(event, "body")) -> forge_composer_event(scope, _) | external_url_failed _
    ForgeIntent.copy
      toast = event_text(event, "label")
      toast_age = 0
      task clipboard write event_text(event, "text")

// The note's words live in the host's composer (`forge_composer`, the chat
// composer over the item's channel on this network as its scope); a send
// arrives here as the trimmed body and the scope it was written in. The
// composer cleared itself before it emitted, so a body the gate refuses —
// or one written for a network the reader has since left — goes back to
// THAT box, never to the item on screen, and a failed send too.
//
// THE SEND CARRIES ITS OWN IDENTITY on both routes — the box it left from
// and its operation id — because a newer note may be in flight by the time
// this one answers. So a failure restores into the scope IT captured, and
// only the completion whose id is still the pending one clears the flag.
on forge_composer_event(scope, body)
  let channel = scope_channel(scope, connected_rpc)
  match submit_verdict(loading, connected, channel, forge_note_pending, !empty(channel), scope, scope)
    SubmitVerdict.refused
      composer_stashed = chat_composer_unsent(scope, body, false)
    SubmitVerdict.admitted
      let op = fresh_operation_id("forge-note")
      forge_note_pending = op
      run every send_message(connected_rpc, password, channel, op, trim(body)) -> forge_note_sent(op, _) | forge_note_failed(scope, op, _)

on forge_note_sent(op, next)
  return if op != forge_note_pending
  forge_note_pending = ""
  error = ""

on forge_note_failed(scope, op, cause)
  composer_stashed = chat_composer_unsent(scope, cause.body, cause.committed)
  return if op != forge_note_pending
  forge_note_pending = ""
  error = cause.message
