// The inline finalization mark — the view half of the "preconfirmed render
// first, confirm inclusion separately" contract (store/finalization.ts).
//
// A row whose entity has an OpRecord shows: a pulsing dot while the write is
// in flight (the row itself IS the preconfirmed render), a quiet checkmark
// once the node's receipt lands — no toast — or a small cross on rejection.
// Hovering the settled mark reveals the inclusion facts: the block height the
// op landed at and, when the node returned one, the op's addressable hash
// (sha256 of the committed payload — fetchable via the node's blob lane).
// Clicking a mark that knows its height jumps to the explorer opened on that
// block (openExplorerAt). The store context is read as OPTIONAL: the mark
// renders on every operation surface, and a bare render (tests, previews)
// must stay a passive indicator, not throw for a missing provider.

import { useContext, useRef, useState } from "react";
import { createPortal } from "react-dom";

import { ConsoleContext } from "../store/context";
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
  const [tip, setTip] = useState<{ left: number; top?: number; bottom?: number } | null>(null);
  const ref = useRef<HTMLSpanElement>(null);
  const store = useContext(ConsoleContext);
  if (!op) return null;

  // The tooltip renders to the document body (not in-flow) so it escapes the
  // chat/list panes' `overflow:hidden` — the old absolute box was cropped at
  // the left pane edge, mangling the text. Coords come from the mark's viewport
  // rect, clamped horizontally and flipped above→below near the window top, so
  // it can never clip regardless of where in the layout the mark sits.
  const TIP_MAX = 280;
  const showTip = () => {
    const el = ref.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    const left = Math.max(8, Math.min(r.left, window.innerWidth - TIP_MAX - 8));
    setTip(
      r.top < 96
        ? { left, top: r.bottom + 6 }
        : { left, bottom: window.innerHeight - r.top + 6 },
    );
  };
  const hideTip = () => setTip(null);

  const label =
    op.phase === "pending"
      ? "awaiting inclusion"
      : op.phase === "finalized"
        ? op.height !== undefined
          ? `included at height ${op.height}`
          : "included"
        : "rejected";

  // The cross-link needs both an inclusion height and a live store.
  const jumpHeight =
    store && op.phase === "finalized" && op.height !== undefined ? op.height : null;
  const openInExplorer =
    jumpHeight === null
      ? undefined
      : (event: { stopPropagation(): void }) => {
          // Marks sit inside clickable rows — the jump must not also fire the
          // row's own open action.
          event.stopPropagation();
          store?.actions.openExplorerAt(jumpHeight);
        };

  return (
    <span
      ref={ref}
      aria-label={label}
      role={openInExplorer ? "button" : undefined}
      tabIndex={openInExplorer ? 0 : undefined}
      onClick={openInExplorer}
      onKeyDown={
        openInExplorer === undefined
          ? undefined
          : (event) => {
              if (event.key !== "Enter" && event.key !== " ") return;
              event.preventDefault();
              openInExplorer(event);
            }
      }
      onMouseEnter={showTip}
      onMouseLeave={hideTip}
      onFocus={showTip}
      onBlur={hideTip}
      style={{
        position: "relative",
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
        width: size + 2,
        height: size + 2,
        flexShrink: 0,
        cursor: openInExplorer ? "pointer" : undefined,
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

      {tip &&
        createPortal(
          <span
            role="tooltip"
            style={{
              position: "fixed",
              left: tip.left,
              top: tip.top,
              bottom: tip.bottom,
              zIndex: 90,
              maxWidth: TIP_MAX,
              padding: "6px 9px",
              background: color.paper,
              border: `1px solid ${color.borderStrong}`,
              borderRadius: radius.sm,
              boxShadow: shadow.pop,
              font: `400 10px/1.55 ${font.mono}`,
              color: color.inkSoft,
              textAlign: "left",
              pointerEvents: "none",
            }}
          >
            <span style={{ display: "block", whiteSpace: "nowrap" }}>{label}</span>
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
            {openInExplorer && (
              <span style={{ display: "block", marginTop: 3, color: color.muted3 }}>
                click to view in explorer
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
          </span>,
          document.body,
        )}
    </span>
  );
}
