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

export interface HuddleControlsProps {
  size: "compact" | "comfortable";
  status: HuddleStatus;
  muted: boolean;
  cameraOn: boolean;
  canEncode: boolean;
  /** Non-empty → the camera control is disabled and shows this as its tooltip. */
  cameraDisabledReason?: string;
  /** PR-C — whether we are currently screen-sharing. */
  sharing?: boolean;
  /** PR-C — screen control is shown only when true and `onToggleScreen` is set. */
  canScreenShare?: boolean;
  onToggleScreen?: () => void;
  /** PR-D — omit → the devices control is hidden. */
  onOpenDevices?: () => void;
  onToggleMute: () => void;
  /** Omit → the camera control is hidden (also hidden when `!canEncode`). */
  onToggleCamera?: () => void;
  onLeave: () => void;
  onRetry: () => void;
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
        Leave
      </HoverButton>
    </div>
  );
}
