// In-app window chrome for desktops without native overlay chrome. macOS
// overlays its traffic lights on the in-app title bar (titleBarStyle Overlay);
// on Linux/Windows the shell drops native decorations instead (main.rs setup),
// so the title bar hosts the window controls and the frame edges own resize.
// Both components render nothing on macOS and on the web build.

import { useState } from "react";
import type { CSSProperties } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { color, radius } from "../theme/tokens";
import { isMacDesktop, isTauri } from "../../domain/node-bootstrap";

// Evaluated per render (two cheap boolean probes) rather than at module load
// so tests can flip the tauri marker before mounting.
const inAppChrome = () => isTauri() && !isMacDesktop();

const GLYPHS = {
  minimize: <path d="M1.5 5h7" />,
  maximize: <rect x="1.5" y="1.5" width="7" height="7" rx="1" />,
  close: <path d="M2 2l6 6M8 2L2 8" />,
} as const;

function ControlButton({
  label,
  glyph,
  danger,
  onClick,
}: {
  label: string;
  glyph: keyof typeof GLYPHS;
  danger?: boolean;
  onClick: () => void;
}) {
  const [hover, setHover] = useState(false);
  return (
    <button
      onClick={onClick}
      aria-label={label}
      title={label}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        all: "unset",
        boxSizing: "border-box",
        cursor: "pointer",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        width: 26,
        height: 26,
        borderRadius: radius.md,
        color: hover && danger ? "#fff" : color.muted,
        background: hover ? (danger ? color.red : color.hover) : "transparent",
        transition: "background .12s",
        flexShrink: 0,
      }}
    >
      <svg
        width={10}
        height={10}
        viewBox="0 0 10 10"
        fill="none"
        stroke="currentColor"
        strokeWidth={1.2}
        strokeLinecap="round"
      >
        {GLYPHS[glyph]}
      </svg>
    </button>
  );
}

/** Minimize / maximize / close for the undecorated Linux/Windows window. */
export function WindowControls() {
  if (!inAppChrome()) return null;
  // ponytail: one static maximize glyph — no is-maximized tracking for a
  // restore icon; add an onResized listener if anyone misses it.
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 2, flexShrink: 0 }}>
      <ControlButton
        label="Minimize"
        glyph="minimize"
        onClick={() => void getCurrentWindow().minimize()}
      />
      <ControlButton
        label="Maximize"
        glyph="maximize"
        onClick={() => void getCurrentWindow().toggleMaximize()}
      />
      <ControlButton
        label="Close window"
        glyph="close"
        danger
        onClick={() => void getCurrentWindow().close()}
      />
    </div>
  );
}

type ResizeDirection = Parameters<
  ReturnType<typeof getCurrentWindow>["startResizeDragging"]
>[0];

// 4px edge strips + 8px corners, viewport-fixed. WebKitGTK gives an
// undecorated window no resize border of its own, so the webview drives the
// WM resize through startResizeDragging.
const EDGES: Array<[ResizeDirection, CSSProperties]> = [
  ["North", { top: 0, left: 8, right: 8, height: 4, cursor: "n-resize" }],
  ["South", { bottom: 0, left: 8, right: 8, height: 4, cursor: "s-resize" }],
  ["West", { left: 0, top: 8, bottom: 8, width: 4, cursor: "w-resize" }],
  // ponytail: the East strip overlaps the outer half of the styled scrollbar;
  // thin it (or gate it off) if scrollbar grabs ever feel off.
  ["East", { right: 0, top: 8, bottom: 8, width: 4, cursor: "e-resize" }],
  ["NorthWest", { top: 0, left: 0, width: 8, height: 8, cursor: "nw-resize" }],
  ["NorthEast", { top: 0, right: 0, width: 8, height: 8, cursor: "ne-resize" }],
  ["SouthWest", { bottom: 0, left: 0, width: 8, height: 8, cursor: "sw-resize" }],
  ["SouthEast", { bottom: 0, right: 0, width: 8, height: 8, cursor: "se-resize" }],
];

/** Invisible resize handles along the undecorated window's edges. */
export function ResizeEdges() {
  if (!inAppChrome()) return null;
  return (
    <>
      {EDGES.map(([direction, style]) => (
        <div
          key={direction}
          data-resize-dir={direction}
          onMouseDown={(event) => {
            if (event.button !== 0) return;
            event.preventDefault();
            void getCurrentWindow().startResizeDragging(direction);
          }}
          // above every in-app layer (modals top out at zIndex 90): a resize
          // grab at the window edge must always win.
          style={{ position: "fixed", zIndex: 120, ...style }}
        />
      ))}
    </>
  );
}
