// The huddle session card BODY — status dot + channel, error row, participant
// pile, mute/Leave controls. Purely presentational (props + callbacks, no
// store): the in-app dock (Huddle.tsx) and the popped-out window
// (views/huddle/HuddleWindow.tsx) both render it, so the two surfaces cannot
// drift. Participants arrive as resolved display names — the window side has
// no member records or profile registry to resolve against.

import type { VoiceError } from "../../../domain/voice-session";
import { color, font, radius } from "../../theme/tokens";
import { HoverButton } from "./HoverButton";

export type HuddleStatus = "idle" | "connecting" | "live" | "error";

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

/** Arrow leaving a box (pop out) or entering it (pop in). */
function PopGlyph({ size = 13, into = false }: { size?: number; into?: boolean }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round">
      <path d="M9 5H6a2 2 0 0 0-2 2v11a2 2 0 0 0 2 2h11a2 2 0 0 0 2-2v-3" />
      {into ? (
        <>
          <path d="M20 4l-7 7" />
          <path d="M13.5 5.5V11H19" />
        </>
      ) : (
        <>
          <path d="M13 11l7-7" />
          <path d="M14.5 4H20v5.5" />
        </>
      )}
    </svg>
  );
}

const STATUS_DOT: Record<HuddleStatus, { color: string; pulse: boolean }> = {
  connecting: { color: color.amber, pulse: true },
  live: { color: color.green, pulse: false },
  error: { color: color.red, pulse: false },
  idle: { color: color.muted2, pulse: false },
};

// What the card says per failure. macOS records a mic denial permanently (it
// never re-prompts), so `mic-denied` must route through System Settings — for
// dev runs the OS attributes the prompt to the launching terminal, not the app.
const ERROR_COPY: Record<VoiceError, string> = {
  "mic-denied": "Mic access is blocked — allow it in System Settings, then retry.",
  "mic-missing": "No usable microphone found.",
  "mic-failed": "Mic setup failed.",
  connection: "Voice connection failed.",
  refused: "Couldn't join this huddle.",
};

const initialsOf = (name: string): string => name.slice(0, 2).toUpperCase();

/** An overlapping pile of participant initials, capped with a "+N" chip. */
function NamePile({ names, size = 24, ring, max = 5 }: { names: string[]; size?: number; ring: string; max?: number }) {
  const shown = names.slice(0, max);
  const extra = names.length - shown.length;
  const chip = (content: string, title: string | undefined, background: string, i: number) => (
    <span
      key={`${title ?? content}-${i}`}
      title={title}
      aria-hidden={title ? "true" : undefined}
      style={{
        marginLeft: i === 0 ? 0 : -8,
        zIndex: title ? shown.length - i : undefined,
        width: size,
        height: size,
        borderRadius: "50%",
        background,
        color: color.muted3,
        border: `2px solid ${ring}`,
        boxSizing: "border-box",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        font: `600 ${size <= 24 ? 9.5 : 11}px ${font.sans}`,
        flexShrink: 0,
      }}
    >
      {content}
    </span>
  );
  return (
    <div style={{ display: "flex", alignItems: "center" }}>
      {shown.map((name, i) => chip(initialsOf(name), name, color.chip, i))}
      {extra > 0 && chip(`+${extra}`, undefined, color.sunken, shown.length)}
    </div>
  );
}

export interface HuddleCardProps {
  /** Channel name without the leading '#'. */
  channelName: string;
  status: HuddleStatus;
  /** Why `status` is "error" — picks the message row. Null otherwise. */
  error: VoiceError | null;
  muted: boolean;
  /** Resolved display names of everyone in the roster (self included). */
  participants: string[];
  /** Ring color behind the avatar pile — the card's own surface color. */
  ring?: string;
  onSetMuted(muted: boolean): void;
  onLeave(): void;
  onRetry(): void;
  /** Dock only: open the separate huddle window. */
  onPopOut?(): void;
  /** Window only: close this window and return to the in-app card. */
  onPopIn?(): void;
}

export function HuddleCard({
  channelName,
  status,
  error,
  muted,
  participants,
  ring = color.paper,
  onSetMuted,
  onLeave,
  onRetry,
  onPopOut,
  onPopIn,
}: HuddleCardProps) {
  const dot = STATUS_DOT[status] ?? STATUS_DOT.idle;
  const live = status === "live";
  const failure = status === "error" ? (error ?? "connection") : null;
  const pop = onPopOut ?? onPopIn;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
      <div style={{ display: "flex", alignItems: "center", gap: 7, minWidth: 0 }}>
        <span
          aria-label={status}
          style={{
            width: 8,
            height: 8,
            borderRadius: "50%",
            background: dot.color,
            flexShrink: 0,
            animation: dot.pulse ? "ik-pulse 1s ease-in-out infinite" : undefined,
          }}
        />
        <span
          style={{
            flex: 1,
            minWidth: 0,
            overflow: "hidden",
            textOverflow: "ellipsis",
            whiteSpace: "nowrap",
            font: `600 12.5px ${font.sans}`,
            color: color.ink,
          }}
        >
          #{channelName}
        </span>
        {!failure && (
          <span style={{ font: `500 10.5px ${font.sans}`, color: color.muted2, flexShrink: 0 }}>
            {status === "connecting" ? "connecting…" : `${participants.length}`}
          </span>
        )}
        {pop && (
          <HoverButton
            onClick={pop}
            title={onPopOut ? "Open in window" : "Return to app"}
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              width: 20,
              height: 20,
              borderRadius: radius.sm,
              color: color.muted2,
              flexShrink: 0,
            }}
            hoverStyle={{ background: color.hover, color: color.ink }}
          >
            <PopGlyph size={13} into={!!onPopIn} />
          </HoverButton>
        )}
      </div>

      {failure && (
        <span style={{ font: `400 11px/1.4 ${font.sans}`, color: color.danger }}>
          {ERROR_COPY[failure]}
        </span>
      )}

      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <div style={{ flex: 1, minWidth: 0 }}>
          {!failure &&
            (participants.length > 0 ? (
              <NamePile names={participants} size={24} ring={ring} />
            ) : (
              <span style={{ font: `400 11px ${font.sans}`, color: color.muted2 }}>Just you</span>
            ))}
        </div>

        {failure ? (
          <HoverButton
            onClick={onRetry}
            title="Retry"
            style={{
              display: "flex",
              alignItems: "center",
              padding: "6px 11px",
              borderRadius: radius.sm,
              border: `1px solid ${color.borderSoft}`,
              background: color.sunken,
              color: color.inkSoft,
              font: `600 11.5px ${font.sans}`,
            }}
            hoverStyle={{ background: color.hover, color: color.ink }}
          >
            Retry
          </HoverButton>
        ) : (
          <HoverButton
            onClick={() => onSetMuted(!muted)}
            title={muted ? "Unmute" : "Mute"}
            disabled={!live}
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              width: 30,
              height: 28,
              borderRadius: radius.sm,
              border: `1px solid ${muted ? color.dangerBorder : color.borderSoft}`,
              background: muted ? color.dangerSoft : live ? color.dark : color.sunken,
              color: muted ? color.danger : live ? color.onDark : color.muted2,
              opacity: live ? 1 : 0.55,
            }}
            hoverStyle={{ filter: "brightness(1.05)" }}
          >
            <MicGlyph size={15} muted={muted} />
          </HoverButton>
        )}

        <HoverButton
          onClick={onLeave}
          title="Leave huddle"
          style={{
            display: "flex",
            alignItems: "center",
            padding: "6px 11px",
            borderRadius: radius.sm,
            background: color.danger,
            color: "#fff",
            font: `600 11.5px ${font.sans}`,
          }}
          hoverStyle={{ filter: "brightness(1.06)" }}
        >
          Leave
        </HoverButton>
      </div>
    </div>
  );
}
