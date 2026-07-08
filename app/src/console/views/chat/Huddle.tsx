// The Slack-huddle surface over a chat channel's voice roster: a header control
// to join/leave, a rail indicator on channels with a live huddle, and the
// bottom-left dock for the session you're in. All roster reads come from
// `channel.huddle` (committed consensus state); whether WE are in a live audio
// session comes from the ephemeral `voice` slice. The card body itself lives in
// HuddleCard.tsx, shared with the popped-out huddle window. Every affordance is
// hidden when the daemon can't do voice (no status.publicKey).

import { useCallback, useEffect, useState } from "react";
import type { CSSProperties } from "react";

import { MAX_VIDEO_PARTICIPANTS } from "../../../domain/call-session";
import { authorName, keyHex } from "../../../domain/chat-client";
import type { Channel, HuddleMember } from "../../../domain/chat-client";
import { isTauri } from "../../../domain/node-bootstrap";
import { buildParticipants } from "../../store/huddle-roster";
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

// ── Video tiles ──────────────────────────────────────────

const tileGrid: CSSProperties = {
  display: "grid",
  gridTemplateColumns: "repeat(2, 1fr)",
  gap: 6,
};

const tileFrame: CSSProperties = {
  position: "relative",
  aspectRatio: "16 / 9",
  borderRadius: radius.sm,
  overflow: "hidden",
  background: color.dark,
  border: `1px solid ${color.borderSoft}`,
};

const tileMedia: CSSProperties = {
  width: "100%",
  height: "100%",
  objectFit: "cover",
  display: "block",
};

const tileIdle: CSSProperties = {
  width: "100%",
  height: "100%",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
};

const tileName: CSSProperties = {
  position: "absolute",
  left: 4,
  bottom: 4,
  maxWidth: "calc(100% - 8px)",
  display: "inline-flex",
  alignItems: "center",
  gap: 3,
  padding: "2px 6px",
  borderRadius: 999,
  background: "rgba(38,37,31,.62)",
  color: color.onDark,
  font: `600 10px ${font.sans}`,
};

const tileNameText: CSSProperties = {
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
};

/** Our own camera preview — the raw local stream, bound to the live session so
 *  it renders whatever `setCamera` acquired. Muted (it's our own audio) and
 *  autoplaying (bindPreview only sets srcObject, it never calls play). */
function SelfTile() {
  const { state, actions } = useDucktape();
  // Pin the ref callback so the 1 s staleness tick doesn't rebind every render.
  const bindPreview = useCallback(
    (el: HTMLVideoElement | null) => actions.getCallSession()?.bindPreview(el),
    [actions],
  );
  const speaking = state.voice.speaking;
  return (
    <div style={speaking ? { ...tileFrame, border: `2px solid ${color.green}` } : tileFrame}>
      <video ref={bindPreview} muted autoPlay playsInline style={tileMedia} />
      <span style={tileName}>
        <span style={tileNameText}>You</span>
      </span>
    </div>
  );
}

function PeerTile({
  member,
  names,
  canDecode,
}: {
  member: HuddleMember;
  names: Record<string, string>;
  canDecode: boolean;
}) {
  const { state, actions } = useDucktape();
  // Beacons key by NODE hex, so two users huddling from one daemon share a
  // beacon; the tile itself is keyed by USER (unique) upstream.
  const nodeHex = keyHex(member.node);
  const beacon = state.voice.peers[nodeHex];
  const name = memberName(member, names);
  // Only paint a <canvas> for a peer we can actually decode — a WebKitGTK viewer
  // with no vp8 DECODER would otherwise show a black tile. Fall back to the
  // initials avatar. Pin the ref callback so the 1 s tick doesn't rebind (and
  // briefly drop) the peer's canvas every render.
  const showVideo = canDecode && !!beacon?.cameraOn;
  const bindTile = useCallback(
    (canvas: HTMLCanvasElement | null) => actions.getCallSession()?.bindTile(nodeHex, canvas),
    [actions, nodeHex],
  );
  return (
    <div style={tileFrame}>
      {showVideo ? (
        <canvas ref={bindTile} style={tileMedia} />
      ) : (
        <div style={tileIdle}>
          <ParticipantAvatar name={name} size={34} ring={color.sunken} />
        </div>
      )}
      <span style={tileName}>
        <span style={tileNameText}>{name}</span>
        {/* Known-muted only: an absent beacon is "unknown", not muted. */}
        {beacon?.muted && <MicGlyph size={10} muted />}
      </span>
    </div>
  );
}

/** The tile grid: our preview (while our camera is on) plus one tile per OTHER
 *  roster member, capped at MAX_VIDEO_PARTICIPANTS with a "+N more" tail so a
 *  larger huddle doesn't silently drop its overflow. Self is matched by node, so
 *  a co-located second local user folds into our preview. */
function TileGrid({
  roster,
  selfHex,
  cameraOn,
  names,
  canDecode,
}: {
  roster: HuddleMember[];
  selfHex: string;
  cameraOn: boolean;
  names: Record<string, string>;
  canDecode: boolean;
}) {
  const others = roster.filter((m) => keyHex(m.node) !== selfHex);
  const peers = others.slice(0, MAX_VIDEO_PARTICIPANTS);
  const overflow = others.length - peers.length;
  return (
    <div>
      <div style={tileGrid}>
        {cameraOn && <SelfTile />}
        {peers.map((m) => (
          <PeerTile key={keyHex(m.user)} member={m} names={names} canDecode={canDecode} />
        ))}
      </div>
      {overflow > 0 && (
        <div style={{ marginTop: 4, font: `500 10px ${font.sans}`, color: color.muted2 }}>
          +{overflow} more not shown
        </div>
      )}
    </div>
  );
}

// ── Bottom-left dock ─────────────────────────────────────

/** The persistent session card, docked at the foot of the channel rail while
 *  we're in a huddle: an optional video-tile grid over the shared HuddleCard
 *  (status + roster with mute/sweep + mute/leave), plus the main-window camera
 *  toggle. Yields entirely to the popped-out huddle window (voice.popped). Thin
 *  wrapper so the card and its per-session staleness tick mount fresh on each
 *  join, keyed by channel. */
export function HuddleDock() {
  const { state } = useDucktape();
  if (!state.voice.channelId) return null;
  return <HuddleDockCard key={state.voice.channelId} />;
}

function HuddleDockCard() {
  const { state, actions } = useDucktape();
  const { voice } = state;

  const channel = state.channels.find((c) => c.id === voice.channelId);
  const roster = channel?.huddle ?? [];
  const live = voice.status === "live";
  // Encode gates the CAMERA (send); decode gates peer-tile RENDERING — they can
  // diverge (a box may decode but not encode). See domain/video-capability.ts.
  const canEncode = state.videoCapability.canEncode;
  const canDecode = state.videoCapability.canDecode;
  const overCap = roster.length > MAX_VIDEO_PARTICIPANTS;
  // Self is matched by node hex (already-lowercase, like the beacon keys).
  const selfHex = (state.status?.publicKey ?? "").toLowerCase();

  // The grid is up while OUR camera is on, or a peer's beacon says camera-on and
  // we can actually decode it (else that peer shows an avatar row, no tile).
  const showTiles =
    voice.cameraOn ||
    (canDecode && roster.some((m) => voice.peers[keyHex(m.node)]?.cameraOn));

  // Staleness is time-driven, so re-render once a second WHILE in a huddle to
  // re-evaluate the roster's sweep affordances (and refresh the tile grid).
  const [nowTick, setNowTick] = useState(() => Date.now());
  useEffect(() => {
    const id = setInterval(() => setNowTick(Date.now()), 1000);
    return () => clearInterval(id);
  }, []);

  // Yield to the popped-out window (it mirrors the same HuddleCard). Every hook
  // above runs first, so this early return is rules-of-hooks safe.
  if (!voice.channelId || voice.popped) return null;
  const channelId = voice.channelId;

  const participants = buildParticipants({
    roster,
    peers: voice.peers,
    selfNodeHex: selfHex,
    authorNames: state.authorNames,
    selfMuted: voice.muted,
    selfSpeaking: voice.speaking,
    sessionStartMs: voice.sessionStartMs,
    now: nowTick,
  });

  return (
    <div
      title={canEncode ? undefined : "Camera needs a VP8 video encoder on this system"}
      style={{
        margin: "8px 8px 2px",
        maxWidth: 340,
        padding: "9px 10px",
        borderRadius: radius.md,
        background: color.paper,
        border: `1px solid ${color.borderStrong}`,
        boxShadow: "0 1px 2px rgba(40,38,34,.05)",
      }}
    >
      {showTiles && (
        <div style={{ marginBottom: 8 }}>
          <TileGrid
            roster={roster}
            selfHex={selfHex}
            cameraOn={voice.cameraOn}
            names={state.authorNames}
            canDecode={canDecode}
          />
        </div>
      )}

      <HuddleCard
        channelName={channel?.name ?? channelId}
        status={voice.status}
        error={voice.error}
        muted={voice.muted}
        participants={participants}
        ring={color.paper}
        maxRows={4}
        onSetMuted={(muted) => actions.setHuddleMuted(muted)}
        onLeave={() => actions.leaveHuddle()}
        onRetry={() => actions.joinHuddle(channelId)}
        onSweep={(user) => actions.sweepHuddle(channelId, user)}
        onPopOut={isTauri() ? () => actions.popOutHuddle() : undefined}
      />

      {/* Camera toggle lives in the dock next to the card (never forked into
          HuddleCard, so the popped window keeps rendering it unmodified). Gated
          on ENCODE capability + live + cap-8. */}
      {canEncode && (
        <div style={{ display: "flex", justifyContent: "flex-end", marginTop: 8 }}>
          <HoverButton
            onClick={() => actions.setCamera(!voice.cameraOn)}
            title={
              overCap
                ? "Video is capped at 8 participants"
                : voice.cameraOn
                  ? "Turn camera off"
                  : "Turn camera on"
            }
            disabled={!live || overCap}
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              width: 30,
              height: 28,
              borderRadius: radius.sm,
              border: `1px solid ${voice.cameraOn ? "transparent" : color.borderSoft}`,
              background: voice.cameraOn ? accentVar : color.sunken,
              color: voice.cameraOn ? color.onDark : color.muted2,
            }}
            hoverStyle={{ filter: "brightness(1.05)" }}
          >
            <CameraGlyph size={15} off={!voice.cameraOn} />
          </HoverButton>
        </div>
      )}
    </div>
  );
}
