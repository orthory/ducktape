// The huddle session card BODY — status dot + channel, error row, participant
// roster (name + per-member mute glyph + a stale-member "remove" control), and
// mute/Leave controls. Purely presentational (props + callbacks, no store): the
// in-app dock (Huddle.tsx) and the popped-out window
// (views/huddle/HuddleWindow.tsx) both render it, so the two surfaces cannot
// drift. Participants arrive fully resolved (name/muted/stale/self) — the window
// side has no member records or profile registry of its own.

import type { HuddleParticipant } from "../../store/huddle-roster";
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

function RowAvatar({ name, size, ring }: { name: string; size: number; ring: string }) {
  return (
    <span
      aria-hidden="true"
      style={{
        width: size,
        height: size,
        borderRadius: "50%",
        background: color.chip,
        color: color.muted3,
        border: `2px solid ${ring}`,
        boxSizing: "border-box",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        font: `600 9.5px ${font.sans}`,
        flexShrink: 0,
      }}
    >
      {initialsOf(name)}
    </span>
  );
}

/** The roster: one row per member — avatar, name (+ "you"), a muted-mic glyph
 *  when known-muted, and a "remove" control on a stale (signal-lost) member.
 *  Capped at `maxRows`, with a "+N more" tail. This is what makes mute + sweep
 *  reachable in an audio-only huddle, where no video tiles render. */
function Roster({
  participants,
  ring,
  maxRows,
  onSweep,
}: {
  participants: HuddleParticipant[];
  ring: string;
  maxRows: number;
  onSweep?: (user: number[]) => void;
}) {
  if (participants.length === 0) {
    return <span style={{ font: `400 11px ${font.sans}`, color: color.muted2 }}>Connecting…</span>;
  }
  if (participants.length === 1 && participants[0].isSelf) {
    return <span style={{ font: `400 11px ${font.sans}`, color: color.muted2 }}>Just you</span>;
  }
  const shown = participants.slice(0, maxRows);
  const extra = participants.length - shown.length;
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
      {shown.map((p) => (
        <div key={p.key} style={{ display: "flex", alignItems: "center", gap: 7, minWidth: 0 }}>
          {/* A green ring while speaking (self only — peer speaking isn't known). */}
          <RowAvatar name={p.name} size={22} ring={p.speaking ? color.green : ring} />
          <span
            title={p.name}
            style={{
              flex: 1,
              minWidth: 0,
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
              font: `${p.isSelf ? 600 : 500} 11.5px ${font.sans}`,
              color: color.ink,
            }}
          >
            {p.name}
            {p.isSelf && <span style={{ color: color.muted2, fontWeight: 400 }}> · you</span>}
          </span>
          {p.muted && (
            <span title="Muted" style={{ color: color.muted2, flexShrink: 0, display: "flex" }}>
              <MicGlyph size={13} muted />
            </span>
          )}
          {p.stale && onSweep && (
            <HoverButton
              onClick={() => onSweep(p.user)}
              title={`No signal from ${p.name} — remove from huddle`}
              style={{
                display: "inline-flex",
                alignItems: "center",
                padding: "1px 6px",
                borderRadius: 999,
                background: color.dangerSoft,
                color: color.danger,
                border: `1px solid ${color.dangerBorder}`,
                font: `600 9.5px ${font.sans}`,
                flexShrink: 0,
              }}
              hoverStyle={{ filter: "brightness(1.05)" }}
            >
              remove
            </HoverButton>
          )}
        </div>
      ))}
      {extra > 0 && (
        <span style={{ font: `500 10.5px ${font.sans}`, color: color.muted2, paddingLeft: 29 }}>
          +{extra} more
        </span>
      )}
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
  /** The roster, fully resolved (name/muted/stale/self). Self included. */
  participants: HuddleParticipant[];
  /** Ring color behind the row avatars — the card's own surface color. */
  ring?: string;
  /** Max roster rows before the "+N more" tail — the narrow dock fits fewer than
   *  the popped window. */
  maxRows?: number;
  onSetMuted(muted: boolean): void;
  onLeave(): void;
  onRetry(): void;
  /** Remove a stale (signal-lost) member from the huddle (SweepHuddle). */
  onSweep?(user: number[]): void;
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
  maxRows = 5,
  onSetMuted,
  onLeave,
  onRetry,
  onSweep,
  onPopOut,
  onPopIn,
}: HuddleCardProps) {
  const dot = STATUS_DOT[status] ?? STATUS_DOT.idle;
  const live = status === "live";
  const failure = status === "error" ? (error ?? "connection") : null;
  const pop = onPopOut ?? onPopIn;
  // Talking into a muted mic — the single most common "why can't they hear me".
  const mutedWhileTalking = participants.some((p) => p.isSelf && p.muted && p.speaking);

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

      {!failure && mutedWhileTalking && (
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 6,
            padding: "5px 8px",
            borderRadius: radius.sm,
            background: color.dangerSoft,
            border: `1px solid ${color.dangerBorder}`,
            color: color.danger,
            font: `600 11px ${font.sans}`,
          }}
        >
          <MicGlyph size={13} muted />
          You&rsquo;re muted
        </div>
      )}

      {!failure && (
        <Roster participants={participants} ring={ring} maxRows={maxRows} onSweep={onSweep} />
      )}

      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <div style={{ flex: 1, minWidth: 0 }} />

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
