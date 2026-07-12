// A persistent reconnecting banner for a MID-SESSION node drop. The heartbeat
// used to flip a lone 6px red dot with no reason while the height sat frozen and
// live-looking; this shows WHAT happened and offers Restart (managed nodes), so
// a node that died in dev is loud, not silent. Clears itself when the node
// comes back (the heartbeat nulls connectionDown on recovery).

import { type CSSProperties } from "react";

import { useDucktape } from "../store/use-ducktape";
import { color, font, radius } from "../theme/tokens";

const restartBtn: CSSProperties = {
  all: "unset",
  cursor: "pointer",
  font: `600 10.5px ${font.sans}`,
  color: "#fff",
  border: "1px solid rgba(255,255,255,.5)",
  borderRadius: radius.sm,
  padding: "3px 9px",
  flexShrink: 0,
};

export function ConnectionBanner() {
  const { state, actions } = useDucktape();
  const down = state.connectionDown;
  if (!down) return null;
  const busy = state.connected && !down.impostor;

  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: 10,
        padding: "7px 13px",
        background: down.impostor ? color.danger : color.amber,
        color: "#fff",
        font: `600 11px ${font.sans}`,
        flexShrink: 0,
      }}
    >
      <span
        style={{ width: 7, height: 7, borderRadius: "50%", background: "#fff", flexShrink: 0 }}
      />
      <span
        style={{
          flex: 1,
          minWidth: 0,
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
        }}
        title={down.reason}
      >
        {busy
          ? down.reason
          : `${down.impostor ? "Connection lost — " : "Lost connection to the node — reconnecting… "}${down.reason}`}
      </span>
      {state.managed && !state.connected && !down.impostor && (
        <button onClick={() => actions.startNode()} style={restartBtn}>
          Restart node
        </button>
      )}
    </div>
  );
}
