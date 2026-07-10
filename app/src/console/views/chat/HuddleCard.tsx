// The huddle session card BODY — status dot + channel, the notice rows
// (error / muted-while-talking / transient media note), and the participant
// roster (name + per-member mute glyph + a stale-member "remove" control).
// Purely presentational (props + callbacks, no store). The in-app dock
// (Huddle.tsx) renders the whole card; the stage and the popped-out window
// (views/huddle/) compose the exported pieces (CardNotices, Roster) under
// their own headers — one implementation, three surfaces, no drift. Media
// controls (mute/camera/leave) live in the shared HuddleControls bar and view
// controls (expand/pop) in each container's header. Participants arrive fully
// resolved (name/muted/stale/self); the window side has no member records or
// profile registry of its own.

import type { HuddleParticipant } from "../../store/huddle-roster";
import { isMacDesktop } from "../../../domain/node-bootstrap";
import type { VoiceError } from "../../../domain/voice-session";
import { color, font, radius } from "../../theme/tokens";
import { HoverButton } from "./HoverButton";

export type HuddleStatus = "idle" | "connecting" | "reconnecting" | "live" | "error";

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

const STATUS_DOT: Record<HuddleStatus, { color: string; pulse: boolean }> = {
  connecting: { color: color.amber, pulse: true },
  reconnecting: { color: color.amber, pulse: true },
  live: { color: color.green, pulse: false },
  error: { color: color.red, pulse: false },
  idle: { color: color.muted2, pulse: false },
};

// What the card says per failure. macOS records a mic denial permanently (it
// never re-prompts), so `mic-denied` must route through System Settings there —
// for dev runs the OS attributes the prompt to the launching terminal, not the
// app. No such Settings pane exists elsewhere (Linux is granted in-app by the
// shell's WebKitGTK hook, Windows by the WebView2 prompt), so non-mac copy
// points at the platform's own mic permissions without naming a macOS pane.
const MIC_DENIED_COPY = isMacDesktop()
  ? "Mic access is blocked — allow it in System Settings, then retry."
  : "Mic access is blocked — check the system's microphone permissions, then retry.";

export const ERROR_COPY: Record<VoiceError, string> = {
  "mic-denied": MIC_DENIED_COPY,
  "mic-missing": "No usable microphone found.",
  "mic-failed": "Mic setup failed.",
  connection: "Voice connection failed.",
  refused: "Couldn't join this huddle.",
  removed: "You're no longer in this huddle — another member or device removed you.",
};

/** Copy for a transient camera/screen acquire failure — the call is fine, the
 *  lane just stayed off; say why instead of a button that silently snaps back. */
const MEDIA_NOTE_COPY: Record<"camera-failed" | "screen-failed", string> = {
  "camera-failed": "Camera didn't start — check camera permissions.",
  "screen-failed": "Screen share didn't start — permission was denied or cancelled.",
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

/** The failure/notice rows shared by every huddle surface: the error copy, the
 *  muted-while-talking banner, and the transient media note. The dock renders
 *  them inside HuddleCard; the stage and the popped window (which own their own
 *  headers) render them directly so the three surfaces say the same things. */
export function CardNotices({
  failure,
  mutedWhileTalking,
  mediaNote,
}: {
  failure: VoiceError | null;
  mutedWhileTalking: boolean;
  mediaNote?: "camera-failed" | "screen-failed" | null;
}) {
  return (
    <>
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

      {!failure && mediaNote && (
        <div
          style={{
            padding: "5px 8px",
            borderRadius: radius.sm,
            background: color.sunken,
            border: `1px solid ${color.borderSoft}`,
            color: color.inkSoft,
            font: `500 11px/1.4 ${font.sans}`,
          }}
        >
          {MEDIA_NOTE_COPY[mediaNote]}
        </div>
      )}
    </>
  );
}

/** The roster: one row per member — avatar, name (+ "you"), a muted-mic glyph
 *  when known-muted, and a "remove" control on a stale (signal-lost) member.
 *  Capped at `maxRows`, with a "+N more" tail. This is what makes mute + sweep
 *  reachable in an audio-only huddle, where no video tiles render. Exported for
 *  the popped window, which composes it under its own header. */
export function Roster({
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
  // Show the first `maxRows` members, PLUS any stale (removable) members that
  // fall in the overflow — otherwise a dead member past the cap could never be
  // swept. "+N more" then counts only the remaining non-actionable rows.
  const head = participants.slice(0, maxRows);
  const hiddenStale = participants.slice(maxRows).filter((p) => p.stale);
  const shown = [...head, ...hiddenStale];
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
  /** A transient camera/screen acquire failure to surface (auto-cleared by the
   *  store). Never fatal — renders as a quiet note, not the error row. */
  mediaNote?: "camera-failed" | "screen-failed" | null;
  /** The roster, fully resolved (name/muted/stale/self). Self included. */
  participants: HuddleParticipant[];
  /** Ring color behind the row avatars — the card's own surface color. */
  ring?: string;
  /** Max roster rows before the "+N more" tail — the narrow dock fits fewer than
   *  the popped window. */
  maxRows?: number;
  /** Remove a stale (signal-lost) member from the huddle (SweepHuddle). */
  onSweep?(user: number[]): void;
}

export function HuddleCard({
  channelName,
  status,
  error,
  mediaNote = null,
  participants,
  ring = color.paper,
  maxRows = 5,
  onSweep,
}: HuddleCardProps) {
  const dot = STATUS_DOT[status] ?? STATUS_DOT.idle;
  const failure = status === "error" ? (error ?? "connection") : null;
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
            {status === "connecting"
              ? "connecting…"
              : status === "reconnecting"
                ? "reconnecting…"
                : `${participants.length}`}
          </span>
        )}
      </div>

      <CardNotices failure={failure} mutedWhileTalking={mutedWhileTalking} mediaNote={mediaNote} />

      {!failure && (
        <Roster participants={participants} ring={ring} maxRows={maxRows} onSweep={onSweep} />
      )}
    </div>
  );
}
