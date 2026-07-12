// The Slack-huddle surface over a chat channel's voice roster: a header control
// to join/leave, a rail indicator on channels with a live huddle, and the
// bottom-left dock for the session you're in. All roster reads come from
// `channel.huddle` (committed consensus state); whether WE are in a live audio
// session comes from the ephemeral `voice` slice. The dock composes the shared
// HuddleCard (status + roster) + CallTiles (video strip) + HuddleControls (the
// media bar) — the same pieces the full stage and popped window use. Every
// affordance is hidden when the daemon can't do voice (no status.publicKey).

import { useCallback, useEffect, useState } from "react";
import type { CSSProperties, ReactNode } from "react";

import { MAX_VIDEO_PARTICIPANTS } from "../../../domain/call-session";
import { keyHex } from "../../../domain/chat-client";
import type { Channel } from "../../../domain/chat-client";
import { isTauri } from "../../../domain/node-bootstrap";
import { buildParticipants } from "../../store/huddle-roster";
import { useDucktape } from "../../store/use-ducktape";
import { accentVar, color, font, radius } from "../../theme/tokens";
import { HoverButton } from "./HoverButton";
import { HuddleCard } from "./HuddleCard";
import { CallTiles } from "../huddle/CallTiles";
import { DevicesMenu } from "../huddle/DevicesMenu";
import { HuddleControls } from "../huddle/HuddleControls";
import { HuddleStage } from "../huddle/HuddleStage";

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

function ExpandGlyph({ size = 14 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round">
      <path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5" />
    </svg>
  );
}

/** Arrow leaving a box — open the huddle in its own window. */
function PopGlyph({ size = 13 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round">
      <path d="M9 5H6a2 2 0 0 0-2 2v11a2 2 0 0 0 2 2h11a2 2 0 0 0 2-2v-3" />
      <path d="M13 11l7-7" />
      <path d="M14.5 4H20v5.5" />
    </svg>
  );
}

/** A small square icon button for the dock header's view cluster (expand / pop). */
function HeaderIconButton({ title, onClick, children }: { title: string; onClick: () => void; children: ReactNode }) {
  return (
    <HoverButton
      onClick={onClick}
      title={title}
      style={{ display: "flex", alignItems: "center", justifyContent: "center", width: 24, height: 22, borderRadius: radius.sm, color: color.muted2 }}
      hoverStyle={{ background: color.hover, color: color.ink }}
    >
      {children}
    </HoverButton>
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
    // The accent fill does NOT invert with the theme, so its text must not either:
    // `color.onDark` (--c-on-filled) flips to near-black in dark mode — 3.33:1 on
    // the accent, below AA. Literal white, as everywhere else we paint on accent.
    ? { ...base, background: accentVar, color: "#fff" }
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
 *  (voice.popped) and to the full-window stage (local `expanded`). Thin wrapper
 *  so the card and its per-session staleness tick mount fresh on each join,
 *  keyed by channel. */
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
  // Encode gates the CAMERA (send); decode gates peer-tile RENDERING — they can
  // diverge (a box may decode but not encode). See domain/video-capability.ts.
  const canEncode = state.videoCapability.canEncode;
  const canDecode = state.videoCapability.canDecode;
  const overCap = roster.length > MAX_VIDEO_PARTICIPANTS;
  // Self is matched by node hex (already-lowercase, like the beacon keys).
  const selfHex = (state.status?.publicKey ?? "").toLowerCase();

  // The tile strip is up while OUR camera is on, or a peer's beacon says
  // camera-on and we can actually decode it (else that peer shows a roster row).
  const showTiles =
    voice.cameraOn ||
    voice.sharing ||
    (canDecode &&
      roster.some((m) => {
        const b = voice.peers[keyHex(m.node)];
        return b?.cameraOn || b?.sharing;
      }));

  // The in-app "stage" — expand the compact dock into a full-window gallery /
  // spotlight. Local state, so it resets on join (the card is keyed by channel)
  // and never conflicts with the popped window (which owns its own surface).
  const [expanded, setExpanded] = useState(false);
  const [devicesOpen, setDevicesOpen] = useState(false);

  // Staleness is time-driven, so re-render once a second WHILE this compact dock
  // is the visible surface — not while popped (the popped window drives its own
  // re-push tick) and not while expanded (the stage owns its own tick; a second
  // timer here would just re-render the stage subtree needlessly).
  const [nowTick, setNowTick] = useState(() => Date.now());
  useEffect(() => {
    if (voice.popped || expanded) return;
    const id = setInterval(() => setNowTick(Date.now()), 1000);
    return () => clearInterval(id);
  }, [voice.popped, expanded]);

  // Stable ref binders so the 1 Hz tick never detaches the tile canvases.
  const bindPreview = useCallback(
    (el: HTMLVideoElement | null) => actions.getCallSession()?.bindPreview(el),
    [actions],
  );
  const bindTile = useCallback(
    (nodeHex: string, el: HTMLCanvasElement | null) => actions.getCallSession()?.bindTile(nodeHex, el),
    [actions],
  );

  // Yield to the popped-out window (it mirrors the same session). Every hook
  // above runs first, so this early return is rules-of-hooks safe.
  if (!voice.channelId || voice.popped) return null;
  const channelId = voice.channelId;

  // Expanded: the stage OWNS the tile/preview bindings, so the compact dock must
  // not also render its tiles — return the stage alone.
  if (expanded) return <HuddleStage onCollapse={() => setExpanded(false)} />;

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
  const memberNodes = Object.fromEntries(roster.map((m) => [keyHex(m.user), keyHex(m.node)]));

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
        position: "relative",
        display: "flex",
        flexDirection: "column",
        gap: 8,
      }}
    >
      {/* Status + roster body, with the view cluster (expand / pop) in the header
          row — media controls live in the bottom bar. */}
      <div style={{ display: "flex", alignItems: "flex-start", gap: 6 }}>
        <div style={{ flex: 1, minWidth: 0 }}>
          <HuddleCard
            channelName={channel?.name ?? channelId}
            status={voice.status}
            error={voice.error}
            errorNote={voice.errorNote}
            mediaNote={voice.mediaNote}
            participants={participants}
            ring={color.paper}
            maxRows={4}
            onSweep={(user) => actions.sweepHuddle(channelId, user)}
          />
        </div>
        <div style={{ display: "flex", gap: 2, flexShrink: 0 }}>
          <HeaderIconButton title="Expand to full stage" onClick={() => setExpanded(true)}>
            <ExpandGlyph size={14} />
          </HeaderIconButton>
          {/* Pop-out hands the MEDIA session to the window — there is nothing to
              hand off from an errored session, so the control hides with it. */}
          {isTauri() && voice.status !== "error" && (
            <HeaderIconButton title="Open in window" onClick={() => actions.popOutHuddle()}>
              <PopGlyph size={13} />
            </HeaderIconButton>
          )}
        </div>
      </div>

      {showTiles && (
        <CallTiles
          layout="strip"
          participants={participants}
          memberNodes={memberNodes}
          peers={voice.peers}
          canEncode={canEncode}
          canDecode={canDecode}
          selfCameraOn={voice.cameraOn}
          selfSharing={voice.sharing}
          bindPreview={bindPreview}
          bindTile={bindTile}
          // +1 so the self tile rides on top of the peer cap, matching the old
          // dock (self shown separately, peers capped at MAX_VIDEO_PARTICIPANTS).
          maxTiles={MAX_VIDEO_PARTICIPANTS + 1}
        />
      )}

      <HuddleControls
        size="compact"
        status={voice.status}
        muted={voice.muted}
        cameraOn={voice.cameraOn}
        canEncode={canEncode}
        cameraDisabledReason={overCap ? "Video is capped at 8 participants" : undefined}
        sharing={voice.sharing}
        canScreenShare={state.videoCapability.canScreenShare && !overCap}
        onToggleScreen={() => actions.setScreenShare(!voice.sharing)}
        onOpenDevices={() => setDevicesOpen((v) => !v)}
        onToggleMute={() => actions.setHuddleMuted(!voice.muted)}
        onToggleCamera={() => actions.setCamera(!voice.cameraOn)}
        onLeave={() => actions.leaveHuddle()}
        onRetry={() => actions.joinHuddle(channelId)}
      />

      {devicesOpen && (
        // Anchored ABOVE the card, not over it — the roster, tiles and mute
        // state stay visible while devices are being switched.
        <div style={{ position: "absolute", left: 0, right: 0, bottom: "calc(100% + 6px)", zIndex: 5 }}>
          <DevicesMenu onClose={() => setDevicesOpen(false)} />
        </div>
      )}
    </div>
  );
}
