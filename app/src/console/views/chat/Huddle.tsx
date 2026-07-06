// The Slack-huddle surface over a chat channel's voice roster: a header control
// to join/leave, a rail indicator on channels with a live huddle, and the
// bottom-left dock for the session you're in. All roster reads come from
// `channel.huddle` (committed consensus state); whether WE are in a live audio
// session comes from the ephemeral `voice` slice. Styling is inline + tokens,
// matching the rest of the chat surface. Every affordance is hidden when the
// daemon can't do voice (no status.publicKey).

import type { CSSProperties } from "react";

import { authorName, keyHex } from "../../../domain/chat-client";
import type { Channel, HuddleMember } from "../../../domain/chat-client";
import { useDucktape } from "../../store/use-ducktape";
import { accentVar, color, font, radius } from "../../theme/tokens";
import { HoverButton } from "./HoverButton";

// ── Local glyphs (Icon.tsx isn't ours to extend — same pattern as MessageItem) ──

function HeadphonesGlyph({ size = 14 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.7} strokeLinecap="round" strokeLinejoin="round">
      <path d="M4 13v-2a8 8 0 0 1 16 0v2" />
      <rect x="3" y="13" width="4.5" height="7" rx="1.6" />
      <rect x="16.5" y="13" width="4.5" height="7" rx="1.6" />
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

// ── Participant name / avatar ────────────────────────────

/** A huddle member's display name — the `profiles` registry wins, else the
 *  utf-8/hex fallback (the app's user identities are readable origin strings). */
const memberName = (member: HuddleMember, names: Record<string, string>): string =>
  authorName({ user: member.user }, names);

const initialsOf = (name: string): string => name.slice(0, 2).toUpperCase();

function ParticipantAvatar({ name, size, ring }: { name: string; size: number; ring: string }) {
  return (
    <span
      title={name}
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
        font: `600 ${size <= 24 ? 9.5 : 11}px ${font.sans}`,
        flexShrink: 0,
      }}
    >
      {initialsOf(name)}
    </span>
  );
}

/** An overlapping pile of participant initials, capped with a "+N" chip. */
function AvatarPile({
  huddle,
  names,
  size = 24,
  ring = color.sidebar,
  max = 5,
}: {
  huddle: HuddleMember[];
  names: Record<string, string>;
  size?: number;
  ring?: string;
  max?: number;
}) {
  const shown = huddle.slice(0, max);
  const extra = huddle.length - shown.length;
  return (
    <div style={{ display: "flex", alignItems: "center" }}>
      {shown.map((member, i) => (
        // keyed by USER, not node: two users huddling from one daemon share a
        // node key, while the roster is unique per user.
        <span key={keyHex(member.user)} style={{ marginLeft: i === 0 ? 0 : -8, zIndex: shown.length - i }}>
          <ParticipantAvatar name={memberName(member, names)} size={size} ring={ring} />
        </span>
      ))}
      {extra > 0 && (
        <span
          style={{
            marginLeft: -8,
            width: size,
            height: size,
            borderRadius: "50%",
            background: color.sunken,
            color: color.muted3,
            border: `2px solid ${ring}`,
            boxSizing: "border-box",
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            font: `600 ${size <= 24 ? 9.5 : 11}px ${font.sans}`,
          }}
        >
          +{extra}
        </span>
      )}
    </div>
  );
}

// ── Header control ───────────────────────────────────────

/** The channel-header huddle button. Idle: subtle glyph. A huddle you're not in:
 *  glyph + "· N". You're in it: filled accent (click leaves). Hidden when the
 *  daemon can't do voice. */
export function HuddleHeaderButton({ channel }: { channel: Channel }) {
  const { state, actions } = useDucktape();
  if (!state.status?.publicKey) return null;

  const roster = channel.huddle ?? [];
  const count = roster.length;
  const inThis = state.voice.channelId === channel.id;

  const base: CSSProperties = {
    display: "inline-flex",
    alignItems: "center",
    gap: 5,
    marginLeft: "auto",
    padding: count > 0 || inThis ? "4px 9px" : "5px 7px",
    borderRadius: 999,
    font: `600 11px ${font.sans}`,
    whiteSpace: "nowrap",
  };

  const resting: CSSProperties = inThis
    ? { ...base, background: accentVar, color: color.onDark }
    : count > 0
      ? { ...base, background: color.sunken, border: `1px solid ${color.borderSoft}`, color: color.inkSoft }
      : { ...base, color: color.muted };

  const hover: CSSProperties = inThis
    ? { filter: "brightness(1.06)" }
    : { background: color.hover, color: color.ink };

  return (
    <HoverButton
      onClick={() => (inThis ? actions.leaveHuddle() : actions.joinHuddle(channel.id))}
      title={inThis ? "Leave huddle" : count > 0 ? `Join huddle · ${count}` : "Start a huddle"}
      style={resting}
      hoverStyle={hover}
    >
      <HeadphonesGlyph size={14} />
      {(count > 0 || inThis) && <span>· {count}</span>}
    </HoverButton>
  );
}

// ── Channel-rail indicator ───────────────────────────────

/** A small headphones glyph + count shown on a rail row whose channel has a
 *  live huddle. Nothing when the roster is empty or voice is unavailable. */
export function HuddleRailBadge({ channel }: { channel: Channel }) {
  const { state } = useDucktape();
  const count = channel.huddle?.length ?? 0;
  if (!state.status?.publicKey || count === 0) return null;
  const active = state.voice.channelId === channel.id;
  return (
    <span
      title={`${count} in huddle`}
      style={{
        display: "inline-flex",
        alignItems: "center",
        gap: 3,
        flexShrink: 0,
        color: active ? accentVar : color.accentAlt2,
        font: `600 10px ${font.sans}`,
      }}
    >
      <HeadphonesGlyph size={12} />
      {count}
    </span>
  );
}

// ── Bottom-left dock ─────────────────────────────────────

const STATUS_DOT: Record<string, { color: string; pulse: boolean }> = {
  connecting: { color: color.amber, pulse: true },
  live: { color: color.green, pulse: false },
  error: { color: color.red, pulse: false },
  idle: { color: color.muted2, pulse: false },
};

/** The persistent session card, docked at the foot of the channel rail while
 *  we're in a huddle: status dot, channel name, participant pile, mute toggle,
 *  and Leave. */
export function HuddleDock() {
  const { state, actions } = useDucktape();
  const { voice } = state;
  if (!voice.channelId) return null;

  const channel = state.channels.find((c) => c.id === voice.channelId);
  const roster = channel?.huddle ?? [];
  const dot = STATUS_DOT[voice.status] ?? STATUS_DOT.idle;
  const live = voice.status === "live";

  return (
    <div
      style={{
        margin: "8px 8px 2px",
        padding: "9px 10px",
        borderRadius: radius.md,
        background: color.paper,
        border: `1px solid ${color.borderStrong}`,
        boxShadow: "0 1px 2px rgba(40,38,34,.05)",
        display: "flex",
        flexDirection: "column",
        gap: 8,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 7, minWidth: 0 }}>
        <span
          aria-label={voice.status}
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
          #{channel?.name ?? voice.channelId}
        </span>
        <span style={{ font: `500 10.5px ${font.sans}`, color: color.muted2, flexShrink: 0 }}>
          {voice.status === "connecting" ? "connecting…" : `${roster.length}`}
        </span>
      </div>

      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <div style={{ flex: 1, minWidth: 0 }}>
          {roster.length > 0 ? (
            <AvatarPile huddle={roster} names={state.authorNames} size={24} ring={color.paper} />
          ) : (
            <span style={{ font: `400 11px ${font.sans}`, color: color.muted2 }}>Just you</span>
          )}
        </div>

        <HoverButton
          onClick={() => actions.setHuddleMuted(!voice.muted)}
          title={voice.muted ? "Unmute" : "Mute"}
          disabled={!live}
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            width: 30,
            height: 28,
            borderRadius: radius.sm,
            border: `1px solid ${voice.muted ? color.dangerBorder : color.borderSoft}`,
            background: voice.muted ? color.dangerSoft : live ? color.dark : color.sunken,
            color: voice.muted ? color.danger : live ? color.onDark : color.muted2,
          }}
          hoverStyle={{ filter: "brightness(1.05)" }}
        >
          <MicGlyph size={15} muted={voice.muted} />
        </HoverButton>

        <HoverButton
          onClick={() => actions.leaveHuddle()}
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
