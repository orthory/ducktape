import type { FleetNode } from "../types";
import { AppTile } from "./AppTile";

// The v1 layout: a responsive wall of live app views. Kept deliberately thin so
// a future <GraphView> can consume the same nodes without touching this.
export function GridView({
  nodes,
  onOpen,
}: {
  nodes: FleetNode[];
  onOpen: (n: FleetNode) => void;
}) {
  if (nodes.length === 0) {
    return (
      <div className="empty">
        No worktrees in the fleet yet — run <code>ops/fleet.sh up</code>.
      </div>
    );
  }
  return (
    <div className="grid">
      {nodes.map((n) => (
        <AppTile key={n.id} node={n} onOpen={onOpen} />
      ))}
    </div>
  );
}
