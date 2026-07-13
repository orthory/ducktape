import { useLayoutEffect, useState, type RefObject } from "react";

import { keyBytes } from "../../../domain/chat-client";
import type { RemotePageCursor } from "../../../domain/page-presence";
import { Avatar } from "../chat/MessageItem";
import { color, font } from "../../theme/tokens";

export interface PagePresencePeer extends RemotePageCursor {
  name: string;
}

const PALETTE = ["#7c3aed", "#0f766e", "#c2410c", "#be123c", "#1d4ed8"];
const peerColor = (peer: string) => {
  let hash = 0;
  for (const char of peer) hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
  return PALETTE[hash % PALETTE.length]!;
};

export function PagePresenceBar({ peers }: { peers: PagePresencePeer[] }) {
  if (peers.length === 0) return null;
  return (
    <div
      role="group"
      aria-label={`${peers.length} other editor${peers.length === 1 ? "" : "s"} here`}
      title={peers.map((peer) => peer.name).join(", ")}
      style={{ display: "flex", alignItems: "center", paddingLeft: 5 }}
    >
      {peers.slice(0, 4).map((peer, index) => (
        <span
          key={peer.peer}
          style={{
            display: "inline-flex",
            marginLeft: index === 0 ? 0 : -7,
            border: `2px solid ${color.paper}`,
            borderRadius: "50%",
            boxShadow: `0 0 0 1px ${peerColor(peer.peer)}33`,
          }}
        >
          <Avatar author={{ user: keyBytes(peer.peer) }} name={peer.name} size={24} />
        </span>
      ))}
      {peers.length > 4 ? (
        <span style={{ marginLeft: 4, font: `600 10px ${font.mono}`, color: color.muted2 }}>
          +{peers.length - 4}
        </span>
      ) : null}
    </div>
  );
}

interface Point {
  peer: PagePresencePeer;
  x: number;
  y: number;
  height: number;
}

const caretPoint = (
  area: HTMLInputElement | HTMLTextAreaElement,
  row: HTMLElement,
  offset: number,
): Omit<Point, "peer"> => {
  const computed = getComputedStyle(area);
  const mirror = document.createElement("div");
  Object.assign(mirror.style, {
    position: "fixed",
    left: "-10000px",
    top: "0",
    visibility: "hidden",
    width: `${area.clientWidth}px`,
    boxSizing: computed.boxSizing,
    padding: computed.padding,
    border: computed.border,
    font: computed.font,
    letterSpacing: computed.letterSpacing,
    lineHeight: computed.lineHeight,
    tabSize: computed.tabSize,
    whiteSpace: "pre-wrap",
    overflowWrap: "break-word",
  });
  mirror.textContent = area.value.slice(0, Math.min(offset, area.value.length));
  const marker = document.createElement("span");
  marker.textContent = "\u200b";
  mirror.append(marker);
  document.body.append(mirror);
  const markerRect = marker.getBoundingClientRect();
  const mirrorRect = mirror.getBoundingClientRect();
  const areaRect = area.getBoundingClientRect();
  const rowRect = row.getBoundingClientRect();
  const lineHeight = Number.parseFloat(computed.lineHeight);
  const point = {
    x: areaRect.left - rowRect.left + markerRect.left - mirrorRect.left - area.scrollLeft,
    y: areaRect.top - rowRect.top + markerRect.top - mirrorRect.top - area.scrollTop,
    height: Number.isFinite(lineHeight) ? lineHeight : Math.max(16, markerRect.height),
  };
  mirror.remove();
  return point;
};

export function RemoteCursors({
  peers,
  areaRef,
  rowRef,
  text,
}: {
  peers: PagePresencePeer[];
  areaRef: RefObject<HTMLInputElement | HTMLTextAreaElement | null>;
  rowRef: RefObject<HTMLDivElement | null>;
  text: string;
}) {
  const [points, setPoints] = useState<Point[]>([]);
  useLayoutEffect(() => {
    const measure = () => {
      const area = areaRef.current;
      const row = rowRef.current;
      if (!area || !row) return setPoints([]);
      setPoints(peers.map((peer) => ({ peer, ...caretPoint(area, row, peer.head) })));
    };
    measure();
    window.addEventListener("resize", measure);
    return () => window.removeEventListener("resize", measure);
  }, [peers, areaRef, rowRef, text]);
  return (
    <>
      {points.map(({ peer, x, y, height }) => {
        const accent = peerColor(peer.peer);
        return (
          <span
            key={peer.peer}
            data-peer-cursor={peer.peer}
            aria-label={`${peer.name}'s cursor`}
            style={{
              position: "absolute",
              zIndex: 8,
              left: x,
              top: y,
              width: 2,
              height,
              background: accent,
              pointerEvents: "none",
            }}
          >
            <span
              style={{
                position: "absolute",
                left: 0,
                top: -18,
                maxWidth: 120,
                padding: "2px 5px",
                borderRadius: "5px 5px 5px 0",
                background: accent,
                color: "#fff",
                font: `600 9.5px ${font.sans}`,
                whiteSpace: "nowrap",
                overflow: "hidden",
                textOverflow: "ellipsis",
              }}
            >
              {peer.name}
            </span>
          </span>
        );
      })}
    </>
  );
}
