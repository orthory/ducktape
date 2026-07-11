import { useRef } from "react";
import type { FleetNode } from "../types";
import { useRfb } from "../useRfb";
import { ActivityFeed } from "./ActivityFeed";

// Click-to-interact: a full-size, INTERACTIVE (viewOnly=false) session for one
// worktree. Same token as its tile — the fleet server routes it to that VNC.
export function Drawer({
  node,
  onClose,
}: {
  node: FleetNode;
  onClose: () => void;
}) {
  const screenRef = useRef<HTMLDivElement>(null);
  const status = useRfb(screenRef, node.token, false);

  return (
    <div className="drawer-backdrop" onClick={onClose}>
      <div className="drawer" onClick={(e) => e.stopPropagation()}>
        <div className="drawer-head">
          <span className={`dot ${node.status}`} />
          <strong>{node.branch}</strong>
          <span className="sha">{node.head.sha}</span>
          <span className="drawer-subj">{node.head.subject}</span>
          <span className="spacer" />
          <span className="drawer-status">{status}</span>
          <button className="close" onClick={onClose} aria-label="Close">
            ✕
          </button>
        </div>
        <div className="drawer-body">
          <div className="drawer-screen">
            <div ref={screenRef} className="rfb interactive" />
            {status !== "connected" && (
              <div className="tile-overlay">{status}…</div>
            )}
          </div>
          <aside className="drawer-side">
            <ActivityFeed activity={node.activity} />
          </aside>
        </div>
      </div>
    </div>
  );
}
