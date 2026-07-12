// The inline finalization mark — the view half of the "preconfirmed render
// first, confirm inclusion separately" contract (store/finalization.ts).
//
// A row whose entity has an OpRecord shows: a single muted check while the
// write is in flight (sent — the row itself IS the preconfirmed render), a
// double check once the node's receipt lands (confirmed) — no toast — or a
// small cross on rejection. Hovering shows the short status; CLICKING opens
// the stats popover: when the op was sent, when it confirmed (plus the
// sent→confirmed latency), the inclusion height, the op's addressable hash
// (sha256 of the committed payload — fetchable via the node's blob lane), and
// a button that jumps to the explorer opened on that block (openExplorerAt).
// The store context is read as OPTIONAL: the mark renders on every operation
// surface, and a bare render (tests, previews) must keep the stats popover
// but drop the explorer jump, not throw for a missing provider.
//
// Two addressing modes, one per surface kind:
//   - `op`: the entity row already holds its ledger record (opKey/opForMessage).
//   - `hash`: the surface only knows a content address (a 64-hex sha256) —
//     the mark resolves the record from the session ledger itself (opByHash).

import { useContext, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

import { ConsoleContext } from "../store/context";
import { opByHash } from "../store/finalization";
import type { OpRecord } from "../store/finalization";
import { color, font, radius, shadow } from "../theme/tokens";
import { Icon } from "./Icon";

// ── Formatting (pure) ───────────────────────────────────

/** Wall-clock ms → local HH:MM:SS — ops settle in seconds, so the popover
 *  needs second precision where the chat meta line does not. */
const clockOf = (ms: number): string =>
  new Date(ms).toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  });

const latencyOf = (ms: number): string =>
  ms < 1_000 ? `+${ms} ms` : `+${(ms / 1_000).toFixed(1)} s`;

const labelOf = (op: OpRecord): string => {
  switch (op.phase) {
    case "pending":
      return "sent — awaiting confirmation";
    case "finalized":
      return op.height !== undefined
        ? `confirmed at height ${op.height}`
        : "confirmed";
    default:
      return "rejected";
  }
};

// ── Popover placement ───────────────────────────────────

type Anchor = { left: number; top?: number; bottom?: number };

/** Fixed-position coords beside the mark, clamped horizontally and flipped
 *  above→below near the window top, so the portal box can never clip
 *  regardless of where in the layout the mark sits. */
const anchorFor = (rect: DOMRect, maxWidth: number, flipBelow: number): Anchor => {
  const left = Math.max(8, Math.min(rect.left, window.innerWidth - maxWidth - 8));
  return rect.top < flipBelow
    ? { left, top: rect.bottom + 6 }
    : { left, bottom: window.innerHeight - rect.top + 6 };
};

const TIP_MAX = 280;
const POP_MAX = 320;

// ── The mark ────────────────────────────────────────────

type MarkSource =
  | {
      /** The entity's ledger record; rows without one render nothing
       *  (committed rows written elsewhere / before this session). */
      op: OpRecord | undefined;
      hash?: undefined;
    }
  | {
      /** A content address (the op's 64-hex sha256) — resolved against the
       *  session ledger; addresses it never saw render nothing. */
      hash: string;
      op?: undefined;
    };

export function FinalizationMark({
  op: opProp,
  hash,
  size = 11,
}: MarkSource & { size?: number }) {
  const [tip, setTip] = useState<Anchor | null>(null);
  const [pop, setPop] = useState<Anchor | null>(null);
  const ref = useRef<HTMLSpanElement>(null);
  const popRef = useRef<HTMLSpanElement>(null);
  const store = useContext(ConsoleContext);
  const op =
    opProp ?? (hash !== undefined && store ? opByHash(store.state.ops, hash) : undefined);

  // Dismiss the stats popover on outside click / Escape — it is interactive
  // (unlike the hover tip), so it must outlive the hover and close on intent.
  useEffect(() => {
    if (!pop) return;
    const onDown = (event: MouseEvent) => {
      const target = event.target as Node;
      if (ref.current?.contains(target) || popRef.current?.contains(target)) return;
      setPop(null);
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setPop(null);
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [pop]);

  if (!op) return null;

  const label = labelOf(op);

  const showTip = () => {
    const el = ref.current;
    if (!el) return;
    setTip(anchorFor(el.getBoundingClientRect(), TIP_MAX, 96));
  };
  const hideTip = () => setTip(null);

  const togglePop = (event: { stopPropagation(): void }) => {
    // Marks sit inside clickable rows — opening the stats must not also fire
    // the row's own open action.
    event.stopPropagation();
    setTip(null);
    const el = ref.current;
    if (!el) return;
    // taller box than the tip — flip below within more of the window top
    setPop(pop ? null : anchorFor(el.getBoundingClientRect(), POP_MAX, 200));
  };

  // The explorer jump needs both an inclusion height and a live store.
  const jumpHeight =
    store && op.phase === "finalized" && op.height !== undefined ? op.height : null;
  const openInExplorer =
    jumpHeight === null
      ? undefined
      : (event: { stopPropagation(): void }) => {
          event.stopPropagation();
          setPop(null);
          store?.actions.openExplorerAt(jumpHeight);
        };

  const statRow = (key: string, value: string) => (
    <span style={{ display: "block", whiteSpace: "nowrap" }}>
      <span style={{ color: color.muted3 }}>{key} </span>
      {value}
    </span>
  );

  return (
    <span
      ref={ref}
      aria-label={label}
      aria-expanded={pop !== null}
      role="button"
      tabIndex={0}
      onClick={togglePop}
      onKeyDown={(event) => {
        if (event.key !== "Enter" && event.key !== " ") return;
        event.preventDefault();
        togglePop(event);
      }}
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
        cursor: "pointer",
      }}
    >
      {op.phase === "pending" ? (
        <Icon
          name="check"
          size={size}
          color={color.muted2}
          strokeWidth={2.2}
          style={{ animation: "ik-pulse 1s ease-in-out infinite" }}
        />
      ) : op.phase === "finalized" ? (
        <Icon name="checks" size={size} color={color.accentAlt2} strokeWidth={2.2} />
      ) : (
        <Icon name="close" size={size} color={color.danger} strokeWidth={2.2} />
      )}

      {tip &&
        !pop &&
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
            <span style={{ display: "block", marginTop: 3, color: color.muted3 }}>
              click for details
            </span>
          </span>,
          document.body,
        )}

      {pop &&
        createPortal(
          <span
            ref={popRef}
            role="dialog"
            aria-label={`operation stats — ${label}`}
            // interactive (unlike the tip): it hosts the explorer button
            onClick={(event) => event.stopPropagation()}
            style={{
              position: "fixed",
              left: pop.left,
              top: pop.top,
              bottom: pop.bottom,
              zIndex: 90,
              maxWidth: POP_MAX,
              padding: "8px 10px",
              background: color.paper,
              border: `1px solid ${color.borderStrong}`,
              borderRadius: radius.sm,
              boxShadow: shadow.pop,
              font: `400 10px/1.7 ${font.mono}`,
              color: color.inkSoft,
              textAlign: "left",
            }}
          >
            <span style={{ display: "block", whiteSpace: "nowrap", fontWeight: 600 }}>
              {label}
            </span>
            {statRow("sent", clockOf(op.startedAt))}
            {op.settledAt !== undefined &&
              statRow(
                op.phase === "failed" ? "rejected" : "confirmed",
                `${clockOf(op.settledAt)} (${latencyOf(op.settledAt - op.startedAt)})`,
              )}
            {op.phase === "finalized" &&
              op.height !== undefined &&
              statRow("height", String(op.height))}
            {op.phase === "finalized" && op.opHash && (
              <span
                style={{
                  display: "block",
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
                  color: color.danger,
                  whiteSpace: "normal",
                  wordBreak: "break-word",
                }}
              >
                {op.error}
              </span>
            )}
            {openInExplorer && (
              <button
                type="button"
                onClick={openInExplorer}
                style={{
                  display: "block",
                  marginTop: 6,
                  padding: "3px 8px",
                  background: color.sunken,
                  border: `1px solid ${color.borderStrong}`,
                  borderRadius: radius.sm,
                  font: `500 10px ${font.mono}`,
                  color: color.inkSoft,
                  cursor: "pointer",
                }}
              >
                view in explorer
              </button>
            )}
          </span>,
          document.body,
        )}
    </span>
  );
}
