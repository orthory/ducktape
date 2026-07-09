# Huddle PR-B — Real Video in the Floating Window (session handoff) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:executing-plans (inline) — the media lifecycle is sequential and shares one worktree. Steps use `- [ ]` tracking.

**Goal:** Give the popped-out huddle window a real, live video surface by handing the media session to it — the window runs its own `CallSession` (WS + mic + camera + decode) while the main window keeps consensus; popping in/out transfers media ownership with a bounded (~sub-second) audio reconnect.

**Architecture:** The popped window becomes a **satellite media client**. Main pushes *context* (nodeUrl, channelId, capability, raw roster, authorNames, selfNodeHex, seed mute) over `ducktape://huddle-context`; the window owns a `CallSession` + its own ephemeral state (peers/muted/camera/speaking/status) via a `useHuddleWindowSession` controller, and renders the same `CallSurface` pieces (CallTiles + HuddleControls + HuddleCard) as the dock. On pop-out main `stopVoice()`s (media only, roster intact) then the window starts its session; on pop-in/close the window stops and main re-takes. A `media-failed` fallback re-takes media in main so pop-out can never strand the call.

**Tech Stack:** React 18 + TS, Tauri v2 events (`@tauri-apps/api/event`), `createCallSession`/`callSocketUrl` (domain), Vitest. `bun run typecheck` + `bun run test` in `app/`.

## Global Constraints

- The consensus roster is ALWAYS owned by the main window (it holds the node subscription + chatClient). The window never submits consensus ops itself — it emits commands and main submits.
- Only ONE `CallSession` (one WS) may exist per node at a time. Handoff order is mandatory: the releasing side `stop()`s before the taking side `start()`s.
- The window mounts under `React.StrictMode` and is OUTSIDE `DucktapeProvider`. A `CallSession` cannot restart after `stop()`. Therefore the controller MUST `createCallSession` **inside** the effect and `stop()` in its cleanup (fresh instance per mount), never reuse a stopped instance.
- Joining/taking media starts MUTED (never a hot-mic moment), matching `actions.joinHuddle`.
- Camera control renders only when `videoCapability.canEncode`; peer canvas only when `canDecode && beacon.cameraOn`. Same gates as PR-A.
- App gates green before each commit: `bun run typecheck`, `bun run test`. Rust gate for huddle.rs: `cargo clippy -p ducktape --tests --no-deps` (bin crate) — build only, no fmt sweep.
- Verification limit: the real cross-webview handoff (getUserMedia in the child webview, live WS, WebCodecs) is UNVERIFIABLE headless on this box (no mic, no VP8 encoder). Unit-test the pure protocol + the controller against a STUBBED CallSession; flag the live path for real-hardware QA. This PR likely lands OPEN pending that QA.

## File Structure

- **Modify** `app/src/console/store/huddle-window.ts` — replace the `huddle-state`(participants) push with a `huddle-context`(raw roster + node context) push; keep `huddle-cmd` and extend it (`toggle-mute`/`toggle-camera` become window-local so they are NOT here; add `media-failed`). Keep pure builders/mappers exported + unit-tested.
- **Create** `app/src/console/views/huddle/useHuddleWindowSession.ts` — the satellite controller hook: owns a `CallSession` + ephemeral state, seeded by context; exposes participants/status/muted/cameraOn + setMuted/setCamera + leave/sweep/retry emitters. Testable with an injected session factory.
- **Modify** `app/src/console/views/huddle/HuddleWindow.tsx` — consume the controller; render `CallTiles` (gallery) + `HuddleCard` (roster) + `HuddleControls` with a live camera button; header pop-in.
- **Modify** `app/src/console/store/actions.ts` — `popOutHuddle` releases main media (`stopVoice`, keep channelId/consensus); `popInHuddle` (and the `huddle-closed` handler) re-takes media (re-create + start the session); a `retakeHuddleMedia` helper; `media-failed` path re-takes in main.
- **Modify** `app/src/console/store/DucktapeProvider.tsx` — push `huddle-context` instead of `huddle-state`; on `ready` replay context; map the new/kept commands; on `huddle-closed`/`media-failed` re-take.
- **Modify** `app/src-tauri/src/huddle.rs` — grow the window to a video size (e.g. 380×300), `resizable(true)` with a min size; keep always-on-top + skip-taskbar.
- **Tests**: `huddle-window.test.ts` (context builder + cmd mapper round-trip), `useHuddleWindowSession.test.tsx` (lifecycle against a stub session), and an action-level test that pop-out/in preserves `channelId` and emits no consensus leave.

---

### Task 1: Protocol — `huddle-context` push + command set

**Files:** Modify `app/src/console/store/huddle-window.ts`; Modify `app/src/console/store/huddle-window.test.ts`.

**Interfaces — Produces:**
```ts
export const HUDDLE_CONTEXT_EVENT = "ducktape://huddle-context"; // main → window
export const HUDDLE_CMD_EVENT = "ducktape://huddle-cmd";         // window → main
export const HUDDLE_CLOSED_EVENT = "ducktape://huddle-closed";   // Rust → main

/** Everything the window needs to RUN its own session + render. Raw roster (not
 *  pre-projected) — the window owns beacons now, so it does buildParticipants
 *  itself with its own peers. */
export interface HuddleContext {
  channelName: string;
  channelId: string;
  nodeUrl: string;
  selfNodeHex: string;
  canEncode: boolean;
  canDecode: boolean;
  authorNames: Record<string, string>;
  roster: HuddleMember[];   // channel.huddle
  seedMuted: boolean;       // main's mute at handoff (window starts from here)
}

export type HuddleWindowCmd =
  | { op: "ready" }
  | { op: "leave" }
  | { op: "retry" }                     // main re-issues nothing; window restarts its own session — retry is window-local, so this op is only for a TERMINAL rejoin request
  | { op: "sweep"; user: number[] }
  | { op: "media-failed" };             // the window could not start/keep a session → main re-takes ownership

export const buildHuddleContext = (
  voice: VoiceSlice, channels: Channel[], authorNames: Record<string, string>,
  nodeUrl: string, selfNodeHex: string, cap: VideoCapability,
): HuddleContext | null; // null when not huddling

export const applyHuddleWindowCmd = (
  cmd: HuddleWindowCmd,
  actions: { leaveHuddle(): void; sweepHuddle(c: string, u: number[]): void; retakeHuddleMedia(): void },
  channelId: string | null,
): void; // leave→leaveHuddle; sweep→sweepHuddle; media-failed→retakeHuddleMedia; ready handled by wiring
```

- [ ] **Step 1 — failing test** (`huddle-window.test.ts`): assert `buildHuddleContext` returns null when `voice.channelId` is null; returns the channel name / roster / nodeUrl / caps when huddling; and `applyHuddleWindowCmd({op:"media-failed"},…)` calls `retakeHuddleMedia`, `{op:"leave"}` calls `leaveHuddle`, `{op:"sweep",user}` calls `sweepHuddle(channelId,user)` (and is a no-op when channelId null).
- [ ] **Step 2** run `cd app && bun run test -- huddle-window` → FAIL (new exports missing).
- [ ] **Step 3** implement: drop `buildHuddleWindowState`/`HuddleWindowState`; add `HuddleContext` + `buildHuddleContext` (pulls `channel.huddle`, `voice.muted` as seed) + the new `HuddleWindowCmd` + `applyHuddleWindowCmd`. Keep `openHuddleWindow`/`closeHuddleWindow`.
- [ ] **Step 4** run test → PASS.
- [ ] **Step 5** (no commit yet — consumers in Tasks 2–5 still reference old names; commit at the Task 5 green boundary.)

---

### Task 2: The satellite session controller

**Files:** Create `app/src/console/views/huddle/useHuddleWindowSession.ts`; Create `app/src/console/views/huddle/useHuddleWindowSession.test.tsx`.

**Interfaces — Produces:**
```ts
export interface WindowSessionView {
  channelName: string;
  status: VoiceStatus;            // from the window's own CallEvent stream
  error: VoiceError | null;
  muted: boolean;
  cameraOn: boolean;
  canEncode: boolean;
  canDecode: boolean;
  participants: HuddleParticipant[]; // buildParticipants(roster, own peers, …, now)
  peers: Record<string, PeerBeacon>;
  memberNodes: Record<string, string>;
  setMuted(m: boolean): void;
  setCamera(on: boolean): void;
  retry(): void;                 // restart the window's own session
  bindPreview(el: HTMLVideoElement | null): void;
  bindTile(nodeHex: string, el: HTMLCanvasElement | null): void;
}
// Injectable factory so tests pass a stub (default: the real createCallSession).
export function useHuddleWindowSession(
  ctx: HuddleContext | null,
  onMediaEnded: (reason: "closed" | "error") => void,
  makeSession: (cb: (e: CallEvent) => void) => CallSession = createCallSession,
): WindowSessionView | null;
```
Behavior: while `ctx` is non-null, an effect (keyed by `ctx.channelId` + `ctx.nodeUrl`) `makeSession(onEvent)` → `setMuted(seedMuted)` → `start(callSocketUrl(nodeUrl, channelId))` → `setRecipients(huddleRecipients(roster, selfNodeHex))`; cleanup `stop()`s. Roster changes re-push recipients (dedup by value). `onEvent` maps peerBeacon→peers, selfSpeaking→speaking, status live/connecting→status, and status `closed`/`error`→`onMediaEnded(reason)` + status. A 1 Hz tick drives staleness. `setMuted`/`setCamera` proxy the session + local state. `retry` bumps a nonce that re-runs the effect (fresh session — never restart a stopped one). Participants via `buildParticipants({roster, peers, selfNodeHex, authorNames, selfMuted:muted, selfSpeaking:speaking, sessionStartMs, now})`.

- [ ] **Step 1 — failing test**: with a stub session (records calls), mount the hook with a context; assert `start` was called with `callSocketUrl(nodeUrl, channelId)` and `setMuted(true)` when `seedMuted`; firing the stub's `peerBeacon` cb surfaces the peer in `participants`; `setCamera(true)` proxies to the stub; unmount calls `stop()`; a stub `status:"error"` event calls `onMediaEnded("error")`. Use `@testing-library/react` `renderHook`.
- [ ] **Step 2** run → FAIL (module missing).
- [ ] **Step 3** implement the hook per the behavior above. Create-session-inside-effect (StrictMode-safe).
- [ ] **Step 4** run → PASS.
- [ ] **Step 5** no commit yet.

---

### Task 3: `HuddleWindow` renders the live surface

**Files:** Modify `app/src/console/views/huddle/HuddleWindow.tsx`.

Consume `useHuddleWindowSession(ctx, onMediaEnded)`. `ctx` arrives via `listen(HUDDLE_CONTEXT_EVENT)`; send `{op:"ready"}` on mount so main replays it, and re-render on later context pushes (roster changes). `onMediaEnded` emits `{op:"media-failed"}` (main re-takes + closes the window). Render: a header row (channel + a pop-in icon → `getCurrentWindow().close()`), `CallTiles layout="gallery"` (or the roster-only `HuddleCard` when no video/participants), and `HuddleControls size="comfortable"` with a real camera button (`canEncode`, `onToggleCamera`→`view.setCamera`), `onToggleMute`→`view.setMuted`, `onLeave`→`send({op:"leave"})`, `onRetry`→`view.retry()`. Sweep via the `HuddleCard` roster → `send({op:"sweep",user})`.

- [ ] **Step 1** rewrite HuddleWindow to the above (no new standalone test — covered by the controller test + manual QA). Keep the `connecting…` placeholder until `ctx` + first status arrive.
- [ ] **Step 2** `bun run typecheck` — expect errors only in DucktapeProvider/actions until Tasks 4–5.

---

### Task 4: Main-side media release / re-take (`actions.ts`)

**Files:** Modify `app/src/console/store/actions.ts`.

Add to the actions object + `ConsoleActions` type:
- `popOutHuddle()`: if in a huddle → `openHuddleWindow()`, `stopVoice()` (release WS/mic/camera — NO submitLeaveHuddle, channelId stays), `update({voice:{…popped:true}})`. Main's dock is hidden while popped; consensus membership + roster untouched.
- `popInHuddle()`: `closeHuddleWindow()`, then `retakeHuddleMedia()`.
- `retakeHuddleMedia()`: if `voice.channelId` set and no live session → `voice = createCallSession(onCallEvent); voice.setMuted(true); voice.start(callSocketUrl(nodeUrl, channelId)); pushRecipients();` and `update({voice:{…popped:false, muted:true, cameraOn:false, peers:{}, sessionStartMs:Date.now(), speaking:false, status:"connecting"}})`. (Re-take always rejoins muted/camera-off — safe default; the fresh session reconnects the dock's canvases.)

Guard: `retakeHuddleMedia` is idempotent (no-op if a session already exists). `onCallEvent`'s existing close/error reconciliation is unchanged (it still `submitLeaveHuddle` on terminal end — correct whether media is main-side or freshly re-taken).

- [ ] **Step 1 — failing test** (extend an actions/store test): calling `popOutHuddle()` then `popInHuddle()` keeps `voice.channelId` unchanged and submits NO `leave_huddle` op (spy the chatClient). (Use the existing store test harness pattern.)
- [ ] **Step 2** run → FAIL.
- [ ] **Step 3** implement.
- [ ] **Step 4** run → PASS.

---

### Task 5: Bridge rewrite + green the tree (`DucktapeProvider.tsx`)

**Files:** Modify `app/src/console/store/DucktapeProvider.tsx`.

Sender half: while `voice.popped && isTauri()`, build `buildHuddleContext(voice, channels, authorNames, nodeUrl, selfNodeHex, videoCapability)` and `emit(HUDDLE_CONTEXT_EVENT, ctx)` on change (fingerprint dedupe over roster+names+caps; a 1 Hz tick is NO LONGER needed here — staleness now lives in the window). Receiver half: on `HUDDLE_CMD_EVENT` `ready` → replay context; else `applyHuddleWindowCmd(cmd, {leaveHuddle, sweepHuddle, retakeHuddleMedia}, channelId)`. On `HUDDLE_CLOSED_EVENT` → `actions.popInHuddle()` (which re-takes media). Remove the old participants push + its stale tick.

- [ ] **Step 1** implement the sender/receiver rewrite.
- [ ] **Step 2 — full green**: `cd app && bun run typecheck && bun run test`. Fix any dangling references to the removed `HuddleWindowState`/`buildHuddleWindowState`.
- [ ] **Step 3 — commit** Tasks 1–5 as one boundary:
```bash
git add app/src/console/store/huddle-window.ts app/src/console/store/huddle-window.test.ts \
        app/src/console/views/huddle/useHuddleWindowSession.ts app/src/console/views/huddle/useHuddleWindowSession.test.tsx \
        app/src/console/views/huddle/HuddleWindow.tsx \
        app/src/console/store/actions.ts app/src/console/store/DucktapeProvider.tsx
git commit -m "feat(huddle): window owns a real media session (video-in-window handoff)"
```

---

### Task 6: Grow the pop-out window for video (`huddle.rs`)

**Files:** Modify `app/src-tauri/src/huddle.rs`.

Bump `WIDTH`/`HEIGHT` to a video size (e.g. 380 × 300), add `.min_inner_size(300.0, 220.0)` and `.resizable(true)`; keep `.always_on_top(true).skip_taskbar(true)`. No logic change.

- [ ] **Step 1** edit the constants + builder.
- [ ] **Step 2** gate: `cargo clippy -p ducktape --tests --no-deps` (from repo root) — expect clean (or only pre-existing warnings unrelated to this file).
- [ ] **Step 3 — commit**:
```bash
git add app/src-tauri/src/huddle.rs
git commit -m "feat(huddle): size the pop-out window for a video surface"
```

---

## Self-Review

**Spec coverage (PR-B):** window hosts the full surface with live video (Tasks 2–3); media session moves between webviews with main release / re-take (Task 4); roster/names still flow main→window (Task 1+5); `media-failed` fallback re-takes so pop-out never strands the call (Tasks 2,4,5); window chrome grows + stays always-on-top (Task 6). Handoff ordering (release-before-take) is enforced by pop-out stopping main media before the window's effect starts, and pop-in stopping the window (unmount) before `retakeHuddleMedia`.

**Placeholder scan:** none — interfaces + behaviors are concrete; UI steps reference the exact props from PR-A's shared components.

**Type consistency:** `HuddleContext` (Task 1) is consumed by `useHuddleWindowSession` (Task 2), `HuddleWindow` (Task 3), and the bridge (Task 5); `retakeHuddleMedia` (Task 4) is referenced by `applyHuddleWindowCmd` (Task 1) and the bridge (Task 5); `WindowSessionView` (Task 2) is consumed by Task 3.

**Risks / verification limits:** the live cross-webview media path (getUserMedia in the child webview, real WS, WebCodecs) is UNVERIFIABLE on this headless/no-encoder box — only the pure protocol + the controller-against-a-stub + the pop-out/in action wiring are unit-tested. StrictMode double-mount is handled by creating the session inside the effect. Real-hardware QA (macOS + a Linux box with an encoder) is required before merge; this PR is expected to land OPEN with that risk noted.
