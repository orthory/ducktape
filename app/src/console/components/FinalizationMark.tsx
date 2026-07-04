// The inline finalization mark — the view half of the "preconfirmed render
// first, confirm inclusion separately" contract (store/finalization.ts).
//
// A row whose entity has an OpRecord shows: a pulsing dot while the write is
// in flight (the row itself IS the preconfirmed render), a quiet checkmark
// once the node's receipt lands — no toast — or a small cross on rejection.
// Hovering the settled mark reveals the inclusion facts: the block height the
// op landed at and, when the node returned one, the op's addressable hash
// (sha256 of the committed payload — fetchable via the node's blob lane).

import { useState } from "react";

import type { OpRecord } from "../store/finalization";
import { color, font, radius, shadow } from "../theme/tokens";
import { Icon } from "./Icon";

export function FinalizationMark({
  op,
  size = 11,
}: {
  /** The entity's ledger record; rows without one render nothing (committed
   *  rows written elsewhere / before this session). */
  op: OpRecord | undefined;
  size?: number;
}) {
  const [hover, setHover] = useState(false);
  if (!op) return null;

  const label =
    op.phase === "pending"
      ? "awaiting inclusion"
      : op.phase === "finalized"
        ? op.height !== undefined
          ? `included at height ${op.height}`
          : "included"
        : "rejected";

  return (
    <span
      aria-label={label}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        position: "relative",
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
        width: size + 2,
        height: size + 2,
        flexShrink: 0,
      }}
    >
      {op.phase === "pending" ? (
        <span
          data-mark="pending"
          style={{
            width: 6,
            height: 6,
            borderRadius: "50%",
            background: color.muted2,
            animation: "ik-pulse 1s ease-in-out infinite",
          }}
        />
      ) : op.phase === "finalized" ? (
        <Icon name="check" size={size} color={color.accentAlt2} strokeWidth={2.2} />
      ) : (
        <Icon name="close" size={size} color={color.danger} strokeWidth={2.2} />
      )}

      {hover && (
        <span
          role="tooltip"
          style={{
            position: "absolute",
            bottom: "calc(100% + 6px)",
            left: "50%",
            transform: "translateX(-50%)",
            zIndex: 60,
            minWidth: 120,
            maxWidth: 260,
            padding: "6px 8px",
            background: color.paper,
            border: `1px solid ${color.borderStrong}`,
            borderRadius: radius.sm,
            boxShadow: shadow.pop,
            font: `400 10px/1.5 ${font.mono}`,
            color: color.inkSoft,
            textAlign: "left",
            pointerEvents: "none",
            whiteSpace: "nowrap",
          }}
        >
          <span style={{ display: "block" }}>{label}</span>
          {op.phase === "finalized" && op.opHash && (
            <span
              style={{
                display: "block",
                marginTop: 3,
                color: color.muted3,
                whiteSpace: "normal",
                wordBreak: "break-all",
              }}
            >
              op {op.opHash}
            </span>
          )}
          {op.phase === "failed" && op.error && (
            <span
              style={{
                display: "block",
                marginTop: 3,
                color: color.danger,
                whiteSpace: "normal",
                wordBreak: "break-word",
              }}
            >
              {op.error}
            </span>
          )}
        </span>
      )}
    </span>
  );
}
