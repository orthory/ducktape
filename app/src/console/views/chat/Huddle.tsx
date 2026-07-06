// The Slack-huddle surface over a chat channel's voice roster: a header control
// to join/leave, a rail indicator on channels with a live huddle, and the
// bottom-left dock for the session you're in. All roster reads come from
// `channel.huddle` (committed consensus state); whether WE are in a live audio
// session comes from the ephemeral `voice` slice. The card body itself lives in
// HuddleCard.tsx, shared with the popped-out huddle window. Every affordance is
// hidden when the daemon can't do voice (no status.publicKey).

import type { CSSProperties } from "react";

import type { Channel } from "../../../domain/chat-client";
import { isTauri } from "../../../domain/node-bootstrap";
import { buildHuddleWindowState } from "../../store/huddle-window";
import { useDucktape } from "../../store/use-ducktape";
import { accentVar, color, font, radius } from "../../theme/tokens";
import { HoverButton } from "./HoverButton";
import { HuddleCard } from "./HuddleCard";

// ── Local glyph (Icon.tsx isn't ours to extend — same pattern as MessageItem) ──

function HeadphonesGlyph({ size = 14 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.7} strokeLinecap="round" strokeLinejoin="round">
      <path d="M4 13v-2a8 8 0 0 1 16 0v2" />
      <rect x="3" y="13" width="4.5" height="7" rx="1.6" />
      <rect x="16.5" y="13" width="4.5" height="7" rx="1.6" />
    </svg>
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

/** The persistent session card, docked at the foot of the channel rail while
 *  we're in a huddle. Yields entirely to the popped-out huddle window
 *  (voice.popped) — that window mirrors the same HuddleCard. */
export function HuddleDock() {
  const { state, actions } = useDucktape();
  const { voice } = state;
  if (!voice.channelId || voice.popped) return null;

  const card = buildHuddleWindowState(voice, state.channels, state.authorNames);
  if (!card) return null;
  const channelId = voice.channelId;

  return (
    <div
      style={{
        margin: "8px 8px 2px",
        padding: "9px 10px",
        borderRadius: radius.md,
        background: color.paper,
        border: `1px solid ${color.borderStrong}`,
        boxShadow: "0 1px 2px rgba(40,38,34,.05)",
      }}
    >
      <HuddleCard
        channelName={card.channelName}
        status={card.status}
        error={card.error}
        muted={card.muted}
        participants={card.participants}
        ring={color.paper}
        onSetMuted={(muted) => actions.setHuddleMuted(muted)}
        onLeave={() => actions.leaveHuddle()}
        onRetry={() => actions.joinHuddle(channelId)}
        onPopOut={isTauri() ? () => actions.popOutHuddle() : undefined}
      />
    </div>
  );
}
