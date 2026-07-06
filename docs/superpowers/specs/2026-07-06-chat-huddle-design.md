# Chat huddles — Slack-style live voice in the chat view

Status: authored autonomously under a `/goal` directive (user: "everything's
clear. just copy and paste the slack huddle view and wire it"), so the design
conversation was skipped and decisions below were made against the codebase.

## Goal

Slack-style huddles in the chat view: any channel can host one always-available
voice room. A member starts/joins it from the channel header, everyone else
sees "huddle active · N" (header pill + channel-rail indicator), participants
show in a bottom-docked huddle panel with mute and leave. Audio rides the
existing native voice engine (`chat::voice` — Opus, jitter, mixer, PR #172).

## What exists / what is missing

- `chat::voice::VoiceEngine` is complete but an island: transport-generic
  (`DataPlaneTransport`, only a sim impl), no audio hardware, no signaling, no
  node wiring.
- The webview talks to its node over HTTP/WS only; nodes talk over the
  commonware authenticated TCP mesh (channels must be registered before
  `network.start()`, on every mode, or peers kill connections).
- Chat has no huddle ops; the app has zero audio code.

## Architecture (one vertical slice, four layers)

### 1. Signaling — consensus chat ops (who is in the huddle)

- `Channel` gains `huddle: Vec<HuddleMember>` (`#[serde(default)]`), where
  `HuddleMember { user: Vec<u8>, node: Vec<u8>, joined_at: u64 }` — join
  order preserved (Slack-style), `node` = the member's node ed25519 key (32
  bytes) so peers know where to route voice frames.
- `ChatMsg::JoinHuddle { channel_id, node }` / `ChatMsg::LeaveHuddle
  { channel_id }`. Author from `Env.origin`; only `AuthorRef::User` may join
  (huddles are human affordances). Members-only channels gate like posting.
  Join is idempotent (re-join updates `node`); leave of a non-participant is a
  deterministic no-op; roster capped at `MAX_HUDDLE_MEMBERS = 32`. Empty
  roster = no huddle.
- Storing the roster inline on `Channel` makes it free to every existing
  `Channels`/`Channel` query — the rail indicator and header pill need no new
  query, and the app's block-driven `refresh()` picks it up automatically.
  Flag-day change per the no-backwards-compat rule.

### 2. Node-to-node audio — `CHANNEL_VOICE = 7` + `DataPlaneTransport` adapter

- New commonware channel `CHANNEL_VOICE: u64 = 7` (free slot below the epoch
  bank at 8), registered in ALL modes: black-holed on sync-only observers and
  parked joiners, wired to the voice hub on the validator path.
- `bin/node/src/voice.rs`: `MeshTransport` implements
  `data_plane::DataPlaneTransport` over the channel's `(Sender, Receiver)`
  pair — datagram-only (voice never opens streams; `connect`/`accept` report
  `Closed`). This keeps `VoiceEngine` untouched and leaves the designed
  UDP-on-WireGuard arm as a drop-in replacement later. TCP head-of-line
  blocking is accepted for v1 (small-team meshes); the jitter buffer absorbs it.
- `DataPlane` admission policy: permit `Service::Voice` flows that this node
  currently has an open local session for (join-gated locally; the mesh is
  already authenticated workspace members only). Default-deny otherwise, drops
  counted per the plane's contract.

### 3. Webview-to-node audio — `GET /v1/voice/ws?channel=<id>`

- New WS route in `bin/noded` (axum, alongside `/v1/ws`): binary frames of
  exactly 1920 bytes = one 20 ms mono 48 kHz PCM frame (960 × i16 LE).
  Client→server = captured mic frames; server→client = the engine's mixed
  playout at a 20 ms tick. Text frames carry JSON control:
  `{"type":"recipients","peers":["<64-hex node key>", ...]}` — the webview
  (which tracks the consensus roster) steers fan-out; the localhost client is
  the node operator's own trusted UI, same trust model as `origin`.
- The app-surface thread starts before the mesh exists, so the seam is an
  mpsc request lane created up front: `NodeHandle::with_voice(tx)`; the
  validator path drains requests and answers with per-session channel handles
  (`pcm_in` mpsc, `mixed_out` mpsc, `recipients` watch). One session per
  channel per node; a second WS for the same channel replaces the first
  (Slack semantics: you are in at most one huddle — the frontend enforces
  leave-before-join too).
- `VoiceHub` (in `bin/node/src/voice.rs`): per-session task owns a
  `VoiceEngine` on `FlowId::derive(b"voice-channel:<id>")`, selects between
  `pcm_in` (→ `send_frame` to current recipients) and a 20 ms interval
  (→ `playout()` → `mixed_out`). Session ends when the WS drops either handle.
- `NodeStatus` gains `public_key` (hex) so the webview can stamp its node key
  into `JoinHuddle.node`.

### 4. Chat view UI (the "copy Slack" part)

- **Header huddle toggle** (right side of the channel header, next to the
  members-only pill): headphones glyph `HoverButton`; idle = "Huddle", active
  elsewhere = "Huddle · N", active here = filled accent state. Click = join;
  click while joined = leave.
- **Channel rail indicator**: small headphones glyph + participant count next
  to channel names with a non-empty roster.
- **Huddle dock** (Slack's bottom-left panel): docked at the bottom of the
  channel rail while joined — channel name, avatar pile (2-letter initials,
  MembersView `Avatar` pattern), mic mute toggle, leave button, connecting/live
  status dot. Inline styles + `theme/tokens`, chat-local glyphs per
  `MessageItem.tsx` precedent.
- **Join muted by default** (user directive): entering a huddle is never a
  hot-mic moment — the mic starts muted and unmuting is the deliberate act.
- **Audio plumbing** (`app/src/domain/voice-session.ts`): `getUserMedia`
  (mono, echoCancellation) → `AudioContext({sampleRate: 48000})` →
  capture `AudioWorklet` accumulating 960-sample i16 frames → WS binary; WS
  binary → playback worklet ring buffer → speakers. Mute = stop sending
  frames (track stays live, Slack-style instant unmute).
- **State**: ephemeral `voice` slice in the console store (`{channelId,
  muted, status}`), explicitly OUTSIDE `ConsoleSnapshot` (committed-state
  only); roster truth stays the consensus `Channel.huddle` delivered by
  `refresh()`. Actions `joinHuddle`/`leaveHuddle` = submitTracked chat op +
  voice session start/stop; recipients pushed to the WS whenever the roster
  changes.
- macOS mic permission: `NSMicrophoneUsageDescription` in the Tauri bundle
  Info.plist.

## Non-goals (v1)

- No speaking indicators / per-speaker volume (engine `speaker_stats()` is the
  seam; UI later).
- No screen share, video, reactions-in-huddle, or "huddle started" system
  messages.
- No observer-mode voice (hub runs on the validator path; other modes
  black-hole the channel).
- No packet-loss concealment beyond the engine's gap→silence.

## Testing

- Chat crate: op-level tests for join/leave (policy gate, idempotency, cap,
  non-user rejection, node-key validation) following existing module tests.
- `bin/node` voice runtime: unit test the mesh adapter + hub session plumbing
  where feasible without a full mesh; the engine's media path is already
  proven by `voice_e2e.rs`.
- App: typecheck + existing vitest suite; manual verification in the real
  Tauri window (tauri-debug) for the UI states.
