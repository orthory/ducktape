import type { Node, Edge } from "@xyflow/react";
import type { FleetNode } from "./types";

// Data carried by each graph node. Must be an index signature for xyflow's
// Node<data> constraint; functions are fine at runtime.
export interface WorktreeNodeData extends Record<string, unknown> {
  node: FleetNode;
  onOpen: (n: FleetNode) => void;
}

export type WorktreeGraphNode = Node<WorktreeNodeData, "worktree">;

const COL_W = 380;
const ROW_H = 320;
const MARGIN = 24;

// Pure: turn the fleet into a branch tree — base (dev) at the top, every other
// worktree a child fanned out beneath it, edge base→child (animated when live).
// Kept free of React/xyflow rendering so it is unit-testable.
export function toGraph(
  nodes: FleetNode[],
  base: string,
  onOpen: (n: FleetNode) => void,
): { rfNodes: WorktreeGraphNode[]; rfEdges: Edge[] } {
  const baseNode = nodes.find((n) => n.branch === base);
  const children = nodes.filter((n) => n.branch !== base);
  const rfNodes: WorktreeGraphNode[] = [];
  const rfEdges: Edge[] = [];

  const spanW = Math.max(children.length, 1) * COL_W;
  if (baseNode) {
    rfNodes.push({
      id: baseNode.id,
      type: "worktree",
      position: { x: MARGIN + spanW / 2 - COL_W / 2, y: 0 },
      data: { node: baseNode, onOpen },
    });
  }
  children.forEach((c, i) => {
    rfNodes.push({
      id: c.id,
      type: "worktree",
      position: { x: MARGIN + i * COL_W, y: ROW_H },
      data: { node: c, onOpen },
    });
    if (baseNode) {
      rfEdges.push({
        id: `${baseNode.id}-${c.id}`,
        source: baseNode.id,
        target: c.id,
        animated: c.status === "up",
      });
    }
  });

  return { rfNodes, rfEdges };
}
