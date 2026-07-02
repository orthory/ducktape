// The app toolbar + content frame. On desktop the native title bar sits above
// this and `data-tauri-drag-region` makes the toolbar draggable; on web the
// attribute is inert. The status dot reflects the last node round-trip and the
// height ticks with finalized blocks.

import type { ReactNode } from "react";

import { accentVar, color, font } from "../theme/tokens";
import { useDucktape } from "../store/use-ducktape";

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
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
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
          flex: 1,
          minWidth: 0,
          // clear the macOS traffic lights on the desktop build
          paddingLeft: 69,
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
            }}
          >
            D
          </span>
          ducktape
          <span
            style={{
              font: `600 8px ${font.mono}`,
              color: "#fff",
              background: accentVar,
              borderRadius: 4,
              padding: "2px 5px",
              letterSpacing: ".05em",
            }}
          >
            {state.managed ? "LOCAL" : "REMOTE"}
          </span>
        </div>
      </div>

      <div
        data-tauri-drag-region
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "flex-end",
          gap: 8,
          flex: 1,
          minWidth: 0,
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
      {children}
    </div>
  );
}
