// The popped-out huddle window (`index.html?view=huddle`) — a pure event
// mirror of the main window's session. It renders exactly the state the main
// window pushes (ducktape://huddle-state) and sends commands back
// (ducktape://huddle-cmd); the audio session itself never leaves the main
// window. Closing this window — the native button or the pop-in control —
// fires Rust's Destroyed hook, which tells the main window to re-mount its
// in-app card. Protocol constants + payload shape: store/huddle-window.ts.

import { useEffect, useState } from "react";
import { emit, listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { HUDDLE_CMD_EVENT, HUDDLE_STATE_EVENT } from "../../store/huddle-window";
import type { HuddleWindowCmd, HuddleWindowState } from "../../store/huddle-window";
import { color, font } from "../../theme/tokens";
import { HuddleCard } from "../chat/HuddleCard";

const send = (cmd: HuddleWindowCmd): void => {
  void emit(HUDDLE_CMD_EVENT, cmd);
};

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
        <HuddleCard
          channelName={card.channelName}
          status={card.status}
          error={card.error}
          muted={card.muted}
          participants={card.participants}
          ring={color.paper}
          onSetMuted={(muted) => send({ op: "set-muted", muted })}
          onLeave={() => send({ op: "leave" })}
          onRetry={() => send({ op: "retry" })}
          onPopIn={() => void getCurrentWindow().close()}
        />
      ) : (
        <span style={{ font: `400 11.5px ${font.sans}`, color: color.muted2, textAlign: "center" }}>
          connecting to session…
        </span>
      )}
    </div>
  );
}
