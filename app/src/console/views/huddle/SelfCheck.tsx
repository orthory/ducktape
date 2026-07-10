// The solo self-check: what a huddle shows when YOU are the only one in it.
// Instead of a bare "connecting…" or a lone muted avatar, it makes the moment
// useful — a big camera self-preview and a live mic meter so a user can verify
// their devices work before anyone else joins. The camera preview and the mic
// level are LOCAL (getUserMedia), so this is a real check even before the
// session is server-`live` and even while muted (capture runs regardless).
// Store-free: the container passes the session's bindPreview + local media
// state, exactly like CallTiles. Shared by the stage and the popped window.

import type { CSSProperties } from "react";

import type { HuddleStatus } from "../chat/HuddleCard";
import { accentVar, color, font, radius } from "../../theme/tokens";
import { HoverButton } from "../chat/HoverButton";

function CameraGlyph({ size = 22, off = false }: { size?: number; off?: boolean }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.7} strokeLinecap="round" strokeLinejoin="round">
      <rect x="2.5" y="6.5" width="12" height="11" rx="2.2" />
      <path d="M14.5 10.5l6-3v9l-6-3z" />
      {off && <path d="M4 4l16 16" strokeWidth={1.9} />}
    </svg>
  );
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

export interface SelfCheckProps {
  channelName?: string;
  status: HuddleStatus;
  cameraOn: boolean;
  sharing: boolean;
  canEncode: boolean;
  muted: boolean;
  /** 0..1 mic input level (throttled) — drives the meter. */
  level: number;
  speaking: boolean;
  bindPreview: (el: HTMLVideoElement | null) => void;
  /** Turn the camera on/off. Works before the session is `live` (local media). */
  onToggleCamera: () => void;
  /** Omit → no in-panel mute toggle (the control bar owns it). */
  onToggleMute?: () => void;
}

/** The mic meter: a track with a level fill and the speaking threshold marked, so
 *  a user can watch their own voice move the bar. Green while active. */
function MicMeter({ level, muted, speaking }: { level: number; muted: boolean; speaking: boolean }) {
  const pct = Math.round(Math.min(1, Math.max(0, level)) * 100);
  const active = speaking || level > 0.08;
  const label = muted
    ? "Mic is muted — it still reacts here, so you can check it"
    : active
      ? "Mic is picking you up"
      : "Say something to test your mic";
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 6, width: "100%" }}>
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <span style={{ color: muted ? color.danger : active ? color.green : color.muted3, display: "flex", flexShrink: 0 }}>
          <MicGlyph size={15} muted={muted} />
        </span>
        <div
          aria-label="microphone level"
          style={{ position: "relative", flex: 1, height: 8, borderRadius: 999, background: color.sunken, overflow: "hidden", border: `1px solid ${color.borderSoft}` }}
        >
          <div
            style={{
              position: "absolute",
              inset: 0,
              width: `${pct}%`,
              background: active ? color.green : color.muted2,
              borderRadius: 999,
              // A short transition so the ~12 Hz updates read as a smooth bar,
              // not a strobe.
              transition: "width 90ms linear, background 120ms linear",
            }}
          />
          {/* the speaking threshold, so "picking you up" lines up with the mark */}
          <span style={{ position: "absolute", top: 0, bottom: 0, left: "8%", width: 1, background: color.borderSoft }} />
        </div>
      </div>
      <span style={{ font: `500 11.5px ${font.sans}`, color: color.muted3, paddingLeft: 23 }}>{label}</span>
    </div>
  );
}

export function SelfCheck({
  status,
  cameraOn,
  sharing,
  canEncode,
  muted,
  level,
  speaking,
  bindPreview,
  onToggleCamera,
  onToggleMute,
}: SelfCheckProps) {
  const showVideo = (cameraOn || sharing) && canEncode;
  const subtitle =
    status === "live"
      ? "You'll see people as they join. In the meantime, check your camera and mic."
      : "Getting your camera and mic ready…";

  const preview: CSSProperties = {
    position: "relative",
    width: "100%",
    aspectRatio: "16 / 9",
    borderRadius: radius.md,
    overflow: "hidden",
    background: color.dark,
    border: `2px solid ${speaking ? color.green : "transparent"}`,
    boxSizing: "border-box",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
  };

  return (
    <div style={{ height: "100%", display: "flex", alignItems: "center", justifyContent: "center", padding: 8 }}>
      <div style={{ width: "100%", maxWidth: 460, display: "flex", flexDirection: "column", gap: 14 }}>
        <div style={preview}>
          {showVideo ? (
            <video ref={bindPreview} muted autoPlay playsInline style={{ width: "100%", height: "100%", objectFit: sharing ? "contain" : "cover", display: "block" }} />
          ) : (
            <div style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: 10, color: color.muted2 }}>
              <CameraGlyph size={26} off />
              {canEncode ? (
                <HoverButton
                  onClick={onToggleCamera}
                  title="Turn on your camera"
                  style={{
                    display: "inline-flex",
                    alignItems: "center",
                    gap: 7,
                    padding: "7px 14px",
                    borderRadius: radius.md,
                    background: accentVar,
                    color: color.onDark,
                    font: `600 12px ${font.sans}`,
                  }}
                  hoverStyle={{ filter: "brightness(1.06)" }}
                >
                  <CameraGlyph size={15} /> Turn on camera
                </HoverButton>
              ) : (
                <span style={{ font: `500 11.5px ${font.sans}` }}>No camera encoder on this system</span>
              )}
            </div>
          )}
          <span
            style={{
              position: "absolute",
              left: 8,
              bottom: 8,
              padding: "2px 8px",
              borderRadius: 999,
              background: "rgba(38,37,31,.62)",
              color: color.onDark,
              font: `600 11px ${font.sans}`,
            }}
          >
            You
          </span>
        </div>

        <div style={{ display: "flex", flexDirection: "column", gap: 4, alignItems: "center", textAlign: "center" }}>
          <span style={{ font: `600 14px ${font.sans}`, color: color.ink }}>You&rsquo;re the only one here</span>
          <span style={{ font: `500 12px ${font.sans}`, color: color.muted3 }}>{subtitle}</span>
        </div>

        <MicMeter level={level} muted={muted} speaking={speaking} />

        {onToggleMute && (
          <div style={{ display: "flex", justifyContent: "center" }}>
            <HoverButton
              onClick={onToggleMute}
              title={muted ? "Unmute" : "Mute"}
              style={{
                display: "inline-flex",
                alignItems: "center",
                gap: 7,
                padding: "6px 14px",
                borderRadius: radius.md,
                border: `1px solid ${muted ? color.dangerBorder : color.borderSoft}`,
                background: muted ? color.dangerSoft : color.sunken,
                color: muted ? color.danger : color.inkSoft,
                font: `600 12px ${font.sans}`,
              }}
              hoverStyle={{ filter: "brightness(1.03)" }}
            >
              <MicGlyph size={15} muted={muted} /> {muted ? "Unmute" : "Mute"}
            </HoverButton>
          </div>
        )}
      </div>
    </div>
  );
}
