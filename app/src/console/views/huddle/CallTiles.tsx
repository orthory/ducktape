// The single huddle tile renderer, shared by the dock (compact "strip"), the
// full stage ("gallery" / "spotlight"), and the popped window. One StageTile
// implementation replaces the dock's old TileGrid and the stage's inline tiles,
// so the video surface can never drift between them. Store-free: the container
// resolves participants → node hex (memberNodes) and passes the session's
// bindPreview/bindTile so this file never touches the store.

import { useCallback } from "react";
import type { CSSProperties } from "react";

import type { HuddleParticipant, PeerBeacon } from "../../store/huddle-roster";
import { color, font, radius } from "../../theme/tokens";
import { galleryColumns, spotlightKey } from "./huddle-stage-layout";

const initialsOf = (name: string): string => name.slice(0, 2).toUpperCase();

const media: CSSProperties = { width: "100%", height: "100%", objectFit: "cover", display: "block" };

/** A muted-mic glyph for a tile's name pill — the video-surface equivalent of the
 *  roster's per-member mute indicator (the stage has no roster fallback). */
function MicGlyph({ size = 11 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={1.9} strokeLinecap="round" strokeLinejoin="round">
      <rect x="9" y="3" width="6" height="11" rx="3" />
      <path d="M5.5 11a6.5 6.5 0 0 0 13 0" />
      <path d="M12 17.5V21" />
      <path d="M4 4l16 16" />
    </svg>
  );
}

export interface CallTilesProps {
  layout: "strip" | "gallery" | "spotlight";
  participants: HuddleParticipant[];
  /** participant.key (user hex) → node hex, for beacon lookup + tile binding. */
  memberNodes: Record<string, string>;
  /** node hex → latest beacon. */
  peers: Record<string, PeerBeacon>;
  canEncode: boolean;
  canDecode: boolean;
  selfCameraOn: boolean;
  /** Our own video lane is a screen share (letterboxed + labelled). */
  selfSharing?: boolean;
  bindPreview: (el: HTMLVideoElement | null) => void;
  bindTile: (nodeHex: string, el: HTMLCanvasElement | null) => void;
  /** strip cap; overflow surfaces a "+N more not shown" tail. */
  maxTiles?: number;
  /** spotlight: participant.key to feature (else the active speaker / first). */
  pinned?: string | null;
  onPin?: (key: string) => void;
}

export function StageTile({
  member,
  nodeHex,
  beacon,
  canEncode,
  canDecode,
  selfCameraOn,
  selfSharing,
  big,
  bindPreview,
  bindTile,
  onPin,
}: {
  member: HuddleParticipant;
  nodeHex?: string;
  beacon?: PeerBeacon;
  canEncode: boolean;
  canDecode: boolean;
  selfCameraOn: boolean;
  selfSharing: boolean;
  big: boolean;
  bindPreview: (el: HTMLVideoElement | null) => void;
  bindTile: (nodeHex: string, el: HTMLCanvasElement | null) => void;
  onPin?: () => void;
}) {
  // One VP8 lane carries the camera OR a screen share; a screen tile is
  // letterboxed (contain) + labelled so it isn't cropped like a camera tile.
  const isScreen = member.isSelf ? selfSharing : !!beacon?.sharing;
  const selfVideo = member.isSelf && (selfCameraOn || selfSharing) && canEncode;
  const peerVideo = !member.isSelf && canDecode && !!(beacon?.cameraOn || beacon?.sharing);
  const mediaStyle: CSSProperties = isScreen ? { ...media, objectFit: "contain" } : media;
  // Pin the ref callbacks so a container's 1 Hz staleness re-render doesn't
  // detach + reattach (briefly dropping) the <video>/<canvas) every tick.
  const videoRef = useCallback((el: HTMLVideoElement | null) => bindPreview(el), [bindPreview]);
  const canvasRef = useCallback(
    (c: HTMLCanvasElement | null) => {
      if (nodeHex) bindTile(nodeHex, c);
    },
    [bindTile, nodeHex],
  );
  const frame: CSSProperties = {
    position: "relative",
    width: "100%",
    height: "100%",
    minHeight: big ? 0 : 84,
    borderRadius: radius.md,
    overflow: "hidden",
    // scrim, not `color.dark` (= --c-filled, which INVERTS): a video letterbox
    // must stay dark in both themes, or dark mode letterboxes in near-white.
    background: color.scrim,
    border: `2px solid ${member.speaking ? color.green : "transparent"}`,
    boxSizing: "border-box",
  };
  return (
    <div style={frame} onDoubleClick={onPin} title={onPin ? "Double-click to spotlight" : undefined}>
      {selfVideo ? (
        <video ref={videoRef} muted autoPlay playsInline style={mediaStyle} />
      ) : peerVideo && nodeHex ? (
        <canvas ref={canvasRef} style={mediaStyle} />
      ) : (
        <div style={{ width: "100%", height: "100%", display: "flex", alignItems: "center", justifyContent: "center" }}>
          <span
            aria-hidden="true"
            style={{
              width: big ? 96 : 40,
              height: big ? 96 : 40,
              borderRadius: "50%",
              background: color.sunken,
              color: color.muted3,
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              font: `600 ${big ? 30 : 14}px ${font.sans}`,
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
          // an always-dark chip needs always-light text. `color.onDark` is
          // --c-on-filled, which flips to near-BLACK in dark mode — the name
          // then disappeared into the chip.
          background: color.scrimSoft,
          color: color.onScrim,
          font: `600 ${big ? 12 : 10.5}px ${font.sans}`,
        }}
      >
        <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
          {member.isSelf ? "You" : member.name}
        </span>
        {isScreen && (
          <span title="Sharing screen" style={{ opacity: 0.85, fontSize: big ? 10 : 9, letterSpacing: 0.2 }}>
            screen
          </span>
        )}
        {member.muted && (
          <span title="Muted" style={{ display: "flex" }}>
            <MicGlyph size={big ? 12 : 11} />
          </span>
        )}
      </span>
      {member.stale && (
        <span style={{ position: "absolute", top: 6, right: 6, padding: "2px 7px", borderRadius: 999, background: color.danger, color: "#fff", font: `600 9.5px ${font.sans}` }}>
          no signal
        </span>
      )}
    </div>
  );
}

export function CallTiles(props: CallTilesProps) {
  const { layout, participants, memberNodes, peers, maxTiles, pinned, onPin } = props;
  const tile = (member: HuddleParticipant, big: boolean) => {
    const nodeHex = memberNodes[member.key];
    return (
      <StageTile
        key={member.key}
        member={member}
        nodeHex={nodeHex}
        beacon={nodeHex ? peers[nodeHex] : undefined}
        canEncode={props.canEncode}
        canDecode={props.canDecode}
        selfCameraOn={props.selfCameraOn}
        selfSharing={props.selfSharing ?? false}
        big={big}
        bindPreview={props.bindPreview}
        bindTile={props.bindTile}
        onPin={onPin ? () => onPin(member.key) : undefined}
      />
    );
  };

  if (layout === "strip") {
    const cap = maxTiles ?? participants.length;
    const shown = participants.slice(0, cap);
    const overflow = participants.length - shown.length;
    return (
      <div>
        <div style={{ display: "grid", gridTemplateColumns: "repeat(2, 1fr)", gap: 6 }}>
          {shown.map((m) => (
            <div key={m.key} style={{ aspectRatio: "16 / 9" }}>{tile(m, false)}</div>
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

  if (layout === "gallery") {
    const cols = galleryColumns(participants.length);
    return (
      <div style={{ display: "grid", gridTemplateColumns: `repeat(${cols}, 1fr)`, gap: 10, height: "100%", gridAutoRows: "1fr" }}>
        {participants.map((m) => tile(m, false))}
      </div>
    );
  }

  const spot = spotlightKey(participants, pinned ?? null);
  const spotMember = participants.find((m) => m.key === spot);
  const others = participants.filter((m) => m.key !== spot);
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 10, height: "100%" }}>
      <div style={{ flex: 1, minHeight: 0 }}>{spotMember && tile(spotMember, true)}</div>
      {others.length > 0 && (
        <div style={{ display: "flex", gap: 8, height: 96, flexShrink: 0, overflowX: "auto" }}>
          {others.map((m) => (
            <div key={m.key} style={{ width: 150, flexShrink: 0 }}>{tile(m, false)}</div>
          ))}
        </div>
      )}
    </div>
  );
}
