# Video Calls in Chat Huddles — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add camera video to the chat huddles PR #178 shipped, implementing the
video arm of `docs/adr/2026-07-06-video-call-module.md` end to end: video wire
(`Service::Video = 3`, 16-byte fragmenting header), call control (keyframe
request / beacon / rate hint), a dedicated mesh lane, the node hub video path,
a typed `/v1/call/ws` bridge, WebCodecs capture/encode/decode in the app, a
tile-grid call surface, and the ADR's `CallSweep` consensus op.

**Architecture:** The ADR's inter-node wire is kept byte-exact (it is declared
gateway-agnostic on purpose): encoded VP8 frames fragment across data-plane
datagrams on `Service::Video = 3` with the ADR's 16-byte header; control rides
a separate flow on `Service::Voice` so audio-only builds still pass control.
The webview↔node seam is NOT the ADR's str0m WebRTC gateway — it is the WS
bridge #178 shipped, extended with typed binary frames: the browser encodes
with WebCodecs (VP8, 720p30 cap) and ships encoded chunks over localhost WS;
the node hub fragments them onto the mesh and reassembles inbound frames per
peer (any missing fragment drops the whole frame; recovery = keyframe
request). RTCP-shaped control maps to WS JSON: PLI ↔ `keyframeRequest`,
REMB ↔ `rateHint` (ladder 1200/800/500/300 kbps, sender takes min across
receivers). Platforms without WebCodecs/camera (Linux WebKitGTK) get
capability-gated UI — roster + audio only, exactly the ADR's soft-dependency
posture; the Chromium companion window is deferred and documented.

**Tech Stack:** Rust (tokio, data-plane crate, commonware p2p mesh channel),
axum WebSocket (noded), TypeScript/React (app), WebCodecs `VideoEncoder`/
`VideoDecoder` (VP8), vitest, cargo test.

## Global Constraints

- Branch `feat/video-calls`, PR based on `dev` (never `main`) — `.project/work.md`.
- Wire commitments are append-only: `Service::Video = 3`; the 16-byte video
  header layout `ver·flags(keyframe)·frame_no:u32·frag_index:u16·frag_count:u16·ts_ms:u32·reserved:u16`;
  `Service::Voice = 2` and its 8-byte header stay byte-identical.
- Consensus carries joins/leaves/sweeps only — never media, never mute toggles.
- `MAX_DATAGRAM = 1372`, `MAX_DATAGRAM_PAYLOAD = 1360` (12-byte plane header) — the plane never fragments; the video layer must.
- Rate ladder: 1200/800/500/300 kbps. Keyframe requests rate-limited ≥1 s. Beacon cadence 1 s. Video capped at 8 participants (`MAX_VIDEO_PARTICIPANTS`); huddle roster cap stays 32.
- Mesh channel ids are append-only: `CHANNEL_VIDEO = 8` (7 = voice, 6 = reachability, …). Every mesh mode must register every channel (unregistered = protocol violation that kills the sender's connection): validator wires it, observer/parked modes black-hole it.
- Commit messages end with `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`.
- All cargo/bun commands run from the worktree root (`.claude/worktrees/feat-video-calls`); app commands from `app/`.

---

### Task 1: `Service::Video = 3` in the data-plane registry

**Files:**
- Modify: `crates/system/data-plane/src/lib.rs` (the `Service` enum + `TryFrom`)

**Interfaces:**
- Produces: `data_plane::Service::Video` (u8 = 3) — used by Tasks 2, 4.

- [ ] **Step 1: Write the failing test** — append to the `tests` module in `crates/system/data-plane/src/wire.rs`:

```rust
    #[test]
    fn service_ids_are_wire_stable() {
        // append-only registry: these numbers are cross-node commitments.
        for (service, id) in [
            (Service::StateSync, 1u8),
            (Service::Voice, 2u8),
            (Service::Video, 3u8),
        ] {
            assert_eq!(service as u8, id);
            assert_eq!(Service::try_from(id), Ok(service));
        }
        assert_eq!(Service::try_from(4u8), Err(4u8));
    }
```

- [ ] **Step 2: Run it — expect FAIL** (`Video` not defined):
`cargo test -p data-plane service_ids_are_wire_stable`

- [ ] **Step 3: Implement** in `crates/system/data-plane/src/lib.rs`: add to the enum after `Voice = 2`:

```rust
    /// Real-time camera video (chat module): encoded frames fragmented
    /// across datagrams — see `chat::video` for the frame layer.
    Video = 3,
```

and a `3 => Ok(Service::Video),` arm in `TryFrom<u8>`.

- [ ] **Step 4: Run to PASS**: `cargo test -p data-plane`
- [ ] **Step 5: Commit** — `feat(data-plane): register Service::Video = 3`

---

### Task 2: `chat::video` — frame wire, fragmentation, reassembly, control

**Files:**
- Create: `crates/apps/chat/src/video/mod.rs`
- Create: `crates/apps/chat/src/video/frame.rs`
- Create: `crates/apps/chat/src/video/assembly.rs`
- Create: `crates/apps/chat/src/video/control.rs`
- Modify: `crates/apps/chat/src/lib.rs` (add `pub mod video;` next to `pub mod voice;`)

**Interfaces:**
- Consumes: `data_plane::MAX_DATAGRAM_PAYLOAD`.
- Produces (used by Task 4):
  - `chat::video::{VIDEO_HEADER_LEN=16, FLAG_KEYFRAME, MAX_FRAGMENT_PAYLOAD, MAX_FRAGS, MAX_FRAME_BYTES}`
  - `fn fragment_frame(frame_no: u32, keyframe: bool, ts_ms: u32, data: &[u8]) -> Result<Vec<Vec<u8>>, VideoError>`
  - `fn decode_fragment(frame: &[u8]) -> Result<(VideoHeader, &[u8]), VideoError>`
  - `struct Reassembler` with `fn insert(&mut self, header: VideoHeader, payload: &[u8]) -> Assembly`, `fn dropped_frames(&self) -> u64`
  - `enum Assembly { Progress, Stale, Complete(CompleteFrame) }`, `struct CompleteFrame { frame_no: u32, keyframe: bool, ts_ms: u32, data: Vec<u8> }`
  - `enum CallControl { KeyframeRequest, Beacon { muted: bool, camera_on: bool }, RateHint { max_kbps: u32 } }` with `encode() -> Vec<u8>` / `decode(&[u8]) -> Result<CallControl, ControlError>`
  - `const RATE_LADDER_KBPS: [u32; 4] = [1200, 800, 500, 300]`, `fn step_down(u32) -> u32`, `fn step_up(u32) -> u32`

- [ ] **Step 1: `frame.rs`** — header + fragmentation (mirrors `voice/media.rs` in tone):

```rust
//! The video frame layer inside a data-plane datagram on `Service::Video`:
//! a 16-byte header, then one fragment of one encoded (VP8) frame. A frame
//! larger than a datagram fragments across several; ANY missing fragment
//! drops the whole frame — no retransmit, recovery is the next keyframe.
//!
//! ```text
//! offset  0        1      2..6              6..8               8..10
//!         ver = 1  flags  frame_no (u32BE)  frag_index (u16BE) frag_count (u16BE)
//! offset  10..14          14..16
//!         ts_ms (u32BE)   reserved = 0
//! ```

use data_plane::MAX_DATAGRAM_PAYLOAD;

pub const VIDEO_VERSION: u8 = 1;
pub const VIDEO_HEADER_LEN: usize = 16;
/// flags bit 0: this frame is a keyframe (decoder sync point).
pub const FLAG_KEYFRAME: u8 = 0b0000_0001;
/// Encoded bytes per fragment: a plane datagram payload minus this header.
pub const MAX_FRAGMENT_PAYLOAD: usize = MAX_DATAGRAM_PAYLOAD - VIDEO_HEADER_LEN;
/// Fragments per frame — bounds reassembly memory. 96 × 1344 ≈ 126 KiB,
/// comfortable for a 720p VP8 keyframe at the top of the rate ladder.
pub const MAX_FRAGS: usize = 96;
pub const MAX_FRAME_BYTES: usize = MAX_FRAGS * MAX_FRAGMENT_PAYLOAD;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VideoHeader {
    pub keyframe: bool,
    pub frame_no: u32,
    pub frag_index: u16,
    pub frag_count: u16,
    pub ts_ms: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum VideoError {
    #[error("encoded frame {len} exceeds {MAX_FRAME_BYTES} bytes")]
    FrameTooLarge { len: usize },
    #[error("empty frame")]
    Empty,
    #[error("video fragment truncated")]
    Truncated,
    #[error("unsupported video version {0}")]
    BadVersion(u8),
    #[error("inconsistent fragment header")]
    BadHeader,
}

pub fn encode_fragment(header: VideoHeader, payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(VIDEO_HEADER_LEN + payload.len());
    frame.push(VIDEO_VERSION);
    frame.push(if header.keyframe { FLAG_KEYFRAME } else { 0 });
    frame.extend_from_slice(&header.frame_no.to_be_bytes());
    frame.extend_from_slice(&header.frag_index.to_be_bytes());
    frame.extend_from_slice(&header.frag_count.to_be_bytes());
    frame.extend_from_slice(&header.ts_ms.to_be_bytes());
    frame.extend_from_slice(&[0, 0]);
    frame.extend_from_slice(payload);
    frame
}

pub fn decode_fragment(frame: &[u8]) -> Result<(VideoHeader, &[u8]), VideoError> {
    if frame.len() < VIDEO_HEADER_LEN {
        return Err(VideoError::Truncated);
    }
    if frame[0] != VIDEO_VERSION {
        return Err(VideoError::BadVersion(frame[0]));
    }
    let header = VideoHeader {
        keyframe: frame[1] & FLAG_KEYFRAME != 0,
        frame_no: u32::from_be_bytes(frame[2..6].try_into().expect("4 bytes")),
        frag_index: u16::from_be_bytes(frame[6..8].try_into().expect("2 bytes")),
        frag_count: u16::from_be_bytes(frame[8..10].try_into().expect("2 bytes")),
        ts_ms: u32::from_be_bytes(frame[10..14].try_into().expect("4 bytes")),
    };
    if header.frag_count == 0
        || header.frag_count as usize > MAX_FRAGS
        || header.frag_index >= header.frag_count
    {
        return Err(VideoError::BadHeader);
    }
    Ok((header, &frame[VIDEO_HEADER_LEN..]))
}

/// Split one encoded frame into ready-to-send datagram payloads.
pub fn fragment_frame(
    frame_no: u32,
    keyframe: bool,
    ts_ms: u32,
    data: &[u8],
) -> Result<Vec<Vec<u8>>, VideoError> {
    if data.is_empty() {
        return Err(VideoError::Empty);
    }
    if data.len() > MAX_FRAME_BYTES {
        return Err(VideoError::FrameTooLarge { len: data.len() });
    }
    let count = data.len().div_ceil(MAX_FRAGMENT_PAYLOAD);
    Ok(data
        .chunks(MAX_FRAGMENT_PAYLOAD)
        .enumerate()
        .map(|(index, chunk)| {
            encode_fragment(
                VideoHeader {
                    keyframe,
                    frame_no,
                    frag_index: index as u16,
                    frag_count: count as u16,
                    ts_ms,
                },
                chunk,
            )
        })
        .collect())
}

/// Wrapping frame_no comparison: is `a` newer than `b`?
pub fn frame_newer(a: u32, b: u32) -> bool {
    (a.wrapping_sub(b) as i32) > 0
}
```

Tests (same file, `#[cfg(test)] mod tests`): round-trip a 3-fragment frame
(`fragment_frame` → `decode_fragment` each → payload concatenation equals the
input, headers carry `frag_index` 0..count and consistent `frag_count`);
exact-multiple-of-`MAX_FRAGMENT_PAYLOAD` input produces `len/MAX` fragments
(no empty tail); oversize/empty inputs error; `decode_fragment` rejects
truncated / bad version / `frag_index >= frag_count` / `frag_count = 0`;
`frame_newer` wraps like `voice::media::seq_newer`.

- [ ] **Step 2: `assembly.rs`** — per-peer reassembly (the hub keys one `Reassembler` per peer):

```rust
//! Frame reassembly for one sender: fragments arrive unordered and lossy;
//! a frame completes when all fragments land. A NEWER frame starting while
//! one is in progress abandons the old one (any missing fragment drops the
//! whole frame — the ADR's contract); frames at-or-below the last emitted
//! frame_no are stale and ignored. `dropped_frames` feeds the keyframe-
//! request path.

use super::frame::{frame_newer, VideoHeader, MAX_FRAGMENT_PAYLOAD};

pub struct CompleteFrame {
    pub frame_no: u32,
    pub keyframe: bool,
    pub ts_ms: u32,
    pub data: Vec<u8>,
}

pub enum Assembly {
    /// fragment stored, frame not yet complete.
    Progress,
    /// stale or duplicate fragment — ignored.
    Stale,
    Complete(CompleteFrame),
}

#[derive(Default)]
pub struct Reassembler {
    current: Option<InProgress>,
    last_emitted: Option<u32>,
    dropped: u64,
}

struct InProgress {
    frame_no: u32,
    keyframe: bool,
    ts_ms: u32,
    parts: Vec<Option<Vec<u8>>>,
    received: usize,
}

impl Reassembler {
    pub fn insert(&mut self, header: VideoHeader, payload: &[u8]) -> Assembly {
        if payload.len() > MAX_FRAGMENT_PAYLOAD {
            return Assembly::Stale;
        }
        if let Some(last) = self.last_emitted
            && !frame_newer(header.frame_no, last)
        {
            return Assembly::Stale;
        }
        match &self.current {
            Some(current) if current.frame_no == header.frame_no => {}
            Some(current) if frame_newer(header.frame_no, current.frame_no) => {
                // a newer frame started before this one completed: the old
                // frame is dead (missing fragments never retransmit).
                self.dropped += 1;
                self.current = Some(InProgress::start(header));
            }
            Some(_) => return Assembly::Stale, // older than in-progress
            None => self.current = Some(InProgress::start(header)),
        }
        let current = self.current.as_mut().expect("just ensured");
        // a fragment disagreeing on the frame's shape poisons the frame —
        // drop it wholesale rather than assemble a chimera.
        if current.parts.len() != header.frag_count as usize
            || current.keyframe != header.keyframe
        {
            self.current = None;
            self.dropped += 1;
            return Assembly::Stale;
        }
        let slot = &mut current.parts[header.frag_index as usize];
        if slot.is_some() {
            return Assembly::Stale; // duplicate
        }
        *slot = Some(payload.to_vec());
        current.received += 1;
        if current.received < current.parts.len() {
            return Assembly::Progress;
        }
        let done = self.current.take().expect("complete frame");
        self.last_emitted = Some(done.frame_no);
        Assembly::Complete(CompleteFrame {
            frame_no: done.frame_no,
            keyframe: done.keyframe,
            ts_ms: done.ts_ms,
            data: done.parts.into_iter().flatten().flatten().collect(),
        })
    }

    /// frames abandoned incomplete since construction (drives keyframe requests).
    pub fn dropped_frames(&self) -> u64 {
        self.dropped
    }
}

impl InProgress {
    fn start(header: VideoHeader) -> Self {
        InProgress {
            frame_no: header.frame_no,
            keyframe: header.keyframe,
            ts_ms: header.ts_ms,
            parts: vec![None; header.frag_count as usize],
            received: 0,
        }
    }
}
```

Tests: out-of-order fragments still complete; losing one fragment then
receiving a newer frame bumps `dropped_frames` and the old frame never
emits; a stale (older/duplicate) fragment returns `Stale` and completed
frame_nos are monotonic; single-fragment frames complete immediately.

- [ ] **Step 3: `control.rs`** — the ctl-flow wire + the rate ladder:

```rust
//! Call control datagrams — on a separate flow over `Service::Voice`, so
//! control keeps working in an audio-only build (ADR §2). One tiny tagged
//! frame per message: `[ver=1][tag][fields…]`, all integers BE.

pub const CTL_VERSION: u8 = 1;
const TAG_KEYFRAME_REQUEST: u8 = 1;
const TAG_BEACON: u8 = 2;
const TAG_RATE_HINT: u8 = 3;

/// The sender-side bitrate ladder (kbps): receivers hint, the sender takes
/// the min across receivers.
pub const RATE_LADDER_KBPS: [u32; 4] = [1200, 800, 500, 300];

/// The next rung below `current` (saturates at the bottom).
pub fn step_down(current: u32) -> u32 {
    RATE_LADDER_KBPS
        .iter()
        .copied()
        .filter(|&r| r < current)
        .max()
        .unwrap_or(*RATE_LADDER_KBPS.last().expect("non-empty ladder"))
}

/// The next rung above `current` (saturates at the top).
pub fn step_up(current: u32) -> u32 {
    RATE_LADDER_KBPS
        .iter()
        .copied()
        .filter(|&r| r > current)
        .min()
        .unwrap_or(RATE_LADDER_KBPS[0])
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallControl {
    /// the receiver lost a frame and needs a decoder sync point. Senders
    /// rate-limit honoring this to one keyframe per second.
    KeyframeRequest,
    /// 1 Hz presence + ephemeral state (drives tiles, NOT consensus).
    Beacon { muted: bool, camera_on: bool },
    /// receiver loss report: send to me at no more than `max_kbps`.
    RateHint { max_kbps: u32 },
}

#[derive(Debug, thiserror::Error)]
pub enum ControlError {
    #[error("control frame truncated")]
    Truncated,
    #[error("unsupported control version {0}")]
    BadVersion(u8),
    #[error("unknown control tag {0}")]
    UnknownTag(u8),
}

impl CallControl {
    pub fn encode(&self) -> Vec<u8> {
        match self {
            CallControl::KeyframeRequest => vec![CTL_VERSION, TAG_KEYFRAME_REQUEST],
            CallControl::Beacon { muted, camera_on } => {
                vec![CTL_VERSION, TAG_BEACON, *muted as u8, *camera_on as u8]
            }
            CallControl::RateHint { max_kbps } => {
                let mut frame = vec![CTL_VERSION, TAG_RATE_HINT];
                frame.extend_from_slice(&max_kbps.to_be_bytes());
                frame
            }
        }
    }

    pub fn decode(frame: &[u8]) -> Result<CallControl, ControlError> {
        if frame.len() < 2 {
            return Err(ControlError::Truncated);
        }
        if frame[0] != CTL_VERSION {
            return Err(ControlError::BadVersion(frame[0]));
        }
        match frame[1] {
            TAG_KEYFRAME_REQUEST => Ok(CallControl::KeyframeRequest),
            TAG_BEACON if frame.len() >= 4 => Ok(CallControl::Beacon {
                muted: frame[2] != 0,
                camera_on: frame[3] != 0,
            }),
            TAG_RATE_HINT if frame.len() >= 6 => Ok(CallControl::RateHint {
                max_kbps: u32::from_be_bytes(frame[2..6].try_into().expect("4 bytes")),
            }),
            TAG_BEACON | TAG_RATE_HINT => Err(ControlError::Truncated),
            other => Err(ControlError::UnknownTag(other)),
        }
    }
}
```

Tests: round-trip all three variants; truncated/bad-version/unknown-tag
errors; ladder steps (1200→800→500→300, saturating both ends; step_down(700)=500).

- [ ] **Step 4: `mod.rs`** — module docs (the ADR pointer, the "consensus never
carries media" boundary) + re-export everything above; add `pub mod video;` to
`crates/apps/chat/src/lib.rs`.

- [ ] **Step 5: Run** `cargo test -p chat video` — all new tests PASS; `cargo clippy -p chat` clean.
- [ ] **Step 6: Commit** — `feat(chat): video frame wire — fragmentation, reassembly, call control`

---

### Task 3: `SweepHuddle` consensus op (the ADR's `CallSweep`)

**Files:**
- Modify: `crates/apps/chat/src/interface.rs` (new `ChatMsg` variant)
- Modify: `crates/apps/chat/src/lib.rs` (staging + dispatch)
- Test: `crates/apps/chat/tests/channel_system.rs`

**Interfaces:**
- Produces: `ChatMsg::SweepHuddle { channel_id: String, user: Vec<u8> }` —
  JSON `{ "sweep_huddle": { "channel_id": …, "user": [bytes…] } }` (used by Task 8's client).

- [ ] **Step 1: Failing tests** in `crates/apps/chat/tests/channel_system.rs`, following the JoinHuddle/LeaveHuddle test idioms already there (same submit/query helpers): (a) member A joins the huddle, member B sweeps A → roster empty; sweeping again → deterministic no-op, still Ok; (b) on a members-only channel, a non-member's sweep is rejected; (c) a module-origin sweep is rejected with "only external users may sweep a huddle".

- [ ] **Step 2: Run to FAIL**: `cargo test -p chat --test channel_system sweep`

- [ ] **Step 3: Implement.** `interface.rs`, after `LeaveHuddle`:

```rust
    /// evict a huddle member — call liveness is not consensus-observable
    /// (a crashed client cannot leave), so cleanup is social: any author the
    /// channel's post policy admits may sweep a stale entry, mirroring
    /// `SetMembership`'s trust posture. sweeping an absent user is a
    /// deterministic no-op.
    SweepHuddle { channel_id: String, user: Vec<u8> },
```

`lib.rs`, after `stage_leave_huddle` (author gate + policy gate mirror `stage_join_huddle`):

```rust
    /// evict `user` from the channel's huddle (staleness cleanup — see
    /// `ChatMsg::SweepHuddle`). gated like posting; absent target = no-op.
    async fn stage_sweep_huddle(
        &mut self,
        author: AuthorRef,
        channel_id: &str,
        user: &[u8],
    ) -> Result<(), Error> {
        Self::validate_non_empty("channel_id", channel_id)?;
        let AuthorRef::User(_) = &author else {
            return Err(Error::Module(
                "only external users may sweep a huddle".into(),
            ));
        };
        let mut channel = self.require_channel(channel_id).await?;
        self.check_post_policy(&channel, &author).await?;
        let before = channel.huddle.len();
        channel.huddle.retain(|m| m.user != user);
        if channel.huddle.len() == before {
            return Ok(());
        }
        self.store_bounded(
            channel_key(channel_id),
            &channel,
            MAX_CHANNEL_RECORD_BYTES,
            "channel",
        )
    }
```

Dispatch arm next to the other huddle ops:

```rust
            ChatMsg::SweepHuddle { channel_id, user } => {
                self.stage_sweep_huddle(author, &channel_id, &user).await
            }
```

- [ ] **Step 4: Run to PASS**: `cargo test -p chat --test channel_system`
- [ ] **Step 5: Commit** — `feat(chat): sweep_huddle op — social eviction of stale huddle entries`

---

### Task 4: noded call-session types + hub video path

**Files:**
- Modify: `bin/noded/src/lib.rs` (session types grow video/control ends; rename `Voice*` → `Call*`)
- Modify: `bin/node/src/voice.rs` (hub: admission by `(Service, FlowId)`, cam+ctl flows, video/ctl pumps; loopback tests)
- Modify: `bin/node/src/main.rs`, `bin/simnode/src/main.rs`, `bin/noded/src/main.rs`, `bin/noded/tests/router.rs` — mechanical rename fallout only (Task 5 rewires the mesh lanes properly).

**Interfaces:**
- Consumes: Task 1's `Service::Video`, Task 2's `fragment_frame`/`decode_fragment`/`Reassembler`/`CallControl`/ladder.
- Produces (used by Tasks 5, 6):
  - noded types (all in `bin/noded/src/lib.rs`):
    `struct CapturedVideo { keyframe: bool, ts_ms: u32, data: Vec<u8> }`,
    `struct PeerVideo { peer: [u8; 32], keyframe: bool, ts_ms: u32, data: Vec<u8> }`,
    `enum CallControlIn { Beacon { muted: bool, camera_on: bool }, KeyframeRequest { peer: [u8; 32] } }`,
    `enum CallControlOut { KeyframeRequest, PeerBeacon { peer: [u8; 32], muted: bool, camera_on: bool }, RateHint { max_kbps: u32 } }`,
    `struct CallSession { pcm_in: mpsc::Sender<Vec<i16>>, mixed_out: mpsc::Receiver<Vec<i16>>, recipients: watch::Sender<Vec<[u8; 32]>>, video_in: mpsc::Sender<CapturedVideo>, video_out: mpsc::Receiver<PeerVideo>, control_in: mpsc::Sender<CallControlIn>, control_out: mpsc::Receiver<CallControlOut> }`,
    `struct CallSessionRequest { channel_id: String, reply: oneshot::Sender<Result<CallSession, String>> }`,
    `type CallLane = mpsc::Sender<CallSessionRequest>`, `NodeHandle::with_call(CallLane)`.
  - hub: `voice::spawn_hub(requests: mpsc::Receiver<noded::CallSessionRequest>) -> (mpsc::Receiver<VoiceDatagram>, mpsc::Receiver<VoiceDatagram>, mpsc::Sender<VoiceDatagram>)` — now returns `(voice_outbound, video_outbound, inbound)`; outbound datagrams are routed by service byte (`frame[1]`).

- [ ] **Step 1: Rename** `noded::VoiceSession` → `CallSession`, `VoiceSessionRequest` → `CallSessionRequest`, `VoiceLane` → `CallLane`, `NodeHandle::with_voice` → `with_call`, field `voice` → `call` (keep `/v1/voice/ws` compiling against the renamed types for now — Task 6 replaces the endpoint). Fix all references (`bin/node/src/main.rs`, `bin/node/src/voice.rs`, `bin/noded/src/main.rs`, `bin/simnode/src/main.rs`, `bin/noded/tests/router.rs`). Run `cargo build -p noded -p node-bin -p simnode` to prove the rename is complete.

- [ ] **Step 2: Extend `CallSession`** with the four new ends and add the four new types above (doc comments explaining webview↔hub direction). Lane depths in `voice.rs`: `const VIDEO_LANE: usize = 32;` (frames, not fragments — ~1 s at 30 fps) and `const CTL_LANE: usize = 32;`.

- [ ] **Step 3: Hub — admission + flows.** In `bin/node/src/voice.rs`:
  - `ActiveFlows(Mutex<HashSet<(Service, FlowId)>>)`; `permits` = `self.0.lock().contains(&(service, flow))`; `insert`/`remove` take `(Service, FlowId)`.
  - Flow derivations next to `channel_flow`:

```rust
/// the camera flow for a chat channel (Service::Video).
fn video_flow(channel_id: &str) -> FlowId {
    FlowId::derive(format!("video-channel:{channel_id}").as_bytes())
}

/// the call-control flow (Service::Voice — control must work in an
/// audio-only build, ADR §2).
fn ctl_flow(channel_id: &str) -> FlowId {
    FlowId::derive(format!("callctl-channel:{channel_id}").as_bytes())
}
```

  - `open_session` registers all three datagram flows (each through the same
    ~1 s retry loop, since a torn-down predecessor releases them
    asynchronously): mic `(Service::Voice, channel_flow)` with `FLOW_QUEUE`,
    cam `(Service::Video, video_flow)` with `max_queued: 256` (fragments —
    ~2 frames of a keyframe burst), ctl `(Service::Voice, ctl_flow)` with
    `max_queued: 32`. Insert all three into `ActiveFlows`; `SessionGuard`
    carries the three `(Service, FlowId)` pairs and removes them all in
    `teardown` (and at the end of `run_session`).

- [ ] **Step 4: Hub — `run_session` grows video + control arms.** Signature:

```rust
async fn run_session<T: DataPlaneTransport>(
    mut engine: VoiceEngine<T>,
    video: DatagramFlow<T>,
    ctl: DatagramFlow<T>,
    mut pcm_in: mpsc::Receiver<Vec<i16>>,
    mixed_out: mpsc::Sender<Vec<i16>>,
    mut video_in: mpsc::Receiver<noded::CapturedVideo>,
    video_out: mpsc::Sender<noded::PeerVideo>,
    mut control_in: mpsc::Receiver<noded::CallControlIn>,
    control_out: mpsc::Sender<noded::CallControlOut>,
    recipients: watch::Receiver<Vec<[u8; 32]>>,
    flows: Arc<ActiveFlows>,
    registered: [(Service, FlowId); 3],
)
```

Session-local state:

```rust
    let mut frame_no: u32 = 0;
    let mut peers: HashMap<[u8; 32], PeerLane> = HashMap::new();
    // what the webview last told us — repeated at 1 Hz as our beacon.
    let (mut muted, mut camera_on) = (true, false);
    // rate hints RECEIVED from each peer about OUR sending; effective = min.
    let mut inbound_hints: HashMap<[u8; 32], u32> = HashMap::new();
    let mut effective_kbps: u32 = RATE_LADDER_KBPS[0];
    // ≥1 s between keyframes we ask our own encoder for.
    let mut last_encoder_kick: Option<Instant> = None;
    let mut ctl_tick = tokio::time::interval(Duration::from_secs(1));
    ctl_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut window: u8 = 0; // 5 ctl ticks = one rate window
```

with per-peer receive state:

```rust
struct PeerLane {
    reassembler: chat::video::Reassembler,
    /// last time we asked THIS peer for a keyframe (≥1 s apart).
    last_keyframe_req: Option<Instant>,
    /// the hint we currently give this peer, and this window's loss counts.
    hint_kbps: u32,             // starts at RATE_LADDER_KBPS[0]
    clean_windows: u8,
    window_complete: u64,
    window_dropped_base: u64,   // reassembler.dropped_frames() at window start
}
```

New `tokio::select!` arms (audio arms unchanged):

```rust
            captured = video_in.recv() => {
                let Some(frame) = captured else { break };
                let recipients_now: Vec<PeerId> =
                    recipients.borrow().iter().map(|raw| PeerId(*raw)).collect();
                if recipients_now.is_empty() { continue; }
                let Ok(fragments) = chat::video::fragment_frame(
                    frame_no, frame.keyframe, frame.ts_ms, &frame.data,
                ) else { continue }; // oversize/empty: drop, stay alive
                frame_no = frame_no.wrapping_add(1);
                for fragment in &fragments {
                    for peer in &recipients_now {
                        // fire-and-forget, same posture as voice.
                        let _ = video.send_to(*peer, fragment).await;
                    }
                }
            }
            inbound = video.recv() => {
                let (peer, bytes) = inbound;
                let Ok((header, payload)) = chat::video::decode_fragment(&bytes) else { continue };
                let lane = peers.entry(peer.0).or_insert_with(PeerLane::new);
                match lane.reassembler.insert(header, payload) {
                    chat::video::Assembly::Complete(done) => {
                        lane.window_complete += 1;
                        // full lane = the webview is behind; a dropped frame
                        // is recovered by the next keyframe request from the
                        // browser decoder, so shed rather than backpressure.
                        let _ = video_out.try_send(noded::PeerVideo {
                            peer: peer.0,
                            keyframe: done.keyframe,
                            ts_ms: done.ts_ms,
                            data: done.data,
                        });
                    }
                    chat::video::Assembly::Progress | chat::video::Assembly::Stale => {
                        // a frame died incomplete since we last looked →
                        // ask its sender for a sync point (rate-limited).
                        request_keyframe_if_due(&ctl, peer, lane).await;
                    }
                }
            }
            inbound = ctl.recv() => {
                let (peer, bytes) = inbound;
                let Ok(message) = chat::video::CallControl::decode(&bytes) else { continue };
                match message {
                    chat::video::CallControl::KeyframeRequest => {
                        // honor at most one encoder kick per second.
                        let due = last_encoder_kick
                            .is_none_or(|at| at.elapsed() >= Duration::from_secs(1));
                        if due {
                            last_encoder_kick = Some(Instant::now());
                            let _ = control_out.try_send(noded::CallControlOut::KeyframeRequest);
                        }
                    }
                    chat::video::CallControl::Beacon { muted, camera_on } => {
                        let _ = control_out.try_send(noded::CallControlOut::PeerBeacon {
                            peer: peer.0, muted, camera_on,
                        });
                    }
                    chat::video::CallControl::RateHint { max_kbps } => {
                        inbound_hints.insert(peer.0, max_kbps);
                        push_effective_rate(
                            &recipients, &inbound_hints, &mut effective_kbps, &control_out,
                        );
                    }
                }
            }
            state = control_in.recv() => {
                let Some(state) = state else { break };
                match state {
                    noded::CallControlIn::Beacon { muted: m, camera_on: c } => {
                        (muted, camera_on) = (m, c);
                        // push immediately so toggles feel live; the 1 Hz
                        // tick keeps late joiners current.
                        send_beacon(&ctl, &recipients, muted, camera_on).await;
                    }
                    noded::CallControlIn::KeyframeRequest { peer } => {
                        if let Some(lane) = peers.get_mut(&peer) {
                            request_keyframe_if_due(&ctl, PeerId(peer), lane).await;
                        }
                    }
                }
            }
            _ = ctl_tick.tick() => {
                send_beacon(&ctl, &recipients, muted, camera_on).await;
                // hints from peers no longer in the roster must not pin our rate.
                let live: HashSet<[u8; 32]> = recipients.borrow().iter().copied().collect();
                inbound_hints.retain(|peer, _| live.contains(peer));
                push_effective_rate(&recipients, &inbound_hints, &mut effective_kbps, &control_out);
                window += 1;
                if window >= 5 {
                    window = 0;
                    evaluate_rate_windows(&ctl, &mut peers).await;
                }
            }
```

Helpers (free functions in `voice.rs`):

```rust
impl PeerLane {
    fn new() -> Self {
        PeerLane {
            reassembler: chat::video::Reassembler::default(),
            last_keyframe_req: None,
            hint_kbps: chat::video::RATE_LADDER_KBPS[0],
            clean_windows: 0,
            window_complete: 0,
            window_dropped_base: 0,
        }
    }
}

/// send a KeyframeRequest to `peer` unless one went out under a second ago.
async fn request_keyframe_if_due<T: DataPlaneTransport>(
    ctl: &DatagramFlow<T>,
    peer: PeerId,
    lane: &mut PeerLane,
) {
    let fresh_drops =
        lane.reassembler.dropped_frames() > lane.window_dropped_base + lane.window_complete;
    let _ = fresh_drops; // drop accounting is window-scoped; the request is event-driven
    if lane
        .last_keyframe_req
        .is_none_or(|at| at.elapsed() >= Duration::from_secs(1))
    {
        lane.last_keyframe_req = Some(Instant::now());
        let _ = ctl
            .send_to(peer, &chat::video::CallControl::KeyframeRequest.encode())
            .await;
    }
}

/// our 1 Hz presence beacon to every current recipient.
async fn send_beacon<T: DataPlaneTransport>(
    ctl: &DatagramFlow<T>,
    recipients: &watch::Receiver<Vec<[u8; 32]>>,
    muted: bool,
    camera_on: bool,
) {
    let frame = chat::video::CallControl::Beacon { muted, camera_on }.encode();
    let peers: Vec<PeerId> = recipients.borrow().iter().map(|raw| PeerId(*raw)).collect();
    for peer in peers {
        let _ = ctl.send_to(peer, &frame).await;
    }
}

/// sender side of REMB: min inbound hint (or the ladder top with no hints),
/// forwarded to the webview encoder only when it changes.
fn push_effective_rate(
    recipients: &watch::Receiver<Vec<[u8; 32]>>,
    inbound_hints: &HashMap<[u8; 32], u32>,
    effective_kbps: &mut u32,
    control_out: &mpsc::Sender<noded::CallControlOut>,
) {
    let live = recipients.borrow();
    let next = live
        .iter()
        .filter_map(|peer| inbound_hints.get(peer))
        .copied()
        .min()
        .unwrap_or(chat::video::RATE_LADDER_KBPS[0]);
    if next != *effective_kbps {
        *effective_kbps = next;
        let _ = control_out.try_send(noded::CallControlOut::RateHint { max_kbps: next });
    }
}

/// receiver side of REMB, every 5 s per sending peer: >10% lost frames steps
/// the hint down; 3 consecutive clean windows step it back up. Hints are
/// sent only when they change.
async fn evaluate_rate_windows<T: DataPlaneTransport>(
    ctl: &DatagramFlow<T>,
    peers: &mut HashMap<[u8; 32], PeerLane>,
) {
    for (raw, lane) in peers.iter_mut() {
        let dropped = lane.reassembler.dropped_frames() - lane.window_dropped_base;
        let complete = lane.window_complete;
        lane.window_dropped_base = lane.reassembler.dropped_frames();
        lane.window_complete = 0;
        if complete + dropped == 0 {
            continue; // peer isn't sending video — nothing to rate
        }
        let lossy = dropped * 10 > (complete + dropped); // >10%
        let next = if lossy {
            lane.clean_windows = 0;
            chat::video::step_down(lane.hint_kbps)
        } else {
            lane.clean_windows = lane.clean_windows.saturating_add(1);
            if lane.clean_windows >= 3 {
                lane.clean_windows = 0;
                chat::video::step_up(lane.hint_kbps)
            } else {
                lane.hint_kbps
            }
        };
        if next != lane.hint_kbps {
            lane.hint_kbps = next;
            let _ = ctl
                .send_to(
                    PeerId(*raw),
                    &chat::video::CallControl::RateHint { max_kbps: next }.encode(),
                )
                .await;
        }
    }
}
```

`open_session` wires the new lanes into `CallSession` and passes everything to
`run_session`. The `ChannelTransport` outbound routing moves to Task 5 — for
this task keep a single outbound lane so the existing loopback test still
passes (`send_datagram` unchanged).

- [ ] **Step 5: Hub loopback tests** (extend the existing two-hub test file):
  - `video_frames_fragment_and_cross_hubs`: same two-hub rig; session A sends
    one `CapturedVideo { keyframe: true, ts_ms: 7, data: vec![0xCD; 5000] }`
    (≥4 fragments); assert session B's `video_out.recv()` yields a
    `PeerVideo` with `peer == key_a`, `keyframe == true`, `ts_ms == 7`, and
    the exact 5000 bytes. Assert a second frame (delta, different fill)
    also crosses intact.
  - `lost_fragment_triggers_keyframe_request`: wire the A→B pump to DROP the
    first datagram whose decoded video header has `frag_index == 1` (peek:
    `frame[1] == Service::Video as u8` on the plane header, then
    `chat::video::decode_fragment(&frame[12..])`). Send frame 0 (drops a
    fragment), then frame 1 (completes). Assert B's `video_out` yields ONLY
    frame 1's bytes, and A's `control_out.recv()` yields
    `CallControlOut::KeyframeRequest` (B's hub asked A's encoder to sync).
  - `beacons_cross_as_peer_state`: session A's `control_in` gets
    `CallControlIn::Beacon { muted: false, camera_on: true }`; assert B's
    `control_out.recv()` yields a matching `PeerBeacon { peer: key_a, .. }`.

Run: `cargo test -p node-bin voice` → all PASS (plus the pre-existing audio test).

- [ ] **Step 6: Commit** — `feat(node): hub video path — fragment/reassemble camera frames + call control over the plane`

---

### Task 5: The video mesh lane — `CHANNEL_VIDEO = 8`

**Files:**
- Modify: `bin/node/src/main.rs` (channel const, validator wiring, both black-hole modes)
- Modify: `bin/node/src/voice.rs` (`ChannelTransport` routes outbound by service byte; `spawn_hub` returns two outbound receivers)

**Interfaces:**
- Consumes: Task 4's hub shape.
- Produces: `spawn_hub(...) -> (voice_out: mpsc::Receiver<VoiceDatagram>, video_out: mpsc::Receiver<VoiceDatagram>, inbound: mpsc::Sender<VoiceDatagram>)`.

- [ ] **Step 1:** `main.rs` const after `CHANNEL_VOICE`:

```rust
/// the video channel: camera-frame fragments between huddle members
/// (`chat::video` fragments inside data-plane datagrams). its own lane so
/// keyframe bursts never queue ahead of voice, with its own per-peer quota
/// sized for the top of the rate ladder plus keyframe bursts.
const CHANNEL_VIDEO: u64 = 8;
```

- [ ] **Step 2:** `ChannelTransport` in `voice.rs` gains a second outbound
lane; route by the plane wire's service byte (offset 1 — stable by the wire
contract this crate co-owns):

```rust
struct ChannelTransport {
    outbound_voice: mpsc::Sender<VoiceDatagram>,
    outbound_video: mpsc::Sender<VoiceDatagram>,
    inbound: tokio::sync::Mutex<mpsc::Receiver<VoiceDatagram>>,
}
```

with `send_datagram` picking the lane:

```rust
        // route by the plane header's service byte: video fragments ride
        // their own mesh lane so a keyframe burst can't queue ahead of voice.
        let lane = if frame.get(1) == Some(&(Service::Video as u8)) {
            &self.outbound_video
        } else {
            &self.outbound_voice
        };
        let _ = lane.try_send((to.0, frame));
```

`spawn_hub` creates both outbound channels (`WIRE_LANE` for voice; `const VIDEO_WIRE_LANE: usize = 512;` for video — ~4 keyframes of fragments) and one shared inbound (both mesh pumps feed it); returns `(outbound_voice_rx, outbound_video_rx, inbound_tx)`. Update the loopback tests' rig: forward BOTH outbound receivers of A into B's inbound sender (and vice versa).

- [ ] **Step 3:** `main.rs` validator section — mirror the voice pumps:

```rust
        {
            let (mut voice_p2p_tx, mut voice_p2p_rx) =
                network.register(CHANNEL_VOICE, quota, MAX_BACKLOG);
            // video's own per-peer quota: 512 × 1372 B ≈ 5.6 Mbps — the
            // 1200 kbps ladder top (~112 fragments/s) plus keyframe bursts.
            let video_quota = Quota::per_second(NZU32!(512));
            let (mut video_p2p_tx, mut video_p2p_rx) =
                network.register(CHANNEL_VIDEO, video_quota, MAX_BACKLOG);
            let (mut voice_egress, mut video_egress, media_ingress) =
                voice::spawn_hub(call_requests);
            // …existing voice_egress/voice_ingress pumps unchanged (inbound
            // sender cloned)…
            context.child("video_egress").spawn(move |_ctx| async move {
                while let Some((to, frame)) = video_egress.recv().await {
                    let Ok(key) = ed25519::PublicKey::decode(&to[..]) else { continue };
                    let _ = video_p2p_tx.send(Recipients::One(key), IoBuf::from(frame), false);
                }
            });
            let video_ingress = media_ingress.clone();
            context.child("video_ingress").spawn(move |_ctx| async move {
                while let Ok((peer, bytes)) = video_p2p_rx.recv().await {
                    let mut raw = [0u8; 32];
                    raw.copy_from_slice(peer.as_ref());
                    let _ = video_ingress.try_send((raw, bytes.into()));
                }
            });
        }
```

- [ ] **Step 4:** Both black-hole modes (sync-only observer ~line 4114, parked joiner ~line 4279): next to each `blackhole_voice`, add the identical drain for `CHANNEL_VIDEO` as `blackhole_video` (register with `quota`, spawn the drain loop).

- [ ] **Step 5:** `cargo test -p node-bin` (loopback tests still green through the two-lane rig) and `cargo test -p node-bin --test cluster_e2e` (proves registration is safe in every mesh mode; slow — run once here).
- [ ] **Step 6: Commit** — `feat(node): CHANNEL_VIDEO mesh lane — video fragments ride their own quota`

---

### Task 6: `/v1/call/ws` — the typed webview bridge (replaces `/v1/voice/ws`)

**Files:**
- Modify: `bin/noded/src/lib.rs` (endpoint + framing; delete `voice_ws`/`voice_session`/`VoiceControl`/`VoiceParams`)
- Modify: `bin/noded/tests/router.rs` (if it touches the voice route)

**Interfaces:**
- Consumes: Task 4's `CallSession` ends.
- Produces — the WS wire the app (Task 7) speaks, on `GET /v1/call/ws?channel=<id>`:
  - binary, first byte tags: `0x01` audio (both directions, exactly `PCM_FRAME_BYTES` after the tag); client→server `0x02` captured video `[0x02][flags u8][ts_ms u32 LE][vp8 chunk]`; server→client `0x03` peer video `[0x03][flags u8][ts_ms u32 LE][peer 32 raw][vp8 chunk]`; flags bit 0 = keyframe.
  - text JSON, client→server: `{"type":"recipients","peers":[hex…]}` · `{"type":"beacon","muted":bool,"cameraOn":bool}` · `{"type":"keyframeRequest","peer":"hex"}`; server→client: `{"type":"keyframeRequest"}` · `{"type":"peerBeacon","peer":"hex","muted":bool,"cameraOn":bool}` · `{"type":"rateHint","maxKbps":n}`.
  - refusals stay #178-shaped: one text frame explaining why, then close; no hub = 503 at upgrade.

- [ ] **Step 1:** Constants + serde types in `bin/noded/src/lib.rs` (replacing `VoiceControl`):

```rust
/// binary ws frame tags on /v1/call/ws (first byte).
const WS_TAG_AUDIO: u8 = 0x01;
const WS_TAG_VIDEO_CAPTURED: u8 = 0x02; // client → server
const WS_TAG_VIDEO_PEER: u8 = 0x03; // server → client
const WS_VIDEO_CAPTURED_HEADER: usize = 6; // tag + flags + ts_ms
const WS_VIDEO_PEER_HEADER: usize = 38; // tag + flags + ts_ms + peer key
const WS_FLAG_KEYFRAME: u8 = 0b0000_0001;

/// client → server control messages on the call socket (text frames).
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CallClientControl {
    /// replace the fan-out set with these hex node keys (self excluded —
    /// the client tracks the consensus huddle roster).
    Recipients { peers: Vec<String> },
    /// this client's ephemeral state; the hub beacons it to peers at 1 Hz.
    Beacon { muted: bool, camera_on: bool },
    /// the decoder lost sync with `peer` — ask it for a keyframe.
    KeyframeRequest { peer: String },
}

/// server → client control messages on the call socket (text frames).
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CallServerControl {
    /// a peer lost sync with US: encode the next frame as a keyframe.
    KeyframeRequest,
    /// a peer's 1 Hz beacon (ephemeral presence/state — never consensus).
    PeerBeacon { peer: String, muted: bool, camera_on: bool },
    /// send at no more than this (min across peers' loss reports).
    RateHint { max_kbps: u32 },
}
```

- [ ] **Step 2:** Route `.route("/v1/call/ws", get(call_ws))` (drop the voice
route); `call_ws` mirrors `voice_ws` (503 without a hub lane, empty-channel
400). `call_session` pumps all seven session ends:

```rust
async fn call_session(mut socket: WebSocket, call: CallLane, channel_id: String) {
    // …open via CallSessionRequest exactly like today's voice_session,
    // refusals answered as one text frame…
    let CallSession {
        pcm_in, mut mixed_out, recipients,
        video_in, mut video_out, control_in, mut control_out,
    } = session;
    loop {
        tokio::select! {
            inbound = socket.recv() => match inbound {
                Some(Ok(Message::Binary(bytes))) => match bytes.first() {
                    Some(&WS_TAG_AUDIO) if bytes.len() == 1 + PCM_FRAME_BYTES => {
                        let frame: Vec<i16> = bytes[1..]
                            .chunks_exact(2)
                            .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
                            .collect();
                        let _ = pcm_in.try_send(frame);
                    }
                    Some(&WS_TAG_VIDEO_CAPTURED)
                        if bytes.len() > WS_VIDEO_CAPTURED_HEADER =>
                    {
                        let _ = video_in.try_send(CapturedVideo {
                            keyframe: bytes[1] & WS_FLAG_KEYFRAME != 0,
                            ts_ms: u32::from_le_bytes(
                                bytes[2..6].try_into().expect("4 bytes"),
                            ),
                            data: bytes[WS_VIDEO_CAPTURED_HEADER..].to_vec(),
                        });
                    }
                    _ => {} // unknown/short frame — drop, stay alive
                },
                Some(Ok(Message::Text(text))) => {
                    match serde_json::from_str::<CallClientControl>(&text) {
                        Ok(CallClientControl::Recipients { peers }) => {
                            let keys: Vec<[u8; 32]> = peers
                                .iter()
                                .filter_map(|hex| files::from_hex_32(hex))
                                .collect();
                            let _ = recipients.send(keys);
                        }
                        Ok(CallClientControl::Beacon { muted, camera_on }) => {
                            let _ = control_in
                                .try_send(CallControlIn::Beacon { muted, camera_on });
                        }
                        Ok(CallClientControl::KeyframeRequest { peer }) => {
                            if let Some(key) = files::from_hex_32(&peer) {
                                let _ = control_in
                                    .try_send(CallControlIn::KeyframeRequest { peer: key });
                            }
                        }
                        Err(_) => {} // unknown control — ignore, stay alive
                    }
                }
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                Some(Ok(_)) => {}
            },
            mixed = mixed_out.recv() => match mixed {
                Some(frame) => {
                    let mut bytes = Vec::with_capacity(1 + frame.len() * 2);
                    bytes.push(WS_TAG_AUDIO);
                    for sample in frame {
                        bytes.extend_from_slice(&sample.to_le_bytes());
                    }
                    if socket.send(Message::Binary(bytes.into())).await.is_err() { break; }
                }
                None => break, // replaced by a newer join
            },
            video = video_out.recv() => match video {
                Some(frame) => {
                    let mut bytes =
                        Vec::with_capacity(WS_VIDEO_PEER_HEADER + frame.data.len());
                    bytes.push(WS_TAG_VIDEO_PEER);
                    bytes.push(if frame.keyframe { WS_FLAG_KEYFRAME } else { 0 });
                    bytes.extend_from_slice(&frame.ts_ms.to_le_bytes());
                    bytes.extend_from_slice(&frame.peer);
                    bytes.extend_from_slice(&frame.data);
                    if socket.send(Message::Binary(bytes.into())).await.is_err() { break; }
                }
                None => break,
            },
            control = control_out.recv() => match control {
                Some(out) => {
                    let message = match out {
                        CallControlOut::KeyframeRequest => CallServerControl::KeyframeRequest,
                        CallControlOut::PeerBeacon { peer, muted, camera_on } => {
                            CallServerControl::PeerBeacon {
                                peer: hex::encode(peer), muted, camera_on,
                            }
                        }
                        CallControlOut::RateHint { max_kbps } => {
                            CallServerControl::RateHint { max_kbps }
                        }
                    };
                    let text = serde_json::to_string(&message).expect("serializable control");
                    if socket.send(Message::Text(text.into())).await.is_err() { break; }
                }
                None => break,
            },
        }
    }
}
```

(If `hex::encode` isn't already a noded dependency, use the repo's existing
hex helper — the inverse of `files::from_hex_32` — check `files` for a
`to_hex` before adding a crate.)

- [ ] **Step 3:** Audio framing changed (a tag byte now precedes PCM both
directions) — this is deliberate: app and node ship lockstep, nothing external
speaks the old endpoint. Grep for `/v1/voice/ws` across the repo (`rg -l 'voice/ws'`)
and update every reference (docs, tests, app comes in Task 7).

- [ ] **Step 4:** `cargo test -p noded` + `cargo build -p node-bin` → green.
- [ ] **Step 5: Commit** — `feat(noded): /v1/call/ws — typed audio+video+control bridge (replaces /v1/voice/ws)`

---

### Task 7: App domain — call frames codec + call session (WebCodecs)

**Files:**
- Create: `app/src/domain/call-frames.ts` (pure, tested)
- Create: `app/src/domain/call-session.ts` (supersedes `createVoiceSession`)
- Modify: `app/src/domain/voice-session.ts` (keep ONLY the pure helpers `floatToPcm16`/`pcm16ToFloat`/`huddleRecipients` + constants; move the audio graph into call-session; update the header comment)
- Modify: `app/src/domain/transport.ts` (`voiceSocketUrl` → `callSocketUrl` pointing at `/v1/call/ws`)
- Test: `app/src/domain/call-frames.test.ts` (new), `app/src/domain/voice-session.test.ts` (keeps passing — pure helpers unchanged)

**Interfaces:**
- Consumes: Task 6's WS wire.
- Produces (used by Task 8):

```ts
// call-frames.ts
export const WS_TAG_AUDIO = 0x01, WS_TAG_VIDEO_CAPTURED = 0x02, WS_TAG_VIDEO_PEER = 0x03;
export const encodeAudioFrame: (pcm: Int16Array) => ArrayBuffer;
export const encodeCapturedVideo: (keyframe: boolean, tsMs: number, data: Uint8Array) => ArrayBuffer;
export type ServerBinaryFrame =
  | { kind: "audio"; pcm: Int16Array }
  | { kind: "video"; peer: string; keyframe: boolean; tsMs: number; data: Uint8Array };
export const decodeServerFrame: (buf: ArrayBuffer) => ServerBinaryFrame | null;

// call-session.ts
export type CallEvent =
  | { kind: "status"; status: VoiceStatus }
  | { kind: "peerBeacon"; peer: string; muted: boolean; cameraOn: boolean; atMs: number };
export interface CallSession {
  start(wsUrl: string): void;
  setRecipients(hexKeys: string[]): void;
  setMuted(muted: boolean): void;
  setCamera(on: boolean): void;          // async acquire inside; beacons on change
  bindTile(peerHex: string, canvas: HTMLCanvasElement | null): void;
  bindPreview(video: HTMLVideoElement | null): void;
  stop(): void;
}
export const supportsVideoCalls: () => boolean;
export const createCallSession: (onEvent: (event: CallEvent) => void) => CallSession;
export const MAX_VIDEO_PARTICIPANTS = 8;
```

- [ ] **Step 1: `call-frames.ts` + failing tests first** (`call-frames.test.ts`): audio round-trip (tag + LE bytes); captured-video encode layout (`[0x02][flags][tsMs LE u32][data]`, keyframe flag set/clear); `decodeServerFrame` parses an audio frame, parses a peer-video frame (peer hex lowercased from 32 raw bytes at offset 6), returns `null` on unknown tags and short frames. Implementation is direct `DataView` code:

```ts
// The binary framing of /v1/call/ws — mirrors bin/noded/src/lib.rs
// (WS_TAG_*). Little-endian on this leg; the mesh leg is BE and node-side.

export const WS_TAG_AUDIO = 0x01;
export const WS_TAG_VIDEO_CAPTURED = 0x02;
export const WS_TAG_VIDEO_PEER = 0x03;
const WS_FLAG_KEYFRAME = 0x01;
const CAPTURED_HEADER = 6;
const PEER_HEADER = 38;

export const encodeAudioFrame = (pcm: Int16Array): ArrayBuffer => {
  const out = new Uint8Array(1 + pcm.length * 2);
  out[0] = WS_TAG_AUDIO;
  out.set(new Uint8Array(pcm.buffer, pcm.byteOffset, pcm.byteLength), 1);
  return out.buffer;
};

export const encodeCapturedVideo = (
  keyframe: boolean,
  tsMs: number,
  data: Uint8Array,
): ArrayBuffer => {
  const out = new Uint8Array(CAPTURED_HEADER + data.length);
  const view = new DataView(out.buffer);
  out[0] = WS_TAG_VIDEO_CAPTURED;
  out[1] = keyframe ? WS_FLAG_KEYFRAME : 0;
  view.setUint32(2, tsMs >>> 0, true);
  out.set(data, CAPTURED_HEADER);
  return out.buffer;
};

const toHex = (bytes: Uint8Array): string =>
  Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");

export type ServerBinaryFrame =
  | { kind: "audio"; pcm: Int16Array }
  | { kind: "video"; peer: string; keyframe: boolean; tsMs: number; data: Uint8Array };

export const decodeServerFrame = (buf: ArrayBuffer): ServerBinaryFrame | null => {
  const bytes = new Uint8Array(buf);
  if (bytes.length < 1) return null;
  if (bytes[0] === WS_TAG_AUDIO && (bytes.length - 1) % 2 === 0 && bytes.length > 1) {
    return { kind: "audio", pcm: new Int16Array(buf, 1 - 1 + 1) as Int16Array };
  }
  if (bytes[0] === WS_TAG_VIDEO_PEER && bytes.length > PEER_HEADER) {
    const view = new DataView(buf);
    return {
      kind: "video",
      keyframe: (bytes[1] & WS_FLAG_KEYFRAME) !== 0,
      tsMs: view.getUint32(2, true),
      peer: toHex(bytes.subarray(6, 38)),
      data: bytes.slice(PEER_HEADER),
    };
  }
  return null;
};
```

⚠️ `new Int16Array(buf, 1)` throws (unaligned offset) — the audio branch must
COPY: `const body = bytes.slice(1); return { kind: "audio", pcm: new Int16Array(body.buffer, 0, body.length / 2) };`
Write the test to catch exactly this (odd 1-byte tag offset).

- [ ] **Step 2: `call-session.ts`.** Move the audio-graph code from
`voice-session.ts` verbatim (worklets, mute semantics, status transitions) and
add: (a) the typed framing — capture callback sends
`encodeAudioFrame(floatToPcm16(frame))`; `onmessage` binary goes through
`decodeServerFrame` (audio → playback worklet; video → per-peer decode); text
frames parse as server control (`keyframeRequest` → force next encode
keyframe; `rateHint` → `encoder.configure` at the new bitrate; `peerBeacon` →
`onEvent`); a refusal BEFORE `live` status still means failure (keep #178's
failed-flag logic — but note text frames are now also control: only treat text
as refusal while status ≠ live). (b) camera:

```ts
  // ── camera / encode ─────────────────────────────────────
  let camStream: MediaStream | null = null;
  let camVideo: HTMLVideoElement | null = null; // hidden frame source + preview
  let encoder: VideoEncoder | null = null;
  let previewEl: HTMLVideoElement | null = null;
  let cameraOn = false;
  let forceKeyframe = true; // first frame, and on server keyframeRequest
  let framesSinceKey = 0;
  let bitrateKbps = 800; // starting rung; rateHint moves it
  const KEYFRAME_INTERVAL = 300; // ~10 s at 30 fps safety net

  const configureEncoder = () => {
    encoder?.configure({
      codec: "vp8",
      width: 1280,
      height: 720,
      bitrate: bitrateKbps * 1000,
      framerate: 30,
      latencyMode: "realtime",
    });
    forceKeyframe = true;
  };

  const startCamera = async () => {
    camStream = await navigator.mediaDevices.getUserMedia({
      video: { width: { ideal: 1280 }, height: { ideal: 720 }, frameRate: { ideal: 30 } },
    });
    encoder = new VideoEncoder({
      output: (chunk) => {
        if (!socket || socket.readyState !== WebSocket.OPEN) return;
        const data = new Uint8Array(chunk.byteLength);
        chunk.copyTo(data);
        socket.send(
          encodeCapturedVideo(chunk.type === "key", Math.round(chunk.timestamp / 1000), data),
        );
      },
      error: () => setCamera(false), // encoder death = camera off, session lives
    });
    configureEncoder();
    const video = document.createElement("video");
    video.muted = true;
    video.playsInline = true;
    video.srcObject = camStream;
    await video.play();
    camVideo = video;
    if (previewEl) previewEl.srcObject = camStream;
    const pump = () => {
      if (!cameraOn || !camVideo || !encoder || encoder.state === "closed") return;
      // rVFC is the portable frame source (Chromium + WebKit) — no
      // MediaStreamTrackProcessor dependency.
      camVideo.requestVideoFrameCallback((_now, meta) => {
        if (cameraOn && encoder && encoder.state === "configured" && encoder.encodeQueueSize < 2) {
          const frame = new VideoFrame(camVideo!, {
            timestamp: Math.round(meta.mediaTime * 1_000_000),
          });
          const key = forceKeyframe || framesSinceKey >= KEYFRAME_INTERVAL;
          if (key) { forceKeyframe = false; framesSinceKey = 0; } else { framesSinceKey += 1; }
          encoder.encode(frame, { keyFrame: key });
          frame.close();
        }
        pump();
      });
    };
    pump();
  };
```

`setCamera(on)`: idempotent; enabling awaits `startCamera()` (failure →
camera stays off, `sendBeacon()` not sent, surface nothing fatal); disabling
closes encoder, stops camera tracks, clears preview `srcObject`, sends beacon.
Every camera/mute change sends `{"type":"beacon","muted","cameraOn"}` (and
`setMuted` keeps the existing local-forwarding-stop semantics). (c) decode:

```ts
  // ── per-peer decode / tiles ─────────────────────────────
  interface PeerPipe {
    decoder: VideoDecoder;
    canvas: HTMLCanvasElement | null;
    awaitingKey: boolean;
    lastRequestMs: number;
  }
  const pipes = new Map<string, PeerPipe>();

  const requestPeerKeyframe = (peerHex: string, pipe: PeerPipe) => {
    const now = Date.now();
    if (now - pipe.lastRequestMs < 1000) return; // ≥1 s, mirroring the hub
    pipe.lastRequestMs = now;
    if (socket?.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify({ type: "keyframeRequest", peer: peerHex }));
    }
  };

  const pipeFor = (peerHex: string): PeerPipe => {
    let pipe = pipes.get(peerHex);
    if (pipe) return pipe;
    const created: PeerPipe = {
      canvas: tileBindings.get(peerHex) ?? null,
      awaitingKey: true,
      lastRequestMs: 0,
      decoder: new VideoDecoder({
        output: (frame) => {
          const canvas = created.canvas;
          if (canvas) {
            canvas.width = frame.displayWidth;
            canvas.height = frame.displayHeight;
            canvas.getContext("2d")?.drawImage(frame, 0, 0);
          }
          frame.close();
        },
        error: () => {
          created.awaitingKey = true;
          requestPeerKeyframe(peerHex, created);
        },
      }),
    };
    created.decoder.configure({ codec: "vp8" });
    pipes.set(peerHex, created);
    return created;
  };

  const onPeerVideo = (peer: string, keyframe: boolean, tsMs: number, data: Uint8Array) => {
    const pipe = pipeFor(peer);
    if (pipe.awaitingKey && !keyframe) {
      requestPeerKeyframe(peer, pipe); // deltas are useless until a sync point
      return;
    }
    pipe.awaitingKey = false;
    pipe.decoder.decode(
      new EncodedVideoChunk({
        type: keyframe ? "key" : "delta",
        timestamp: tsMs * 1000,
        data,
      }),
    );
  };
```

`bindTile(peerHex, canvas)` stores into `tileBindings: Map<string, HTMLCanvasElement>`
and updates any existing pipe; `bindPreview(video)` stores `previewEl` and
attaches `camStream` if live. `stop()` additionally closes every decoder, the
encoder, and camera tracks. (d) capability:

```ts
export const supportsVideoCalls = (): boolean =>
  typeof VideoEncoder !== "undefined" &&
  typeof VideoDecoder !== "undefined" &&
  typeof (HTMLVideoElement.prototype as { requestVideoFrameCallback?: unknown })
    .requestVideoFrameCallback === "function" &&
  !!navigator.mediaDevices?.getUserMedia;
```

- [ ] **Step 3:** `transport.ts`: replace `voiceSocketUrl` with

```ts
export const callSocketUrl = (baseUrl: string, channel: string): string =>
  `${wsBase(baseUrl)}/v1/call/ws?channel=${encodeURIComponent(channel)}`;
```

- [ ] **Step 4:** `cd app && bun run test && bun run typecheck` → green (vitest covers call-frames + the surviving voice-session pure helpers; the session itself is browser-runtime, exercised in Task 10).
- [ ] **Step 5: Commit** — `feat(app): call session — typed /v1/call/ws framing + WebCodecs camera pipeline`

---

### Task 8: App store — call slice, camera/sweep actions, sweep client op

**Files:**
- Modify: `app/src/domain/chat-client.ts` (`sweepHuddle` op)
- Modify: `app/src/console/store/state.ts` (voice slice grows `cameraOn` + `peers`)
- Modify: `app/src/console/store/optimistic.ts` (`huddleSwept`)
- Modify: `app/src/console/store/actions.ts` (session swap + new actions)
- Test: `app/src/domain/chat-client.test.ts`, `app/src/console/store/optimistic.test.ts`

**Interfaces:**
- Consumes: Task 7's `createCallSession`/`callSocketUrl`/`supportsVideoCalls`/`MAX_VIDEO_PARTICIPANTS`, Task 3's op JSON.
- Produces (used by Task 9): state `voice: { channelId, muted, status, cameraOn, peers: Record<nodeHex, { muted: boolean; cameraOn: boolean; atMs: number }> }`; actions `setCamera(on: boolean)`, `sweepHuddle(channelId: string, user: number[])`; selector-ish helper `actions.videoSupported(): boolean`.

- [ ] **Step 1: Failing tests.** `chat-client.test.ts`: `sweepHuddle` submits
`{ sweep_huddle: { channel_id, user } }` to target `chat` (mirror the
join/leave test shape). `optimistic.test.ts`: `huddleSwept` prunes exactly the
swept user from the channel's roster and leaves others (mirror `huddleLeft`'s
test, but keyed by the target user's bytes, not self node).

- [ ] **Step 2: Implement client + optimistic.** `chat-client.ts` next to `leaveHuddle`:

```ts
/** Evict a (stale) huddle member — consensus cleanup for clients that died
 *  without leaving. Gated server-side like posting. */
export const sweepHuddle = (
  live: LiveTransport,
  params: { channelId: string; user: number[]; origin: string },
): Promise<SubmitReceipt> =>
  submit(
    live,
    { sweep_huddle: { channel_id: params.channelId, user: params.user } },
    params.origin,
  );
```

(match `joinHuddle`'s ACTUAL submit helper/signature in the file — the shape
above tracks the module op; the wrapper idiom is whatever `leaveHuddle` uses.)
`optimistic.ts`: `huddleSwept(prev, channelId, userKeyHex)` maps channels,
filters the roster by `keyHex(m.user) !== userKeyHex` — same structure as
`huddleLeft` (which filters by node/self).

- [ ] **Step 3: State.** `state.ts` `VoiceSlice` gains:

```ts
  /** Local camera state (ephemeral, beaconed to peers — never consensus). */
  cameraOn: boolean;
  /** Per-peer ephemeral call state from 1 Hz beacons, keyed by NODE hex.
   *  Staleness (no beacon for >10 s) drives the sweep affordance. */
  peers: Record<string, { muted: boolean; cameraOn: boolean; atMs: number }>;
```

initial value `{ channelId: null, muted: false, status: "idle", cameraOn: false, peers: {} }` (update every construction site the compiler flags).

- [ ] **Step 4: Actions.** In `actions.ts`:
  - swap imports: `createCallSession`/`CallSession`/`CallEvent`/`supportsVideoCalls`/`MAX_VIDEO_PARTICIPANTS` from `call-session`, `callSocketUrl` from transport.
  - the session variable becomes `CallSession`; `createCallSession((event) => …)` maps `status` events exactly as today's `onStatus` (including the leave-reconciliation), and `peerBeacon` events into the slice:

```ts
      if (event.kind === "peerBeacon") {
        update((prev) => ({
          voice: {
            ...prev.voice,
            peers: {
              ...prev.voice.peers,
              [event.peer]: { muted: event.muted, cameraOn: event.cameraOn, atMs: event.atMs },
            },
          },
        }));
        return;
      }
```

  - join/leave/stop paths reset `cameraOn: false, peers: {}` alongside the existing slice reset.
  - new actions on the returned object:

```ts
    setCamera: (on) => {
      if (!voice) return;
      if (on && !supportsVideoCalls()) return; // capability-gated UI should prevent this
      const channel = getState().channels.find((c) => c.id === getState().voice.channelId);
      if (on && (channel?.huddle?.length ?? 0) > MAX_VIDEO_PARTICIPANTS) return;
      voice.setCamera(on);
      update((prev) => ({ voice: { ...prev.voice, cameraOn: on } }));
    },
    videoSupported: () => supportsVideoCalls(),
    sweepHuddle: (channelId, user) => {
      submitOp(
        opKey.huddle(channelId),
        (live) => chatClient.sweepHuddle(live, { channelId, user, origin: getState().author }),
        (prev) => optimistic.huddleSwept(prev, channelId, keyHex(user)),
      );
    },
```

(match `submitOp`'s real name/shape from the join/leave code, and add the three
signatures to the actions interface with doc comments in the file's voice section.)
  - `bindTile`/`bindPreview` need to reach the session from components: expose
    `getCallSession: () => CallSession | null` on actions (one-liner returning
    the module-level `voice` variable).

- [ ] **Step 5:** `bun run test && bun run typecheck` → green.
- [ ] **Step 6: Commit** — `feat(app): call store — camera state, peer beacons, huddle sweep`

---

### Task 9: UI — camera toggle + tile grid + stale-peer sweep

**Files:**
- Modify: `app/src/console/views/chat/Huddle.tsx` (dock grows a camera toggle + the tile grid)
- Modify: `app/src/console/layout/ConsoleShell.tsx` (only if the grid mounts outside the dock — prefer keeping it all inside `HuddleDock`)

**Interfaces:**
- Consumes: Task 8's slice/actions.

- [ ] **Step 1: Camera toggle** in `HuddleDock`, matching the mute button's
idiom (a `CameraGlyph` SVG next to `MicGlyph`, same 30×28 `HoverButton`):
hidden entirely when `!actions.videoSupported()` (WebKitGTK: roster+audio
only, the ADR's soft-dependency posture — `title` on the dock explains
"Video needs a Chromium-based window"); disabled with a reason title when
`roster.length > 8` ("Video is capped at 8 participants"); active style mirrors
the un-muted mic (accent when on). `onClick: () => actions.setCamera(!voice.cameraOn)`.

- [ ] **Step 2: Tiles.** Inside `HuddleDock`, when `voice.cameraOn || roster.some((m) => state.voice.peers[keyHex(m.node)]?.cameraOn)`, render a tile grid ABOVE the dock's header row (the dock stays one card, grows upward — `maxWidth` ~340, tiles in a 2-col CSS grid):

```tsx
function PeerTile({ member, names }: { member: HuddleMember; names: Record<string, string> }) {
  const { state, actions } = useDucktape();
  const nodeHex = keyHex(member.node);
  const beacon = state.voice.peers[nodeHex];
  const name = memberName(member, names);
  const stale = !beacon || Date.now() - beacon.atMs > STALE_BEACON_MS;
  const canvasRef = (canvas: HTMLCanvasElement | null) =>
    actions.getCallSession()?.bindTile(nodeHex, canvas);
  return (
    <div style={tileFrame}>
      {beacon?.cameraOn ? (
        <canvas ref={canvasRef} style={{ width: "100%", height: "100%", objectFit: "cover", borderRadius: radius.sm }} />
      ) : (
        <div style={tileIdle}><ParticipantAvatar name={name} size={34} ring={color.sunken} /></div>
      )}
      <span style={tileName}>
        {name}
        {beacon?.muted !== false && <MicGlyph size={10} muted />}
      </span>
      {stale && (
        <HoverButton
          onClick={() => actions.sweepHuddle(state.voice.channelId!, member.user)}
          title={`No signal from ${name} — remove from huddle`}
          style={staleChip}
          hoverStyle={{ filter: "brightness(1.08)" }}
        >
          stale · remove
        </HoverButton>
      )}
    </div>
  );
}
```

with `const STALE_BEACON_MS = 10_000;`, a self tile (`<video muted playsInline ref={(el) => actions.getCallSession()?.bindPreview(el)} />` shown while `voice.cameraOn`), a 1 s `setInterval` re-render tick inside the tiles block (staleness is time-driven), and tiles only for roster members whose node ≠ self (self gets the preview tile). Skip beacons/tiles entirely when not in the huddle (`state.voice.channelId !== channel.id` never renders — the dock already gates this).

- [ ] **Step 3:** Roster members on the SAME node share one beacon key — key
tiles by `keyHex(member.user)` (unique) while looking up beacons by node hex;
accept the shared-beacon limitation with a one-line comment (matches the
recipients dedup posture).

- [ ] **Step 4:** `bun run typecheck && bun run build` green; visual check
comes in Task 10's live QA.
- [ ] **Step 5: Commit** — `feat(app): call surface — camera toggle, video tiles, stale-peer sweep`

---

### Task 10: Docs + full verification + live e2e

**Files:**
- Create: `docs/superpowers/specs/2026-07-06-video-call-build.md` (what shipped vs the ADR: WS-bridge gateway instead of str0m SDP + why; flow-label strings; `sweep_huddle` naming; roster cap 32 with video gate 8; Chromium companion window + macOS/Windows entitlements plumbing deferred; screenshare/recording/simulcast still later)
- Modify: `docs/adr/2026-07-06-video-call-module.md` (status line: built — pointer to the spec; note the gateway deviation in one sentence)

- [ ] **Step 1:** Write both docs.
- [ ] **Step 2:** Full suites, from the worktree root: `cargo test --workspace` (the `daemon_e2e` port-collision flake is known — rerun once if it alone fails) and `cd app && bun run test && bun run typecheck && bun run build`.
- [ ] **Step 3: Live e2e** (mirrors #178's verification): bring up a 2-validator cluster (`ops/dev.sh` — read its usage first), run two `bun run dev` previews pointed at the two nodes, drive two headless Chromium instances with `--use-fake-device-for-media-stream --use-fake-ui-for-media-stream --autoplay-policy=no-user-gesture-required` (the fake device renders a rolling test pattern). Assert: (a) both join the same huddle, rosters converge via consensus; (b) enable camera on A → B's tile canvas paints non-blank pixels (sample `getImageData` variance via CDP/screenshot); (c) beacons drive A's mute badge on B within ~2 s; (d) kill A's browser mid-call → B sees A stale within ~15 s and the sweep affordance appears; sweep → roster shrinks on consensus. Capture evidence (screenshots + console notes) in the PR body.
- [ ] **Step 4: Commit docs** — `docs(spec): video-call build — what shipped vs the ADR`; push branch; open PR against `dev` titled `feat(chat): video in huddles — Service::Video wire + WebCodecs camera over the mesh`, body = what/why/deviations/verification evidence.

---

## Self-Review Notes

- ADR §2 wire commitments — Tasks 1–2 (service id, header, ctl semantics, ladder). §1 ops — #178 shipped join/leave; Task 3 adds sweep. §3 admission — hub `ActiveFlows` posture carried over (documented client-side-roster limitation stands, spec notes it). §4 gateway — deliberately replaced by the shipped WS seam (recorded in Task 10's spec + ADR note). §5 platforms — capability gating ships; companion window deferred (spec). §6 slice order — slices 1–3 built, 4 partially (gating), 0 remains the sim/mesh-channel transport #178 chose.
- Types cross-checked: `CapturedVideo`/`PeerVideo`/`CallControlIn`/`CallControlOut` (Tasks 4↔6), WS tags and layouts (Tasks 6↔7), `sweep_huddle` JSON (Tasks 3↔8), slice/actions names (Tasks 8↔9).
- Known intentional deviations from the ADR text, to record in the spec: flow-label strings (`video-channel:`/`callctl-channel:` following #178's `voice-channel:` rather than the ADR's illustrative `chat/call/{id}/…`), huddle roster cap 32 (shipped) with video gated at 8, control-plane events on the call socket instead of `/v1/ws` `WsFrame::Call`.
