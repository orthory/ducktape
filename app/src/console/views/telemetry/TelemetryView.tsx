// The telemetry surface: the node-local observability plane. One row per
// finalized block, newest-first — the host's deterministic dispatch trace
// (which modules ran, what triggered them, their intent fan-out) decorated
// with THIS node's wall-clock apply latency. Read-only; frames arrive live
// over the ws stream and are backfilled from the node's ring on connect.

import { useDucktape } from "../../store/use-ducktape";
import type { TelemetryDispatch, TelemetryFrame } from "../../../domain/transport";
import { color, font, radius } from "../../theme/tokens";

/** Microseconds → a compact, legible duration. */
const formatLatency = (us: number): string =>
  us < 1000 ? `${us} µs` : `${(us / 1000).toFixed(2)} ms`;

/** Agreed logical clock (unix millis) → wall-clock time, or "—" pre-consensus. */
const formatClock = (ms: number): string =>
  ms > 0 ? new Date(ms).toLocaleTimeString() : "—";

function DispatchChip({ dispatch }: { dispatch: TelemetryDispatch }) {
  const fanout = [
    dispatch.emittedMsgs > 0 ? `▸${dispatch.emittedMsgs}` : null,
    dispatch.emittedEvents > 0 ? `◆${dispatch.emittedEvents}` : null,
  ]
    .filter(Boolean)
    .join(" ");
  return (
    <span
      style={{
        display: "inline-flex",
        alignItems: "baseline",
        gap: 6,
        padding: "3px 8px",
        borderRadius: radius.sm,
        border: `1px solid ${color.border}`,
        background: color.paper,
      }}
    >
      <span style={{ font: `600 11.5px ${font.sans}`, color: color.ink }}>{dispatch.module}</span>
      <span style={{ font: `400 10.5px ${font.mono}`, color: color.muted2 }}>{dispatch.origin}</span>
      {fanout && (
        <span style={{ font: `500 10.5px ${font.mono}`, color: color.muted3 }}>{fanout}</span>
      )}
    </span>
  );
}

function FrameRow({ frame }: { frame: TelemetryFrame }) {
  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        gap: 8,
        padding: "11px 13px",
        borderRadius: radius.md,
        border: `1px solid ${color.border}`,
        background: color.paper,
      }}
    >
      <div style={{ display: "flex", alignItems: "baseline", gap: 12 }}>
        <span style={{ font: `600 13px ${font.mono}`, color: color.ink }}>
          #{frame.height.toLocaleString()}
        </span>
        <span style={{ font: `600 11.5px ${font.mono}`, color: color.accent }}>
          {formatLatency(frame.latencyUs)}
        </span>
        <span style={{ font: `400 11px ${font.mono}`, color: color.muted2, marginLeft: "auto" }}>
          {formatClock(frame.consensusTime)}
        </span>
      </div>

      <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
        {frame.dispatches.map((dispatch, index) => (
          <DispatchChip key={`${dispatch.module}-${index}`} dispatch={dispatch} />
        ))}
      </div>

      {frame.events.length > 0 && (
        <div style={{ display: "flex", flexDirection: "column", gap: 3 }}>
          {frame.events.map((event, index) => (
            <div
              key={`${event.source}-${index}`}
              style={{ font: `400 11px ${font.mono}`, color: color.muted3, wordBreak: "break-word" }}
            >
              <span style={{ color: color.inkSofter }}>{event.source}</span>
              {event.payload && ` — ${event.payload}`}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

export function TelemetryView() {
  const { state, actions } = useDucktape();
  // State keeps frames oldest-first; the timeline reads newest-first.
  const frames = [...state.telemetry].reverse();
  // Telemetry is an in-memory, live stream; the Explorer holds the durable
  // block history. So an empty timeline has three honest causes we tell apart
  // below: disconnected, connected-but-no-committed-history-yet, or history
  // exists but no telemetry has streamed on this connection.
  const hasHistory = state.blocks.length > 0;

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
        <span style={{ font: `600 13px ${font.sans}`, color: color.ink }}>Telemetry</span>
        <span style={{ font: `500 11px ${font.mono}`, color: color.muted }}>
          {frames.length > 0 ? `${frames.length} blocks` : "—"}
        </span>
      </div>

      <div style={{ padding: 17, display: "flex", flexDirection: "column", gap: 9, overflowY: "auto" }}>
        {frames.length === 0 ? (
          <div style={{ font: `400 12px ${font.sans}`, color: color.muted2 }}>
            {!state.connected ? (
              "Not connected. Telemetry is a live stream — it resumes once the node is reachable again."
            ) : !hasHistory ? (
              "Waiting for the first block. Telemetry streams in live as the node commits blocks."
            ) : (
              <>
                No live telemetry on this connection yet. This node's committed blocks are in the{" "}
                <button
                  type="button"
                  onClick={() => actions.setScreen("explorer")}
                  style={{
                    font: "inherit",
                    color: color.accent,
                    background: "none",
                    border: "none",
                    padding: 0,
                    cursor: "pointer",
                    textDecoration: "underline",
                  }}
                >
                  Explorer
                </button>{" "}
                — frames appear here as new blocks commit, if the node streams telemetry.
              </>
            )}
          </div>
        ) : (
          frames.map((frame) => <FrameRow key={frame.height} frame={frame} />)
        )}
      </div>
    </div>
  );
}
