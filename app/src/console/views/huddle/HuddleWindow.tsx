// The popped-out huddle window (`index.html?view=huddle`) — a pure event
// mirror of the main window's session. It renders exactly the state the main
// window pushes (ducktape://huddle-state) and sends commands back
// (ducktape://huddle-cmd); the audio session itself never leaves the main
// window (real video-in-window is PR-B). Closing this window — the native button
// or the pop-in control — fires Rust's Destroyed hook, which tells the main
// window to re-mount its in-app card. Protocol constants + payload shape:
// store/huddle-window.ts. The body reuses the shared HuddleCard + HuddleControls
// so the popped surface matches the dock and stage; camera is absent here since
// the window owns no media session yet.

import { useEffect, useState } from "react";
import { emit, listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { HUDDLE_CMD_EVENT, HUDDLE_STATE_EVENT } from "../../store/huddle-window";
import type { HuddleWindowCmd, HuddleWindowState } from "../../store/huddle-window";
import { color, font, radius } from "../../theme/tokens";
import { HoverButton } from "../chat/HoverButton";
import { HuddleCard } from "../chat/HuddleCard";
import { HuddleControls } from "./HuddleControls";

const send = (cmd: HuddleWindowCmd): void => {
  void emit(HUDDLE_CMD_EVENT, cmd);
};

/** Arrow entering a box — return the huddle to the in-app dock. */
function PopInGlyph({ size = 13 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round">
      <path d="M9 5H6a2 2 0 0 0-2 2v11a2 2 0 0 0 2 2h11a2 2 0 0 0 2-2v-3" />
      <path d="M20 4l-7 7" />
      <path d="M13.5 5.5V11H19" />
    </svg>
  );
}

export function HuddleWindow() {
  const [card, setCard] = useState<HuddleWindowState | null>(null);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void listen(HUDDLE_STATE_EVENT, (event) => {
      setCard(event.payload as HuddleWindowState);
    }).then((un) => {
      if (cancelled) un();
      else unlisten = un;
    });
    // the main window replays the current state in response.
    send({ op: "ready" });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  return (
    <div
      style={{
        minHeight: "100vh",
        boxSizing: "border-box",
        background: color.paper,
        padding: "10px 12px",
        display: "flex",
        flexDirection: "column",
        justifyContent: "center",
      }}
    >
      {card ? (
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          <div style={{ display: "flex", alignItems: "flex-start", gap: 6 }}>
            <div style={{ flex: 1, minWidth: 0 }}>
              <HuddleCard
                channelName={card.channelName}
                status={card.status}
                error={card.error}
                participants={card.participants}
                ring={color.paper}
                onSweep={(user) => send({ op: "sweep", user })}
              />
            </div>
            <HoverButton
              onClick={() => void getCurrentWindow().close()}
              title="Return to app"
              style={{ display: "flex", alignItems: "center", justifyContent: "center", width: 24, height: 22, borderRadius: radius.sm, color: color.muted2, flexShrink: 0 }}
              hoverStyle={{ background: color.hover, color: color.ink }}
            >
              <PopInGlyph size={13} />
            </HoverButton>
          </div>
          <HuddleControls
            size="compact"
            status={card.status}
            muted={card.muted}
            cameraOn={false}
            canEncode={false}
            onToggleMute={() => send({ op: "set-muted", muted: !card.muted })}
            onLeave={() => send({ op: "leave" })}
            onRetry={() => send({ op: "retry" })}
          />
        </div>
      ) : (
        <span style={{ font: `400 11.5px ${font.sans}`, color: color.muted2, textAlign: "center" }}>
          connecting to session…
        </span>
      )}
    </div>
  );
}
