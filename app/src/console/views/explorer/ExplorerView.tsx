// The block explorer: recent NON-EMPTY finalized blocks (heartbeat nops never
// reach the app), newest-first. One row per block — height, frame hash, commit
// (post-block app-hash), proposer (the frame's verified signer), op count —
// and clicking a row opens the block: its coordinates in full plus the
// transactions inside (the deterministic dispatch trace + the root op's
// payload). Read-only; records re-pull from the node's ring on every block.

import { useState } from "react";

import type { BlockRecord, TelemetryDispatch } from "../../../domain/transport";
import { useDucktape } from "../../store/use-ducktape";
import { color, font, radius } from "../../theme/tokens";

/** A 64-hex digest → its leading 10 chars, enough to eyeball identity. */
const shortHex = (hex: string): string =>
  hex.length > 10 ? `${hex.slice(0, 10)}…` : hex || "—";

/** Grid template shared by the list header and every row. */
const ROW_GRID = "72px 1.4fr 1.4fr 1.4fr 52px";

function ColumnHeaders() {
  const cell = { font: `600 10.5px ${font.sans}`, color: color.muted };
  return (
    <div
      style={{
        display: "grid",
        gridTemplateColumns: ROW_GRID,
        gap: 12,
        padding: "0 13px 5px",
      }}
    >
      <span style={cell}>HEIGHT</span>
      <span style={cell}>HASH</span>
      <span style={cell}>COMMIT</span>
      <span style={cell}>PROPOSER</span>
      <span style={{ ...cell, textAlign: "right" as const }}>OPS</span>
    </div>
  );
}

function BlockRow({ block, onOpen }: { block: BlockRecord; onOpen: () => void }) {
  return (
    <button
      type="button"
      onClick={onOpen}
      style={{
        display: "grid",
        gridTemplateColumns: ROW_GRID,
        gap: 12,
        alignItems: "baseline",
        padding: "9px 13px",
        borderRadius: radius.md,
        border: `1px solid ${color.border}`,
        background: color.paper,
        cursor: "pointer",
        textAlign: "left",
        width: "100%",
      }}
    >
      <span style={{ font: `600 12.5px ${font.mono}`, color: color.ink }}>
        #{block.height.toLocaleString()}
      </span>
      <span style={{ font: `400 11.5px ${font.mono}`, color: color.inkSofter }}>
        {shortHex(block.hash)}
      </span>
      <span style={{ font: `400 11.5px ${font.mono}`, color: color.muted3 }}>
        {shortHex(block.commitHash)}
      </span>
      <span style={{ font: `400 11.5px ${font.mono}`, color: color.muted3 }}>
        {shortHex(block.proposer)}
      </span>
      <span
        style={{
          font: `600 11.5px ${font.mono}`,
          color: block.disposition === "rejected" ? color.red : color.accent,
          textAlign: "right",
        }}
      >
        {block.disposition === "rejected" ? "✕" : block.operations.length}
      </span>
    </button>
  );
}

/** One full-width labeled digest line in the block detail. */
function DigestLine({ label, value }: { label: string; value: string }) {
  return (
    <div style={{ display: "flex", gap: 10, alignItems: "baseline" }}>
      <span style={{ font: `600 10.5px ${font.sans}`, color: color.muted, width: 72, flexShrink: 0 }}>
        {label}
      </span>
      <span style={{ font: `400 11.5px ${font.mono}`, color: color.inkSofter, wordBreak: "break-all" }}>
        {value || "—"}
      </span>
    </div>
  );
}

function OperationRow({ op, index }: { op: TelemetryDispatch; index: number }) {
  const fanout = [
    op.emittedMsgs > 0 ? `▸${op.emittedMsgs} msgs` : null,
    op.emittedEvents > 0 ? `◆${op.emittedEvents} events` : null,
  ]
    .filter(Boolean)
    .join("  ");
  return (
    <div
      style={{
        display: "flex",
        alignItems: "baseline",
        gap: 12,
        padding: "8px 11px",
        borderRadius: radius.sm,
        border: `1px solid ${color.border}`,
        background: color.paper,
      }}
    >
      <span style={{ font: `500 11px ${font.mono}`, color: color.muted2 }}>{index}</span>
      <span style={{ font: `600 12px ${font.sans}`, color: color.ink }}>{op.module}</span>
      <span style={{ font: `400 11px ${font.mono}`, color: color.muted2 }}>{op.origin}</span>
      {fanout && (
        <span style={{ font: `500 10.5px ${font.mono}`, color: color.muted3, marginLeft: "auto" }}>
          {fanout}
        </span>
      )}
    </div>
  );
}

function BlockDetail({ block, onBack }: { block: BlockRecord; onBack: () => void }) {
  return (
    <div style={{ padding: 17, display: "flex", flexDirection: "column", gap: 13, overflowY: "auto" }}>
      <div style={{ display: "flex", alignItems: "baseline", gap: 12 }}>
        <button
          type="button"
          onClick={onBack}
          style={{
            font: `500 11.5px ${font.sans}`,
            color: color.muted3,
            background: "none",
            border: "none",
            padding: 0,
            cursor: "pointer",
          }}
        >
          ← Blocks
        </button>
        <span style={{ font: `600 14px ${font.mono}`, color: color.ink }}>
          #{block.height.toLocaleString()}
        </span>
        <span
          style={{
            font: `600 10.5px ${font.sans}`,
            color: block.disposition === "rejected" ? color.red : color.green,
          }}
        >
          {block.disposition}
        </span>
        <span style={{ font: `400 11px ${font.mono}`, color: color.muted2, marginLeft: "auto" }}>
          {block.target}
        </span>
      </div>

      <div
        style={{
          display: "flex",
          flexDirection: "column",
          gap: 7,
          padding: "11px 13px",
          borderRadius: radius.md,
          border: `1px solid ${color.border}`,
          background: color.paper,
        }}
      >
        <DigestLine label="HASH" value={block.hash} />
        <DigestLine label="COMMIT" value={block.commitHash} />
        <DigestLine label="PROPOSER" value={block.proposer} />
      </div>

      <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
        <span style={{ font: `600 10.5px ${font.sans}`, color: color.muted }}>
          TRANSACTIONS ({block.operations.length})
        </span>
        {block.operations.length === 0 ? (
          <div style={{ font: `400 12px ${font.sans}`, color: color.muted2 }}>
            {block.disposition === "rejected"
              ? "The op finalized but was rejected — a deterministic no-op, so no dispatches ran."
              : "No dispatches recorded."}
          </div>
        ) : (
          block.operations.map((op, index) => (
            <OperationRow key={`${op.module}-${index}`} op={op} index={index} />
          ))
        )}
      </div>

      {block.payload && (
        <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          <span style={{ font: `600 10.5px ${font.sans}`, color: color.muted }}>PAYLOAD</span>
          <pre
            style={{
              font: `400 11px ${font.mono}`,
              color: color.inkSofter,
              background: color.sunken,
              border: `1px solid ${color.border}`,
              borderRadius: radius.md,
              padding: "9px 11px",
              margin: 0,
              whiteSpace: "pre-wrap",
              wordBreak: "break-all",
            }}
          >
            {block.payload}
          </pre>
        </div>
      )}
    </div>
  );
}

export function ExplorerView() {
  const { state } = useDucktape();
  // The open block is held as the record itself, not a height lookup: a
  // finalized block is immutable, and holding the snapshot keeps the detail
  // stable even if the ring evicts the record mid-view.
  const [open, setOpen] = useState<BlockRecord | null>(null);
  // State keeps blocks oldest-first; the explorer reads newest-first.
  const blocks = [...state.blocks].reverse();

  return (
    <div style={{ flex: 1, minWidth: 0, display: "flex", flexDirection: "column" }}>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          padding: "11px 17px",
          borderBottom: `1px solid ${color.borderSoft}`,
        }}
      >
        <span style={{ font: `600 13px ${font.sans}`, color: color.ink }}>Explorer</span>
        <span style={{ font: `500 11px ${font.mono}`, color: color.muted }}>
          {blocks.length > 0 ? `${blocks.length} blocks` : "—"}
        </span>
      </div>

      {open ? (
        <BlockDetail block={open} onBack={() => setOpen(null)} />
      ) : (
        <div style={{ padding: 17, display: "flex", flexDirection: "column", gap: 7, overflowY: "auto" }}>
          {blocks.length === 0 ? (
            <div style={{ font: `400 12px ${font.sans}`, color: color.muted2 }}>
              No blocks yet — empty heartbeat blocks are skipped, so rows appear
              once real ops commit.
            </div>
          ) : (
            <>
              <ColumnHeaders />
              {blocks.map((block) => (
                <BlockRow
                  key={block.height}
                  block={block}
                  onOpen={() => setOpen(block)}
                />
              ))}
            </>
          )}
        </div>
      )}
    </div>
  );
}
