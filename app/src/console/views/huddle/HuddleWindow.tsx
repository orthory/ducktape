// The popped-out huddle window (`index.html?view=huddle`). In PR-B it is a real
// video surface: it runs its OWN media session (useHuddleWindowSession), seeded
// by the context the main window pushes (ducktape://huddle-context), and renders
// the same CallTiles + HuddleControls the dock/stage use. Consensus stays in the
// main window — only leave/sweep cross back over ducktape://huddle-cmd. If the
// session dies (or the user closes the window), the window closes and Rust's
// Destroyed hook tells main to re-take the session, so the call is never
// stranded in a dead float. Protocol: store/huddle-window.ts.

import { useCallback, useEffect, useState } from "react";
import { emit, listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

import { MAX_VIDEO_PARTICIPANTS } from "../../../domain/call-session";
import { HUDDLE_CMD_EVENT, HUDDLE_CONTEXT_EVENT } from "../../store/huddle-window";
import type { HuddleContext, HuddleWindowCmd } from "../../store/huddle-window";
import { color, font, radius } from "../../theme/tokens";
import { HoverButton } from "../chat/HoverButton";
import { CallTiles } from "./CallTiles";
import { HuddleControls } from "./HuddleControls";
import { useHuddleWindowSession } from "./useHuddleWindowSession";

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
  const [ctx, setCtx] = useState<HuddleContext | null>(null);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    void listen(HUDDLE_CONTEXT_EVENT, (event) => {
      setCtx(event.payload as HuddleContext);
    }).then((un) => {
      if (cancelled) un();
      else unlisten = un;
    });
    // the main window replays the current context in response.
    send({ op: "ready" });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  // A dead session (hard error / replaced) closes the window — Rust's Destroyed
  // hook then tells main to re-take, so the call falls back to the working dock.
  const onMediaEnded = useCallback(() => {
    void getCurrentWindow().close();
  }, []);

  const view = useHuddleWindowSession(ctx, onMediaEnded);
  const overCap = view ? view.participants.length > MAX_VIDEO_PARTICIPANTS : false;

  return (
    <div
      style={{
        minHeight: "100vh",
        boxSizing: "border-box",
        background: color.paper,
        display: "flex",
        flexDirection: "column",
      }}
    >
      {view ? (
        <>
          {/* header — channel + count + pop-in */}
          <div style={{ display: "flex", alignItems: "center", gap: 8, padding: "8px 10px", borderBottom: `1px solid ${color.borderSoft}` }}>
            <span
              aria-label={view.status}
              style={{ width: 8, height: 8, borderRadius: "50%", flexShrink: 0, background: view.status === "live" ? color.green : view.status === "error" ? color.red : color.amber }}
            />
            <span style={{ flex: 1, minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", font: `600 12.5px ${font.sans}`, color: color.ink }}>
              #{view.channelName}
            </span>
            <span style={{ font: `500 10.5px ${font.sans}`, color: color.muted2, flexShrink: 0 }}>{view.participants.length}</span>
            <HoverButton
              onClick={() => void getCurrentWindow().close()}
              title="Return to app"
              style={{ display: "flex", alignItems: "center", justifyContent: "center", width: 24, height: 22, borderRadius: radius.sm, color: color.muted2, flexShrink: 0 }}
              hoverStyle={{ background: color.hover, color: color.ink }}
            >
              <PopInGlyph size={13} />
            </HoverButton>
          </div>

          {/* tiles */}
          <div style={{ flex: 1, minHeight: 0, padding: 10, overflow: "auto" }}>
            {view.participants.length === 0 ? (
              <div style={{ height: "100%", display: "flex", alignItems: "center", justifyContent: "center", color: color.muted2, font: `500 12px ${font.sans}` }}>
                connecting…
              </div>
            ) : (
              <CallTiles
                layout="gallery"
                participants={view.participants}
                memberNodes={view.memberNodes}
                peers={view.peers}
                canEncode={view.canEncode}
                canDecode={view.canDecode}
                selfCameraOn={view.cameraOn}
                bindPreview={view.bindPreview}
                bindTile={view.bindTile}
              />
            )}
          </div>

          {/* control bar */}
          <div style={{ display: "flex", alignItems: "center", justifyContent: "center", padding: "8px 10px", borderTop: `1px solid ${color.borderSoft}` }}>
            <HuddleControls
              size="comfortable"
              status={view.status}
              muted={view.muted}
              cameraOn={view.cameraOn}
              canEncode={view.canEncode}
              cameraDisabledReason={overCap ? "Video is capped at 8 participants" : undefined}
              onToggleMute={() => {
                const next = !view.muted;
                view.setMuted(next);
                send({ op: "mute", muted: next }); // let main re-take with the same mute
              }}
              onToggleCamera={() => view.setCamera(!view.cameraOn)}
              onLeave={() => send({ op: "leave" })}
              // No in-window retry — a dead session closes the float and the dock
              // re-takes (see useHuddleWindowSession); the error state never shows.
              onRetry={() => {}}
            />
          </div>
        </>
      ) : (
        <div style={{ flex: 1, display: "flex", alignItems: "center", justifyContent: "center" }}>
          <span style={{ font: `400 11.5px ${font.sans}`, color: color.muted2 }}>connecting to session…</span>
        </div>
      )}
    </div>
  );
}
