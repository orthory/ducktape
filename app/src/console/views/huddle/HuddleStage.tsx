// The expanded huddle "stage" — a full-window video surface with a gallery grid
// and a spotlight mode (one big tile + filmstrip, following the active speaker).
// Reuses the shared roster projection (buildParticipants) and the SAME call
// session as the dock (bindTile/bindPreview via getCallSession) — the media
// session stays single-owner in this main window. Rendered ONLY while expanded,
// so it and the compact dock never both bind the (single) preview/tile canvases.
//
// Video vs avatar per tile is decode/encode-capability aware: self shows its
// preview only when it can ENCODE; a peer shows its canvas only when we can
// DECODE and its beacon says camera-on. Everything else is the initials avatar.

import { useCallback, useEffect, useState } from "react";
import type { CSSProperties } from "react";

import { keyHex } from "../../../domain/chat-client";
import { buildParticipants } from "../../store/huddle-roster";
import type { HuddleParticipant } from "../../store/huddle-roster";
import { isTauri } from "../../../domain/node-bootstrap";
import { useDucktape } from "../../store/use-ducktape";
import { accentVar, color, font, radius } from "../../theme/tokens";
import { HoverButton } from "../chat/HoverButton";
import { galleryColumns, spotlightKey } from "./huddle-stage-layout";

// ── glyphs (kept local, matching the dock) ──────────────

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
function CameraGlyph({ size = 16, off = false }: { size?: number; off?: boolean }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.7} strokeLinecap="round" strokeLinejoin="round">
      <rect x="2.5" y="6.5" width="12" height="11" rx="2.2" />
      <path d="M14.5 10.5l6-3v9l-6-3z" />
      {off && <path d="M4 4l16 16" strokeWidth={1.9} />}
    </svg>
  );
}
function GridGlyph({ size = 15 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round">
      <rect x="3" y="3" width="7" height="7" rx="1.5" /><rect x="14" y="3" width="7" height="7" rx="1.5" />
      <rect x="3" y="14" width="7" height="7" rx="1.5" /><rect x="14" y="14" width="7" height="7" rx="1.5" />
    </svg>
  );
}
function SpotlightGlyph({ size = 15 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round">
      <rect x="3" y="4" width="18" height="12" rx="1.6" /><path d="M7 20h10" />
    </svg>
  );
}
function CollapseGlyph({ size = 15 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round">
      <path d="M9 9L4 4M9 9V5M9 9H5" /><path d="M15 15l5 5M15 15v4M15 15h4" />
    </svg>
  );
}

const initialsOf = (name: string): string => name.slice(0, 2).toUpperCase();

// ── one tile ────────────────────────────────────────────

/** A stage tile: peer canvas / self preview when video is available, else a big
 *  initials avatar. Reads its own beacon + binds to the shared call session, the
 *  same way the dock's tiles do. `pinned`-aware ring for the spotlight target. */
function StageTile({
  member,
  big,
  onPin,
}: {
  member: HuddleParticipant;
  big: boolean;
  onPin?: () => void;
}) {
  const { state, actions } = useDucktape();
  const { voice, videoCapability } = state;
  // The tile's live camera state comes from the node beacon (peer) or our own
  // slice (self). Beacons key by NODE hex; find this member's node via the roster.
  const memberNodeHex = memberNodeOf(state, member.key);
  const beacon = memberNodeHex ? voice.peers[memberNodeHex] : undefined;
  const selfVideo = member.isSelf && voice.cameraOn && videoCapability.canEncode;
  const peerVideo = !member.isSelf && videoCapability.canDecode && !!beacon?.cameraOn;

  const bindPreview = useCallback(
    (el: HTMLVideoElement | null) => actions.getCallSession()?.bindPreview(el),
    [actions],
  );
  const bindTile = useCallback(
    (canvas: HTMLCanvasElement | null) =>
      memberNodeHex ? actions.getCallSession()?.bindTile(memberNodeHex, canvas) : undefined,
    [actions, memberNodeHex],
  );

  const frame: CSSProperties = {
    position: "relative",
    width: "100%",
    height: "100%",
    minHeight: big ? 0 : 84,
    borderRadius: radius.md,
    overflow: "hidden",
    background: color.dark,
    border: `2px solid ${member.speaking ? color.green : "transparent"}`,
    boxSizing: "border-box",
  };
  const media: CSSProperties = { width: "100%", height: "100%", objectFit: "cover", display: "block" };

  return (
    <div style={frame} onDoubleClick={onPin} title={onPin ? "Double-click to spotlight" : undefined}>
      {selfVideo ? (
        <video ref={bindPreview} muted autoPlay playsInline style={media} />
      ) : peerVideo ? (
        <canvas ref={bindTile} style={media} />
      ) : (
        <div style={{ width: "100%", height: "100%", display: "flex", alignItems: "center", justifyContent: "center" }}>
          <span
            aria-hidden="true"
            style={{
              width: big ? 96 : 44,
              height: big ? 96 : 44,
              borderRadius: "50%",
              background: color.sunken,
              color: color.muted3,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              font: `600 ${big ? 30 : 15}px ${font.sans}`,
            }}
          >
            {initialsOf(member.name)}
          </span>
        </div>
      )}
      <span
        style={{
          position: "absolute",
          left: 6,
          bottom: 6,
          maxWidth: "calc(100% - 12px)",
          display: "inline-flex",
          alignItems: "center",
          gap: 4,
          padding: "2px 7px",
          borderRadius: 999,
          background: "rgba(38,37,31,.62)",
          color: color.onDark,
          font: `600 ${big ? 12 : 10.5}px ${font.sans}`,
        }}
      >
        <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
          {member.isSelf ? "You" : member.name}
        </span>
        {member.muted && <MicGlyph size={big ? 12 : 10} muted />}
      </span>
      {member.stale && (
        <span
          style={{
            position: "absolute",
            top: 6,
            right: 6,
            padding: "2px 7px",
            borderRadius: 999,
            background: color.danger,
            color: "#fff",
            font: `600 9.5px ${font.sans}`,
          }}
        >
          no signal
        </span>
      )}
    </div>
  );
}

/** Resolve a participant (user-keyed) back to its NODE hex via the roster — the
 *  beacons + tile bindings are node-keyed. */
function memberNodeOf(state: ReturnType<typeof useDucktape>["state"], userKey: string): string | undefined {
  const channel = state.channels.find((c) => c.id === state.voice.channelId);
  const m = (channel?.huddle ?? []).find((mm) => keyHex(mm.user) === userKey);
  return m ? keyHex(m.node) : undefined;
}

// ── the stage ───────────────────────────────────────────

export function HuddleStage({ onCollapse }: { onCollapse: () => void }) {
  const { state, actions } = useDucktape();
  const { voice, videoCapability } = state;
  const [mode, setMode] = useState<"gallery" | "spotlight">("gallery");
  const [pinned, setPinned] = useState<string | null>(null);
  const [nowTick, setNowTick] = useState(() => Date.now());
  useEffect(() => {
    const id = setInterval(() => setNowTick(Date.now()), 1000);
    return () => clearInterval(id);
  }, []);

  const channel = state.channels.find((c) => c.id === voice.channelId);
  const selfHex = (state.status?.publicKey ?? "").toLowerCase();
  const participants = buildParticipants({
    roster: channel?.huddle ?? [],
    peers: voice.peers,
    selfNodeHex: selfHex,
    authorNames: state.authorNames,
    selfMuted: voice.muted,
    selfSpeaking: voice.speaking,
    sessionStartMs: voice.sessionStartMs,
    now: nowTick,
  });

  const live = voice.status === "live";
  const spot = spotlightKey(participants, pinned);
  const spotMember = participants.find((p) => p.key === spot);
  const others = participants.filter((p) => p.key !== spot);
  const cols = galleryColumns(participants.length);

  const barBtn = (activeOn: boolean): CSSProperties => ({
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    gap: 6,
    height: 36,
    minWidth: 36,
    padding: "0 12px",
    borderRadius: radius.md,
    border: `1px solid ${activeOn ? "transparent" : color.borderSoft}`,
    background: activeOn ? accentVar : color.sunken,
    color: activeOn ? color.onDark : color.inkSoft,
    font: `600 12px ${font.sans}`,
  });

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        zIndex: 60,
        background: color.paper,
        display: "flex",
        flexDirection: "column",
      }}
    >
      {/* header */}
      <div style={{ display: "flex", alignItems: "center", gap: 10, padding: "10px 14px", borderBottom: `1px solid ${color.borderSoft}` }}>
        <span
          aria-label={voice.status}
          style={{ width: 9, height: 9, borderRadius: "50%", background: live ? color.green : voice.status === "error" ? color.red : color.amber }}
        />
        <span style={{ font: `600 14px ${font.sans}`, color: color.ink }}>#{channel?.name ?? voice.channelId}</span>
        <span style={{ font: `500 11px ${font.sans}`, color: color.muted2 }}>{participants.length} in call</span>
        <div style={{ marginLeft: "auto", display: "flex", gap: 8 }}>
          <HoverButton
            onClick={() => setMode(mode === "gallery" ? "spotlight" : "gallery")}
            title={mode === "gallery" ? "Spotlight view" : "Gallery view"}
            style={barBtn(false)}
            hoverStyle={{ background: color.hover }}
          >
            {mode === "gallery" ? <SpotlightGlyph /> : <GridGlyph />}
            {mode === "gallery" ? "Spotlight" : "Gallery"}
          </HoverButton>
          <HoverButton onClick={onCollapse} title="Collapse to dock" style={barBtn(false)} hoverStyle={{ background: color.hover }}>
            <CollapseGlyph /> Collapse
          </HoverButton>
        </div>
      </div>

      {/* tiles */}
      <div style={{ flex: 1, minHeight: 0, padding: 14, overflow: "auto" }}>
        {participants.length === 0 ? (
          <div style={{ height: "100%", display: "flex", alignItems: "center", justifyContent: "center", color: color.muted2, font: `500 13px ${font.sans}` }}>
            connecting…
          </div>
        ) : mode === "gallery" ? (
          <div style={{ display: "grid", gridTemplateColumns: `repeat(${cols}, 1fr)`, gap: 10, height: "100%", gridAutoRows: "1fr" }}>
            {participants.map((p) => (
              <StageTile key={p.key} member={p} big={false} onPin={() => { setPinned(p.key); setMode("spotlight"); }} />
            ))}
          </div>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: 10, height: "100%" }}>
            <div style={{ flex: 1, minHeight: 0 }}>
              {spotMember && <StageTile key={spotMember.key} member={spotMember} big />}
            </div>
            {others.length > 0 && (
              <div style={{ display: "flex", gap: 8, height: 96, flexShrink: 0, overflowX: "auto" }}>
                {others.map((p) => (
                  <div key={p.key} style={{ width: 150, flexShrink: 0 }}>
                    <StageTile member={p} big={false} onPin={() => setPinned(p.key)} />
                  </div>
                ))}
              </div>
            )}
          </div>
        )}
      </div>

      {/* control bar */}
      <div style={{ display: "flex", alignItems: "center", justifyContent: "center", gap: 10, padding: "12px 14px", borderTop: `1px solid ${color.borderSoft}` }}>
        <HoverButton
          onClick={() => actions.setHuddleMuted(!voice.muted)}
          title={voice.muted ? "Unmute" : "Mute"}
          disabled={!live}
          style={{ ...barBtn(false), background: voice.muted ? color.dangerSoft : color.sunken, color: voice.muted ? color.danger : color.inkSoft, border: `1px solid ${voice.muted ? color.dangerBorder : color.borderSoft}` }}
          hoverStyle={{ filter: "brightness(1.04)" }}
        >
          <MicGlyph muted={voice.muted} /> {voice.muted ? "Muted" : "Mute"}
        </HoverButton>

        {videoCapability.canEncode && (
          <HoverButton
            onClick={() => actions.setCamera(!voice.cameraOn)}
            title={voice.cameraOn ? "Turn camera off" : "Turn camera on"}
            disabled={!live}
            style={barBtn(voice.cameraOn)}
            hoverStyle={{ filter: "brightness(1.04)" }}
          >
            <CameraGlyph off={!voice.cameraOn} /> {voice.cameraOn ? "Camera on" : "Camera"}
          </HoverButton>
        )}

        {isTauri() && (
          <HoverButton onClick={() => actions.popOutHuddle()} title="Pop out to a window" style={barBtn(false)} hoverStyle={{ background: color.hover }}>
            Pop out
          </HoverButton>
        )}

        <HoverButton
          onClick={() => actions.leaveHuddle()}
          title="Leave huddle"
          style={{ ...barBtn(false), background: color.danger, color: "#fff", border: "1px solid transparent" }}
          hoverStyle={{ filter: "brightness(1.06)" }}
        >
          Leave
        </HoverButton>
      </div>
    </div>
  );
}
