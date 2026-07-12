// The expanded huddle "stage" — a full-window video surface with a gallery grid
// and a spotlight mode (one big tile + filmstrip, following the active speaker).
// Reuses the shared roster projection (buildParticipants), the shared CallTiles
// renderer, and the shared HuddleControls media bar — the SAME call session as
// the dock (bindTile/bindPreview via getCallSession), so the media session stays
// single-owner in this main window. Rendered ONLY while expanded, so it and the
// compact dock never both bind the (single) preview/tile canvases. The header
// carries the view controls (gallery/spotlight toggle, pop-out, collapse); the
// bottom bar carries the media controls.

import { useCallback, useEffect, useState } from "react";
import type { CSSProperties } from "react";

import { MAX_VIDEO_PARTICIPANTS } from "../../../domain/call-session";
import { keyHex } from "../../../domain/chat-client";
import { buildParticipants } from "../../store/huddle-roster";
import { isTauri } from "../../../domain/node-bootstrap";
import { useDucktape } from "../../store/use-ducktape";
import { accentVar, color, font, radius } from "../../theme/tokens";
import { HoverButton } from "../chat/HoverButton";
import { CardNotices } from "../chat/HuddleCard";
import { CallTiles } from "./CallTiles";
import { DevicesMenu } from "./DevicesMenu";
import { HuddleControls } from "./HuddleControls";
import { SelfCheck } from "./SelfCheck";

// ── glyphs (view controls, kept local) ──────────────────

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

// ── the stage ───────────────────────────────────────────

export function HuddleStage({ onCollapse }: { onCollapse: () => void }) {
  const { state, actions } = useDucktape();
  const { voice, videoCapability } = state;
  const [mode, setMode] = useState<"gallery" | "spotlight">("gallery");
  const [pinned, setPinned] = useState<string | null>(null);
  const [devicesOpen, setDevicesOpen] = useState(false);
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
  const memberNodes = Object.fromEntries(
    (channel?.huddle ?? []).map((m) => [keyHex(m.user), keyHex(m.node)]),
  );

  const bindPreview = useCallback(
    (el: HTMLVideoElement | null) => actions.getCallSession()?.bindPreview(el),
    [actions],
  );
  const bindTile = useCallback(
    (nodeHex: string, el: HTMLCanvasElement | null) => actions.getCallSession()?.bindTile(nodeHex, el),
    [actions],
  );

  const live = voice.status === "live";
  // Alone = no one else on the roster: either the roster hasn't settled yet
  // (length 0) or the only member is us. Drives the self-check surface.
  const solo = participants.length === 0 || (participants.length === 1 && participants[0].isSelf);
  const overCap = (channel?.huddle?.length ?? 0) > MAX_VIDEO_PARTICIPANTS;

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
      {/* header — view controls */}
      <div style={{ display: "flex", alignItems: "center", gap: 10, padding: "10px 14px", borderBottom: `1px solid ${color.borderSoft}` }}>
        <span
          aria-label={voice.status}
          style={{ width: 9, height: 9, borderRadius: "50%", background: live ? color.green : voice.status === "error" ? color.red : color.amber }}
        />
        <span style={{ font: `600 14px ${font.sans}`, color: color.ink }}>#{channel?.name ?? voice.channelId}</span>
        <span style={{ font: `500 11px ${font.sans}`, color: color.muted2 }}>{participants.length} in call</span>
        <div style={{ marginLeft: "auto", display: "flex", gap: 8 }}>
          <HoverButton
            onClick={() => {
              // Leaving spotlight also clears the pin, so the next spotlight
              // follows the active speaker again instead of a stale choice.
              if (mode === "spotlight") setPinned(null);
              setMode(mode === "gallery" ? "spotlight" : "gallery");
            }}
            title={mode === "gallery" ? "Spotlight view" : "Gallery view"}
            style={barBtn(false)}
            hoverStyle={{ background: color.hover }}
          >
            {mode === "gallery" ? <SpotlightGlyph /> : <GridGlyph />}
            {mode === "gallery" ? "Spotlight" : "Gallery"}
          </HoverButton>
          {isTauri() && live && (
            <HoverButton onClick={() => actions.popOutHuddle()} title="Pop out to a window" style={barBtn(false)} hoverStyle={{ background: color.hover }}>
              Pop out
            </HoverButton>
          )}
          <HoverButton onClick={onCollapse} title="Collapse to dock" style={barBtn(false)} hoverStyle={{ background: color.hover }}>
            <CollapseGlyph /> Collapse
          </HoverButton>
        </div>
      </div>

      {/* the shared notice rows (error / muted-while-talking / media note),
          centered over their own strip so the stage says the same things the
          dock card does. */}
      {(voice.status === "error" || (voice.muted && voice.speaking) || voice.mediaNote) && (
        <div style={{ display: "flex", justifyContent: "center", padding: "8px 14px 0" }}>
          <div style={{ display: "flex", flexDirection: "column", gap: 6, minWidth: 260, maxWidth: 420 }}>
            <CardNotices
              failure={voice.status === "error" ? (voice.error ?? "connection") : null}
              failureNote={voice.errorNote}
              mutedWhileTalking={voice.muted && voice.speaking}
              mediaNote={voice.mediaNote}
            />
          </div>
        </div>
      )}

      {/* tiles */}
      <div style={{ flex: 1, minHeight: 0, padding: 14, overflow: "auto" }}>
        {solo ? (
          // Alone in the huddle: a device self-check (camera preview + mic meter)
          // instead of a bare "connecting…" — the local media works even before
          // the session is server-live, so the user can verify their gear while
          // they wait for others.
          <SelfCheck
            status={voice.status}
            cameraOn={voice.cameraOn}
            sharing={voice.sharing}
            canEncode={videoCapability.canEncode}
            muted={voice.muted}
            level={voice.level}
            speaking={voice.speaking}
            bindPreview={bindPreview}
            onToggleCamera={() => actions.setCamera(!voice.cameraOn)}
          />
        ) : (
          <CallTiles
            layout={mode}
            participants={participants}
            memberNodes={memberNodes}
            peers={voice.peers}
            canEncode={videoCapability.canEncode}
            canDecode={videoCapability.canDecode}
            selfCameraOn={voice.cameraOn}
            selfSharing={voice.sharing}
            bindPreview={bindPreview}
            bindTile={bindTile}
            pinned={pinned}
            onPin={(key) => {
              // Double-clicking the pinned tile again unpins (back to
              // speaker-follow); any other tile moves the pin.
              setPinned((prev) => (prev === key ? null : key));
              setMode("spotlight");
            }}
          />
        )}
      </div>

      {devicesOpen && (
        <div style={{ position: "absolute", bottom: 66, left: "50%", transform: "translateX(-50%)", width: 260, zIndex: 5 }}>
          <DevicesMenu onClose={() => setDevicesOpen(false)} />
        </div>
      )}

      {/* control bar — media controls. The bar spans a fixed comfortable width
          so Leave's own margin can actually isolate it at the far right. */}
      <div style={{ display: "flex", alignItems: "center", justifyContent: "center", padding: "12px 14px", borderTop: `1px solid ${color.borderSoft}` }}>
        <div style={{ flex: 1, maxWidth: 460 }}>
          <HuddleControls
            size="comfortable"
            status={voice.status}
            muted={voice.muted}
            cameraOn={voice.cameraOn}
            canEncode={videoCapability.canEncode}
            cameraDisabledReason={overCap ? "Video is capped at 8 participants" : undefined}
            sharing={voice.sharing}
            canScreenShare={videoCapability.canScreenShare && !overCap}
            onToggleScreen={() => actions.setScreenShare(!voice.sharing)}
            onOpenDevices={() => setDevicesOpen((v) => !v)}
            onToggleMute={() => actions.setHuddleMuted(!voice.muted)}
            onToggleCamera={() => actions.setCamera(!voice.cameraOn)}
            onLeave={() => actions.leaveHuddle()}
            onRetry={() => {
              if (voice.channelId) actions.joinHuddle(voice.channelId);
            }}
          />
        </div>
      </div>
    </div>
  );
}
