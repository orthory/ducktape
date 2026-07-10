// The app toolbar + content frame. On desktop the native title bar sits above
// this and `data-tauri-drag-region` makes the toolbar draggable; on web the
// attribute is inert. The status dot reflects the last node round-trip and the
// height ticks with finalized blocks.

import { useState } from "react";
import type { ReactNode } from "react";

import { Icon } from "../components/Icon";
import { accentVar, color, font, radius } from "../theme/tokens";
import { useDucktape } from "../store/use-ducktape";
import { isMacDesktop } from "../../domain/node-bootstrap";
import { ErrorBoundary } from "./ErrorBoundary";
import { ConnectionBanner } from "./ConnectionBanner";
import { ResizeEdges, WindowControls } from "./WindowChrome";

// Left inset that clears the macOS traffic lights. Only the macOS desktop build
// overlays them on the content (see isMacDesktop); on Linux/Windows desktop and
// on web the brand sits flush at the bar's normal 13px edge, symmetric with the
// status half. The platform is fixed for the process lifetime, so this is
// resolved once at module load rather than per TitleBar render (which re-renders
// on every finalized block).
const TRAFFIC_LIGHT_INSET = isMacDesktop() ? 69 : 0;

// The palette shortcut as the local platform types it (the handler accepts
// both meta and ctrl — see ConsoleShell). Same module-load resolution as the
// inset above.
const SEARCH_KEY_LABEL = isMacDesktop() ? "⌘K" : "Ctrl K";

// The centered search affordance in the title bar: a compact field that opens
// the ⌘K palette (see ConsoleShell / SearchModal). It sits in the middle cell
// of the bar's `1fr auto 1fr` grid, so it tracks the window's true midpoint
// (the two 1fr halves stay equal even when the left one carries the macOS
// traffic-light inset) and reserves its own space — the brand/status halves are never
// occluded. It is in flow rather than an overlay, so the surrounding halves keep
// their drag regions; only the button footprint is non-draggable, as a control
// should be.
function SearchBar() {
  const { actions } = useDucktape();
  const [hover, setHover] = useState(false);
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        minWidth: 0,
      }}
    >
      <button
        onClick={actions.openSearch}
        title={`Search (${SEARCH_KEY_LABEL})`}
        aria-label="Search"
        onMouseEnter={() => setHover(true)}
        onMouseLeave={() => setHover(false)}
        style={{
          all: "unset",
          boxSizing: "border-box",
          cursor: "pointer",
          display: "flex",
          alignItems: "center",
          gap: 8,
          width: 340,
          maxWidth: "100%",
          height: 28,
          padding: "0 10px",
          borderRadius: radius.md,
          background: hover ? color.hover : color.sunken,
          border: `1px solid ${color.border}`,
          transition: "background .12s",
        }}
      >
        <Icon name="search" size={14} color={color.muted2} />
        <span
          style={{
            flex: 1,
            minWidth: 0,
            textAlign: "left",
            font: `500 11.5px ${font.sans}`,
            color: color.muted,
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
          }}
        >
          Search
        </span>
        <span
          style={{
            flexShrink: 0,
            padding: "1px 5px",
            borderRadius: 5,
            border: `1px solid ${color.borderSoft}`,
            font: `600 9.5px ${font.mono}`,
            color: color.muted2,
            background: color.paper,
          }}
        >
          {SEARCH_KEY_LABEL}
        </span>
      </button>
    </div>
  );
}

function TitleBar() {
  const { state } = useDucktape();
  const dot = state.connected ? color.green : color.red;
  const label = state.connected ? "Connected" : "Disconnected";

  return (
    <div
      data-tauri-drag-region
      style={{
        position: "relative",
        height: 44,
        flexShrink: 0,
        // `1fr auto 1fr`: the search bar (center cell) is window-centered because
        // the two 1fr halves stay equal, while each half reserves its own track
        // so the centered bar never overlaps the brand/status text.
        display: "grid",
        gridTemplateColumns: "1fr auto 1fr",
        alignItems: "center",
        gap: 13,
        padding: "0 13px",
        background: color.titlebar,
        borderBottom: `1px solid ${color.border}`,
        zIndex: 5,
      }}
    >
      <div
        data-tauri-drag-region
        style={{
          display: "flex",
          alignItems: "center",
          gap: 13,
          minWidth: 0,
          overflow: "hidden",
          // clear the macOS traffic lights (macOS desktop only; 0 elsewhere)
          paddingLeft: TRAFFIC_LIGHT_INSET,
        }}
      >
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 7,
            font: `600 11.5px ${font.sans}`,
            color: color.inkSoft,
            whiteSpace: "nowrap",
            // let the name span ellipsize instead of pushing the badge out
            minWidth: 0,
          }}
        >
          <span
            style={{
              width: 16,
              height: 16,
              borderRadius: 4,
              background: color.dark,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              font: `600 8px ${font.mono}`,
              color: color.onDark,
              flexShrink: 0,
            }}
          >
            D
          </span>
          {/* The active workspace names the window; the brand is the correct
              fallback wherever none exists (web build, remote node, the
              pre-onboarding gate). */}
          <span
            style={{
              minWidth: 0,
              overflow: "hidden",
              textOverflow: "ellipsis",
            }}
          >
            {state.workspace?.name ?? "ducktape"}
          </span>
          <span
            style={{
              font: `600 8px ${font.mono}`,
              color: "#fff",
              background: accentVar,
              borderRadius: 4,
              padding: "2px 5px",
              letterSpacing: ".05em",
              flexShrink: 0,
            }}
          >
            {state.managed ? "LOCAL" : "REMOTE"}
          </span>
        </div>
      </div>

      {/* search opens the ⌘K palette over a connected console — with the
          onboarding gate up or a join in flight there is nothing to search,
          so the affordance hides (an empty middle cell keeps the grid's two
          1fr halves, and thus the bar's centering, intact). */}
      {state.needsOnboarding || state.onboardingPhase ? (
        <div data-tauri-drag-region />
      ) : (
        <SearchBar />
      )}

      <div
        data-tauri-drag-region
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "flex-end",
          gap: 8,
          minWidth: 0,
          overflow: "hidden",
        }}
      >
        <span
          style={{
            display: "flex",
            alignItems: "center",
            gap: 5,
            font: `500 10.5px ${font.mono}`,
            color: color.muted2,
            whiteSpace: "nowrap",
          }}
          title={label}
        >
          <span style={{ width: 6, height: 6, borderRadius: "50%", background: dot }} />
          {"h " + (state.status?.height ?? 0).toLocaleString()}
        </span>
        {/* Undecorated Linux/Windows window controls; renders nothing on
            macOS (native traffic lights) and on web. */}
        <WindowControls />
      </div>
    </div>
  );
}

export function WindowFrame({ children }: { children: ReactNode }) {
  return (
    <div
      style={{
        position: "relative",
        width: "100vw",
        height: "100vh",
        background: color.paper,
        overflow: "hidden",
        display: "flex",
        flexDirection: "column",
      }}
    >
      <TitleBar />
      {/* A mid-session node drop shows a loud reconnecting banner here, not just
          a lone red dot beside a frozen height. */}
      <ConnectionBanner />
      {/* Keep the title bar if a body view throws — the boundary replaces only
          the content below, never the whole window (which had no boundary and
          went blank white on any render throw). */}
      <ErrorBoundary>{children}</ErrorBoundary>
      {/* Edge resize handles for the undecorated non-mac desktop window. */}
      <ResizeEdges />
    </div>
  );
}
