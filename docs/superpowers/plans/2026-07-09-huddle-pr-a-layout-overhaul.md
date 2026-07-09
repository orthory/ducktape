# Huddle PR-A — Layout Overhaul Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Collapse the three divergent huddle surfaces (dock / stage / popped window) onto one shared media control bar (`HuddleControls`) and one shared tile renderer (`CallTiles`), fixing the scattered-button layout — no behavior change beyond layout.

**Architecture:** Two new presentational components — `HuddleControls` (the bottom media bar: `[Mic] [Camera] [Screen] [⋯ Devices] · · · [Leave]`, error → `[Retry] · · · [Leave]`) and `CallTiles` (one renderer for `strip | gallery | spotlight`, wrapping the shared `StageTile`). View controls (expand / collapse / pop) live in each container's header, not the media bar. Dock, stage, and the audio card + popped window are rewired onto these; the old per-surface renderers are deleted.

**Tech Stack:** React 18 + TypeScript, inline-style design tokens (`console/theme/tokens`), `HoverButton` primitive, Vitest + @testing-library/react. Package manager: `bun` (run `bun run typecheck|test|lint` in `app/`).

## Global Constraints

- Screen + Devices controls are **slots only** in this PR: when their callback prop is omitted the control is not rendered. PR-C/PR-D fill them. The button ORDER must already reserve their position (`[Mic] [Camera] [Screen] [Devices] … [Leave]`).
- Media controls (mic / camera / screen / devices / leave) live in `HuddleControls`; view controls (expand / collapse / pop-out / pop-in) live in the container header. Do not mix them.
- Destructive **Leave** is the last item in the bar, danger-styled, separated by a growing spacer (`marginLeft: auto`) so it never sits flush against Mute.
- Camera control renders only when `canEncode` is true (VP8 encoder present) — never a dead control. Peer tiles render a `<canvas>` only when `canDecode` is true, else the initials avatar (no black tiles). See `domain/video-capability.ts`.
- Accessible name of an icon-only `HoverButton` is its `title`. Tests query `getByRole("button", { name: /…/i })` against the title text — every control must set a `title` containing the queried word.
- No `cargo fmt --all`; no consensus/protocol changes; app-only PR.
- Per-crate/app gates green before each commit: `bun run typecheck`, `bun run test`, `bun run lint` (in `app/`).

---

## File Structure

- **Create** `app/src/console/views/huddle/HuddleControls.tsx` — the shared bottom media bar (presentational; props + callbacks, no store).
- **Create** `app/src/console/views/huddle/HuddleControls.test.tsx` — button-order / visibility / disabled matrix (RTL).
- **Create** `app/src/console/views/huddle/CallTiles.tsx` — the shared tile renderer (`strip | gallery | spotlight`) + exported `StageTile`. Uses `huddle-stage-layout.ts` (`galleryColumns`, `spotlightKey`).
- **Create** `app/src/console/views/huddle/CallTiles.test.tsx` — layout selection + tile-source (self/peer/avatar) + overflow.
- **Modify** `app/src/console/views/chat/HuddleCard.tsx` — reduce to the audio body (header without pop icon + banners + roster); remove its control row and pop icon; drop `onSetMuted`/`onLeave`/`onRetry`/`onPopOut`/`onPopIn` props.
- **Modify** `app/src/console/views/chat/HuddleCard.test.tsx` — update for the slimmed body.
- **Modify** `app/src/console/views/chat/Huddle.tsx` — dock header gets expand/pop icons; body = `HuddleCard` + `CallTiles layout="strip"`; bottom = `HuddleControls`. Delete `TileGrid`, `SelfTile`, `PeerTile`, and the old dock-controls row.
- **Modify** `app/src/console/views/huddle/HuddleStage.tsx` — header keeps the gallery/spotlight toggle + collapse + pop; tiles = `CallTiles`; bottom = `HuddleControls size="comfortable"`. Delete its inline `StageTile` and control bar.
- **Modify** `app/src/console/views/huddle/HuddleStage.test.tsx` — update queries for the shared bar.
- **Modify** `app/src/console/views/huddle/HuddleWindow.tsx` — render `HuddleCard` (audio body) + `HuddleControls home="window"` (mute / leave / pop-in; no camera — the window owns no session in this PR).

`StageTile` moves out of `HuddleStage.tsx` into `CallTiles.tsx` (single owner). `memberNodeOf` (user-key → node-hex) is replaced by a `memberNodes: Record<string,string>` map the container passes in, so `CallTiles` stays store-free and unit-testable.

---

### Task 1: `HuddleControls` — the shared media bar

**Files:**
- Create: `app/src/console/views/huddle/HuddleControls.tsx`
- Test: `app/src/console/views/huddle/HuddleControls.test.tsx`

**Interfaces:**
- Consumes: `HoverButton` (`views/chat/HoverButton`), tokens (`accentVar, color, font, radius` from `console/theme/tokens`), `HuddleStatus` (`views/chat/HuddleCard`).
- Produces:
  ```ts
  export type ControlHome = "dock" | "stage" | "window";
  export interface HuddleControlsProps {
    size: "compact" | "comfortable";
    status: HuddleStatus;              // "idle" | "connecting" | "live" | "error"
    muted: boolean;
    cameraOn: boolean;
    canEncode: boolean;                // camera control shown only when true
    cameraDisabledReason?: string;     // non-empty → camera disabled + this tooltip
    sharing?: boolean;                 // PR-C
    canScreenShare?: boolean;          // PR-C — screen control shown only when true
    onToggleScreen?: () => void;       // PR-C — omit → screen control hidden
    onOpenDevices?: () => void;        // PR-D — omit → devices control hidden
    onToggleMute: () => void;
    onToggleCamera?: () => void;       // omit → camera control hidden
    onLeave: () => void;
    onRetry: () => void;
    home: ControlHome;                 // reserved; the bar itself has no view controls
  }
  export function HuddleControls(props: HuddleControlsProps): JSX.Element;
  ```
  Order when `status !== "error"`: `[Mic] [Camera?] [Screen?] [Devices?]` then a `flex:1` spacer then `[Leave]`. When `status === "error"`: `[Retry]` spacer `[Leave]`. Mic/Camera/Screen disabled when `status !== "live"`. `home` is accepted for symmetry with later PRs but the bar renders no expand/pop controls (those are header-owned).

- [ ] **Step 1: Write the failing test**

```tsx
// app/src/console/views/huddle/HuddleControls.test.tsx
import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { HuddleControls } from "./HuddleControls";
import type { HuddleControlsProps } from "./HuddleControls";

const base: HuddleControlsProps = {
  size: "compact",
  status: "live",
  muted: false,
  cameraOn: false,
  canEncode: true,
  onToggleMute: vi.fn(),
  onToggleCamera: vi.fn(),
  onLeave: vi.fn(),
  onRetry: vi.fn(),
  home: "dock",
};

const order = () =>
  screen.getAllByRole("button").map((b) => b.getAttribute("title") ?? "");

describe("HuddleControls", () => {
  it("orders mic, camera, then Leave last with no screen/devices by default", () => {
    render(<HuddleControls {...base} />);
    const titles = order();
    const mic = titles.findIndex((t) => /mute/i.test(t));
    const cam = titles.findIndex((t) => /camera/i.test(t));
    const leave = titles.findIndex((t) => /leave/i.test(t));
    expect(mic).toBeGreaterThanOrEqual(0);
    expect(cam).toBeGreaterThan(mic);
    expect(leave).toBe(titles.length - 1);
    expect(titles.some((t) => /screen/i.test(t))).toBe(false);
    expect(titles.some((t) => /device/i.test(t))).toBe(false);
  });

  it("hides the camera control when the box cannot encode", () => {
    render(<HuddleControls {...base} canEncode={false} />);
    expect(screen.queryByRole("button", { name: /camera/i })).toBeNull();
  });

  it("reserves the screen slot between camera and devices when both are enabled", () => {
    render(
      <HuddleControls
        {...base}
        canScreenShare
        onToggleScreen={vi.fn()}
        onOpenDevices={vi.fn()}
      />,
    );
    const titles = order();
    const cam = titles.findIndex((t) => /camera/i.test(t));
    const screenIdx = titles.findIndex((t) => /screen/i.test(t));
    const dev = titles.findIndex((t) => /device/i.test(t));
    expect(cam).toBeLessThan(screenIdx);
    expect(screenIdx).toBeLessThan(dev);
  });

  it("shows Retry (not Mute) in the error state, Leave still last", () => {
    render(<HuddleControls {...base} status="error" />);
    expect(screen.getByRole("button", { name: /retry/i })).toBeTruthy();
    expect(screen.queryByRole("button", { name: /mute/i })).toBeNull();
    const titles = order();
    expect(/leave/i.test(titles[titles.length - 1])).toBe(true);
  });

  it("disables mic and camera when not live", () => {
    render(<HuddleControls {...base} status="connecting" />);
    expect(screen.getByRole("button", { name: /mute/i })).toBeDisabled();
    expect(screen.getByRole("button", { name: /camera/i })).toBeDisabled();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd app && bun run test -- HuddleControls`
Expected: FAIL — `Cannot find module "./HuddleControls"`.

- [ ] **Step 3: Write minimal implementation**

```tsx
// app/src/console/views/huddle/HuddleControls.tsx
// The huddle's single media control bar — shared by the dock, the full stage,
// and the popped-out window so the button set can never drift between surfaces.
// It owns ONLY the in-call media controls: mic, camera, screen-share (PR-C),
// a devices menu (PR-D), and the destructive Leave (isolated at the far right).
// View controls (expand / collapse / pop) belong to each container's header.
// Purely presentational: props + callbacks, no store.

import type { CSSProperties } from "react";

import type { HuddleStatus } from "../chat/HuddleCard";
import { accentVar, color, font, radius } from "../../theme/tokens";
import { HoverButton } from "../chat/HoverButton";

export type ControlHome = "dock" | "stage" | "window";

export interface HuddleControlsProps {
  size: "compact" | "comfortable";
  status: HuddleStatus;
  muted: boolean;
  cameraOn: boolean;
  canEncode: boolean;
  cameraDisabledReason?: string;
  sharing?: boolean;
  canScreenShare?: boolean;
  onToggleScreen?: () => void;
  onOpenDevices?: () => void;
  onToggleMute: () => void;
  onToggleCamera?: () => void;
  onLeave: () => void;
  onRetry: () => void;
  home: ControlHome;
}

function MicGlyph({ size = 15, muted = false }: { size?: number; muted?: boolean }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.7} strokeLinecap="round" strokeLinejoin="round">
      <rect x="9" y="3" width="6" height="11" rx="3" />
      <path d="M5.5 11a6.5 6.5 0 0 0 13 0" />
      <path d="M12 17.5V21" />
      {muted && <path d="M4 4l16 16" strokeWidth={1.9} />}
    </svg>
  );
}
function CameraGlyph({ size = 15, off = false }: { size?: number; off?: boolean }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.7} strokeLinecap="round" strokeLinejoin="round">
      <rect x="2.5" y="6.5" width="12" height="11" rx="2.2" />
      <path d="M14.5 10.5l6-3v9l-6-3z" />
      {off && <path d="M4 4l16 16" strokeWidth={1.9} />}
    </svg>
  );
}
function ScreenGlyph({ size = 15, on = false }: { size?: number; on?: boolean }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.7} strokeLinecap="round" strokeLinejoin="round">
      <rect x="3" y="4" width="18" height="12" rx="1.8" />
      <path d="M8 20h8" />
      {on && <path d="M12 8v5M9.5 10.5L12 8l2.5 2.5" />}
    </svg>
  );
}
function DevicesGlyph({ size = 15 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.9} strokeLinecap="round" strokeLinejoin="round">
      <circle cx="5" cy="12" r="1.4" /><circle cx="12" cy="12" r="1.4" /><circle cx="19" cy="12" r="1.4" />
    </svg>
  );
}

export function HuddleControls({
  size,
  status,
  muted,
  cameraOn,
  canEncode,
  cameraDisabledReason,
  sharing = false,
  canScreenShare = false,
  onToggleScreen,
  onOpenDevices,
  onToggleMute,
  onToggleCamera,
  onLeave,
  onRetry,
}: HuddleControlsProps) {
  const live = status === "live";
  const failed = status === "error";
  const comfortable = size === "comfortable";
  const h = comfortable ? 36 : 28;
  const pad = comfortable ? "0 12px" : "0 10px";
  const gap = comfortable ? 10 : 8;

  const btn = (extra: CSSProperties): CSSProperties => ({
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    gap: 6,
    height: h,
    minWidth: h,
    padding: pad,
    borderRadius: radius.md,
    border: `1px solid ${color.borderSoft}`,
    background: color.sunken,
    color: color.inkSoft,
    font: `600 ${comfortable ? 12 : 11.5}px ${font.sans}`,
    ...extra,
  });

  const leaveBtn: CSSProperties = {
    ...btn({}),
    marginLeft: "auto",
    background: color.danger,
    color: "#fff",
    border: "1px solid transparent",
  };

  return (
    <div style={{ display: "flex", alignItems: "center", gap }}>
      {failed ? (
        <HoverButton onClick={onRetry} title="Retry" style={btn({})} hoverStyle={{ background: color.hover, color: color.ink }}>
          Retry
        </HoverButton>
      ) : (
        <>
          <HoverButton
            onClick={onToggleMute}
            title={muted ? "Unmute" : "Mute"}
            disabled={!live}
            style={btn(
              muted
                ? { background: color.dangerSoft, color: color.danger, border: `1px solid ${color.dangerBorder}` }
                : live
                  ? { background: color.dark, color: color.onDark }
                  : { opacity: 0.55 },
            )}
            hoverStyle={{ filter: "brightness(1.05)" }}
          >
            <MicGlyph size={comfortable ? 16 : 15} muted={muted} />
            {comfortable && <span>{muted ? "Muted" : "Mute"}</span>}
          </HoverButton>

          {canEncode && onToggleCamera && (
            <HoverButton
              onClick={onToggleCamera}
              title={cameraDisabledReason ?? (cameraOn ? "Turn camera off" : "Turn camera on")}
              disabled={!live || !!cameraDisabledReason}
              style={btn(cameraOn ? { background: accentVar, color: color.onDark, border: "1px solid transparent" } : {})}
              hoverStyle={{ filter: "brightness(1.05)" }}
            >
              <CameraGlyph size={comfortable ? 16 : 15} off={!cameraOn} />
              {comfortable && <span>{cameraOn ? "Camera on" : "Camera"}</span>}
            </HoverButton>
          )}

          {canScreenShare && onToggleScreen && (
            <HoverButton
              onClick={onToggleScreen}
              title={sharing ? "Stop screen share" : "Share screen"}
              disabled={!live}
              style={btn(sharing ? { background: accentVar, color: color.onDark, border: "1px solid transparent" } : {})}
              hoverStyle={{ filter: "brightness(1.05)" }}
            >
              <ScreenGlyph size={comfortable ? 16 : 15} on={sharing} />
              {comfortable && <span>{sharing ? "Sharing" : "Screen"}</span>}
            </HoverButton>
          )}

          {onOpenDevices && (
            <HoverButton
              onClick={onOpenDevices}
              title="Devices"
              style={btn({})}
              hoverStyle={{ background: color.hover, color: color.ink }}
            >
              <DevicesGlyph size={comfortable ? 16 : 15} />
            </HoverButton>
          )}
        </>
      )}

      <HoverButton onClick={onLeave} title="Leave huddle" style={leaveBtn} hoverStyle={{ filter: "brightness(1.06)" }}>
        {comfortable ? "Leave" : "Leave"}
      </HoverButton>
    </div>
  );
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd app && bun run test -- HuddleControls`
Expected: PASS (5 tests).

- [ ] **Step 5: Typecheck + commit**

Run: `cd app && bun run typecheck`
Expected: no errors.

```bash
git add app/src/console/views/huddle/HuddleControls.tsx app/src/console/views/huddle/HuddleControls.test.tsx
git commit -m "feat(huddle): shared HuddleControls media bar (mic/cam/screen/devices/leave)"
```

---

### Task 2: `CallTiles` — the shared tile renderer

**Files:**
- Create: `app/src/console/views/huddle/CallTiles.tsx`
- Test: `app/src/console/views/huddle/CallTiles.test.tsx`

**Interfaces:**
- Consumes: `HuddleParticipant` (`store/huddle-roster`), `PeerBeacon` (`store/huddle-roster`), `galleryColumns`/`spotlightKey` (`./huddle-stage-layout`), tokens, `keyHex` is NOT needed (containers pass resolved maps).
- Produces:
  ```ts
  export interface CallTilesProps {
    layout: "strip" | "gallery" | "spotlight";
    participants: HuddleParticipant[];  // resolved rows, roster order, self included
    memberNodes: Record<string, string>; // participant.key (user hex) → node hex
    peers: Record<string, PeerBeacon>;   // node hex → beacon
    canEncode: boolean;                  // self preview gate
    canDecode: boolean;                  // peer canvas gate
    selfCameraOn: boolean;
    bindPreview: (el: HTMLVideoElement | null) => void;         // self <video>
    bindTile: (nodeHex: string, el: HTMLCanvasElement | null) => void; // peer <canvas>
    maxTiles?: number;                   // strip cap; overflow → "+N more not shown"
    pinned?: string | null;              // spotlight: participant.key to feature
    onPin?: (key: string) => void;       // spotlight/gallery double-click
  }
  export function CallTiles(props: CallTilesProps): JSX.Element;
  // Also exports StageTile for the stage's filmstrip cells if needed:
  export function StageTile(props: {
    member: HuddleParticipant; nodeHex?: string; beacon?: PeerBeacon;
    canEncode: boolean; canDecode: boolean; selfCameraOn: boolean; big: boolean;
    bindPreview: (el: HTMLVideoElement | null) => void;
    bindTile: (nodeHex: string, el: HTMLCanvasElement | null) => void;
    onPin?: () => void;
  }): JSX.Element;
  ```
  Tile source: self → `<video>` when `canEncode && selfCameraOn`; peer → `<canvas>` when `canDecode && beacon?.cameraOn`; else the initials avatar. `strip` = 2-col grid capped at `maxTiles` with a `+N more not shown` tail; `gallery` = `galleryColumns(n)` grid; `spotlight` = big `pinned`/`spotlightKey` tile + a horizontal filmstrip of the rest.

- [ ] **Step 1: Write the failing test**

```tsx
// app/src/console/views/huddle/CallTiles.test.tsx
import { describe, expect, it, vi } from "vitest";
import { render } from "@testing-library/react";
import { CallTiles } from "./CallTiles";
import type { HuddleParticipant } from "../../store/huddle-roster";

const p = (over: Partial<HuddleParticipant> & { key: string }): HuddleParticipant => ({
  name: over.key.toUpperCase(),
  muted: false,
  stale: false,
  isSelf: false,
  speaking: false,
  user: [1],
  ...over,
});

const common = {
  memberNodes: { self: "nself", ab: "nab" } as Record<string, string>,
  peers: { nab: { muted: false, cameraOn: true, atMs: 0 } },
  canEncode: true,
  canDecode: true,
  selfCameraOn: true,
  bindPreview: vi.fn(),
  bindTile: vi.fn(),
};

describe("CallTiles", () => {
  it("renders the self preview <video> when the self camera is on and encode is available", () => {
    const { container } = render(
      <CallTiles layout="gallery" participants={[p({ key: "self", isSelf: true })]} {...common} />,
    );
    expect(container.querySelector("video")).not.toBeNull();
  });

  it("renders a peer <canvas> only when decode is available and the beacon says camera-on", () => {
    const { container, rerender } = render(
      <CallTiles layout="gallery" participants={[p({ key: "ab" })]} {...common} />,
    );
    expect(container.querySelector("canvas")).not.toBeNull();
    rerender(<CallTiles layout="gallery" participants={[p({ key: "ab" })]} {...common} canDecode={false} />);
    expect(container.querySelector("canvas")).toBeNull();
  });

  it("caps a strip and surfaces the overflow tail", () => {
    const many = Array.from({ length: 10 }, (_, i) => p({ key: `u${i}` }));
    const { getByText } = render(
      <CallTiles layout="strip" participants={many} {...common} maxTiles={4} />,
    );
    expect(getByText(/\+6 more not shown/i)).toBeTruthy();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd app && bun run test -- CallTiles`
Expected: FAIL — `Cannot find module "./CallTiles"`.

- [ ] **Step 3: Write minimal implementation**

```tsx
// app/src/console/views/huddle/CallTiles.tsx
// The single huddle tile renderer, shared by the dock (compact "strip"), the
// full stage ("gallery" / "spotlight"), and the popped window. One StageTile
// implementation replaces the dock's old TileGrid and the stage's inline tiles,
// so the video surface can never drift between them. Store-free: the container
// resolves participants → node hex (memberNodes) and passes the session's
// bindPreview/bindTile so this file never touches the store.

import type { CSSProperties } from "react";

import type { HuddleParticipant, PeerBeacon } from "../../store/huddle-roster";
import { color, font, radius } from "../../theme/tokens";
import { galleryColumns, spotlightKey } from "./huddle-stage-layout";

const initialsOf = (name: string): string => name.slice(0, 2).toUpperCase();

export interface CallTilesProps {
  layout: "strip" | "gallery" | "spotlight";
  participants: HuddleParticipant[];
  memberNodes: Record<string, string>;
  peers: Record<string, PeerBeacon>;
  canEncode: boolean;
  canDecode: boolean;
  selfCameraOn: boolean;
  bindPreview: (el: HTMLVideoElement | null) => void;
  bindTile: (nodeHex: string, el: HTMLCanvasElement | null) => void;
  maxTiles?: number;
  pinned?: string | null;
  onPin?: (key: string) => void;
}

const media: CSSProperties = { width: "100%", height: "100%", objectFit: "cover", display: "block" };

export function StageTile({
  member,
  nodeHex,
  beacon,
  canEncode,
  canDecode,
  selfCameraOn,
  big,
  bindPreview,
  bindTile,
  onPin,
}: {
  member: HuddleParticipant;
  nodeHex?: string;
  beacon?: PeerBeacon;
  canEncode: boolean;
  canDecode: boolean;
  selfCameraOn: boolean;
  big: boolean;
  bindPreview: (el: HTMLVideoElement | null) => void;
  bindTile: (nodeHex: string, el: HTMLCanvasElement | null) => void;
  onPin?: () => void;
}) {
  const selfVideo = member.isSelf && selfCameraOn && canEncode;
  const peerVideo = !member.isSelf && canDecode && !!beacon?.cameraOn;
  const frame: CSSProperties = {
    position: "relative",
    width: "100%",
    height: "100%",
    minHeight: big ? 0 : 84,
    borderRadius: radius.md,
    overflow: "hidden",
    background: color.dark,
    border: `2px solid ${member.speaking ? color.green : "transparent"}`,
    boxSizing: "border-box",
  };
  return (
    <div style={frame} onDoubleClick={onPin} title={onPin ? "Double-click to spotlight" : undefined}>
      {selfVideo ? (
        <video ref={bindPreview} muted autoPlay playsInline style={media} />
      ) : peerVideo && nodeHex ? (
        <canvas ref={(c) => bindTile(nodeHex, c)} style={media} />
      ) : (
        <div style={{ width: "100%", height: "100%", display: "flex", alignItems: "center", justifyContent: "center" }}>
          <span
            aria-hidden="true"
            style={{
              width: big ? 96 : 40,
              height: big ? 96 : 40,
              borderRadius: "50%",
              background: color.sunken,
              color: color.muted3,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              font: `600 ${big ? 30 : 14}px ${font.sans}`,
            }}
          >
            {initialsOf(member.name)}
          </span>
        </div>
      )}
      <span
        style={{
          position: "absolute",
          left: 6,
          bottom: 6,
          maxWidth: "calc(100% - 12px)",
          display: "inline-flex",
          alignItems: "center",
          gap: 4,
          padding: "2px 7px",
          borderRadius: 999,
          background: "rgba(38,37,31,.62)",
          color: color.onDark,
          font: `600 ${big ? 12 : 10.5}px ${font.sans}`,
        }}
      >
        <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
          {member.isSelf ? "You" : member.name}
        </span>
      </span>
      {member.stale && (
        <span style={{ position: "absolute", top: 6, right: 6, padding: "2px 7px", borderRadius: 999, background: color.danger, color: "#fff", font: `600 9.5px ${font.sans}` }}>
          no signal
        </span>
      )}
    </div>
  );
}

export function CallTiles(props: CallTilesProps) {
  const { layout, participants, memberNodes, peers, maxTiles, pinned, onPin } = props;
  const tile = (member: HuddleParticipant, big: boolean) => {
    const nodeHex = memberNodes[member.key];
    return (
      <StageTile
        key={member.key}
        member={member}
        nodeHex={nodeHex}
        beacon={nodeHex ? peers[nodeHex] : undefined}
        canEncode={props.canEncode}
        canDecode={props.canDecode}
        selfCameraOn={props.selfCameraOn}
        big={big}
        bindPreview={props.bindPreview}
        bindTile={props.bindTile}
        onPin={onPin ? () => onPin(member.key) : undefined}
      />
    );
  };

  if (layout === "strip") {
    const cap = maxTiles ?? participants.length;
    const shown = participants.slice(0, cap);
    const overflow = participants.length - shown.length;
    return (
      <div>
        <div style={{ display: "grid", gridTemplateColumns: "repeat(2, 1fr)", gap: 6 }}>
          {shown.map((m) => (
            <div key={m.key} style={{ aspectRatio: "16 / 9" }}>{tile(m, false)}</div>
          ))}
        </div>
        {overflow > 0 && (
          <div style={{ marginTop: 4, font: `500 10px ${font.sans}`, color: color.muted2 }}>
            +{overflow} more not shown
          </div>
        )}
      </div>
    );
  }

  if (layout === "gallery") {
    const cols = galleryColumns(participants.length);
    return (
      <div style={{ display: "grid", gridTemplateColumns: `repeat(${cols}, 1fr)`, gap: 10, height: "100%", gridAutoRows: "1fr" }}>
        {participants.map((m) => tile(m, false))}
      </div>
    );
  }

  // spotlight
  const spot = spotlightKey(participants, pinned ?? null);
  const spotMember = participants.find((m) => m.key === spot);
  const others = participants.filter((m) => m.key !== spot);
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 10, height: "100%" }}>
      <div style={{ flex: 1, minHeight: 0 }}>{spotMember && tile(spotMember, true)}</div>
      {others.length > 0 && (
        <div style={{ display: "flex", gap: 8, height: 96, flexShrink: 0, overflowX: "auto" }}>
          {others.map((m) => (
            <div key={m.key} style={{ width: 150, flexShrink: 0 }}>{tile(m, false)}</div>
          ))}
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd app && bun run test -- CallTiles`
Expected: PASS (3 tests).

- [ ] **Step 5: Typecheck + commit**

Run: `cd app && bun run typecheck`
Expected: no errors.

```bash
git add app/src/console/views/huddle/CallTiles.tsx app/src/console/views/huddle/CallTiles.test.tsx
git commit -m "feat(huddle): shared CallTiles renderer (strip/gallery/spotlight)"
```

---

### Task 3: Slim `HuddleCard` to the audio body

**Files:**
- Modify: `app/src/console/views/chat/HuddleCard.tsx`
- Modify: `app/src/console/views/chat/HuddleCard.test.tsx`

**Interfaces:**
- Produces (new prop shape — controls removed, header pop icon removed):
  ```ts
  export interface HuddleCardProps {
    channelName: string;
    status: HuddleStatus;
    error: VoiceError | null;
    participants: HuddleParticipant[];
    ring?: string;
    maxRows?: number;
    onSweep?(user: number[]): void;
  }
  ```
  `HuddleStatus` stays exported here (Task 1 + others import it). The component renders: status header (dot + `#channelName` + count/`connecting…`, **no** pop icon), error copy row, the muted-while-talking banner, and the roster. `STATUS_DOT`, `ERROR_COPY`, `Roster`, `RowAvatar` stay. `onSetMuted`/`onLeave`/`onRetry`/`onPopOut`/`onPopIn` and the whole bottom control `<div>` and the `PopGlyph` are deleted (moved to `HuddleControls` / container headers).

- [ ] **Step 1: Update the test first (red)**

Replace control-oriented assertions with body assertions. Full new test file:

```tsx
// app/src/console/views/chat/HuddleCard.test.tsx
import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { HuddleCard } from "./HuddleCard";
import type { HuddleParticipant } from "../../store/huddle-roster";

const p = (over: Partial<HuddleParticipant> & { key: string }): HuddleParticipant => ({
  name: over.key.toUpperCase(),
  muted: false,
  stale: false,
  isSelf: false,
  speaking: false,
  user: [1],
  ...over,
});

describe("HuddleCard (audio body)", () => {
  it("shows the channel name and a member count, and no control buttons", () => {
    render(
      <HuddleCard
        channelName="design"
        status="live"
        error={null}
        participants={[p({ key: "self", isSelf: true }), p({ key: "ab" })]}
      />,
    );
    expect(screen.getByText("#design")).toBeTruthy();
    // controls live in HuddleControls now — the body has no mute/leave buttons.
    expect(screen.queryByRole("button", { name: /leave/i })).toBeNull();
    expect(screen.queryByRole("button", { name: /mute/i })).toBeNull();
  });

  it("warns when the self row is muted while speaking", () => {
    render(
      <HuddleCard
        channelName="design"
        status="live"
        error={null}
        participants={[p({ key: "self", isSelf: true, muted: true, speaking: true })]}
      />,
    );
    expect(screen.getByText(/you.re muted/i)).toBeTruthy();
  });

  it("renders the error copy in the error state", () => {
    render(
      <HuddleCard channelName="design" status="error" error="mic-denied" participants={[]} />,
    );
    expect(screen.getByText(/allow it in System Settings/i)).toBeTruthy();
  });
});
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd app && bun run test -- HuddleCard`
Expected: FAIL — old `HuddleCard` still requires `onSetMuted`/`onLeave`/`onRetry` (type error) or renders buttons.

- [ ] **Step 3: Rewrite `HuddleCard.tsx` to the audio body**

Delete the `PopGlyph`, the `onSetMuted`/`onLeave`/`onRetry`/`onPopOut`/`onPopIn` props, the header pop button, and the entire bottom control `<div>` (mute/retry/leave). Keep `MicGlyph` (used by the banner + roster), `STATUS_DOT`, `ERROR_COPY`, `RowAvatar`, `Roster`. New `HuddleCard`:

```tsx
export interface HuddleCardProps {
  channelName: string;
  status: HuddleStatus;
  error: VoiceError | null;
  participants: HuddleParticipant[];
  ring?: string;
  maxRows?: number;
  onSweep?(user: number[]): void;
}

export function HuddleCard({
  channelName,
  status,
  error,
  participants,
  ring = color.paper,
  maxRows = 5,
  onSweep,
}: HuddleCardProps) {
  const dot = STATUS_DOT[status] ?? STATUS_DOT.idle;
  const failure = status === "error" ? (error ?? "connection") : null;
  const mutedWhileTalking = participants.some((p) => p.isSelf && p.muted && p.speaking);

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
      <div style={{ display: "flex", alignItems: "center", gap: 7, minWidth: 0 }}>
        <span
          aria-label={status}
          style={{ width: 8, height: 8, borderRadius: "50%", background: dot.color, flexShrink: 0, animation: dot.pulse ? "ik-pulse 1s ease-in-out infinite" : undefined }}
        />
        <span style={{ flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", font: `600 12.5px ${font.sans}`, color: color.ink }}>
          #{channelName}
        </span>
        {!failure && (
          <span style={{ font: `500 10.5px ${font.sans}`, color: color.muted2, flexShrink: 0 }}>
            {status === "connecting" ? "connecting…" : `${participants.length}`}
          </span>
        )}
      </div>

      {failure && (
        <span style={{ font: `400 11px/1.4 ${font.sans}`, color: color.danger }}>{ERROR_COPY[failure]}</span>
      )}

      {!failure && mutedWhileTalking && (
        <div style={{ display: "flex", alignItems: "center", gap: 6, padding: "5px 8px", borderRadius: radius.sm, background: color.dangerSoft, border: `1px solid ${color.dangerBorder}`, color: color.danger, font: `600 11px ${font.sans}` }}>
          <MicGlyph size={13} muted />
          You&rsquo;re muted
        </div>
      )}

      {!failure && <Roster participants={participants} ring={ring} maxRows={maxRows} onSweep={onSweep} />}
    </div>
  );
}
```

Remove the now-unused `radius` import only if `radius.sm` is no longer referenced (it still is, in the banner) — keep it. Remove `HoverButton` import if no longer used (it is used by `Roster`'s sweep chip — keep it).

- [ ] **Step 4: Run tests + typecheck**

Run: `cd app && bun run test -- HuddleCard && bun run typecheck`
Expected: HuddleCard tests PASS. Typecheck will now FAIL in `Huddle.tsx` and `HuddleWindow.tsx` (they still pass removed props) — that is fixed in Tasks 4–6. Do not commit yet.

- [ ] **Step 5: Note (no commit)**

Commit happens at the end of Task 6 once all consumers compile again (Tasks 4–6 form one green boundary). This keeps the tree buildable per commit.

---

### Task 4: Rewire the dock (`Huddle.tsx`)

**Files:**
- Modify: `app/src/console/views/chat/Huddle.tsx`

**Interfaces:**
- Consumes: `HuddleCard` (slim body, Task 3), `HuddleControls` (Task 1), `CallTiles` (Task 2), `buildParticipants`, `MAX_VIDEO_PARTICIPANTS`, `keyHex`, tokens, `isTauri`, `useDucktape`.
- Produces: no exported surface change (`HuddleDock`, `HuddleHeaderButton`, `HuddleRailBadge` keep their signatures).

Delete `TileGrid`, `SelfTile`, `PeerTile`, `tileGrid`/`tileFrame`/`tileMedia`/`tileIdle`/`tileName`/`tileNameText` styles, and the `CameraGlyph`/`ExpandGlyph` usages that move. Keep `HeadphonesGlyph` (header button + rail badge). Add a small `ExpandGlyph` + `PopGlyph` for the header view cluster.

- [ ] **Step 1: Replace `HuddleDockCard`'s body**

New `HuddleDockCard` return (header view cluster + audio body + tiles strip + `HuddleControls`):

```tsx
function HuddleDockCard() {
  const { state, actions } = useDucktape();
  const { voice } = state;

  const channel = state.channels.find((c) => c.id === voice.channelId);
  const roster = channel?.huddle ?? [];
  const live = voice.status === "live";
  const canEncode = state.videoCapability.canEncode;
  const canDecode = state.videoCapability.canDecode;
  const overCap = roster.length > MAX_VIDEO_PARTICIPANTS;
  const selfHex = (state.status?.publicKey ?? "").toLowerCase();

  const showTiles =
    voice.cameraOn ||
    (canDecode && roster.some((m) => voice.peers[keyHex(m.node)]?.cameraOn));

  const [nowTick, setNowTick] = useState(() => Date.now());
  useEffect(() => {
    if (voice.popped) return;
    const id = setInterval(() => setNowTick(Date.now()), 1000);
    return () => clearInterval(id);
  }, [voice.popped]);

  const [expanded, setExpanded] = useState(false);

  if (!voice.channelId || voice.popped) return null;
  const channelId = voice.channelId;
  if (expanded) return <HuddleStage onCollapse={() => setExpanded(false)} />;

  const participants = buildParticipants({
    roster,
    peers: voice.peers,
    selfNodeHex: selfHex,
    authorNames: state.authorNames,
    selfMuted: voice.muted,
    selfSpeaking: voice.speaking,
    sessionStartMs: voice.sessionStartMs,
    now: nowTick,
  });
  const memberNodes = Object.fromEntries(roster.map((m) => [keyHex(m.user), keyHex(m.node)]));
  const bindPreview = (el: HTMLVideoElement | null) => actions.getCallSession()?.bindPreview(el);
  const bindTile = (nodeHex: string, el: HTMLCanvasElement | null) => actions.getCallSession()?.bindTile(nodeHex, el);

  const cameraDisabledReason = overCap ? "Video is capped at 8 participants" : undefined;

  return (
    <div
      style={{
        margin: "8px 8px 2px",
        maxWidth: 340,
        padding: "9px 10px",
        borderRadius: radius.md,
        background: color.paper,
        border: `1px solid ${color.borderStrong}`,
        boxShadow: "0 1px 2px rgba(40,38,34,.05)",
        display: "flex",
        flexDirection: "column",
        gap: 8,
      }}
    >
      {/* view cluster in the header row: expand + pop (media controls live below) */}
      <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
        <div style={{ flex: 1, minWidth: 0 }}>
          <HuddleCard
            channelName={channel?.name ?? channelId}
            status={voice.status}
            error={voice.error}
            participants={participants}
            ring={color.paper}
            maxRows={4}
            onSweep={(user) => actions.sweepHuddle(channelId, user)}
          />
        </div>
        <div style={{ display: "flex", gap: 2, flexShrink: 0, alignSelf: "flex-start" }}>
          <HeaderIconButton title="Expand to full stage" onClick={() => setExpanded(true)}>
            <ExpandGlyph size={14} />
          </HeaderIconButton>
          {isTauri() && (
            <HeaderIconButton title="Open in window" onClick={() => actions.popOutHuddle()}>
              <PopGlyph size={13} />
            </HeaderIconButton>
          )}
        </div>
      </div>

      {showTiles && (
        <CallTiles
          layout="strip"
          participants={participants}
          memberNodes={memberNodes}
          peers={voice.peers}
          canEncode={canEncode}
          canDecode={canDecode}
          selfCameraOn={voice.cameraOn}
          bindPreview={bindPreview}
          bindTile={bindTile}
          maxTiles={MAX_VIDEO_PARTICIPANTS}
        />
      )}

      <HuddleControls
        size="compact"
        home="dock"
        status={voice.status}
        muted={voice.muted}
        cameraOn={voice.cameraOn}
        canEncode={canEncode}
        cameraDisabledReason={live ? cameraDisabledReason : undefined}
        onToggleMute={() => actions.setHuddleMuted(!voice.muted)}
        onToggleCamera={() => actions.setCamera(!voice.cameraOn)}
        onLeave={() => actions.leaveHuddle()}
        onRetry={() => actions.joinHuddle(channelId)}
      />
    </div>
  );
}
```

Add the small local helpers `HeaderIconButton`, `ExpandGlyph`, `PopGlyph` (near the other glyphs):

```tsx
function HeaderIconButton({ title, onClick, children }: { title: string; onClick: () => void; children: React.ReactNode }) {
  return (
    <HoverButton
      onClick={onClick}
      title={title}
      style={{ display: "flex", alignItems: "center", justifyContent: "center", width: 24, height: 22, borderRadius: radius.sm, color: color.muted2 }}
      hoverStyle={{ background: color.hover, color: color.ink }}
    >
      {children}
    </HoverButton>
  );
}
function ExpandGlyph({ size = 14 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round">
      <path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5" />
    </svg>
  );
}
function PopGlyph({ size = 13 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round">
      <path d="M9 5H6a2 2 0 0 0-2 2v11a2 2 0 0 0 2 2h11a2 2 0 0 0 2-2v-3" />
      <path d="M13 11l7-7" /><path d="M14.5 4H20v5.5" />
    </svg>
  );
}
```

Update imports: add `CallTiles` from `../huddle/CallTiles`, `HuddleControls` from `../huddle/HuddleControls`; drop `MicGlyph`/`CameraGlyph` if now unused elsewhere in the file (the header button + rail badge only use `HeadphonesGlyph`; the dock's `CameraGlyph` is gone). Keep `useCallback` only if still used — the inline binds above drop it; remove the unused import to satisfy lint. Add `import type React from "react"` or use `ReactNode` import for `HeaderIconButton` typing.

- [ ] **Step 2: (verification happens in Task 6)**

No standalone run — Tasks 4–6 compile together. Proceed to Task 5.

---

### Task 5: Rewire the stage (`HuddleStage.tsx`)

**Files:**
- Modify: `app/src/console/views/huddle/HuddleStage.tsx`
- Modify: `app/src/console/views/huddle/HuddleStage.test.tsx`

**Interfaces:**
- Consumes: `CallTiles` (Task 2), `HuddleControls` (Task 1), `buildParticipants`, `galleryColumns` no longer needed here (moved into CallTiles), `spotlightKey` no longer needed here.
- Produces: `HuddleStage({ onCollapse })` unchanged signature.

Delete the inline `StageTile`, `memberNodeOf`, the gallery/spotlight grid JSX, and the bespoke control bar. Keep the header (dot + name + count + mode toggle + collapse + pop), the `mode`/`pinned`/`nowTick` state, and the fixed-overlay chrome.

- [ ] **Step 1: Update the stage test queries**

The mute/camera/leave buttons now come from `HuddleControls` (compact-vs-comfortable text). In comfortable size the mute button shows text "Mute"/"Muted" and camera "Camera"/"Camera on", leave "Leave" — queries by those names keep working. Replace any query that assumed the old inline bar's exact labels. Full relevant test additions:

```tsx
// in HuddleStage.test.tsx — after rendering an expanded live stage:
expect(screen.getByRole("button", { name: /mute/i })).toBeTruthy();
expect(screen.getByRole("button", { name: /leave/i })).toBeTruthy();
expect(screen.getByRole("button", { name: /gallery|spotlight/i })).toBeTruthy();
expect(screen.getByRole("button", { name: /collapse/i })).toBeTruthy();
```

- [ ] **Step 2: Replace the tiles + control bar in `HuddleStage.tsx`**

Body (tiles) becomes:

```tsx
const memberNodes = Object.fromEntries(
  (channel?.huddle ?? []).map((m) => [keyHex(m.user), keyHex(m.node)]),
);
const bindPreview = (el: HTMLVideoElement | null) => actions.getCallSession()?.bindPreview(el);
const bindTile = (nodeHex: string, el: HTMLCanvasElement | null) => actions.getCallSession()?.bindTile(nodeHex, el);
```

```tsx
{/* tiles */}
<div style={{ flex: 1, minHeight: 0, padding: 14, overflow: "auto" }}>
  {participants.length === 0 ? (
    <div style={{ height: "100%", display: "flex", alignItems: "center", justifyContent: "center", color: color.muted2, font: `500 13px ${font.sans}` }}>
      connecting…
    </div>
  ) : (
    <CallTiles
      layout={mode}
      participants={participants}
      memberNodes={memberNodes}
      peers={voice.peers}
      canEncode={videoCapability.canEncode}
      canDecode={videoCapability.canDecode}
      selfCameraOn={voice.cameraOn}
      bindPreview={bindPreview}
      bindTile={bindTile}
      pinned={pinned}
      onPin={(key) => { setPinned(key); setMode("spotlight"); }}
    />
  )}
</div>
```

Control bar becomes:

```tsx
{/* control bar */}
<div style={{ display: "flex", alignItems: "center", justifyContent: "center", padding: "12px 14px", borderTop: `1px solid ${color.borderSoft}` }}>
  <HuddleControls
    size="comfortable"
    home="stage"
    status={voice.status}
    muted={voice.muted}
    cameraOn={voice.cameraOn}
    canEncode={videoCapability.canEncode}
    onToggleMute={() => actions.setHuddleMuted(!voice.muted)}
    onToggleCamera={() => actions.setCamera(!voice.cameraOn)}
    onLeave={() => actions.leaveHuddle()}
    onRetry={() => actions.joinHuddle(voice.channelId!)}
  />
</div>
```

The pop-out button in the stage moves to the header cluster (next to Collapse), matching the dock's view-cluster grouping:

```tsx
<div style={{ marginLeft: "auto", display: "flex", gap: 8 }}>
  <HoverButton onClick={() => setMode(mode === "gallery" ? "spotlight" : "gallery")} title={mode === "gallery" ? "Spotlight view" : "Gallery view"} style={barBtn(false)} hoverStyle={{ background: color.hover }}>
    {mode === "gallery" ? <SpotlightGlyph /> : <GridGlyph />}
    {mode === "gallery" ? "Spotlight" : "Gallery"}
  </HoverButton>
  {isTauri() && (
    <HoverButton onClick={() => actions.popOutHuddle()} title="Pop out to a window" style={barBtn(false)} hoverStyle={{ background: color.hover }}>
      Pop out
    </HoverButton>
  )}
  <HoverButton onClick={onCollapse} title="Collapse to dock" style={barBtn(false)} hoverStyle={{ background: color.hover }}>
    <CollapseGlyph /> Collapse
  </HoverButton>
</div>
```

Delete the now-unused `MicGlyph`, `CameraGlyph` (moved to HuddleControls/CallTiles), `StageTile`, `memberNodeOf`, and the `galleryColumns`/`spotlightKey` imports. Keep `GridGlyph`, `SpotlightGlyph`, `CollapseGlyph`, `barBtn`, `keyHex`.

- [ ] **Step 3: (verification in Task 6)**

Proceed to Task 6.

---

### Task 6: Rewire the popped window + green the tree

**Files:**
- Modify: `app/src/console/views/huddle/HuddleWindow.tsx`

**Interfaces:**
- Consumes: `HuddleCard` (slim body), `HuddleControls`, `HuddleWindowState`/`HuddleWindowCmd` (unchanged in this PR).

The window is still an audio mirror in PR-A (media handoff is PR-B). Render the slim `HuddleCard` body + `HuddleControls home="window"` with mute / leave / retry and **no camera** (the window owns no session yet), plus a pop-in via the native close.

- [ ] **Step 1: Rewrite `HuddleWindow`'s card block**

```tsx
{card ? (
  <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
    <HuddleCard
      channelName={card.channelName}
      status={card.status}
      error={card.error}
      participants={card.participants}
      ring={color.paper}
    />
    <HuddleControls
      size="compact"
      home="window"
      status={card.status}
      muted={card.muted}
      cameraOn={false}
      canEncode={false}
      onToggleMute={(next => () => send({ op: "set-muted", muted: next }))(!card.muted)}
      onLeave={() => send({ op: "leave" })}
      onRetry={() => send({ op: "retry" })}
    />
  </div>
) : ( /* unchanged connecting… block */ )}
```

Note: `onToggleMute` must send the toggled value — write it plainly as `onToggleMute={() => send({ op: "set-muted", muted: !card.muted })}`. Pop-in is the native window close (already wired via `getCurrentWindow().close()` on the OS button and the `huddle-closed` hook); no in-card pop-in button is required for the audio pill. If a pop-in affordance is wanted, add a header icon like the dock's — deferred to PR-B where the window gains real chrome.

- [ ] **Step 2: Full green — typecheck, lint, tests**

Run: `cd app && bun run typecheck && bun run lint && bun run test`
Expected: typecheck clean, lint clean (remove any now-unused imports flagged), all tests pass (HuddleControls, CallTiles, HuddleCard, HuddleStage, plus the untouched huddle-window/roster/stage-layout suites).

- [ ] **Step 3: Commit the rewire as one green boundary**

```bash
git add app/src/console/views/chat/HuddleCard.tsx app/src/console/views/chat/HuddleCard.test.tsx \
        app/src/console/views/chat/Huddle.tsx \
        app/src/console/views/huddle/HuddleStage.tsx app/src/console/views/huddle/HuddleStage.test.tsx \
        app/src/console/views/huddle/HuddleWindow.tsx
git commit -m "feat(huddle): rewire dock/stage/window onto HuddleControls + CallTiles"
```

---

## Self-Review

**Spec coverage (PR-A section):** one control bar with fixed order (Task 1); Screen/Devices reserved slots (Task 1 constraints + test); tiles unified into one renderer (Task 2); dock coherent one-bar + view cluster (Task 4); stage on the same bar (Task 5); audio-only huddle keeps mute+sweep — mute now in `HuddleControls`, sweep in `HuddleCard` roster (Tasks 3+4); popped window untouched behaviorally, restyled onto the shared bar (Task 6). Old duplicated renderers deleted (Tasks 4+5).

**Placeholder scan:** none — every step carries full code or an exact command.

**Type consistency:** `HuddleControlsProps` (Task 1) is consumed verbatim in Tasks 4/5/6; `CallTilesProps` (Task 2) consumed in Tasks 4/5; slim `HuddleCardProps` (Task 3) consumed in Tasks 4/6; `memberNodes` map shape (`Record<userHex, nodeHex>`) is built identically in Tasks 4 and 5; `bindTile(nodeHex, el)` signature matches `getCallSession().bindTile`. `HuddleStatus` stays exported from `HuddleCard` and imported by `HuddleControls`.

**Verification limits:** real camera-send + macOS unverifiable here (no encoder / no Mac) — behavior is layout-only, capability-gated; flagged for real-hardware QA.
