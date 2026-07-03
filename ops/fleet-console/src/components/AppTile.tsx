import { useRef } from "react";
import type { FleetNode } from "../types";
import { useRfb } from "../useRfb";

function AheadBehind({ ahead, behind }: { ahead: number; behind: number }) {
  if (!ahead && !behind) return null;
  return (
    <span className="aheadbehind">
      {ahead ? <span className="ahead">↑{ahead}</span> : null}
      {behind ? <span className="behind">↓{behind}</span> : null}
    </span>
  );
}

// One worktree = one live, view-only app view. Clicking opens the interactive
// drawer. The RFB canvas has pointer-events:none (see css) so the click lands on
// the tile, not the (input-disabled) remote screen.
export function AppTile({
  node,
  onOpen,
}: {
  node: FleetNode;
  onOpen: (n: FleetNode) => void;
}) {
  const screenRef = useRef<HTMLDivElement>(null);
  const connectable = node.status === "up" && Boolean(node.token);
  const status = useRfb(screenRef, connectable ? node.token : undefined, true);

  return (
    <div
      className="tile"
      data-status={node.status}
      onClick={() => connectable && onOpen(node)}
      role={connectable ? "button" : undefined}
      title={connectable ? "Click to interact" : undefined}
    >
      <div className="tile-head">
        <span className={`dot ${node.status}`} />
        <span className="branch">{node.branch}</span>
        <span className="sha">{node.head.sha}</span>
        <AheadBehind ahead={node.ahead} behind={node.behind} />
      </div>
      <div className="tile-screen">
        {connectable ? (
          <>
            <div ref={screenRef} className="rfb" />
            {status !== "connected" && (
              <div className="tile-overlay">{status}…</div>
            )}
          </>
        ) : (
          <div className="tile-overlay muted">
            {node.status === "building" ? "building…" : "not running"}
          </div>
        )}
      </div>
      <div className="tile-foot">{node.head.subject}</div>
    </div>
  );
}
